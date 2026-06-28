#!/usr/bin/env python3
"""Check Star Sentinel naming policy."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

TEXT_SUFFIXES = {
    ".md",
    ".txt",
    ".yaml",
    ".yml",
    ".json",
    ".toml",
    ".py",
}

SKIP_DIRS = {
    ".git",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
    "__pycache__",
}

LEGACY_ALIAS = "auto" + "code-guard"
LEGACY_ALIAS_ALLOWED_FILES = {
    "builtin-tools/star-sentinel/tool.yaml",
}

BAD_NAME_TERMS = (
    "Star" + "Sentinel",
    "Star" + "-Sentinel",
    "Star" + "_Sentinel",
)

BAD_NAME_PATTERNS = tuple(re.compile(rf"\b{re.escape(term)}\b") for term in BAD_NAME_TERMS)

OFFICIAL_NAMES = (
    "Star Sentinel",
    "star-sentinel",
    "star_sentinel",
    "star.sentinel",
)


def is_skipped(path: Path) -> bool:
    relative_parts = path.relative_to(ROOT).parts
    return any(part in SKIP_DIRS for part in relative_parts)


def iter_text_files() -> list[Path]:
    files: list[Path] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or is_skipped(path):
            continue
        if path.suffix.lower() in TEXT_SUFFIXES:
            files.append(path)
    return sorted(files)


def line_number_for_offset(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def check_legacy_alias(path: Path, text: str, errors: list[str]) -> None:
    relative_path = path.relative_to(ROOT).as_posix()
    if LEGACY_ALIAS not in text:
        return
    if relative_path in LEGACY_ALIAS_ALLOWED_FILES:
        return
    for match in re.finditer(re.escape(LEGACY_ALIAS), text):
        line = line_number_for_offset(text, match.start())
        errors.append(f"{relative_path}:{line}: legacy alias is only allowed in tool.yaml")


def check_bad_names(path: Path, text: str, errors: list[str]) -> None:
    relative_path = path.relative_to(ROOT).as_posix()
    official = ", ".join(OFFICIAL_NAMES)
    for pattern in BAD_NAME_PATTERNS:
        for match in pattern.finditer(text):
            line = line_number_for_offset(text, match.start())
            errors.append(
                f"{relative_path}:{line}: use one of [{official}] instead of {match.group(0)!r}"
            )


def main() -> int:
    errors: list[str] = []

    for path in iter_text_files():
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        check_legacy_alias(path, text, errors)
        check_bad_names(path, text, errors)

    if errors:
        print("ERROR: Star Sentinel naming policy check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Star Sentinel naming policy check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
