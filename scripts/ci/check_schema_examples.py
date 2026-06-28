#!/usr/bin/env python3
"""Validate canonical JSON examples against the current schema subset."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]

VALIDATION_CASES = [
    ("specs/schemas/job.schema.json", "examples/runs/J-0001/job.json"),
    ("specs/schemas/run-state.schema.json", "examples/runs/J-0001/run-state.json"),
    ("specs/schemas/route.schema.json", "configs/templates/route-template.json"),
    ("specs/schemas/route.schema.json", "examples/fake/route-done.json"),
    ("specs/schemas/route.schema.json", "examples/runs/J-0001/route.json"),
    ("specs/schemas/route.schema.json", "examples/router-contracts/route-approval-required.example.json"),
    ("specs/schemas/router-decision.schema.json", "examples/router-contracts/router-decision.schema-change.example.json"),
    ("specs/schemas/execution-request.schema.json", "examples/execution-contracts/execution-request.fake.example.json"),
    ("specs/schemas/execution-attempt.schema.json", "examples/execution-contracts/execution-attempt.success.example.json"),
    ("specs/schemas/workspec.schema.json", "examples/runs/J-0001/workspecs/implement.json"),
    ("specs/schemas/report.schema.json", "configs/templates/report-template.json"),
    ("specs/schemas/report.schema.json", "examples/fake/impl-report-done.json"),
    ("specs/schemas/event.schema.json", "examples/core/event.example.json"),
    ("specs/schemas/artifact-ref.schema.json", "examples/core/artifact-ref.example.json"),
    ("specs/schemas/error.schema.json", "examples/core/error.example.json"),
    ("specs/schemas/provider-manifest.schema.json", "examples/provider-contracts/provider-manifest.fake.example.json"),
    ("specs/schemas/provider-instance.schema.json", "examples/provider-contracts/provider-instance.fake.example.json"),
    ("specs/schemas/capability-profile.schema.json", "examples/provider-contracts/capability-profile.fake.example.json"),
    ("specs/schemas/provider-registry.schema.json", "examples/provider-contracts/provider-registry.example.json"),
    ("specs/schemas/provider-run-result.schema.json", "examples/provider-contracts/provider-run-result.success.example.json"),
    ("specs/schemas/provider-run-result.schema.json", "examples/execution-contracts/fake-provider-response.success.example.json"),
    ("specs/schemas/config.schema.json", "examples/config-contracts/config.example.json"),
    ("specs/schemas/policy.schema.json", "examples/config-contracts/policy.example.json"),
    ("specs/schemas/hook.schema.json", "examples/config-contracts/hook.example.json"),
    ("specs/schemas/role.schema.json", "examples/config-contracts/role.example.json"),
    ("specs/schemas/renderer.schema.json", "examples/config-contracts/renderer.example.json"),
    ("specs/schemas/skill.schema.json", "examples/config-contracts/skill.example.json"),
    ("builtin-tools/star-sentinel/schemas/approval.schema.json", "builtin-tools/star-sentinel/examples/p0/approval-block.example.json"),
    ("builtin-tools/star-sentinel/schemas/sentinel-task.schema.json", "builtin-tools/star-sentinel/examples/p0/sentinel-task.example.json"),
    ("builtin-tools/star-sentinel/schemas/diagnostic.schema.json", "builtin-tools/star-sentinel/examples/p0/diagnostic-block.example.json"),
    ("builtin-tools/star-sentinel/schemas/ledger-event.schema.json", "builtin-tools/star-sentinel/examples/p0/ledger-event.example.json"),
    ("builtin-tools/star-sentinel/schemas/validation-run.schema.json", "builtin-tools/star-sentinel/examples/p0/validation-run.example.json"),
    ("builtin-tools/star-sentinel/schemas/review-pack.schema.json", "builtin-tools/star-sentinel/examples/p0/review-pack-human-review.example.json"),
    ("builtin-tools/star-sentinel/schemas/repo-map.schema.json", "builtin-tools/star-sentinel/examples/p0/repo-map.example.json"),
    ("builtin-tools/star-sentinel/schemas/changed-lines.schema.json", "builtin-tools/star-sentinel/examples/p0/changed-lines.example.json"),
]


def load_json(relative_path: str) -> Any:
    with (ROOT / relative_path).open("r", encoding="utf-8") as file:
        return json.load(file)


def type_name(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, dict):
        return "object"
    if isinstance(value, list):
        return "array"
    if isinstance(value, str):
        return "string"
    if isinstance(value, (int, float)):
        return "number"
    return type(value).__name__


def type_matches(value: Any, expected: str) -> bool:
    return {
        "null": value is None,
        "boolean": isinstance(value, bool),
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "number": isinstance(value, (int, float)) and not isinstance(value, bool),
        "integer": isinstance(value, int) and not isinstance(value, bool),
    }.get(expected, False)


def validate_value(value: Any, schema: dict[str, Any], location: str, errors: list[str]) -> None:
    if "const" in schema and value != schema["const"]:
        errors.append(f"{location}: expected const {schema['const']!r}, got {value!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{location}: expected one of {schema['enum']!r}, got {value!r}")

    expected_type = schema.get("type")
    if expected_type is not None:
        expected_types = expected_type if isinstance(expected_type, list) else [expected_type]
        if not all(isinstance(item, str) for item in expected_types):
            errors.append(f"{location}: schema type must be a string or list of strings")
        elif not any(type_matches(value, expected) for expected in expected_types):
            errors.append(f"{location}: expected type {expected_types}, got {type_name(value)}")

    if isinstance(value, str):
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            errors.append(f"{location}: string shorter than minLength {min_length}")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.fullmatch(pattern, value) is None:
            errors.append(f"{location}: value {value!r} does not match pattern {pattern!r}")

    if isinstance(value, dict):
        properties = schema.get("properties", {})
        for key in schema.get("required", []):
            if isinstance(key, str) and key not in value:
                errors.append(f"{location}: missing required property {key!r}")
        if isinstance(properties, dict):
            for key, child_schema in properties.items():
                if key in value and isinstance(child_schema, dict):
                    validate_value(value[key], child_schema, f"{location}.{key}", errors)
        additional_properties = schema.get("additionalProperties")
        if isinstance(additional_properties, dict) and isinstance(properties, dict):
            for key, item in value.items():
                if key not in properties:
                    validate_value(item, additional_properties, f"{location}.{key}", errors)

    if isinstance(value, list) and isinstance(schema.get("items"), dict):
        for index, item in enumerate(value):
            validate_value(item, schema["items"], f"{location}[{index}]", errors)


def main() -> int:
    errors: list[str] = []
    for schema_path, document_path in VALIDATION_CASES:
        case_errors: list[str] = []
        validate_value(load_json(document_path), load_json(schema_path), document_path, case_errors)
        errors.extend(f"{document_path} against {schema_path}: {error}" for error in case_errors)

    if errors:
        print("ERROR: schema example check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"schema example check passed: {len(VALIDATION_CASES)} case(s) validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
