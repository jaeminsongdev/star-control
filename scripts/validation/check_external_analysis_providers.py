#!/usr/bin/env python3
"""Validate external-analysis descriptors and optionally record live discovery.

Discovery proves only the executable/version identity observed on this host. It
does not claim registration, protocol verification, or a successful analysis.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG = ROOT / "catalog" / "external-analysis-providers.toml"
EXPECTED_IDS = {
    "cargo-llvm-cov",
    "cargo-nextest",
    "cargo-mutants",
    "rust-analyzer",
    "buf",
    "oasdiff",
    "cargo-semver-checks",
    "libabigail",
    "syft",
    "cargo-deny",
    "cargo-audit",
    "diffoscope",
    "sanitizer",
    "generator-doctest",
    "loom",
    "builtin-near-clone",
}
ALLOWED_STABILITY = {"stable", "unstable", "human_text"}
ALLOWED_DETAIL = {"structured", "exit_classification", "raw_only"}
PRODUCT_SOURCE_ROOTS = (
    ".codex",
    ".github",
    "apps",
    "catalog",
    "crates",
    "docs",
    "integrations",
    "packaging",
    "schemas",
    "scripts",
    "specs",
    "tools",
)


def digest_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def source_fingerprint() -> str:
    digest = hashlib.sha256()
    for root_name in PRODUCT_SOURCE_ROOTS:
        root = ROOT / root_name
        if not root.exists():
            continue
        paths = [root] if root.is_file() else sorted(path for path in root.rglob("*") if path.is_file())
        for path in paths:
            relative = path.relative_to(ROOT).as_posix()
            if relative.startswith(("dist/", "target/")):
                continue
            digest.update(relative.encode("utf-8"))
            digest.update(b"\0")
            digest.update(bytes.fromhex(digest_file(path).removeprefix("sha256:")))
    return "sha256:" + digest.hexdigest()


def canonical_digest(value: object) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


def validate_catalog(document: dict[str, object]) -> list[str]:
    errors: list[str] = []
    if document.get("schema_id") != "star.external-analysis-provider-catalog":
        errors.append("catalog schema_id is invalid")
    if document.get("schema_version") != 1:
        errors.append("catalog schema_version must be 1")
    if document.get("normalization_owner") != "star-application":
        errors.append("normalization must remain application-owned")
    if document.get("gateway_tool_specific_parser") is not False:
        errors.append("Gateway tool-specific parsers are prohibited")
    if document.get("auto_install") is not False:
        errors.append("external providers must never be auto-installed")
    providers = document.get("providers")
    if not isinstance(providers, list):
        return [*errors, "providers must be an array"]
    ids = [provider.get("id") for provider in providers if isinstance(provider, dict)]
    if set(ids) != EXPECTED_IDS or len(ids) != len(EXPECTED_IDS):
        errors.append("provider IDs must match the complete unique expected set")
    for provider in providers:
        if not isinstance(provider, dict):
            errors.append("provider entry must be a table")
            continue
        provider_id = provider.get("id", "<missing>")
        stability = provider.get("stability")
        detail = provider.get("detail_level")
        machine = provider.get("machine_readable")
        normalize = provider.get("application_normalization")
        discovery = provider.get("discovery")
        candidates = provider.get("executable_candidates")
        version_command = provider.get("version_command")
        if stability not in ALLOWED_STABILITY or detail not in ALLOWED_DETAIL:
            errors.append(f"{provider_id}: invalid protocol classification")
        if not isinstance(machine, bool) or not isinstance(normalize, bool):
            errors.append(f"{provider_id}: machine/normalization flags must be booleans")
        if normalize and not (stability == "stable" and detail == "structured" and machine):
            errors.append(f"{provider_id}: only stable structured machine protocols may normalize")
        if detail == "raw_only" and normalize:
            errors.append(f"{provider_id}: raw-only protocol cannot normalize")
        if not provider.get("observation_schema") or not provider.get("limitation"):
            errors.append(f"{provider_id}: schema and limitation are required")
        if discovery not in {"path", "project_declared", "builtin"}:
            errors.append(f"{provider_id}: invalid discovery mode")
        if not isinstance(candidates, list) or not isinstance(version_command, list):
            errors.append(f"{provider_id}: discovery commands must be arrays")
        elif discovery == "path" and (not candidates or not version_command):
            errors.append(f"{provider_id}: path discovery requires executable and version command")
        elif discovery != "path" and (candidates or version_command):
            errors.append(f"{provider_id}: non-path discovery cannot imply a host executable")
    return errors


def discover(provider: dict[str, object]) -> dict[str, object]:
    provider_id = str(provider["id"])
    descriptor_fingerprint = canonical_digest(provider)
    mode = str(provider["discovery"])
    base = {
        "provider_id": provider_id,
        "descriptor_fingerprint": descriptor_fingerprint,
        "discovery": mode,
        "completeness": "unverified",
        "limitations": [
            "discovery proves executable identity only; no analysis result or protocol artifact was verified",
            str(provider["limitation"]),
        ],
    }
    if mode == "builtin":
        return {**base, "availability": "builtin_available", "executable": None}
    if mode == "project_declared":
        return {**base, "availability": "unavailable", "executable": None}

    executable = next(
        (shutil.which(str(candidate)) for candidate in provider["executable_candidates"] if shutil.which(str(candidate))),
        None,
    )
    if executable is None:
        return {**base, "availability": "unavailable", "executable": None}
    resolved = pathlib.Path(executable).resolve()
    command = [str(part) for part in provider["version_command"]]
    try:
        result = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=15,
            check=False,
        )
        raw = (result.stdout + result.stderr).strip()
        version_line = raw.splitlines()[0][:512] if raw else None
        return {
            **base,
            "availability": "available_unverified" if result.returncode == 0 else "version_failed",
            "executable": {
                "name": resolved.name,
                "sha256": digest_file(resolved),
                "version_command": command,
                "version_exit_code": result.returncode,
                "version_line": version_line,
                "version_output_sha256": "sha256:" + hashlib.sha256(raw.encode("utf-8")).hexdigest(),
            },
        }
    except (OSError, subprocess.TimeoutExpired) as error:
        return {
            **base,
            "availability": "version_failed",
            "executable": {
                "name": resolved.name,
                "sha256": digest_file(resolved),
                "version_command": command,
                "error_type": type(error).__name__,
            },
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    document = tomllib.loads(CATALOG.read_text(encoding="utf-8"))
    errors = validate_catalog(document)
    if errors:
        for error in errors:
            print(f"external-analysis-provider: ERROR {error}", file=sys.stderr)
        return 1
    providers = [discover(provider) for provider in document["providers"]]
    inventory = {
        "schema_id": "star.external-analysis-provider-inventory",
        "schema_version": 1,
        "source_fingerprint": source_fingerprint(),
        "catalog_fingerprint": canonical_digest(document),
        "providers": providers,
        "completeness": "unverified",
        "limitations": [
            "provider discovery is not an analysis run",
            "missing project declarations, inputs, or stable result artifacts remain unavailable or unverified",
        ],
    }
    if args.output is not None:
        output = args.output if args.output.is_absolute() else ROOT / args.output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    available = sum(item["availability"] in {"available_unverified", "builtin_available"} for item in providers)
    print(f"external-analysis-provider: PASS descriptors={len(providers)} available={available} completeness=unverified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
