#!/usr/bin/env python3
"""Star-Control 저장소 구조와 기본 정책을 검사한다."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED_FILES = [
    "README.md",
    "AGENTS.md",
    "docs/decisions/source-absorption-map.md",
    "builtin-tools/star-sentinel/tool.yaml",
]

REQUIRED_DIRS = [
    "docs",
    "specs",
    "configs",
    "packages",
    "builtin-providers",
    "builtin-tools/star-sentinel",
    "examples",
]

FORBIDDEN_REPO_DIRS = [
    ".ai-runs",
]


def fail(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)


def main() -> int:
    errors: list[str] = []

    for relative_path in REQUIRED_FILES:
        path = ROOT / relative_path
        if not path.is_file():
            errors.append(f"필수 파일이 없습니다: {relative_path}")

    for relative_path in REQUIRED_DIRS:
        path = ROOT / relative_path
        if not path.is_dir():
            errors.append(f"필수 디렉터리가 없습니다: {relative_path}")

    for relative_path in FORBIDDEN_REPO_DIRS:
        path = ROOT / relative_path
        if path.exists():
            errors.append(
                f"실행 결과 디렉터리는 Star-Control repo에 저장하지 않습니다: {relative_path}"
            )

    star_sentinel_manifest = ROOT / "builtin-tools/star-sentinel/tool.yaml"
    star_sentinel_package = ROOT / "packages/star-sentinel"
    if star_sentinel_manifest.exists() and not star_sentinel_package.exists():
        errors.append(
            "Star Sentinel manifest가 있으므로 packages/star-sentinel 스캐폴드도 있어야 합니다."
        )

    if errors:
        for error in errors:
            fail(error)
        return 1

    print("repository policy check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
