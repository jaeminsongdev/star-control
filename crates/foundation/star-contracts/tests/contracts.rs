use std::{fs, path::PathBuf};

use star_contracts::{
    canonical::{canonical_sha256, jcs_bytes},
    fixed_mcp::{FIXED_TOOLS, RiskLane, fixed_input_valid, fixed_tool},
    ids::{ApprovalId, OperationId, RequestId, ToolCacheId, ToolTrustId},
    ipc::IpcHello,
    manifest::{ManifestError, ManifestSource, UpdatePolicy, parse_manifest_v1, risk_lane},
    schema::generated_documents,
};

#[test]
fn generated_ids_have_valid_default_values() {
    for id in [
        RequestId::default().to_string(),
        OperationId::default().to_string(),
        ApprovalId::default().to_string(),
        ToolTrustId::default().to_string(),
        ToolCacheId::default().to_string(),
    ] {
        assert!(id.len() > 4);
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf()
}

#[test]
fn fixed_mcp_surface_is_exactly_twelve_tools_in_contract_order() {
    assert_eq!(FIXED_TOOLS.len(), 12);
    assert_eq!(FIXED_TOOLS[0].name, "star_tool_search");
    assert_eq!(FIXED_TOOLS[11].name, "star_approval_resolve");
    assert_eq!(fixed_tool("tools/listChanged"), None);
}

#[test]
// matrix: MCP-M001 MCP-M004
fn manifest_valid_invalid_and_duplicate_fixtures_are_stable() {
    let fixture_root = root().join("specs/fixtures/mcp/manifests");
    let valid =
        fs::read_to_string(fixture_root.join("valid/tool-package-manifest-v1.toml")).unwrap();
    let manifest = parse_manifest_v1(&valid, ManifestSource::User).unwrap();
    assert_eq!(manifest.actions.len(), 1);
    assert_eq!(manifest.actions[0].expected_duration_ms, 1_000);
    let long = parse_manifest_v1(
        &valid.replace(
            "permission_actions = [\"local_read\", \"process_run\"]",
            "permission_actions = [\"local_read\", \"process_run\"]\nexpected_duration_ms = 30001",
        ),
        ManifestSource::User,
    )
    .unwrap();
    assert_eq!(long.actions[0].expected_duration_ms, 30_001);
    for name in [
        "tool-package-manifest-unknown-key.toml",
        "tool-package-manifest-duplicate-key.toml",
    ] {
        let invalid = fs::read_to_string(fixture_root.join("invalid").join(name)).unwrap();
        assert!(
            parse_manifest_v1(&invalid, ManifestSource::User).is_err(),
            "{name} must be rejected"
        );
    }
    let compatible =
        fs::read_to_string(fixture_root.join("valid/version-compatible-v1.toml")).unwrap();
    let compatible = parse_manifest_v1(&compatible, ManifestSource::User).unwrap();
    assert_eq!(
        compatible.executables[0].update_policy,
        UpdatePolicy::VersionCompatible
    );

    let future =
        fs::read_to_string(fixture_root.join("invalid/tool-package-manifest-future-version.toml"))
            .unwrap();
    assert!(matches!(
        parse_manifest_v1(&future, ManifestSource::User),
        Err(ManifestError::FutureFormatVersion(2))
    ));
}

#[test]
fn manifest_nested_inline_and_array_table_keys_fail_closed() {
    let valid = include_str!("../../../../specs/examples/valid/tool-package-manifest-v1.toml");
    let cases = [
        valid.replace(
            "[actions.output]\n",
            "[actions.output]\nunknown_output_key = true\n",
        ),
        valid.replace(
            "format = \"text\"",
            "format = \"text\"\nformat = \"json\"",
        ),
        valid.replace(
            "backend_kinds = [\"process\"]",
            "backend_kinds = [\"process\"]\nreplaces = [{ package_id = \"user.fake.old\", version_req = \"*\", unknown = true }]",
        ),
        valid.replace(
            "backend_kinds = [\"process\"]",
            "backend_kinds = [\"process\"]\nreplaces = [{ package_id = \"user.fake.old\", package_id = \"user.fake.other\", version_req = \"*\" }]",
        ),
        valid.replace(
            "required = true",
            "required = true\nunknown_parameter_key = true",
        ),
        valid.replace("name = \"value\"", "name = \"value\"\nname = \"other\""),
    ];
    for candidate in cases {
        assert!(
            parse_manifest_v1(&candidate, ManifestSource::User).is_err(),
            "nested, inline and array-table unknown or duplicate keys must fail closed"
        );
    }
}

#[test]
// matrix: MCP-S001
fn project_manifest_cannot_claim_trust_or_introduce_unknown_policy_keys() {
    let valid = include_str!("../../../../specs/examples/valid/tool-package-manifest-v1.toml");
    assert!(
        parse_manifest_v1(
            &format!("{valid}\ntrust = \"trusted\"\n"),
            ManifestSource::Project,
        )
        .is_err()
    );
    assert!(
        parse_manifest_v1(
            &valid.replace(
                "update_policy = \"pinned_hash\"",
                "update_policy = \"follow_path\""
            ),
            ManifestSource::Project,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-M002 MCP-M003
fn manifest_accepts_json_stdio_and_release_core_only() {
    let json_stdio = r#"
format_version = 1
package_id = "user.fake.json"
package_version = "1.0.0"
display_name = "Fake JSON"
description = "Contract fixture JSON-STDIO process tool."
backend_kinds = ["process"]

[[executables]]
executable_id = "fake-json"
locator_kind = "absolute"
path = "C:\\Tools\\fake-json.exe"
update_policy = "pinned_hash"
sha256 = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
protocol = "star_json_stdio_v1"
interface_version_req = "*"

[[actions]]
tool_id = "user.fake.json.run"
backend_kind = "process"
backend_ref = "fake-json"
display_name = "Run fake JSON"
summary = "Returns JSON."
description = "Contract fixture action."
permission_actions = ["local_read", "process_run"]
paid_action = "no"
idempotency = "read_only"
output_schema_file = "schemas/output.json"

[actions.output]
format = "json"
encoding = "utf8"
stderr_encoding = "utf8"
inline_limit_bytes = 1024
overflow = "artifact"
stdout_role = "data"
stderr_role = "log"
"#;
    assert!(parse_manifest_v1(json_stdio, ManifestSource::User).is_ok());

    let core = r#"
format_version = 1
package_id = "star.control.core"
package_version = "1.0.0"
display_name = "Star Control Core"
description = "Release-owned typed Controller actions."
required = true
backend_kinds = ["controller_command"]

[[actions]]
tool_id = "star.core.goal.start"
backend_kind = "controller_command"
backend_ref = "goal.start"
display_name = "Start Goal"
summary = "Start one goal."
description = "Starts a durable Star-Control goal."
permission_actions = ["local_write"]
paid_action = "no"
idempotency = "non_idempotent"
"#;
    assert!(parse_manifest_v1(core, ManifestSource::Release).is_ok());
    assert!(parse_manifest_v1(core, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M003
fn required_release_core_package_declares_exactly_thirteen_owned_actions() {
    let source =
        fs::read_to_string(root().join("catalog/tool-packages/star-control-core.toml")).unwrap();
    let manifest = parse_manifest_v1(&source, ManifestSource::Release).unwrap();
    assert!(manifest.required);
    assert_eq!(manifest.package_id, "star.control.core");
    let actual: Vec<_> = manifest
        .actions
        .iter()
        .map(|action| {
            (
                action.tool_id.as_str(),
                action.backend_ref.as_str(),
                risk_lane(&action.permission_actions).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            ("star.core.goal.start", "goal.start", RiskLane::WriteClosed),
            (
                "star.core.goal.answer",
                "goal.answer",
                RiskLane::WriteClosed
            ),
            ("star.core.plan.get", "plan.get", RiskLane::ReadClosed),
            (
                "star.core.plan.update",
                "plan.update",
                RiskLane::WriteClosed
            ),
            (
                "star.core.run.continue",
                "run.continue",
                RiskLane::DestructiveOpen
            ),
            ("star.core.status.get", "goal.status", RiskLane::ReadClosed),
            ("star.core.goal.pause", "goal.pause", RiskLane::WriteClosed),
            (
                "star.core.goal.resume",
                "goal.resume",
                RiskLane::WriteClosed
            ),
            (
                "star.core.goal.cancel",
                "goal.cancel",
                RiskLane::DestructiveOpen
            ),
            (
                "star.core.evidence.get",
                "evidence.get",
                RiskLane::ReadClosed
            ),
            (
                "star.core.merge.status",
                "merge.status",
                RiskLane::ReadClosed
            ),
            ("star.core.handoff.get", "handoff.get", RiskLane::ReadClosed),
            ("star.core.doctor", "doctor.run", RiskLane::ReadClosed),
        ]
    );
}

#[test]
// matrix: MCP-M018 MCP-M019 MCP-M021 MCP-M025
fn manifest_rejects_future_and_project_unsafe_modes_but_allows_disabled_draft() {
    let valid = include_str!("../../../../specs/examples/valid/tool-package-manifest-v1.toml");
    assert!(matches!(
        parse_manifest_v1(
            &valid.replacen("format_version = 1", "format_version = 2", 1),
            ManifestSource::User
        ),
        Err(star_contracts::manifest::ManifestError::FutureFormatVersion(2))
    ));
    assert!(
        parse_manifest_v1(
            &valid.replace(
                "architectures = [\"x86_64\"]",
                "working_directory = \"fixed\"\nfixed_working_directory = \"C:\\\\Tools\""
            ),
            ManifestSource::Project,
        )
        .is_err()
    );
    assert!(
        parse_manifest_v1(
            &valid.replace(
                "interface_version_req = \"*\"",
                "interface_version_req = \"*\"\nproduct_version_req = \">=1.0.0\""
            ),
            ManifestSource::User,
        )
        .is_err()
    );

    let draft = valid
        .replace("enabled = true", "enabled = false")
        .split("[[actions]]")
        .next()
        .unwrap()
        .to_owned();
    assert!(parse_manifest_v1(&draft, ManifestSource::User).is_ok());
    let enabled_zero = draft.replace("enabled = false", "enabled = true");
    assert!(parse_manifest_v1(&enabled_zero, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M007
fn manifest_rejects_a_binding_to_an_unknown_parameter() {
    let valid = include_str!("../../../../specs/examples/valid/tool-package-manifest-v1.toml");
    assert!(
        parse_manifest_v1(
            &format!("{valid}\n[[actions.argv]]\nkind = \"positional\"\ninput = \"missing\"\n"),
            ManifestSource::User,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-M008
fn manifest_rejects_two_stdin_bindings_for_one_action() {
    let source = argv_fixture().replace(
        "[[actions.argv]]\nkind = \"positional\"\ninput = \"value\"",
        "[[actions.argv]]\nkind = \"stdin_text\"\ninput = \"value\"\n\n[[actions.argv]]\nkind = \"stdin_text\"\ninput = \"value\"",
    );
    assert!(parse_manifest_v1(&source, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M009
fn manifest_rejects_overlapping_exit_code_sets() {
    let source = argv_fixture().replace("empty = []", "empty = [0]");
    assert!(parse_manifest_v1(&source, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M010
fn manifest_rejects_paid_metadata_permission_mismatches() {
    let missing_permission =
        argv_fixture().replace("paid_action = \"no\"", "paid_action = \"yes\"");
    assert!(parse_manifest_v1(&missing_permission, ManifestSource::User).is_err());
    let missing_metadata = argv_fixture().replace(
        "permission_actions = [\"local_read\", \"process_run\"]",
        "permission_actions = [\"local_read\", \"process_run\", \"paid_action\"]",
    );
    assert!(parse_manifest_v1(&missing_metadata, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M011
fn manifest_rejects_unknown_permission_action_ids() {
    let valid = argv_fixture();
    assert!(
        parse_manifest_v1(
            &valid.replace(
                "permission_actions = [\"local_read\", \"process_run\"]",
                "permission_actions = [\"local_read\", \"process_run\", \"made_up_permission\"]",
            ),
            ManifestSource::User,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-M012
fn manifest_rejects_shell_script_hosts_scripts_and_path_lookup() {
    for path in [
        r"C:\\Windows\\System32\\cmd.exe",
        r"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        r"C:\\Windows\\System32\\wscript.exe",
        r"C:\\Tools\\tool.ps1",
        "cmd.exe",
    ] {
        let source = argv_fixture().replace(r"C:\\Tools\\fake-echo.exe", path);
        assert!(
            parse_manifest_v1(&source, ManifestSource::User).is_err(),
            "forbidden shell or script path accepted: {path}"
        );
    }
    let anchored_shell = argv_fixture()
        .replace(
            "locator_kind = \"absolute\"",
            "locator_kind = \"anchor_relative\"",
        )
        .replace(
            "path = \"C:\\\\Tools\\\\fake-echo.exe\"",
            "path = \"cmd.exe\"\nanchor = \"program_files\"",
        );
    assert!(parse_manifest_v1(&anchored_shell, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M013
fn manifest_rejects_invalid_locator_field_combinations() {
    let absolute_with_anchor = argv_fixture().replace(
        "path = \"C:\\\\Tools\\\\fake-echo.exe\"",
        "path = \"C:\\\\Tools\\\\fake-echo.exe\"\nanchor = \"program_files\"",
    );
    assert!(parse_manifest_v1(&absolute_with_anchor, ManifestSource::User).is_err());

    let anchor_without_anchor = argv_fixture()
        .replace(
            "locator_kind = \"absolute\"",
            "locator_kind = \"anchor_relative\"",
        )
        .replace(
            "path = \"C:\\\\Tools\\\\fake-echo.exe\"",
            "path = \"tool.exe\"",
        );
    assert!(parse_manifest_v1(&anchor_without_anchor, ManifestSource::User).is_err());

    let ref_with_path = argv_fixture()
        .replace(
            "locator_kind = \"absolute\"",
            "locator_kind = \"location_ref\"",
        )
        .replace(
            "path = \"C:\\\\Tools\\\\fake-echo.exe\"",
            "path = \"C:\\\\Tools\\\\fake-echo.exe\"\nlocation_ref = \"tool.install\"",
        );
    assert!(parse_manifest_v1(&ref_with_path, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M014
fn manifest_rejects_version_compatible_without_a_probe() {
    let source = argv_fixture()
        .replace("update_policy = \"pinned_hash\"", "update_policy = \"version_compatible\"")
        .replace(
            "sha256 = \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
            "authenticode_policy = \"require_subject\"\nauthenticode_subject = \"Example Publisher\"\n",
        );
    assert!(parse_manifest_v1(&source, ManifestSource::User).is_err());
}

#[test]
// matrix: MCP-M020
fn manifest_requires_output_contract_and_schema_for_json_and_jsonl() {
    assert!(
        parse_manifest_v1(
            &argv_fixture().replace("[actions.output]", "# output removed"),
            ManifestSource::User,
        )
        .is_err()
    );
    for format in ["json", "jsonl"] {
        let source = argv_fixture().replace("format = \"text\"", &format!("format = \"{format}\""));
        assert!(parse_manifest_v1(&source, ManifestSource::User).is_err());
    }
}

fn argv_fixture() -> &'static str {
    include_str!("../../../../specs/examples/valid/tool-package-manifest-v1.toml")
}

fn json_stdio_fixture() -> String {
    argv_fixture()
        .replace("protocol = \"argv_v1\"", "protocol = \"star_json_stdio_v1\"")
        .replace(
            "description = \"Contract fixture action.\"",
            "description = \"Contract fixture action.\"\noutput_schema_file = \"output.json\"",
        )
        .replace(
            "[[actions.argv]]\nkind = \"positional\"\ninput = \"value\"\n\n[actions.exit_codes]\nsuccess = [0]\nempty = []\nwarning = []\nretryable = []\n\n",
            "",
        )
}

#[test]
fn appcontainer_adapter_contract_rejects_unbrokered_or_privileged_manifests() {
    let valid = json_stdio_fixture().replace(
        "interface_version_req = \"*\"",
        "interface_version_req = \"*\"\nworking_directory = \"artifact_root\"\nisolation_compatibility = [\"appcontainer_adapter\"]",
    );
    assert!(parse_manifest_v1(&valid, ManifestSource::User).is_ok());
    for invalid in [
        valid.replace(
            "protocol = \"star_json_stdio_v1\"",
            "protocol = \"argv_v1\"",
        ),
        valid.replace(
            "working_directory = \"artifact_root\"",
            "working_directory = \"project_root\"",
        ),
        valid.replace(
            "permission_actions = [\"local_read\", \"process_run\"]",
            "permission_actions = [\"network_read\", \"process_run\"]",
        ),
        valid.replace("paid_action = \"no\"", "paid_action = \"yes\""),
    ] {
        assert!(parse_manifest_v1(&invalid, ManifestSource::User).is_err());
    }
}

#[test]
// matrix: MCP-M001 MCP-M002
fn complete_argv_and_json_stdio_manifests_round_trip() {
    for source in [argv_fixture().to_owned(), json_stdio_fixture()] {
        let parsed = parse_manifest_v1(&source, ManifestSource::User).unwrap();
        let encoded = toml::to_string(&parsed).unwrap();
        let reparsed = parse_manifest_v1(&encoded, ManifestSource::User).unwrap();
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            serde_json::to_value(reparsed).unwrap()
        );
    }
}

#[test]
// matrix: MCP-M003
fn controller_commands_are_release_core_only() {
    let core = r#"
format_version = 1
package_id = "star.control.core"
package_version = "1.0.0"
display_name = "Star Control Core"
description = "Required release controller commands."
required = true
backend_kinds = ["controller_command"]

[[actions]]
tool_id = "star.core.plan.get"
backend_kind = "controller_command"
backend_ref = "plan.get"
display_name = "Get plan"
summary = "Returns the current plan."
description = "Returns the current controller-owned plan."
permission_actions = ["local_read"]
paid_action = "no"
idempotency = "read_only"
"#;
    assert!(parse_manifest_v1(core, ManifestSource::Release).is_ok());
    assert!(parse_manifest_v1(core, ManifestSource::User).is_err());
    assert!(
        parse_manifest_v1(
            &core.replace(
                "backend_ref = \"plan.get\"",
                "backend_ref = \"unknown.command\""
            ),
            ManifestSource::Release,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-M015 MCP-M021 MCP-M024
fn probe_regex_product_version_and_subject_rules_fail_closed() {
    let no_probe = argv_fixture().replace(
        "interface_version_req = \"*\"",
        "interface_version_req = \"*\"\nproduct_version_req = \">=1.0.0\"",
    );
    assert!(parse_manifest_v1(&no_probe, ManifestSource::User).is_err());

    let compatible = argv_fixture()
        .replace(
            "update_policy = \"pinned_hash\"\nsha256 = \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"",
            "update_policy = \"version_compatible\"\nauthenticode_policy = \"require_subject\"\nauthenticode_subject = \"Example Publisher\"",
        )
        .replace(
            "architectures = [\"x86_64\"]",
            "architectures = [\"x86_64\"]\n\n[executables.probe]\nkind = \"argv\"\nargs = [\"--version\"]\noutput_format = \"semver_line\"\nversion_pattern = \"^(?<product>[0-9]+\\\\.[0-9]+\\\\.[0-9]+)$\"",
        );
    assert!(parse_manifest_v1(&compatible, ManifestSource::User).is_ok());
    assert!(
        parse_manifest_v1(
            &compatible.replace(
                "authenticode_policy = \"require_subject\"\nauthenticode_subject = \"Example Publisher\"",
                "authenticode_policy = \"record\"",
            ),
            ManifestSource::User,
        )
        .is_err()
    );
    assert!(
        parse_manifest_v1(
            &compatible.replace(
                "version_pattern = \"^(?<product>[0-9]+\\\\.[0-9]+\\\\.[0-9]+)$\"",
                &format!("version_pattern = \"(?<product>{})\"", "x".repeat(257)),
            ),
            ManifestSource::User,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-M016 MCP-M019 MCP-M022
fn path_project_and_environment_boundaries_are_static_fail_closed() {
    for path in [
        r"\\server\\share\\tool.exe",
        r"\\?\\C:\\Tools\\tool.exe",
        r"C:\\Tools\\tool.exe:stream",
    ] {
        assert!(
            parse_manifest_v1(
                &argv_fixture().replace(r"C:\\Tools\\fake-echo.exe", path),
                ManifestSource::User,
            )
            .is_err(),
            "unsafe path accepted: {path}"
        );
    }

    let fixed = argv_fixture().replace(
        "architectures = [\"x86_64\"]",
        "architectures = [\"x86_64\"]\nworking_directory = \"fixed\"\nfixed_working_directory = \"C:\\\\Tools\"",
    );
    assert!(parse_manifest_v1(&fixed, ManifestSource::User).is_ok());
    assert!(parse_manifest_v1(&fixed, ManifestSource::Project).is_err());

    for names in [
        "environment_allow = [\"PATH\"]",
        "environment_allow = [\"STAR_VALUE\", \"star_value\"]",
    ] {
        let source = argv_fixture().replace(
            "architectures = [\"x86_64\"]",
            &format!("architectures = [\"x86_64\"]\n{names}"),
        );
        assert!(parse_manifest_v1(&source, ManifestSource::User).is_err());
    }
}

#[test]
// matrix: MCP-M018 MCP-M025
fn future_versions_and_disabled_drafts_never_become_executable() {
    assert!(matches!(
        parse_manifest_v1(
            &argv_fixture().replace("format_version = 1", "format_version = 2"),
            ManifestSource::User,
        ),
        Err(ManifestError::FutureFormatVersion(2))
    ));

    let disabled = r#"
format_version = 1
package_id = "user.scaffold.disabled"
package_version = "0.1.0"
display_name = "Disabled draft"
description = "Draft metadata only."
enabled = false
backend_kinds = ["process"]

[[executables]]
executable_id = "tool"
locator_kind = "absolute"
path = "C:\\Tools\\draft.exe"
update_policy = "pinned_hash"
sha256 = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
protocol = "argv_v1"
interface_version_req = "*"
"#;
    assert!(parse_manifest_v1(disabled, ManifestSource::User).is_ok());
    assert!(
        parse_manifest_v1(
            &disabled.replace("enabled = false", "enabled = true"),
            ManifestSource::User,
        )
        .is_err()
    );
}

#[test]
// matrix: MCP-R017 MCP-R018
fn project_sources_cannot_follow_paths_or_dispatch_controller_commands() {
    let follow_path = argv_fixture()
        .replace("update_policy = \"pinned_hash\"", "update_policy = \"follow_path\"")
        .replace(
            "sha256 = \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
            "",
        );
    assert!(parse_manifest_v1(&follow_path, ManifestSource::Project).is_err());

    let controller = r#"
format_version = 1
package_id = "star.control.core"
package_version = "1.0.0"
display_name = "Core"
description = "Controller command attempt."
backend_kinds = ["controller_command"]
[[actions]]
tool_id = "star.core.plan.get"
backend_kind = "controller_command"
backend_ref = "plan.get"
display_name = "Plan"
summary = "Read plan."
description = "Read plan from controller."
permission_actions = ["local_read"]
paid_action = "no"
idempotency = "read_only"
"#;
    assert!(parse_manifest_v1(controller, ManifestSource::Project).is_err());
}

#[test]
fn permissions_calculate_contract_lanes() {
    assert_eq!(
        risk_lane(&["local_read".to_owned(), "process_run".to_owned()]).unwrap(),
        RiskLane::ReadClosed
    );
    assert_eq!(
        risk_lane(&["external_write".to_owned()]).unwrap(),
        RiskLane::WriteOpen
    );
    assert_eq!(
        risk_lane(&["local_delete".to_owned()]).unwrap(),
        RiskLane::DestructiveClosed
    );
}

#[test]
// matrix: MCP-H001
fn jcs_matches_the_rfc_8785_official_canonicalization_vector_and_hash() {
    let fixture_root = root().join("specs/fixtures/mcp/canonicalization");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture_root.join("rfc8785-input.json")).unwrap())
            .unwrap();
    let expected = fs::read_to_string(fixture_root.join("rfc8785-expected.txt")).unwrap();
    assert_eq!(
        String::from_utf8(jcs_bytes(&value).unwrap()).unwrap(),
        expected.trim_end()
    );
    assert_eq!(
        canonical_sha256(&value).unwrap().as_str(),
        fs::read_to_string(fixture_root.join("rfc8785-sha256.txt"))
            .unwrap()
            .trim()
    );
}

#[test]
// matrix: MCP-H010
fn jcs_does_not_unicode_normalize_distinct_argument_strings() {
    let composed = canonical_sha256(&serde_json::json!({"value":"é"})).unwrap();
    let decomposed = canonical_sha256(&serde_json::json!({"value":"e\u{301}"})).unwrap();
    assert_ne!(composed, decomposed);
}

#[test]
fn fixed_mcp_query_and_action_argument_bounds_match_the_advertised_surface() {
    assert!(!fixed_input_valid(
        "star_tool_search",
        serde_json::json!({"query":format!("{}x", " ".repeat(256))}),
    ));

    let maximum = 4 * 1024 * 1024;
    let inner = serde_json::json!({"value":"x".repeat(maximum - br#"{"value":""}"#.len())});
    assert_eq!(jcs_bytes(&inner).unwrap().len(), maximum);
    assert!(fixed_input_valid(
        "star_tool_call_read_closed",
        serde_json::json!({
            "tool_id":"user.boundary.read",
            "descriptor_hash":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "arguments":inner
        }),
    ));
}

#[test]
// matrix: MCP-M017
fn generated_schemas_match_checked_in_output() {
    for (name, document) in generated_documents() {
        let expected = serde_json::to_vec_pretty(&document).unwrap();
        let actual = fs::read(root().join("specs/schemas/v1").join(name)).unwrap();
        assert_eq!(actual, expected, "schema drift: {name}");
    }
}

#[test]
fn v1_ipc_fixture_decodes_with_a_prefixed_request_id() {
    let bytes = fs::read(root().join("specs/compatibility/ipc/v1-hello.json")).unwrap();
    let hello: IpcHello = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(hello.schema_id, "star.ipc.hello");
}
