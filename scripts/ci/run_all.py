#!/usr/bin/env python3
"""Run all local Star-Control contract checks in CI order."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

COMMANDS: tuple[tuple[str, ...], ...] = (
    (sys.executable, "scripts/ci/check_repo_policy.py"),
    (sys.executable, "scripts/ci/check_data_formats.py"),
    (sys.executable, "scripts/ci/check_manifest_contracts.py"),
    (sys.executable, "scripts/ci/check_star_sentinel_naming.py"),
    (sys.executable, "scripts/ci/check_schema_examples.py"),
    (sys.executable, "scripts/ci/check_implementation_docs.py"),
    (sys.executable, "scripts/ci/check_work_queue_consistency.py"),
)


def command_label(command: tuple[str, ...]) -> str:
    executable, *args = command
    if Path(executable).name.startswith("python"):
        executable = "python"
    return " ".join((executable, *args))


def main() -> int:
    for command in COMMANDS:
        print(f"$ {command_label(command)}", flush=True)
        result = subprocess.run(command, cwd=ROOT, check=False)
        if result.returncode != 0:
            print(
                f"ERROR: command failed with exit code {result.returncode}: {command_label(command)}",
                file=sys.stderr,
            )
            return result.returncode

    print(f"all local contract checks passed: {len(COMMANDS)} command(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
