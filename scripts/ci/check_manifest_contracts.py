#!/usr/bin/env python3
"""Validate the built-in Star Sentinel manifest contract."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[2]
STAR_SENTINEL_MANIFEST = ROOT / "builtin-tools/star-sentinel/tool.yaml"

EXPECTED_SCALARS = {
    "id": "star.sentinel",
    "name": "Star Sentinel",
    "kind": "builtin",
    "package": "star-sentinel",
    "entrypoint": "star_sentinel.main",
}

REQUIRED_FIELDS = (
    "id",
    "name",
    "kind",
    "package",
    "entrypoint",
    "description",
    "legacy_aliases",
    "commands",
    "profiles",
    "outputs",
)

REQUIRED_COMMANDS = {
    "check",
    "review-pack",
    "gate",
    "selfcheck",
}

REQUIRED_PROFILES = {
    "quick",
    "near",
    "full",
    "security",
    "release",
    "validator",
}

REQUIRED_OUTPUTS = {
    "repo_map.json",
    "changed_lines.json",
    "diagnostics.json",
    "validation_runs.json",
    "review_pack.md",
    "approval.json",
    "ledger.jsonl",
}

REQUIRED_LEGACY_ALIASES = {
    "autocode-guard",
}


def load_yaml(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as file:
        return yaml.safe_load(file)


def strings_from_list(value: Any) -> set[str]:
    if not isinstance(value, list):
        return set()
    return {item for item in value if isinstance(item, str)}


def command_names(value: Any, errors: list[str]) -> set[str]:
    if not isinstance(value, list):
        errors.append("commands must be a list")
        return set()

    names: set[str] = set()
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            errors.append(f"commands[{index}] must be a mapping")
            continue
        name = item.get("name")
        description = item.get("description")
        if not isinstance(name, str) or not name:
            errors.append(f"commands[{index}].name must be a non-empty string")
            continue
        if name in names:
            errors.append(f"duplicate command name: {name}")
        names.add(name)
        if not isinstance(description, str) or not description.strip():
            errors.append(f"commands[{index}].description must be a non-empty string")
    return names


def require_mapping(value: Any, errors: list[str]) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        errors.append("manifest must be a YAML mapping")
        return None
    return value


def check_required_fields(manifest: dict[str, Any], errors: list[str]) -> None:
    missing = [field for field in REQUIRED_FIELDS if field not in manifest]
    if missing:
        errors.append("missing required field(s): " + ", ".join(missing))


def check_scalar_values(manifest: dict[str, Any], errors: list[str]) -> None:
    for key, expected in EXPECTED_SCALARS.items():
        actual = manifest.get(key)
        if actual != expected:
            errors.append(f"{key} must be {expected!r}; got {actual!r}")


def check_description(manifest: dict[str, Any], errors: list[str]) -> None:
    description = manifest.get("description")
    if not isinstance(description, str) or not description.strip():
        errors.append("description must be a non-empty string")


def check_list_contract(
    field_name: str,
    actual_values: set[str],
    required_values: set[str],
    errors: list[str],
) -> None:
    missing = sorted(required_values - actual_values)
    if missing:
        errors.append(f"{field_name} missing required value(s): {', '.join(missing)}")


def main() -> int:
    errors: list[str] = []

    if not STAR_SENTINEL_MANIFEST.is_file():
        print("ERROR: missing builtin-tools/star-sentinel/tool.yaml", file=sys.stderr)
        return 1

    raw_manifest = load_yaml(STAR_SENTINEL_MANIFEST)
    manifest = require_mapping(raw_manifest, errors)
    if manifest is None:
        return 1

    check_required_fields(manifest, errors)
    check_scalar_values(manifest, errors)
    check_description(manifest, errors)

    commands = command_names(manifest.get("commands"), errors)
    check_list_contract("commands", commands, REQUIRED_COMMANDS, errors)

    profiles = strings_from_list(manifest.get("profiles"))
    if not isinstance(manifest.get("profiles"), list):
        errors.append("profiles must be a list")
    check_list_contract("profiles", profiles, REQUIRED_PROFILES, errors)

    outputs = strings_from_list(manifest.get("outputs"))
    if not isinstance(manifest.get("outputs"), list):
        errors.append("outputs must be a list")
    check_list_contract("outputs", outputs, REQUIRED_OUTPUTS, errors)

    legacy_aliases = strings_from_list(manifest.get("legacy_aliases"))
    if not isinstance(manifest.get("legacy_aliases"), list):
        errors.append("legacy_aliases must be a list")
    check_list_contract("legacy_aliases", legacy_aliases, REQUIRED_LEGACY_ALIASES, errors)

    if errors:
        print("ERROR: manifest contract check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("manifest contract check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
