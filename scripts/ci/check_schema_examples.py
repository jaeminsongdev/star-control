#!/usr/bin/env python3
"""Validate canonical JSON examples against the current schema subset.

This check intentionally avoids external dependencies. It supports only the JSON Schema
keywords currently used by Star-Control's canonical examples and templates.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]

VALIDATION_CASES = (
    (
        "specs/schemas/route.schema.json",
        "configs/templates/route-template.json",
    ),
    (
        "specs/schemas/route.schema.json",
        "examples/fake/route-done.json",
    ),
    (
        "specs/schemas/report.schema.json",
        "configs/templates/report-template.json",
    ),
    (
        "specs/schemas/report.schema.json",
        "examples/fake/impl-report-done.json",
    ),
    (
        "builtin-tools/star-sentinel/schemas/approval.schema.json",
        "builtin-tools/star-sentinel/examples/p0/approval-block.example.json",
    ),
)


def load_json(relative_path: str) -> Any:
    path = ROOT / relative_path
    with path.open("r", encoding="utf-8") as file:
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
    if expected == "null":
        return value is None
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    return False


def validate_type(value: Any, schema: dict[str, Any], location: str, errors: list[str]) -> None:
    expected_type = schema.get("type")
    if expected_type is None:
        return

    expected_types = expected_type if isinstance(expected_type, list) else [expected_type]
    if not all(isinstance(item, str) for item in expected_types):
        errors.append(f"{location}: schema type must be a string or list of strings")
        return

    if not any(type_matches(value, expected) for expected in expected_types):
        errors.append(
            f"{location}: expected type {expected_types}, got {type_name(value)}"
        )


def validate_value(value: Any, schema: dict[str, Any], location: str, errors: list[str]) -> None:
    if "const" in schema and value != schema["const"]:
        errors.append(f"{location}: expected const {schema['const']!r}, got {value!r}")

    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{location}: expected one of {schema['enum']!r}, got {value!r}")

    validate_type(value, schema, location, errors)

    if isinstance(value, str):
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            errors.append(f"{location}: string shorter than minLength {min_length}")

        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.fullmatch(pattern, value) is None:
            errors.append(f"{location}: value {value!r} does not match pattern {pattern!r}")

    if isinstance(value, dict):
        required = schema.get("required", [])
        if isinstance(required, list):
            for key in required:
                if isinstance(key, str) and key not in value:
                    errors.append(f"{location}: missing required property {key!r}")

        properties = schema.get("properties", {})
        if isinstance(properties, dict):
            for key, child_schema in properties.items():
                if key in value and isinstance(child_schema, dict):
                    validate_value(value[key], child_schema, f"{location}.{key}", errors)

        additional_properties = schema.get("additionalProperties")
        if isinstance(additional_properties, dict) and isinstance(properties, dict):
            for key, item in value.items():
                if key not in properties:
                    validate_value(item, additional_properties, f"{location}.{key}", errors)

    if isinstance(value, list):
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                validate_value(item, item_schema, f"{location}[{index}]", errors)


def validate_case(schema_path: str, document_path: str, errors: list[str]) -> None:
    schema = load_json(schema_path)
    document = load_json(document_path)

    if not isinstance(schema, dict):
        errors.append(f"{schema_path}: schema root must be an object")
        return

    case_errors: list[str] = []
    validate_value(document, schema, document_path, case_errors)
    for error in case_errors:
        errors.append(f"{document_path} against {schema_path}: {error}")


def main() -> int:
    errors: list[str] = []

    for schema_path, document_path in VALIDATION_CASES:
        validate_case(schema_path, document_path, errors)

    if errors:
        print("ERROR: schema example check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"schema example check passed: {len(VALIDATION_CASES)} case(s) validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
