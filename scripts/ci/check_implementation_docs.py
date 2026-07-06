#!/usr/bin/env python3
"""Validate that canonical implementation documents and CI checks are wired."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

BRIEF_DOCS = tuple(
    f"docs/implementation/briefs/{name}"
    for name in (
        "README.md",
        "E01-schema-validator.md",
        "E02-state-store.md",
        "E03-artifact-layout-writer.md",
        "E04-provider-registry.md",
        "E05-fake-provider-adapter.md",
        "E06-router-engine.md",
        "E07-execution-engine.md",
        "E08-cli-fake-flow.md",
        "E09-star-sentinel-p0.md",
        "E10-validation-engine.md",
        "E11-integration-smoke.md",
    )
)

REQUIRED_DOCS = (
    "docs/decisions/0002-runtime-stack.md",
    "docs/decisions/0003-fake-provider-instance.md",
    "docs/decisions/0004-star-sentinel-p0-scope.md",
    "docs/decisions/0005-full-implementation-defaults.md",
    "docs/implementation/target-architecture.md",
    "docs/implementation/current-repository-map.md",
    "docs/implementation/repository-layout.md",
    "docs/implementation/refactoring-hardcoding-guidelines.md",
    "docs/implementation/complete-implementation-roadmap.md",
    "docs/implementation/data-contracts.md",
    "docs/implementation/handoff-vocabularies.md",
    "docs/implementation/run-lifecycle.md",
    "docs/implementation/artifact-layout.md",
    "docs/implementation/artifact-naming.md",
    "docs/implementation/state-store.md",
    "docs/implementation/state-store-recovery.md",
    "docs/implementation/schema-validator.md",
    "docs/implementation/provider-system.md",
    "docs/implementation/config-system.md",
    "docs/implementation/router-decision-matrix.md",
    "docs/implementation/router-engine.md",
    "docs/implementation/execution-engine.md",
    "docs/implementation/validation-engine.md",
    "docs/implementation/validation-handoff.md",
    "docs/implementation/star-sentinel-p0-contracts.md",
    "docs/implementation/star-sentinel-p0-implementation-split.md",
    "docs/implementation/star-sentinel-full-spec.md",
    "docs/implementation/approval-review-flow.md",
    "docs/implementation/policy-profiles.md",
    "docs/implementation/cli-command-reference.md",
    "docs/implementation/cli-daemon-api-ui.md",
    "docs/implementation/daemon-contract.md",
    "docs/implementation/api-contract.md",
    "docs/implementation/ui-shell-contract.md",
    "docs/implementation/security-cost-observability.md",
    "docs/implementation/security-privacy-observability-contracts.md",
    "docs/implementation/testing-ci-release.md",
    "docs/implementation/ci-contract-validation.md",
    "docs/implementation/release-readiness.md",
    "docs/implementation/codex-long-run-workflow.md",
    "docs/implementation/codex-work-queue.md",
    "docs/implementation/codex-work-queue-current.md",
    "docs/implementation/codex-pr-template.md",
    "docs/implementation/codex-validation-report.md",
    *BRIEF_DOCS,
)

REQUIRED_EXAMPLE_DIRS = (
    "examples/core",
    "examples/runs/J-0001",
    "examples/provider-contracts",
    "examples/config-contracts",
    "examples/router-contracts",
    "examples/execution-contracts",
    "examples/validation-contracts",
    "examples/cli-contracts",
    "examples/surface-contracts",
    "examples/security-contracts",
    "examples/release-contracts",
    "builtin-tools/star-sentinel/examples/p0",
)

REQUIRED_CI_COMMANDS = (
    "scripts/ci/check_repo_policy.py",
    "scripts/ci/check_data_formats.py",
    "scripts/ci/check_manifest_contracts.py",
    "scripts/ci/check_star_sentinel_naming.py",
    "scripts/ci/check_schema_examples.py",
    "scripts/ci/check_implementation_docs.py",
    "scripts/ci/check_work_queue_consistency.py",
)

LOCAL_RUNNER = "scripts/ci/run_all.py"


def read_text(relative_path: str) -> str:
    return (ROOT / relative_path).read_text(encoding="utf-8")


def check_paths(paths: tuple[str, ...], errors: list[str]) -> None:
    for relative_path in paths:
        path = ROOT / relative_path
        if not path.exists():
            errors.append(f"missing required path: {relative_path}")
            continue
        if path.is_file() and not path.read_text(encoding="utf-8").strip():
            errors.append(f"required file is empty: {relative_path}")


def check_ci_commands(errors: list[str]) -> None:
    workflow = read_text(".github/workflows/ci.yml")
    for command in REQUIRED_CI_COMMANDS:
        if command not in workflow:
            errors.append(f"CI workflow does not reference {command}")


def check_local_runner(errors: list[str]) -> None:
    runner_path = ROOT / LOCAL_RUNNER
    if not runner_path.is_file():
        errors.append(f"missing local CI runner: {LOCAL_RUNNER}")
        return

    runner = runner_path.read_text(encoding="utf-8")
    for command in REQUIRED_CI_COMMANDS:
        if command not in runner:
            errors.append(f"local CI runner does not reference {command}")


def main() -> int:
    errors: list[str] = []
    check_paths(REQUIRED_DOCS, errors)
    check_paths(REQUIRED_EXAMPLE_DIRS, errors)
    check_ci_commands(errors)
    check_local_runner(errors)

    if errors:
        print("ERROR: implementation documentation check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        "implementation documentation check passed: "
        f"{len(REQUIRED_DOCS)} doc(s), {len(REQUIRED_EXAMPLE_DIRS)} example dir(s), "
        "local runner wired"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
