#!/usr/bin/env python3
"""Star-Control 내장 manifest의 최소 계약을 검사한다."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[2]
STAR_SENTINEL_MANIFEST = ROOT / "builtin-tools/star-sentinel/tool.yaml"

REQUIRED_STAR_SENTINEL_FIELDS = {
    "id",
    "name",
    "kind",
    "package",
    "entrypoint",
    "commands",
    "profiles",
    "outputs",
}

REQUIRED_STAR_SENTINEL_COMMANDS = {
    "check",
    "review-pack",
    "gate",
    "selfcheck",
}

REQUIRED_STAR_SENTINEL_PROFILES = {
    "quick",
    "near",
    "full",
    "security",
    "release",
    "validator",
}

REQUIRED_STAR_SENTINEL_OUTPUTS = {
    "repo_map.json",
    "changed_lines.json",
    "diagnostics.json",
    "validation_runs.json",
    "review_pack.md",
    "approval.json",
    "ledger.jsonl",
}


def load_yaml(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as file:
        return yaml.safe_load(file)


def names_from_command_list(value: Any) -> set[str]:
    if not isinstance(value, list):
        return set()
    names: set[str] = set()
    for item in value:
        if isinstance(item, dict) and isinstance(item.get("name"), str):
            names.add(item["name"])
    return names


def strings_from_list(value: Any) -> set[str]:
    if not isinstance(value, list):
        return set()
    return {item for item in value if isinstance(item, str)}


def main() -> int:
    errors: list[str] = []

    if not STAR_SENTINEL_MANIFEST.is_file():
        print(
            "ERROR: builtin-tools/star-sentinel/tool.yaml 파일이 없습니다.",
            file=sys.stderr,
        )
        return 1

    manifest = load_yaml(STAR_SENTINEL_MANIFEST)
    if not isinstance(manifest, dict):
        print("ERROR: Star Sentinel manifest는 YAML mapping이어야 합니다.", file=sys.stderr)
        return 1

    missing_fields = REQUIRED_STAR_SENTINEL_FIELDS - set(manifest)
    if missing_fields:
        errors.append(f"필수 필드 누락: {sorted(missing_fields)}")

    expected_scalars = {
        "id": "star.sentinel",
        "name": "Star Sentinel",
        "kind": "builtin",
        "package": "star-sentinel",
        "entrypoint": "star_sentinel.main",
    }
    for key, expected_value in expected_scalars.items():
        actual_value = manifest.get(key)
        if actual_value != expected_value:
            errors.append(f"{key} 값은 {expected_value!r}이어야 합니다. 현재: {actual_value!r}")

    command_names = names_from_command_list(manifest.get("commands"))
    missing_commands = REQUIRED_STAR_SENTINEL_COMMANDS - command_names
    if missing_commands:
        errors.append(f"필수 command 누락: {sorted(missing_commands)}")

    profiles = strings_from_list(manifest.get("profiles"))
    missing_profiles = REQUIRED_STAR_SENTINEL_PROFILES - profiles
    if missing_profiles:
        errors.append(f"필수 profile 누락: {sorted(missing_profiles)}")

    outputs = strings_from_list(manifest.get("outputs"))
    missing_outputs = REQUIRED_STAR_SENTINEL_OUTPUTS - outputs
    if missing_outputs:
        errors.append(f"필수 output 누락: {sorted(missing_outputs)}")

    if errors:
        print("ERROR: manifest contract check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("manifest contract check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
