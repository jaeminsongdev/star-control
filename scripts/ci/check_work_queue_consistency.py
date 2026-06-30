#!/usr/bin/env python3
"""Validate current Codex work queue authority and handoff structure."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CURRENT_QUEUE = "docs/implementation/codex-work-queue-current.md"
LONG_QUEUE_NAME = "codex-work-queue.md"

DOCUMENT_AUTHORITY_CHECKS = (
    ("README.md", CURRENT_QUEUE),
    ("AGENTS.md", CURRENT_QUEUE),
    ("docs/implementation/README.md", "codex-work-queue-current.md"),
    ("docs/implementation/codex-long-run-workflow.md", "codex-work-queue-current.md"),
    ("docs/implementation/repository-layout.md", "codex-work-queue-current.md"),
)

EPICS = (
    "E01 Schema / Runtime Validator",
    "E02 File-based StateStore",
    "E03 Artifact Layout Writer",
    "E04 Provider Registry",
    "E05 FakeProviderAdapter",
    "E06 RouterEngine",
    "E07 ExecutionEngine",
    "E08 CLI read-only + fake run",
    "E09 Star Sentinel P0",
    "E10 ValidationEngine",
    "E11 Integration Smoke",
)

SECTION_MARKERS = (
    "선행 문서:",
    "허용 파일:",
    "금지 파일:",
    "입력 artifact:",
    "출력 artifact:",
    "핵심 TASK:",
    "완료 기준:",
    "다음 EPIC handoff:",
)

E08_SPLIT_MARKERS = (
    "status/report",
    "run dry-run",
    "run with fake provider",
    "approve/cancel/resume",
)

E09_SPLIT_MARKERS = (
    "P0 evaluator",
    "gate writer",
    "review-pack writer",
    "selfcheck",
)

RESERVED_MARKERS = (
    "Local Process Provider",
    "Local Model Provider",
    "Cloud CLI Provider",
    "Cloud API Provider",
    "Daemon",
    "API",
    "UI Shell",
)


def read_text(relative_path: str) -> str:
    return (ROOT / relative_path).read_text(encoding="utf-8")


def require_contains(text: str, needle: str, context: str, errors: list[str]) -> None:
    if needle not in text:
        errors.append(f"{context}: missing {needle!r}")


def check_document_authority(errors: list[str]) -> None:
    for relative_path, expected in DOCUMENT_AUTHORITY_CHECKS:
        text = read_text(relative_path)
        require_contains(text, expected, relative_path, errors)


def check_current_queue(errors: list[str]) -> None:
    text = read_text(CURRENT_QUEUE)
    require_contains(text, "현재 구현 착수 순서의 최상위 기준", CURRENT_QUEUE, errors)
    require_contains(text, LONG_QUEUE_NAME, CURRENT_QUEUE, errors)

    for epic in EPICS:
        require_contains(text, f"## {epic}", CURRENT_QUEUE, errors)

    for marker in SECTION_MARKERS:
        count = text.count(marker)
        if count < len(EPICS):
            errors.append(
                f"{CURRENT_QUEUE}: marker {marker!r} appears {count} time(s), "
                f"expected at least {len(EPICS)}"
            )

    for marker in E08_SPLIT_MARKERS:
        require_contains(text, marker, "E08 split guidance", errors)

    for marker in E09_SPLIT_MARKERS:
        require_contains(text, marker, "E09 split guidance", errors)

    reserved_start = text.find("## RESERVED")
    if reserved_start == -1:
        errors.append(f"{CURRENT_QUEUE}: missing RESERVED section")
        return

    reserved_text = text[reserved_start:]
    for marker in RESERVED_MARKERS:
        require_contains(reserved_text, marker, "RESERVED section", errors)


def check_long_queue_relationship(errors: list[str]) -> None:
    text = read_text("docs/implementation/codex-long-run-workflow.md")
    require_contains(text, CURRENT_QUEUE, "codex-long-run-workflow.md", errors)
    require_contains(text, "장기 backlog", "codex-long-run-workflow.md", errors)


def main() -> int:
    errors: list[str] = []
    check_document_authority(errors)
    check_current_queue(errors)
    check_long_queue_relationship(errors)

    if errors:
        print("ERROR: work queue consistency check failed", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("work queue consistency check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
