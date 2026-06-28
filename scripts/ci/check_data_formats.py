#!/usr/bin/env python3
"""Parse JSON, YAML, and TOML files in canonical Star-Control data paths."""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path
from typing import Callable

import yaml

ROOT = Path(__file__).resolve().parents[2]

INCLUDED_ROOTS = (
    ".github/workflows",
    "configs",
    "specs",
    "builtin-tools",
    "builtin-providers",
    "examples",
)

PARSERS: dict[str, Callable[[Path], None]] = {}


def parse_json(path: Path) -> None:
    with path.open("r", encoding="utf-8") as file:
        json.load(file)


def parse_yaml(path: Path) -> None:
    with path.open("r", encoding="utf-8") as file:
        yaml.safe_load(file)


def parse_toml(path: Path) -> None:
    with path.open("rb") as file:
        tomllib.load(file)


PARSERS.update(
    {
        ".json": parse_json,
        ".yaml": parse_yaml,
        ".yml": parse_yaml,
        ".toml": parse_toml,
    }
)


def iter_data_files() -> list[Path]:
    files: list[Path] = []
    for relative_root in INCLUDED_ROOTS:
        root = ROOT / relative_root
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if path.is_file() and path.suffix.lower() in PARSERS:
                files.append(path)
    return sorted(files)


def main() -> int:
    errors: list[str] = []
    checked = 0

    for path in iter_data_files():
        parser = PARSERS[path.suffix.lower()]
        relative_path = path.relative_to(ROOT).as_posix()
        try:
            parser(path)
            checked += 1
        except Exception as exc:  # noqa: BLE001 - preserve parser failure context in CI.
            errors.append(f"{relative_path}: {exc}")

    if errors:
        print("ERROR: data format check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"data format check passed: {checked} file(s) parsed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
