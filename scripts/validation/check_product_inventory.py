"""Fail-closed source audit for the 23 product features and 16 profiles.

This check deliberately validates physical source surfaces.  A declaration in
the inventory is not accepted unless its document, generated schema, owning
handler, CLI/Controller route, four test classes, and current bytes all exist.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[2]
INVENTORY_PATH = ROOT / "catalog/product-features.toml"
SCHEMA_ROOT = ROOT / "specs/schemas/v1"
SCHEMA_MANIFEST = ROOT / "specs/schemas/manifest.json"
SOURCE_EVIDENCE_PATH = ROOT / "catalog/product-source-evidence.json"
CORE_TOOL_PACKAGE_PATH = ROOT / "catalog/tool-packages/star-control-core.toml"
PROJECT_CATALOG_PATH = ROOT / "catalog/projects.toml"
CROSS_REPO_FINAL_LOCK_PATH = (
    ROOT / "catalog/cross-repo-change-bundles/pr0-pr31-final-lock.toml"
)
FIXED_MCP_PATH = ROOT / "crates/foundation/star-contracts/src/fixed_mcp.rs"
CODEX_SKILL_ROOT = (
    ROOT
    / "integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/star-control-operations"
)
CODEX_SKILL_PATH = CODEX_SKILL_ROOT / "SKILL.md"
CODEX_SKILL_AGENT_PATH = CODEX_SKILL_ROOT / "agents/openai.yaml"
CODEX_ROUTING_MATRIX_PATH = CODEX_SKILL_ROOT / "references/routing-matrix.md"
CODEX_PARALLEL_SKILL_ROOT = (
    ROOT
    / "integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/orchestrate-parallel-implementation"
)
CODEX_PARALLEL_COMPONENT_PATHS = (
    CODEX_PARALLEL_SKILL_ROOT / "SKILL.md",
    CODEX_PARALLEL_SKILL_ROOT / "agents/openai.yaml",
    CODEX_PARALLEL_SKILL_ROOT / "references/decomposition.md",
    CODEX_PARALLEL_SKILL_ROOT / "references/scheduling-and-lifecycle.md",
    CODEX_PARALLEL_SKILL_ROOT / "references/workspace-and-integration.md",
    CODEX_PARALLEL_SKILL_ROOT / "references/safety-and-validation.md",
    CODEX_PARALLEL_SKILL_ROOT / "assets/worker-context-pack.md",
    CODEX_PARALLEL_SKILL_ROOT / "assets/worker-report.md",
    CODEX_PARALLEL_SKILL_ROOT / "assets/controller-report.md",
)

FEATURE_IDS = [
    *(f"A{index:02d}" for index in range(1, 11)),
    *(f"B{index:02d}" for index in range(1, 10)),
    "C01",
    "D01",
    "D02",
    "D03",
]
PROFILE_IDS = [
    "project_understanding",
    "docs_config_environment",
    "change_planning",
    "test_correctness",
    "architecture_quality",
    "ai_development_validation",
    "refactor_codemod",
    "api_contract_change",
    "rust_style_auto_fix",
    "debug_recovery",
    "security_supply_chain",
    "dependency_upgrade",
    "data_config_db_migration",
    "performance_build",
    "language_platform_migration",
    "ci_release_deploy",
]
RUNTIME_EXECUTABLES = {
    "apps/star-cli/Cargo.toml": "star",
    "apps/star-controller/Cargo.toml": "star-controller",
    "apps/star-mcp/Cargo.toml": "star-mcp",
    "apps/star-updater/Cargo.toml": "star-updater",
}
ALLOWED_TOP_LEVEL = {
    ".agents",
    ".cargo",
    ".codex",
    ".github",
    ".star-control",
    "apps",
    "catalog",
    "crates",
    "dist",
    "docs",
    "integrations",
    "packaging",
    "schemas",
    "scripts",
    "specs",
    "target",
    "tools",
}
PRODUCT_SOURCE_TOP_LEVEL = {
    ".codex",
    ".github",
    "apps",
    "catalog",
    "crates",
    "docs",
    "integrations",
    "packaging",
    "schemas",
    "scripts",
    "specs",
    "tools",
}
TEST_FIELDS = ("positive_test", "negative_test", "failure_test", "recovery_test")
PROFILE_CONFORMANCE_REFS = (
    (
        "resolution_handler",
        "crates/foundation/star-contracts/src/profile.rs#pub fn resolve_development_profiles",
        False,
    ),
    (
        "resolution_test",
        "crates/foundation/star-contracts/src/profile.rs#exact_builtin_set_resolves_deterministically",
        True,
    ),
    (
        "validation_handler",
        "crates/control/star-planning/src/lib.rs#fn select_validation_plan",
        False,
    ),
    (
        "validation_test",
        "crates/control/star-planning/src/lib.rs#missing_required_check_is_blocked_not_not_applicable",
        True,
    ),
    (
        "permission_handler",
        "apps/star-controller/src/main.rs#fn create_permission_plan",
        False,
    ),
    (
        "permission_test",
        "apps/star-controller/src/main.rs#effective_permission_policy_prompts_or_denies_before_dispatch",
        True,
    ),
    (
        "recovery_handler",
        "apps/star-controller/src/main.rs#fn record_stage_result",
        False,
    ),
    (
        "recovery_test",
        "crates/foundation/star-contracts/src/stage.rs#stage_result_recovery_requires_effect_boundary_and_action",
        True,
    ),
    (
        "rollback_handler",
        "crates/control/star-execution/src/lib.rs#pub fn rollback_applied",
        False,
    ),
    (
        "rollback_test",
        "crates/control/star-execution/src/lib.rs#exact_hash_apply_preserves_unrelated_dirty_file_and_safe_rollback_restores_target",
        True,
    ),
)


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_json_fingerprint(value: object) -> str:
    return sha256_bytes(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode(
            "utf-8"
        )
    )


def relative_file(value: str, errors: list[str]) -> pathlib.Path | None:
    candidate = pathlib.PurePosixPath(value)
    if candidate.is_absolute() or ".." in candidate.parts or not candidate.parts:
        errors.append(f"unsafe relative path: {value}")
        return None
    lexical = ROOT.joinpath(*candidate.parts)
    try:
        resolved = lexical.resolve(strict=True)
        resolved.relative_to(ROOT.resolve(strict=True))
    except (OSError, ValueError):
        errors.append(f"path escapes repository: {value}")
        return None
    if lexical.is_symlink() or not resolved.is_file():
        errors.append(f"missing file: {value}")
        return None
    return resolved


def split_ref(value: str, errors: list[str]) -> tuple[pathlib.Path, str] | None:
    if value.count("#") != 1:
        errors.append(f"reference must be path#marker: {value}")
        return None
    path_text, marker = value.split("#", 1)
    path = relative_file(path_text, errors)
    if path is None or not marker.strip():
        if not marker.strip():
            errors.append(f"empty reference marker: {value}")
        return None
    return path, marker


def verify_test_ref(feature_id: str, field: str, value: str, errors: list[str]) -> pathlib.Path | None:
    parsed = split_ref(value, errors)
    if parsed is None:
        return None
    path, marker = parsed
    source = path.read_text(encoding="utf-8")
    function_name = marker.rsplit(" ", 1)[-1]
    match = re.search(rf"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+{re.escape(function_name)}\s*\(", source)
    if match is None:
        errors.append(f"{feature_id} {field} test function missing: {value}")
        return path
    attributes = source[max(0, match.start() - 600) : match.start()]
    nearest_attribute = attributes.rsplit("}", 1)[-1]
    if not re.search(r"#\[(?:tokio::)?test", nearest_attribute):
        errors.append(f"{feature_id} {field} is not an executable test: {value}")
    lowered = nearest_attribute.lower()
    if "#[ignore" in lowered or "#[should_panic" in lowered or "quarantine" in lowered:
        errors.append(f"{feature_id} {field} is ignored or quarantined: {value}")
    return path


def cli_declares(command: str, source: str) -> bool:
    if f'"{command}"' in source:
        return True
    parts = [re.escape(part) for part in command.split(".")]
    # Compound CLI families use forms such as `release promote|show|status` and
    # construct the backend command with format!.  Requiring all command parts
    # in order on one source line proves that the user-facing route is declared
    # without mistaking a Controller-only string for a CLI path.
    return re.search(r"(?m)^.*" + r"[^\r\n]{0,96}".join(parts) + r".*$", source) is not None


def git_untracked_top_levels(errors: list[str]) -> None:
    result = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        errors.append("git untracked inventory could not be read")
        return
    unexpected = sorted(
        {
            pathlib.PurePosixPath(line.strip().replace("\\", "/")).parts[0]
            for line in result.stdout.splitlines()
            if line.strip()
            and pathlib.PurePosixPath(line.strip().replace("\\", "/")).parts[0]
            not in ALLOWED_TOP_LEVEL
        }
    )
    if unexpected:
        errors.append(f"unexpected untracked top-level paths: {unexpected}")


def git_product_source_files(errors: list[str]) -> set[pathlib.Path]:
    """Return all tracked and non-ignored product-source files.

    Per-feature references prove the six required layers, while this complete
    set makes the aggregate source fingerprint change for any product source,
    contract, test, packaging, or canonical-document byte. Generated/runtime
    state roots are intentionally excluded.
    """

    observed: set[pathlib.Path] = set()
    for arguments in (
        ["git", "ls-files", "-z"],
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
    ):
        result = subprocess.run(
            arguments,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if result.returncode != 0:
            errors.append(f"product source inventory could not be read: {' '.join(arguments)}")
            continue
        for line in result.stdout.split("\0"):
            relative_text = line.strip().replace("\\", "/")
            if not relative_text or relative_text == "catalog/product-source-evidence.json":
                continue
            relative = pathlib.PurePosixPath(relative_text)
            if len(relative.parts) > 1 and relative.parts[0] not in PRODUCT_SOURCE_TOP_LEVEL:
                continue
            path = ROOT.joinpath(*relative.parts)
            try:
                resolved = path.resolve(strict=True)
                resolved.relative_to(ROOT.resolve(strict=True))
            except (OSError, ValueError):
                errors.append(f"product source path escapes or is missing: {relative_text}")
                continue
            if path.is_symlink() or not resolved.is_file():
                errors.append(f"product source is not a regular file: {relative_text}")
                continue
            observed.add(resolved)
    return observed


def validate_cross_repo_final_lock(
    errors: list[str], fingerprint_files: set[pathlib.Path]
) -> None:
    for path in (PROJECT_CATALOG_PATH, CROSS_REPO_FINAL_LOCK_PATH):
        if not path.is_file():
            errors.append(f"required cross-repo source manifest is missing: {path.relative_to(ROOT)}")
            return
        fingerprint_files.add(path)

    project_catalog = tomllib.loads(PROJECT_CATALOG_PATH.read_text(encoding="utf-8"))
    projects = project_catalog.get("projects", [])
    project_keys = [str(project.get("project_key")) for project in projects]
    if project_catalog.get("registration_enabled") is not True:
        errors.append("project catalog registration must remain enabled")
    if len(project_keys) != 15 or len(set(project_keys)) != 15:
        errors.append(f"project catalog must contain exactly 15 unique projects: {project_keys}")
    expected_new_projects = {
        "server": ("서버", "https://github.com/jaeminsongdev/server.git"),
        "deployment": ("배포", "https://github.com/jaeminsongdev/release.git"),
    }
    projects_by_key = {str(project.get("project_key")): project for project in projects}
    for project_key, (relative_path, expected_origin) in expected_new_projects.items():
        project = projects_by_key.get(project_key, {})
        if (
            project.get("relative_path") != relative_path
            or project.get("role") != "active_canonical"
            or project.get("repository_kind") != "git"
            or project.get("expected_origin") != expected_origin
        ):
            errors.append(f"{project_key} project catalog registration is incomplete")

    bundle = tomllib.loads(CROSS_REPO_FINAL_LOCK_PATH.read_text(encoding="utf-8"))
    if (
        bundle.get("wire_schema_id") != "star.cross-repo-change-bundle"
        or bundle.get("wire_schema_version") != 1
        or bundle.get("participant_wire_schema_id") != "star.change-bundle-participant"
        or bundle.get("participant_wire_schema_version") != 2
    ):
        errors.append("PR0-31 source descriptor does not bind the CrossRepoChangeBundle contracts")
    if bundle.get("coverage_prs") != list(range(32)):
        errors.append("CrossRepoChangeBundle coverage must contain PR0 through PR31 exactly once")
    if bundle.get("active_control_canonical") != "star-control":
        errors.append("active control canonical must be Star-Control")
    if bundle.get("forbidden_control_targets") != ["Star-Workflow"]:
        errors.append("Star-Workflow must remain the only explicit forbidden control target")
    if bundle.get("aggregate_state") != "held":
        errors.append("PR0-31 aggregate must remain held")
    for policy in (
        "partial_promotion_forbidden",
        "unverified_promotion_forbidden",
        "external_signing_promotion_forbidden",
    ):
        if bundle.get(policy) is not True:
            errors.append(f"CrossRepoChangeBundle must enforce {policy}")

    participants = bundle.get("participants", [])
    participant_keys = [str(participant.get("project_key")) for participant in participants]
    expected_pr31_pins = {
        "star-control": "36f8dc1e4c7cbdbf6607cace456f181654589372",
        "devtools": "f9b25576c41c47810c80898070c9a0e43293cc90",
        "content": "1c6187a01266bfbc3b2cf09ecec07bc921460b94",
        "server": "aa94f037b3d8eaa2c083bb53275645223e1cc0d3",
        "deployment": "a15d8426e9f873fc2cadb04ef5117ab24e817b53",
        "danpung": "eb01ea33209b2f67761ae7652dda8917fb00c11e",
        "emulink": "f4324596131ef0c3bc10a4253eb6d185cfab4f5b",
        "format": "b2a5e130109bb25549ce710f8fdd293126b99e67",
        "adapter": "415877cc3f1b22b8b49d617246f6c2e9dbedb914",
        "mod-foundry": "5729d70bb7d71a9343593c9853eac8494f6cd66c",
        "language": "09aaf6d23249ec5a43a85a5247837e56fc7f51a8",
        "storage": "4d39477429b09354097def95e638ef78c18c30dd",
        "ecosystem-canonical": "a5bc4ec0049d92520a6b8440e65d77d399525edd",
        "knowledge": "2075c5e827922feaa295d8bf29488a955f1b1e11",
        "core": "95573fd5c1ae0f477c45ed4e85dbc22ed18c3eb2",
    }
    if len(participant_keys) != len(set(participant_keys)):
        errors.append("CrossRepoChangeBundle participant keys contain duplicates")
    if set(participant_keys) != set(project_keys):
        errors.append(
            "CrossRepoChangeBundle participants must match the active project catalog: "
            f"missing={sorted(set(project_keys) - set(participant_keys))} "
            f"extra={sorted(set(participant_keys) - set(project_keys))}"
        )

    sha_pattern = re.compile(r"^[0-9a-f]{40}$")
    validation_states = {"pass", "partial", "unverified", "not_run"}
    evidence_states = {"current", "partial", "missing", "stale"}
    approval_states = {"approved", "pending", "not_required"}
    rollback_states = {"passed", "partial", "not_run", "not_applicable", "required"}
    release_states = {"not_applicable", "not_run", "not_provided", "passed", "published"}
    participant_states = {"completed", "held", "rollback_required", "outcome_unknown"}
    has_non_pass = False
    has_external_release_hold = False
    for participant in participants:
        project_key = str(participant.get("project_key"))
        base_commit_oid = str(participant.get("base_commit_oid", ""))
        if sha_pattern.fullmatch(base_commit_oid) is None:
            errors.append(f"{project_key} participant exact commit is invalid")
        elif base_commit_oid != expected_pr31_pins.get(project_key):
            errors.append(f"{project_key} participant exact commit is stale or unexpected")
        if participant.get("required") is not True:
            errors.append(f"{project_key} participant must remain required")
        if participant.get("participant_state") not in participant_states:
            errors.append(f"{project_key} participant state is invalid")
        if participant.get("validation_state") not in validation_states:
            errors.append(f"{project_key} validation state is invalid")
        if participant.get("evidence_state") not in evidence_states:
            errors.append(f"{project_key} evidence state is invalid")
        if participant.get("approval_state") not in approval_states:
            errors.append(f"{project_key} approval state is invalid")
        if participant.get("rollback_state") not in rollback_states:
            errors.append(f"{project_key} rollback state is invalid")
        if participant.get("release_signing_state") not in release_states:
            errors.append(f"{project_key} release signing state is invalid")
        if participant.get("release_publication_state") not in release_states:
            errors.append(f"{project_key} release publication state is invalid")
        if participant.get("validation_state") != "pass" or participant.get("evidence_state") != "current":
            has_non_pass = True
            if participant.get("participant_state") == "completed":
                errors.append(f"{project_key} incomplete evidence cannot be completed")
        if participant.get("release_signing_state") in {"not_run", "not_provided"} or participant.get(
            "release_publication_state"
        ) in {"not_run", "not_provided"}:
            has_external_release_hold = True
        evidence_refs = participant.get("evidence_refs", [])
        if not evidence_refs:
            errors.append(f"{project_key} participant has no evidence reference")
        for evidence_ref in evidence_refs:
            evidence_path = relative_file(str(evidence_ref), errors)
            if evidence_path is not None:
                fingerprint_files.add(evidence_path)

    if not has_non_pass:
        errors.append("PR0-31 bundle must preserve at least one partial or unverified participant")
    if not has_external_release_hold:
        errors.append("PR0-31 bundle must preserve external signing/publication hold state")

    external_roots = bundle.get("external_evidence_roots", [])
    if len(external_roots) != 1 or external_roots[0].get("project_key") != "graphics":
        errors.append("Graphics PR22-26 evidence must remain one explicit external evidence root")
    else:
        graphics = external_roots[0]
        if (
            graphics.get("catalog_registration_state") != "not_registered"
            or graphics.get("base_commit_oid")
            != "8fab0c98d2239ae30c085c619ae8efe1ed8657fd"
        ):
            errors.append("Graphics external evidence registration boundary is invalid")
        for evidence_ref in graphics.get("evidence_refs", []):
            evidence_path = relative_file(str(evidence_ref), errors)
            if evidence_path is not None:
                fingerprint_files.add(evidence_path)


def main() -> int:
    errors: list[str] = []
    fingerprint_files: set[pathlib.Path] = set()
    fingerprint_files.add(pathlib.Path(__file__).resolve())
    inventory = tomllib.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
    fingerprint_files.add(INVENTORY_PATH)

    features = inventory.get("features", [])
    ids = [feature.get("id") for feature in features]
    if inventory.get("schema_version") != 1:
        errors.append("product inventory schema_version must be 1")
    if inventory.get("expected_feature_count") != len(FEATURE_IDS) or ids != FEATURE_IDS:
        errors.append(f"feature IDs must be exact and ordered: observed={ids}")
    if len(set(ids)) != len(ids):
        errors.append("feature IDs contain duplicates")

    manifest = json.loads(SCHEMA_MANIFEST.read_text(encoding="utf-8"))
    fingerprint_files.add(SCHEMA_MANIFEST)
    manifest_entries = manifest.get("files", [])
    manifest_map = {entry.get("file"): entry.get("hash") for entry in manifest_entries}
    if len(manifest_map) != len(manifest_entries):
        errors.append("generated Schema manifest contains duplicate file names")
    if len(manifest_entries) != inventory.get("expected_generated_schema_count"):
        errors.append(f"generated Schema manifest count mismatch: {len(manifest_entries)}")
    for name, expected_hash in manifest_map.items():
        schema_path = relative_file(f"specs/schemas/v1/{name}", errors)
        if schema_path is not None:
            fingerprint_files.add(schema_path)
            if sha256_bytes(schema_path.read_bytes()) != expected_hash:
                errors.append(f"generated Schema hash mismatch: {name}")

    cli_source = (ROOT / "apps/star-cli/src/main.rs").read_text(encoding="utf-8")
    controller_source = (ROOT / "apps/star-controller/src/main.rs").read_text(encoding="utf-8")
    mcp_source = (ROOT / "apps/star-mcp/src/lib.rs").read_text(encoding="utf-8")
    core_tool_package = tomllib.loads(CORE_TOOL_PACKAGE_PATH.read_text(encoding="utf-8"))
    core_backend_refs = {
        str(action.get("backend_ref")) for action in core_tool_package.get("actions", [])
    }
    fixed_mcp_source = FIXED_MCP_PATH.read_text(encoding="utf-8")
    fixed_mcp_tools = set(re.findall(r'\bname:\s*"(star_[a-z_]+)"', fixed_mcp_source))
    feature_index = ROOT / "docs/features/README.md"
    for fixed in (
        ROOT / "apps/star-cli/src/main.rs",
        ROOT / "apps/star-controller/src/main.rs",
        ROOT / "apps/star-mcp/src/lib.rs",
        FIXED_MCP_PATH,
        CORE_TOOL_PACKAGE_PATH,
        CODEX_SKILL_PATH,
        CODEX_SKILL_AGENT_PATH,
        CODEX_ROUTING_MATRIX_PATH,
        *CODEX_PARALLEL_COMPONENT_PATHS,
        ROOT / "Cargo.lock",
        ROOT / "rust-toolchain.toml",
        feature_index,
    ):
        if fixed.is_file():
            fingerprint_files.add(fixed)
    fingerprint_files.update(git_product_source_files(errors))
    validate_cross_repo_final_lock(errors, fingerprint_files)

    for feature in features:
        feature_id = str(feature.get("id", "?"))
        owner = relative_file(str(feature.get("owner_doc", "")), errors)
        if owner is not None:
            fingerprint_files.add(owner)
            if feature_id not in feature_index.read_text(encoding="utf-8"):
                errors.append(f"{feature_id} is absent from the canonical feature index")

        schemas = feature.get("schema_files", [])
        if not schemas:
            errors.append(f"{feature_id} has no generated Schema")
        for schema_name in schemas:
            if schema_name not in manifest_map:
                errors.append(f"{feature_id} Schema is absent from generated manifest: {schema_name}")

        handler_refs = feature.get("handler_refs", [])
        if not handler_refs:
            errors.append(f"{feature_id} has no owning handler reference")
        for value in handler_refs:
            parsed = split_ref(value, errors)
            if parsed is None:
                continue
            path, marker = parsed
            fingerprint_files.add(path)
            if marker not in path.read_text(encoding="utf-8"):
                errors.append(f"{feature_id} handler marker missing: {value}")

        commands = feature.get("cli_commands", [])
        if not commands:
            errors.append(f"{feature_id} has no CLI command surface")
        for command in commands:
            quoted = f'"{command}"'
            if not cli_declares(command, cli_source):
                errors.append(f"{feature_id} CLI command missing: {command}")
            if quoted not in controller_source:
                errors.append(f"{feature_id} Controller command missing: {command}")

        test_refs = [feature.get(field, "") for field in TEST_FIELDS]
        if len(set(test_refs)) != len(TEST_FIELDS):
            errors.append(f"{feature_id} test classes must use four distinct tests")
        for field, value in zip(TEST_FIELDS, test_refs, strict=True):
            test_path = verify_test_ref(feature_id, field, str(value), errors)
            if test_path is not None:
                fingerprint_files.add(test_path)

        mcp_required = feature.get("mcp_required")
        mcp_actions = feature.get("mcp_actions", [])
        if not isinstance(mcp_required, bool):
            errors.append(f"{feature_id} mcp_required must be boolean")
        elif mcp_required != bool(mcp_actions):
            errors.append(f"{feature_id} mcp_required does not match mcp_actions")
        if mcp_required and ("FIXED_TOOLS" not in mcp_source or "ipc_command" not in mcp_source):
            errors.append(f"{feature_id} MCP gateway path is missing")
        for action in mcp_actions:
            if action not in core_backend_refs and action not in fixed_mcp_tools:
                errors.append(f"{feature_id} MCP action is not registered: {action}")

        codex_required = feature.get("codex_required")
        codex_refs = feature.get("codex_refs", [])
        if not isinstance(codex_required, bool):
            errors.append(f"{feature_id} codex_required must be boolean")
        elif codex_required != bool(codex_refs):
            errors.append(f"{feature_id} codex_required does not match codex_refs")
        for value in codex_refs:
            parsed = split_ref(value, errors)
            if parsed is None:
                continue
            path, marker = parsed
            fingerprint_files.add(path)
            if marker not in path.read_text(encoding="utf-8"):
                errors.append(f"{feature_id} Codex path marker missing: {value}")

    profile_files = sorted((ROOT / "catalog/profiles").glob("*.toml"))
    profile_ids: list[str] = []
    profile_versions: dict[str, str] = {}
    profiles: list[dict] = []
    for path in profile_files:
        descriptor = tomllib.loads(path.read_text(encoding="utf-8"))
        profiles.append(descriptor)
        profile_ids.append(str(descriptor.get("profile_id")))
        profile_versions[str(descriptor.get("profile_id"))] = str(descriptor.get("profile_version"))
        fingerprint_files.add(path)
        if descriptor.get("schema_version") != 2:
            errors.append(f"profile must use schema_version 2: {path.name}")
        if descriptor.get("profile_version") != "1.1.0":
            errors.append(f"profile must use version 1.1.0: {path.name}")
        for required in (
            "gate_phases",
            "required_rule_families",
            "required_check_families",
            "permission_floor",
            "unknown_outcome_policy",
            "rollback_policy",
        ):
            if not descriptor.get(required):
                errors.append(f"profile lacks {required}: {path.name}")
    if (
        inventory.get("expected_profile_count") != len(PROFILE_IDS)
        or set(profile_ids) != set(PROFILE_IDS)
        or len(profile_ids) != len(PROFILE_IDS)
    ):
        errors.append(f"profile IDs must resolve exactly 16/16: observed={sorted(profile_ids)}")

    skill_agent = (
        CODEX_SKILL_AGENT_PATH.read_text(encoding="utf-8")
        if CODEX_SKILL_AGENT_PATH.is_file()
        else ""
    )
    if not skill_agent:
        errors.append("Codex Skill agent metadata is missing")
    for required in (
        'display_name: "Star-Control Operations"',
        'short_description: "Route development and code health through Star-Control"',
        'default_prompt: "Use $star-control-operations',
        'type: "mcp"',
        'value: "star-control"',
        "allow_implicit_invocation: true",
    ):
        if required not in skill_agent:
            errors.append(f"Codex Skill agent metadata is missing: {required}")
    dependency_types = re.findall(
        r'^\s*-\s+type:\s*"([^"]+)"\s*$', skill_agent, flags=re.MULTILINE
    )
    dependency_values = re.findall(
        r'^\s+value:\s*"([^"]+)"\s*$', skill_agent, flags=re.MULTILINE
    )
    if dependency_types != ["mcp"] or dependency_values != ["star-control"]:
        errors.append("Codex Skill metadata must declare exactly the star-control MCP dependency")

    missing_parallel_components = [
        path.relative_to(ROOT).as_posix()
        for path in CODEX_PARALLEL_COMPONENT_PATHS
        if not path.is_file()
    ]
    if missing_parallel_components:
        errors.append(
            f"parallel implementation Skill components are missing: {missing_parallel_components}"
        )
    parallel_skill = (
        CODEX_PARALLEL_COMPONENT_PATHS[0].read_text(encoding="utf-8")
        if CODEX_PARALLEL_COMPONENT_PATHS[0].is_file()
        else ""
    )
    for required in (
        "name: orchestrate-parallel-implementation",
        "중앙 작업 자체를 `create_goal`로 등록하지 않는다",
        "list_projects({})",
        "list_threads({limit: ...})",
        'model: "gpt-5.6-sol"',
        'thinking: "max"',
        'model: "gpt-5.6-terra"',
        'thinking: "high"',
        "BOOTSTRAP_ONLY bundle_id=<unique bundle_id>",
        "this is not a Bundle assignment",
        "complete Context Pack은 post-create identity",
        "target: {",
        "projectId: <list_projects projectId>",
        'environment: { type: "worktree" }',
        "goal_pursuit: required",
        "create_thread({",
        "wait_threads({ targets: [{ threadId, hostId, afterCursor }]",
        "read_thread({ threadId, hostId })",
        "send_message_to_thread({ threadId, prompt:",
        "clientThreadId",
        "THREAD_IDENTITY_CONFIRMED",
        "ACTIVATE_BUNDLE",
        "activation ACK와 Goal active",
        "0건 timeout 또는 복수 match",
        "SOL_REVIEW_PENDING",
        "EXISTING_GOAL_RESUMED",
        "awaiting_external_sol_review",
        "Sol 승인 전 `update_goal",
        "`VERIFIED`",
    ):
        if required not in parallel_skill:
            errors.append(f"parallel implementation Skill contract is missing: {required}")
    for forbidden in ("spawn_agent", "followup_task", "wait_agent", "interrupt_agent"):
        if forbidden in parallel_skill:
            errors.append(f"parallel implementation Skill retains obsolete collaboration API: {forbidden}")
    invalid_thread_call_patterns = (
        "create_thread({\n  message:",
        "create_thread({\n  project:",
    )
    for component_path in CODEX_PARALLEL_COMPONENT_PATHS:
        component_text = component_path.read_text(encoding="utf-8") if component_path.is_file() else ""
        for invalid_thread_call in invalid_thread_call_patterns:
            if invalid_thread_call in component_text:
                errors.append(
                    "parallel implementation rendered component retains actual non-schema "
                    f"create_thread field: {component_path.name} {invalid_thread_call}"
                )
            negative_candidate = f"{component_text}\n{invalid_thread_call} <invalid value>\n}})"
            if invalid_thread_call not in negative_candidate:
                errors.append(
                    "parallel implementation negative append detector is inactive: "
                    f"{component_path.name} {invalid_thread_call}"
                )
    parallel_agent = (
        CODEX_PARALLEL_COMPONENT_PATHS[1].read_text(encoding="utf-8")
        if CODEX_PARALLEL_COMPONENT_PATHS[1].is_file()
        else ""
    )
    for required in (
        'display_name: "Parallel Implementation"',
        'default_prompt: "Use $orchestrate-parallel-implementation',
        "allow_implicit_invocation: true",
    ):
        if required not in parallel_agent:
            errors.append(f"parallel implementation Skill metadata is missing: {required}")
    if "dependencies:" in parallel_agent:
        errors.append("parallel implementation Skill must not invent a direct tool dependency")
    parallel_scheduling = (
        CODEX_PARALLEL_COMPONENT_PATHS[3].read_text(encoding="utf-8")
        if CODEX_PARALLEL_COMPONENT_PATHS[3].is_file()
        else ""
    )
    for required in (
        "create_goal",
        "create_thread",
        "list_projects",
        "list_threads",
        "wait_threads",
        "read_thread",
        "send_message_to_thread",
        "clientThreadId",
        "threadId",
        "hostId",
        "afterCursor",
        "SOL_REVIEW_PENDING",
        "EXISTING_GOAL_RESUMED",
        "awaiting_external_sol_review",
        "GOAL_COMPLETE",
        "FINAL_VALIDATION -> VERIFIED",
        "BOOTSTRAP_ONLY",
        "THREAD_IDENTITY_CONFIRMED",
        "ACTIVATE_BUNDLE",
        "0건 timeout 또는 복수 match",
    ):
        if required not in parallel_scheduling:
            errors.append(f"parallel implementation lifecycle is missing: {required}")
    parallel_safety = (
        CODEX_PARALLEL_COMPONENT_PATHS[5].read_text(encoding="utf-8")
        if CODEX_PARALLEL_COMPONENT_PATHS[5].is_file()
        else ""
    )
    scenario_section = parallel_safety.partition("## 필수 forward scenario")[2].partition(
        "## 완료 증거"
    )[0]
    observed_scenarios = [
        int(value)
        for value in re.findall(r"(?m)^(\d+)\.\s", scenario_section)
    ]
    if observed_scenarios != list(range(1, 13)):
        errors.append(
            f"parallel implementation forward scenarios must be exact 1..12: observed={observed_scenarios}"
        )
    for required in (
        "일반 구현 요청은 새 Codex App thread 0건",
        "unique bundle_id, BOOTSTRAP_ONLY",
        "direct threadId/hostId도 project/worktree identity",
        "clientThreadId only는 bounded list_threads",
        "activation 전에는 Bundle assignment가 아니며",
        "same file/contract ownership은 한 Bundle",
        "preexisting dirty paths와 owned worktree baseline/head/fingerprint",
        "WORKER_COMPLETE 한 번 뒤 Sol review를 polling하지 않고",
        "자동 Goal turn 3회 뒤 blocked는",
        "blocked 뒤 correction/approval은 same threadId",
        "exact baseline_sha/head_sha/diff_fingerprint Sol 승인",
        "승인 없는 dependency 설치·삭제·push",
    ):
        if required not in parallel_safety:
            errors.append(f"parallel implementation safety contract is missing: {required}")

    routing_matrix = (
        CODEX_ROUTING_MATRIX_PATH.read_text(encoding="utf-8")
        if CODEX_ROUTING_MATRIX_PATH.is_file()
        else ""
    )
    if not routing_matrix:
        errors.append("Codex routing matrix is missing")
    feature_matches = list(
        re.finditer(r"^\|\s*([A-D]\d{2})\s*\|(.*)$", routing_matrix, flags=re.MULTILINE)
    )
    feature_rows = {
        match.group(1): match.group(2)
        for match in feature_matches
    }
    observed_feature_ids = [match.group(1) for match in feature_matches]
    if observed_feature_ids != FEATURE_IDS:
        errors.append(
            f"Codex routing matrix must cover exact ordered 23/23 features: observed={observed_feature_ids}"
        )
    for feature in features:
        feature_id = str(feature.get("id"))
        row = feature_rows.get(feature_id, "")
        mcp_actions = [str(value) for value in feature.get("mcp_actions", [])]
        expected_route = "MCP-first" if mcp_actions else "CLI-only"
        if expected_route not in row:
            errors.append(f"{feature_id} Codex route must be {expected_route}")
        surfaces = mcp_actions or [str(value) for value in feature.get("cli_commands", [])]
        for surface in surfaces:
            if f"`{surface}`" not in row:
                errors.append(f"{feature_id} Codex routing surface missing: {surface}")

    profile_matches = list(
        re.finditer(r"^\|\s*`([^`]+)`\s*\|(.*)$", routing_matrix, flags=re.MULTILINE)
    )
    profile_rows = {
        match.group(1): match.group(2)
        for match in profile_matches
    }
    observed_profile_ids = [match.group(1) for match in profile_matches]
    if observed_profile_ids != PROFILE_IDS:
        errors.append(
            f"Codex routing matrix must cover exact ordered 16/16 Profiles: observed={observed_profile_ids}"
        )
    for descriptor in profiles:
        profile_id = str(descriptor.get("profile_id"))
        default_stop_state = str(descriptor.get("default_stop_state"))
        if f"`{default_stop_state}`" not in profile_rows.get(profile_id, ""):
            errors.append(
                f"{profile_id} Codex routing default stop is stale: {default_stop_state}"
            )

    for descriptor in profiles:
        parent = descriptor.get("parent_profile")
        if parent and profile_versions.get(parent.get("profile_id")) != parent.get("profile_version"):
            errors.append(f"profile parent version is not exact: {descriptor.get('profile_id')}")

    profile_conformance_evidence: list[dict[str, str]] = []
    for role, value, executable_test in PROFILE_CONFORMANCE_REFS:
        parsed = split_ref(value, errors)
        if parsed is None:
            continue
        path, marker = parsed
        fingerprint_files.add(path)
        source = path.read_text(encoding="utf-8")
        if marker not in source:
            errors.append(f"Profile conformance {role} marker missing: {value}")
        if executable_test:
            verify_test_ref("C01", role, value, errors)
        path_text, _ = value.split("#", 1)
        profile_conformance_evidence.append(
            {
                "path": path_text,
                "marker": marker,
                "source_sha256": sha256_bytes(path.read_bytes()),
            }
        )
    if len(profile_conformance_evidence) != len(PROFILE_CONFORMANCE_REFS):
        errors.append("Profile conformance evidence must resolve exactly 10/10")

    runtime_names: list[str] = []
    for manifest_path, expected_name in RUNTIME_EXECUTABLES.items():
        path = relative_file(manifest_path, errors)
        if path is None:
            continue
        fingerprint_files.add(path)
        parsed = tomllib.loads(path.read_text(encoding="utf-8"))
        binaries = [entry.get("name") for entry in parsed.get("bin", [])]
        if expected_name not in binaries:
            errors.append(f"Runtime executable is missing: {expected_name}")
        else:
            runtime_names.append(expected_name)
    if (
        inventory.get("expected_runtime_executable_count") != len(RUNTIME_EXECUTABLES)
        or len(runtime_names) != len(RUNTIME_EXECUTABLES)
    ):
        errors.append(f"Runtime executable count mismatch: {len(runtime_names)}")

    error_catalog = relative_file("catalog/stable-error-codes.txt", errors)
    error_codes: list[str] = []
    if error_catalog is not None:
        fingerprint_files.add(error_catalog)
        error_codes = [
            line.strip()
            for line in error_catalog.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        if error_codes != sorted(set(error_codes)):
            errors.append("stable error catalog must be sorted and unique")
        if len(error_codes) != inventory.get("expected_stable_error_count"):
            errors.append(f"stable error catalog count mismatch: {len(error_codes)}")

    matrix_path = relative_file("docs/testing/mcp-verification-matrix.md", errors)
    matrix_ids: set[str] = set()
    if matrix_path is not None:
        fingerprint_files.add(matrix_path)
        matrix_ids = set(re.findall(r"MCP-[A-Z]+[0-9]{3}", matrix_path.read_text(encoding="utf-8")))
        mapped: set[str] = set()
        for base in (ROOT / "apps", ROOT / "crates"):
            for path in base.rglob("*.rs"):
                source = path.read_text(encoding="utf-8")
                mapped.update(re.findall(r"// matrix:.*?(MCP-[A-Z]+[0-9]{3})", source))
                for line in source.splitlines():
                    if "// matrix:" in line:
                        mapped.update(re.findall(r"MCP-[A-Z]+[0-9]{3}", line))
        if len(matrix_ids) != inventory.get("expected_mcp_matrix_count") or matrix_ids - mapped:
            errors.append(
                f"MCP matrix coverage mismatch: declared={len(matrix_ids)} mapped={len(matrix_ids & mapped)}"
            )

    git_untracked_top_levels(errors)

    fingerprint_lines = []
    for path in sorted(fingerprint_files):
        relative = path.relative_to(ROOT).as_posix()
        fingerprint_lines.append(f"{relative}\0{sha256_bytes(path.read_bytes())}")
    source_fingerprint = sha256_bytes("\n".join(fingerprint_lines).encode("utf-8"))
    feature_evidence = []
    for feature in features:
        def reference_evidence(value: str) -> dict[str, str]:
            path_text, marker = value.split("#", 1)
            path = ROOT.joinpath(*pathlib.PurePosixPath(path_text).parts)
            return {
                "path": path_text,
                "marker": marker,
                "source_sha256": sha256_bytes(path.read_bytes()),
            }

        owner_path = ROOT.joinpath(*pathlib.PurePosixPath(feature["owner_doc"]).parts)
        item = {
            "feature_id": feature["id"],
            "owner_document": {
                "path": feature["owner_doc"],
                "source_sha256": sha256_bytes(owner_path.read_bytes()),
            },
            "generated_schemas": [
                {
                    "path": f"specs/schemas/v1/{name}",
                    # Missing generated schemas are already reported above. Keep
                    # the audit result serializable so a failed generation gate
                    # produces actionable JSON instead of an uncaught KeyError.
                    "source_sha256": manifest_map.get(
                        name, sha256_bytes(b"missing-generated-schema")
                    ),
                }
                for name in feature["schema_files"]
            ],
            "handler_refs": [reference_evidence(value) for value in feature["handler_refs"]],
            "cli_commands": feature["cli_commands"],
            "product_surface_fingerprints": [
                sha256_bytes((ROOT / "apps/star-cli/src/main.rs").read_bytes()),
                sha256_bytes((ROOT / "apps/star-controller/src/main.rs").read_bytes()),
            ],
            "mcp_required": feature["mcp_required"],
            "mcp_actions": feature["mcp_actions"],
            "codex_required": feature["codex_required"],
            "codex_refs": [reference_evidence(value) for value in feature["codex_refs"]],
            "test_refs": {
                field.removesuffix("_test"): reference_evidence(feature[field])
                for field in TEST_FIELDS
            },
        }
        if feature["mcp_required"]:
            item["product_surface_fingerprints"].extend(
                [
                    sha256_bytes((ROOT / "apps/star-mcp/src/lib.rs").read_bytes()),
                    sha256_bytes(CORE_TOOL_PACKAGE_PATH.read_bytes()),
                    sha256_bytes(FIXED_MCP_PATH.read_bytes()),
                ]
            )
        if feature["codex_required"]:
            item["product_surface_fingerprints"].extend(
                reference["source_sha256"] for reference in item["codex_refs"]
            )
        item["product_surface_fingerprints"] = sorted(
            set(item["product_surface_fingerprints"])
        )
        item["feature_fingerprint"] = sha256_bytes(
            json.dumps(item, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode(
                "utf-8"
            )
        )
        feature_evidence.append(item)

    profile_evidence = []
    profile_by_id = {
        descriptor["profile_id"]: (descriptor, path)
        for descriptor, path in zip(profiles, profile_files, strict=True)
    }
    for profile_id in PROFILE_IDS:
        descriptor, path = profile_by_id[profile_id]
        descriptor_contract = dict(descriptor)
        # `DevelopmentProfileExtensionsV1` is a defaulted, non-skipped field in
        # the Rust descriptor.  TOML omits the table for profiles without an
        # extension, while serde serializes the semantic value as `{}`.
        descriptor_contract.setdefault("extensions", {})
        activation_inputs = {
            "parent_profile": descriptor.get("parent_profile"),
            "triggers": descriptor["triggers"],
            "stage_template": descriptor["stage_template"],
            "context_rules": descriptor["context_rules"],
            "route_hints": descriptor["route_hints"],
        }
        conformance_policy = {
            "required_rule_families": descriptor["required_rule_families"],
            "required_check_families": descriptor["required_check_families"],
            "gate_phases": descriptor["gate_phases"],
            "permission_actions": descriptor["permission_actions"],
            "approval_checkpoints": descriptor["approval_checkpoints"],
            "allowed_effect_classes": descriptor["allowed_effect_classes"],
            "permission_floor": descriptor["permission_floor"],
            "unknown_outcome_policy": descriptor["unknown_outcome_policy"],
            "rollback_policy": descriptor["rollback_policy"],
        }
        profile_evidence.append(
            {
                "profile_id": profile_id,
                "profile_version": descriptor["profile_version"],
                "definition_fingerprint": sha256_bytes(path.read_bytes()),
                "descriptor_definition_hash": canonical_json_fingerprint(
                    descriptor_contract
                ),
                "definition_source": {
                    "path": path.relative_to(ROOT).as_posix(),
                    "source_sha256": sha256_bytes(path.read_bytes()),
                },
                "activation_inputs_fingerprint": canonical_json_fingerprint(activation_inputs),
                "conformance_policy_fingerprint": canonical_json_fingerprint(conformance_policy),
                "required_rule_families": descriptor["required_rule_families"],
                "required_check_families": descriptor["required_check_families"],
                "gate_phases": descriptor["gate_phases"],
                "permission_actions": descriptor["permission_actions"],
                "approval_checkpoints": descriptor["approval_checkpoints"],
                "allowed_effect_classes": descriptor["allowed_effect_classes"],
                "permission_floor": descriptor["permission_floor"],
                "unknown_outcome_policy": descriptor["unknown_outcome_policy"],
                "rollback_policy": descriptor["rollback_policy"],
                "conformance_refs": profile_conformance_evidence,
            }
        )

    source_evidence = {
        "schema_id": "star.product-source-evidence",
        "schema_version": 1,
        "source_fingerprint": source_fingerprint,
        "inventory_fingerprint": sha256_bytes(INVENTORY_PATH.read_bytes()),
        "generated_schema_manifest_fingerprint": sha256_bytes(SCHEMA_MANIFEST.read_bytes()),
        "stable_error_catalog_fingerprint": sha256_bytes(error_catalog.read_bytes())
        if error_catalog is not None
        else sha256_bytes(b""),
        "mcp_matrix_fingerprint": sha256_bytes(matrix_path.read_bytes())
        if matrix_path is not None
        else sha256_bytes(b""),
        "feature_count": len(features),
        "profile_count": len(profile_ids),
        "runtime_executables": runtime_names,
        "generated_schema_count": len(manifest_entries),
        "stable_error_count": len(error_codes),
        "mcp_matrix_count": len(matrix_ids),
        "features": feature_evidence,
        "profiles": profile_evidence,
    }
    source_evidence["evidence_fingerprint"] = sha256_bytes(
        json.dumps(
            source_evidence, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
    )
    evidence_bytes = (
        json.dumps(source_evidence, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    ).encode("utf-8")
    write_evidence = sys.argv[1:] == ["--write-evidence"]
    if sys.argv[1:] not in ([], ["--write-evidence"]):
        errors.append("usage: check_product_inventory.py [--write-evidence]")
    if write_evidence:
        if errors:
            errors.append("product source evidence was not written because the audit failed")
        else:
            SOURCE_EVIDENCE_PATH.write_bytes(evidence_bytes)
    elif not SOURCE_EVIDENCE_PATH.is_file() or SOURCE_EVIDENCE_PATH.read_bytes() != evidence_bytes:
        errors.append("product source evidence is missing or stale; run --write-evidence")
    result = {
        "audit_schema_version": 1,
        "features": f"{len(features)}/{len(FEATURE_IDS)}",
        "profiles": f"{len(profile_ids)}/{len(PROFILE_IDS)}",
        "runtime_executables": f"{len(runtime_names)}/{len(RUNTIME_EXECUTABLES)}",
        "generated_schemas": len(manifest_entries),
        "stable_error_codes": len(error_codes),
        "mcp_matrix": f"{len(matrix_ids)}/{inventory.get('expected_mcp_matrix_count')}",
        "source_fingerprint": source_fingerprint,
        "source_evidence_fingerprint": source_evidence["evidence_fingerprint"],
        "status": "pass" if not errors else "fail",
        "errors": errors,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
