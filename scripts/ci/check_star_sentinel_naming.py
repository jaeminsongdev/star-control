#!/usr/bin/env python3
"""Star Sentinel 명칭과 과거 별칭 사용 위치를 검사한다."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

LEGACY_ALIAS = "autocode-guard"
ALLOWED_LEGACY_ALIAS_PATHS = {
    "builtin-tools/star-sentinel/tool.yaml",
    "docs/decisions/source-absorption-map.md",
}

SKIP_DIR_NAMES = {
    ".git",
    ".github",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
    "__pycache__",
}

TEXT_SUFFIXES = {
    ".md",
    ".txt",
    ".json",
    ".jsonl",
    ".yaml",
    ".yml",
    ".toml",
    ".py",
    ".rs",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
}

PROVIDER_PRODUCT_NAMES = {
    "openai",
    "claude",
    "gemini",
    "codex",
}

PROVIDER_PRODUCT_ALLOWED_ROOTS = {
    "builtin-providers",
    "docs/providers",
    "examples/rendered-provider-artifacts",
}


def should_skip(path: Path) -> bool:
    relative_parts = path.relative_to(ROOT).parts
    return any(part in SKIP_DIR_NAMES for part in relative_parts)


def is_text_candidate(path: Path) -> bool:
    return path.suffix.lower() in TEXT_SUFFIXES


def is_provider_product_allowed(relative_path: str) -> bool:
    return any(
        relative_path == root or relative_path.startswith(f"{root}/")
        for root in PROVIDER_PRODUCT_ALLOWED_ROOTS
    )


def main() -> int:
    errors: list[str] = []

    for path in ROOT.rglob("*"):
        if not path.is_file() or should_skip(path) or not is_text_candidate(path):
            continue

        relative_path = path.relative_to(ROOT).as_posix()
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue

        if LEGACY_ALIAS in text and relative_path not in ALLOWED_LEGACY_ALIAS_PATHS:
            errors.append(
                f"{relative_path}: 과거 별칭 {LEGACY_ALIAS!r}는 허용 위치에서만 사용할 수 있습니다."
            )

        if relative_path.startswith("packages/"):
            lower_path = relative_path.lower()
            for product_name in PROVIDER_PRODUCT_NAMES:
                if product_name in lower_path and not is_provider_product_allowed(relative_path):
                    errors.append(
                        f"{relative_path}: core package 경로에 provider 제품명 {product_name!r}를 넣지 마세요."
                    )

    if errors:
        print("ERROR: Star Sentinel naming policy check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Star Sentinel naming policy check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
