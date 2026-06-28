#!/usr/bin/env python3
"""Check the Star-Control repository baseline structure."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED_FILES = (
    "README.md",
    "AGENTS.md",
    ".github/workflows/ci.yml",
    "docs/operations/ci-roadmap.md",
    "builtin-tools/star-sentinel/tool.yaml",
    "scripts/ci/check_manifest_contracts.py",
)

REQUIRED_DIRS = (
    "docs",
    "specs",
    "configs",
    "packages",
    "builtin-providers",
    "builtin-tools/star-sentinel",
    "examples",
)

DISALLOWED_REPO_PATHS = (
    ".ai-runs",
)


def repo_path(relative_path: str) -> Path:
    return ROOT / relative_path


def check_required_files(errors: list[str]) -> None:
    for relative_path in REQUIRED_FILES:
        path = repo_path(relative_path)
        if not path.is_file():
            errors.append(f"missing required file: {relative_path}")


def check_required_dirs(errors: list[str]) -> None:
    for relative_path in REQUIRED_DIRS:
        path = repo_path(relative_path)
        if not path.is_dir():
            errors.append(f"missing required directory: {relative_path}")


def check_disallowed_repo_paths(errors: list[str]) -> None:
    for relative_path in DISALLOWED_REPO_PATHS:
        path = repo_path(relative_path)
        if path.exists():
            errors.append(f"disallowed repository path exists: {relative_path}")


def main() -> int:
    errors: list[str] = []

    check_required_files(errors)
    check_required_dirs(errors)
    check_disallowed_repo_paths(errors)

    if errors:
        print("ERROR: repository policy check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("repository policy check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
