from __future__ import annotations

import re
from pathlib import Path

from scaffold_from_design_sources import ROOT, V3, V4, map_v3_source, map_v4_source, normalize_design_text, read_text


def source_files(root: Path) -> list[Path]:
    return sorted([p for p in root.rglob("*") if p.is_file()], key=lambda p: p.as_posix().lower())


def rel_posix(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def target_exists(target: str) -> bool:
    if target == "docs/decisions/source-absorption-map.md":
        return True
    if "provider-*.schema.json" in target:
        return any((ROOT / "specs" / "schemas").glob("provider-*.schema.json"))
    return (ROOT / target).exists()


def target_text(target: str) -> str:
    return read_text(ROOT / target)


def normalized_source_text(path: Path) -> str:
    return normalize_design_text(read_text(path)).strip()


def assert_text_absorbed(source: Path, target: str, failures: list[str]) -> None:
    source_norm = normalized_source_text(source)
    if source.name == ".gitkeep":
        return
    if not source_norm:
        return
    if not target_exists(target):
        failures.append(f"missing target: {source} -> {target}")
        return
    text = target_text(target)
    if source_norm not in text:
        failures.append(f"text not found in target: {source} -> {target}")


def audit() -> tuple[list[str], list[str]]:
    failures: list[str] = []
    notes: list[str] = []

    v3_files = source_files(V3)
    v4_files = source_files(V4)
    expected_total = len(v3_files) + len(v4_files)

    rows = []
    map_path = ROOT / "docs" / "decisions" / "source-absorption-map.md"
    for line in read_text(map_path).splitlines():
        match = re.match(r"^\| `([^`]+)` \| `([^`]+)` \| ([^|]+) \| ([^|]+) \|$", line)
        if match:
            rows.append(match.groups())

    if len(rows) != expected_total:
        failures.append(f"source map row count mismatch: rows={len(rows)} expected={expected_total}")

    for source, target, _status, _note in rows:
        if not target_exists(target):
            failures.append(f"mapped target missing: {source} -> {target}")

    for src in v3_files:
        rel = src.relative_to(V3)
        target, _status, _note = map_v3_source(rel)
        first = rel.parts[0]
        if first == "providers":
            group, slug = target.split("/")[1], target.split("/")[2]
            source_target = f"builtin-providers/{group}/{slug}/docs/v3-provider-source.yaml"
            assert_text_absorbed(src, source_target, failures)
            continue
        if first == "provider-features" and src.name.endswith(".features.yaml"):
            group, slug = target.split("/")[1], target.split("/")[2]
            source_target = f"builtin-providers/{group}/{slug}/docs/v3-features-source.yaml"
            assert_text_absorbed(src, source_target, failures)
            continue
        if first == "schemas" and src.name == "provider.schema.json":
            if not any((ROOT / "specs" / "schemas").glob("provider-*.schema.json")):
                failures.append("provider.schema.json split target missing")
            continue
        if first in {"quality", "retrieval", "vcs", "tools", "control-plane", "runs"} and src.name == ".gitkeep":
            if not target_exists(target):
                failures.append(f"scaffold .gitkeep target missing: {src} -> {target}")
            continue
        assert_text_absorbed(src, target, failures)

    for src in v4_files:
        rel = src.relative_to(V4)
        target, _status, _note = map_v4_source(rel)
        assert_text_absorbed(src, target, failures)

    notes.append(f"v3 files: {len(v3_files)}")
    notes.append(f"v4 files: {len(v4_files)}")
    notes.append(f"source map rows: {len(rows)}")
    notes.append(f"missing mapped targets: 0" if not any("mapped target missing" in f for f in failures) else "missing mapped targets: present")
    notes.append(f"content absorption failures: {len(failures)}")
    return failures, notes


def write_report(failures: list[str], notes: list[str]) -> None:
    lines = [
        "# Source Absorption Audit",
        "",
        "## Summary",
        "",
    ]
    for note in notes:
        lines.append(f"- {note}")
    lines.extend(
        [
            "",
            "## Result",
            "",
            "PASS" if not failures else "FAIL",
            "",
            "## Failures",
            "",
        ]
    )
    if failures:
        lines.extend(f"- {failure}" for failure in failures)
    else:
        lines.append("- None.")
    lines.extend(
        [
            "",
            "## Coverage Rules",
            "",
            "- Every file under both source folders must have one row in `docs/decisions/source-absorption-map.md`.",
            "- Every mapped target must exist, except split schema targets represented by `provider-*.schema.json`.",
            "- Directly absorbed text files must contain the normalized source text.",
            "- Provider manifests and feature matrices must also preserve the original v3 YAML under each builtin provider `docs/` directory.",
        ]
    )
    (ROOT / "docs" / "decisions" / "source-absorption-audit.md").write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


if __name__ == "__main__":
    failures, notes = audit()
    write_report(failures, notes)
    for note in notes:
        print(note)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        raise SystemExit(1)
    print("PASS")
