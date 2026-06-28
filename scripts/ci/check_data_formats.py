#!/usr/bin/env python3
"""JSON / YAML / TOML 파일의 기본 파싱 가능성을 검사한다."""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
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

JSON_SUFFIXES = {".json"}
YAML_SUFFIXES = {".yaml", ".yml"}
TOML_SUFFIXES = {".toml"}


def should_skip(path: Path) -> bool:
    return any(part in SKIP_DIR_NAMES for part in path.relative_to(ROOT).parts)


def parse_json(path: Path) -> None:
    with path.open("r", encoding="utf-8") as file:
        json.load(file)


def parse_yaml(path: Path) -> None:
    with path.open("r", encoding="utf-8") as file:
        yaml.safe_load(file)


def parse_toml(path: Path) -> None:
    with path.open("rb") as file:
        tomllib.load(file)


def main() -> int:
    errors: list[str] = []

    for path in ROOT.rglob("*"):
        if not path.is_file() or should_skip(path):
            continue

        relative_path = path.relative_to(ROOT).as_posix()
        try:
            if path.suffix in JSON_SUFFIXES:
                parse_json(path)
            elif path.suffix in YAML_SUFFIXES:
                parse_yaml(path)
            elif path.suffix in TOML_SUFFIXES:
                parse_toml(path)
        except Exception as exc:  # noqa: BLE001 - CI에서는 원인 메시지를 보존한다.
            errors.append(f"{relative_path}: {exc}")

    if errors:
        print("ERROR: data format parse check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("data format check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
