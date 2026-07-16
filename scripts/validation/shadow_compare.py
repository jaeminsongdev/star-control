#!/usr/bin/env python3
"""Compare an authority validation result with a project-validation shadow report."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any


PASSING_STATUS = "pass"


def _load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8-sig") as stream:
        value = json.load(stream)
    if not isinstance(value, dict):
        raise ValueError(f"expected a JSON object: {path}")
    return value


def _is_subsequence(expected: list[str], actual: list[str]) -> bool:
    if not expected:
        return True
    position = 0
    for argument in actual:
        if argument == expected[position]:
            position += 1
            if position == len(expected):
                return True
    return False


def _command_relation(mapping: dict[str, Any], check: dict[str, Any]) -> str:
    expectation = mapping.get("command", {})
    command = check.get("command")
    if not isinstance(command, dict):
        return "not_run"
    actual_executable = str(command.get("executable", ""))
    expected_executable = str(expectation.get("executable", ""))
    if os.path.basename(actual_executable).casefold() != expected_executable.casefold():
        return "mismatch"
    actual_arguments = [str(value) for value in command.get("arguments", [])]
    required_arguments = [str(value) for value in expectation.get("required_arguments", [])]
    if not _is_subsequence(required_arguments, actual_arguments):
        return "mismatch"
    actual_basenames = {os.path.basename(value).casefold() for value in actual_arguments}
    required_basenames = {
        str(value).casefold() for value in expectation.get("required_argument_basenames", [])
    }
    if not required_basenames.issubset(actual_basenames):
        return "mismatch"
    alternatives = expectation.get("any_of_required_argument_sequences", [])
    if alternatives and not any(
        _is_subsequence([str(value) for value in sequence], actual_arguments)
        for sequence in alternatives
    ):
        return "mismatch"
    return "match"


def _expected_units(mapping: dict[str, Any], profile: str) -> list[str]:
    values = mapping.get("expected_units_by_profile", {}).get(profile, [])
    return [str(value) for value in values]


def compare(
    contract: dict[str, Any],
    candidate: dict[str, Any],
    legacy_result: str,
    candidate_ref: str,
) -> dict[str, Any]:
    if contract.get("schema_id") != "star.validation-shadow-contract":
        raise ValueError("unsupported shadow contract schema_id")
    if contract.get("schema_version") != 1:
        raise ValueError("unsupported shadow contract schema_version")

    profile = str(candidate.get("effective_profile", "unknown"))
    candidate_schema_valid = candidate.get("schema_id") == "star.project-validation-report"
    report_status = str(candidate.get("status", "unverified"))
    raw_checks = candidate.get("checks", [])
    checks = [item for item in raw_checks if isinstance(item, dict)]
    checks_by_id: dict[str, dict[str, Any]] = {}
    duplicate_ids: list[str] = []
    for check in checks:
        check_id = str(check.get("id", ""))
        if check_id in checks_by_id:
            duplicate_ids.append(check_id)
        checks_by_id[check_id] = check

    observations: list[dict[str, Any]] = []
    required_candidate_ids: set[str] = set()
    for mapping in contract.get("mappings", []):
        profiles = [str(value) for value in mapping.get("profiles", [])]
        if profile not in profiles:
            continue
        legacy_id = str(mapping["legacy_id"])
        candidate_id = str(mapping["candidate_id"])
        required_candidate_ids.add(candidate_id)
        check = checks_by_id.get(candidate_id)
        if check is None:
            observations.append(
                {
                    "legacy_id": legacy_id,
                    "legacy_unit": mapping.get("legacy_unit"),
                    "legacy_status": legacy_result,
                    "legacy_command_contract": mapping.get("command"),
                    "candidate_id": candidate_id,
                    "candidate_unit": None,
                    "selection_relation": "missing",
                    "unit_relation": "not_run",
                    "command_relation": "not_run",
                    "result_relation": "mismatch" if legacy_result == PASSING_STATUS else "unverified",
                    "candidate_status": "not_run",
                    "candidate_command": None,
                    "candidate_exit_code": None,
                    "candidate_duration_ms": None,
                    "candidate_failure_summary": "candidate check was not selected",
                    "candidate_log_ref": None,
                }
            )
            continue

        candidate_unit = str(check.get("unit", ""))
        expected_units = _expected_units(mapping, profile)
        unit_relation = "observed"
        if expected_units:
            unit_relation = "match" if candidate_unit in expected_units else "mismatch"
        check_status = str(check.get("status", "unverified"))
        result_relation = "unverified"
        if legacy_result == PASSING_STATUS:
            result_relation = "match" if check_status == PASSING_STATUS else "mismatch"
        observations.append(
            {
                "legacy_id": legacy_id,
                "legacy_unit": mapping.get("legacy_unit"),
                "legacy_status": legacy_result,
                "legacy_command_contract": mapping.get("command"),
                "candidate_id": candidate_id,
                "candidate_unit": candidate_unit,
                "selection_relation": "selected",
                "unit_relation": unit_relation,
                "command_relation": _command_relation(mapping, check),
                "result_relation": result_relation,
                "candidate_status": check_status,
                "candidate_command": check.get("command"),
                "candidate_exit_code": check.get("exit_code"),
                "candidate_duration_ms": check.get("duration_ms"),
                "candidate_failure_summary": check.get("failure_summary"),
                "candidate_log_ref": check.get("log_ref"),
            }
        )

    missing = [item["legacy_id"] for item in observations if item["selection_relation"] == "missing"]
    command_mismatches = [item["legacy_id"] for item in observations if item["command_relation"] == "mismatch"]
    unit_mismatches = [item["legacy_id"] for item in observations if item["unit_relation"] == "mismatch"]
    result_mismatches = [item["legacy_id"] for item in observations if item["result_relation"] == "mismatch"]
    comparison_status = "pass"
    candidate_status_mismatch = legacy_result == PASSING_STATUS and report_status != PASSING_STATUS
    if (
        missing
        or command_mismatches
        or unit_mismatches
        or result_mismatches
        or duplicate_ids
        or not candidate_schema_valid
        or candidate_status_mismatch
        or not observations
    ):
        comparison_status = "fail"
    elif legacy_result != PASSING_STATUS:
        comparison_status = "partial"

    impact = candidate.get("impact", {}) if isinstance(candidate.get("impact"), dict) else {}
    blockers = [
        "shadow candidate is not an authority gate",
        "one observation cannot satisfy promotion evidence",
    ]
    if comparison_status != "pass":
        blockers.append("selection, command, unit, or result comparison is incomplete")

    return {
        "schema_id": "star.validation-shadow-comparison",
        "schema_version": 1,
        "project_id": contract.get("project_id"),
        "legacy": {
            "status": legacy_result,
            "authority": True,
        },
        "candidate": {
            "status": report_status,
            "schema_valid": candidate_schema_valid,
            "requested_profile": candidate.get("requested_profile"),
            "effective_profile": profile,
            "affected_units": impact.get("affected_units", []),
            "report_ref": candidate_ref,
            "authority": False,
        },
        "comparison_status": comparison_status,
        "observations": observations,
        "missing_legacy_checks": missing,
        "command_mismatches": command_mismatches,
        "unit_mismatches": unit_mismatches,
        "result_mismatches": result_mismatches,
        "candidate_status_mismatch": candidate_status_mismatch,
        "duplicate_candidate_check_ids": sorted(set(duplicate_ids)),
        "unmapped_candidate_checks": sorted(
            check_id for check_id in checks_by_id if check_id not in required_candidate_ids
        ),
        "promotion_eligible": False,
        "promotion_blockers": blockers,
    }


def _self_test() -> int:
    contract = {
        "schema_id": "star.validation-shadow-contract",
        "schema_version": 1,
        "project_id": "self-test",
        "mappings": [
            {
                "legacy_id": "legacy-check",
                "candidate_id": "candidate-check",
                "legacy_unit": "workspace",
                "profiles": ["target"],
                "command": {"executable": "cargo", "required_arguments": ["check", "--locked"]},
            }
        ],
    }
    candidate = {
        "schema_id": "star.project-validation-report",
        "status": "pass",
        "requested_profile": "target",
        "effective_profile": "target",
        "impact": {"affected_units": ["example"]},
        "checks": [
            {
                "id": "candidate-check",
                "unit": "example",
                "status": "pass",
                "command": {"executable": "cargo", "arguments": ["check", "-p", "example", "--locked"]},
            }
        ],
    }
    passing = compare(contract, candidate, "pass", "self-test.json")
    if passing["comparison_status"] != "pass" or passing["promotion_eligible"]:
        raise AssertionError("passing comparison contract failed")
    candidate["checks"][0]["command"]["arguments"] = ["test", "--locked"]
    mismatched = compare(contract, candidate, "pass", "self-test.json")
    if mismatched["comparison_status"] != "fail" or mismatched["command_mismatches"] != ["legacy-check"]:
        raise AssertionError("command mismatch was not detected")
    candidate["checks"] = []
    missing = compare(contract, candidate, "pass", "self-test.json")
    if missing["missing_legacy_checks"] != ["legacy-check"]:
        raise AssertionError("missing candidate check was not detected")
    entry_error = {"schema_id": "star.project-validation-entry-error", "status": "unverified"}
    invalid = compare(contract, entry_error, "pass", "self-test.json")
    if invalid["comparison_status"] != "fail" or invalid["candidate"]["schema_valid"]:
        raise AssertionError("candidate entry error was not rejected")
    print("shadow comparison self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--legacy-result", choices=["pass", "fail", "cancelled", "skipped"], default="pass")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return _self_test()
    if not args.contract or not args.candidate or not args.output:
        parser.error("--contract, --candidate, and --output are required")

    try:
        contract = _load_json(args.contract)
        candidate = _load_json(args.candidate)
        result = compare(contract, candidate, args.legacy_result, str(args.candidate))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("w", encoding="utf-8", newline="\n") as stream:
            json.dump(result, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        print(
            "shadow comparison: "
            f"legacy={args.legacy_result} candidate={result['candidate']['status']} "
            f"profile={result['candidate']['effective_profile']} comparison={result['comparison_status']} "
            f"output={args.output}"
        )
        return 0 if result["comparison_status"] == "pass" else 1
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"shadow comparison error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
