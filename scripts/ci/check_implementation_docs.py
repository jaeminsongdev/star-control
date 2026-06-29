#!/usr/bin/env python3
"""Validate that canonical implementation documents and CI checks are wired."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED_DOCS = (
    "docs/implementation/target-architecture.md",
    "docs/implementation/current-repository-map.md",
    "docs/implementation/repository-layout.md",
    "docs/implementation/data-contracts.md",
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
    "docs/implementation/codex-long-run-workflow.md",
    "docs/implementation/codex-work-queue.md",
    "docs/implementation/codex-pr-template.md",
    "docs/implementation/codex-validation-report.md",
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
    "builtin-tools/star-sentinel/examples/p0",
)

REQUIRED_CI_COMMANDS = (
    "scripts/ci/check_repo_policy.py",
    "scripts/ci/check_data_formats.py",
    "scripts/ci/check_manifest_contracts.py",
    "scripts/ci/check_star_sentinel_naming.py",
    "scripts/ci/check_schema_examples.py",
    "scripts/ci/check_implementation_docs.py",
)


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


def main() -> int:
    errors: list[str] = []
    check_paths(REQUIRED_DOCS, errors)
    check_paths(REQUIRED_EXAMPLE_DIRS, errors)
    check_ci_commands(errors)

    if errors:
        print("ERROR: implementation documentation check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        "implementation documentation check passed: "
        f"{len(REQUIRED_DOCS)} doc(s), {len(REQUIRED_EXAMPLE_DIRS)} example dir(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
