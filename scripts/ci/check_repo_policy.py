#!/usr/bin/env python3
"""Star-Control 저장소 구조와 기본 운영 정책을 검사한다."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED_FILES = (
    "README.md",
    "AGENTS.md",
    ".github/workflows/ci.yml",
    "docs/decisions/source-absorption-map.md",
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

FORBIDDEN_REPO_PATHS = (
    ".ai-runs",
)


def repo_path(relative_path: str) -> Path:
    return ROOT / relative_path


def check_required_files(errors: list[str]) -> None:
    for relative_path in REQUIRED_FILES:
        path = repo_path(relative_path)
        if not path.is_file():
            errors.append(f"필수 파일 없음: {relative_path}")


def check_required_dirs(errors: list[str]) -> None:
    for relative_path in REQUIRED_DIRS:
        path = repo_path(relative_path)
        if not path.is_dir():
            errors.append(f"필수 디렉터리 없음: {relative_path}")


def check_forbidden_repo_paths(errors: list[str]) -> None:
    for relative_path in FORBIDDEN_REPO_PATHS:
        path = repo_path(relative_path)
        if path.exists():
            errors.append(
                f"금지된 실행 산출물 경로가 repository에 존재함: {relative_path}"
            )


def main() -> int:
    errors: list[str] = []

    check_required_files(errors)
    check_required_dirs(errors)
    check_forbidden_repo_paths(errors)

    if errors:
        print("ERROR: repository policy check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("repository policy check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
