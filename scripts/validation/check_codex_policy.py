from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".codex" / "config.toml"
RULES = ROOT / ".codex" / "rules" / "star-control.rules"
HOOKS = (
    ROOT
    / "integrations"
    / "codex-plugin-template"
    / "marketplace-root"
    / "plugins"
    / "star-control"
    / "hooks"
    / "hooks.json"
)
HOOK_SOURCE = ROOT / "apps" / "star-cli" / "src" / "local_commands.rs"
AGENTS = ROOT / "AGENTS.md"


def extract_rule_blocks(text: str) -> list[str]:
    blocks: list[str] = []
    offset = 0
    marker = "prefix_rule("
    while True:
        start = text.find(marker, offset)
        if start < 0:
            return blocks
        depth = 0
        quote: str | None = None
        escaped = False
        for index in range(start + len("prefix_rule"), len(text)):
            character = text[index]
            if quote is not None:
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == quote:
                    quote = None
                continue
            if character in {'"', "'"}:
                quote = character
            elif character == "(":
                depth += 1
            elif character == ")":
                depth -= 1
                if depth == 0:
                    blocks.append(text[start : index + 1])
                    offset = index + 1
                    break
        else:
            raise ValueError("unterminated prefix_rule block")


def run_execpolicy(arguments: list[str]) -> str | None:
    completed = subprocess.run(
        [
            "codex",
            "execpolicy",
            "check",
            "--rules",
            str(RULES),
            "--",
            *arguments,
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=15,
    )
    if completed.returncode != 0:
        raise ValueError(
            f"codex execpolicy check failed for {arguments!r}: "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )
    payload = json.loads(completed.stdout)
    decision = payload.get("decision")
    if decision not in {None, "allow", "prompt", "forbidden"}:
        raise ValueError(f"unexpected execpolicy decision: {decision!r}")
    return decision


def main() -> int:
    failures: list[str] = []
    config = tomllib.loads(CONFIG.read_text(encoding="utf-8"))
    if config.get("sandbox_mode") != "danger-full-access":
        failures.append("sandbox_mode must remain danger-full-access")
    if config.get("approval_policy") != "never":
        failures.append("approval_policy must remain never")
    if "approvals_reviewer" in config:
        failures.append("approvals_reviewer is inert under never and must be omitted")
    if config.get("features", {}).get("hooks") is not True:
        failures.append("Codex Hooks must remain enabled")

    rules_text = RULES.read_text(encoding="utf-8")
    blocks = extract_rule_blocks(rules_text)
    if not blocks:
        failures.append("no prefix_rule declarations found")
    prompt_blocks = [block for block in blocks if 'decision = "prompt"' in block]
    if prompt_blocks:
        failures.append("prompt rules are incompatible with approval_policy=never")
    allowed_blocks = [block for block in blocks if 'decision = "allow"' in block]
    for block in allowed_blocks:
        if 'pattern = ["git", "status", "--short"]' not in block:
            failures.append("broad or unreviewed allow rule detected")
        if re.search(r"P-\d{4}|temporary|task[-_ ]specific|expires?", block, re.IGNORECASE):
            failures.append("stale task-specific allow rule detected")

    required_rule_fragments = {
        'pattern = ["git", "clean"]': 'decision = "forbidden"',
        'pattern = ["git", "reset", "--hard"]': 'decision = "forbidden"',
        '"$CODEX_HOME/plugins/cache"': 'decision = "forbidden"',
        'pattern = ["sqlite3", ["$CODEX_HOME/state_5.sqlite"': 'decision = "forbidden"',
    }
    for pattern, decision in required_rule_fragments.items():
        if not any(pattern in block and decision in block for block in blocks):
            failures.append(f"missing reviewed rule: {pattern} -> {decision}")

    hook_config = json.loads(HOOKS.read_text(encoding="utf-8"))
    hook_sets = hook_config.get("hooks", {})
    expected_hooks = {
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "Stop",
        "PreToolUse",
        "PostToolUse",
        "SubagentStart",
        "SubagentStop",
    }
    if set(hook_sets) != expected_hooks:
        failures.append("Hook template must contain exactly the reviewed 8-event set")
    source = HOOK_SOURCE.read_text(encoding="utf-8")
    for marker in (
        "validate_typed_hook_input",
        "force push is forbidden regardless of flag position",
        "git reset --hard is forbidden",
        "git clean is forbidden",
        "protected_generated_state_reference",
        "recursive_delete_or_move_is_unverified",
        "PostToolUse는 이미 발생한 side effect를 되돌리지 않는다",
    ):
        if marker not in source:
            failures.append(f"typed Hook guard marker missing: {marker}")

    agents = AGENTS.read_text(encoding="utf-8")
    for boundary in (
        'approval_policy = "never"',
        'sandbox_mode = "danger-full-access"',
        "`prompt` Rules",
    ):
        if boundary not in agents:
            failures.append(f"AGENTS execution-policy boundary missing: {boundary}")

    codex = shutil.which("codex")
    if codex is not None:
        policy_cases = [
            (["git", "status", "--short"], "allow"),
            (["git", "push", "origin", "main"], None),
            (["git", "push", "--force", "origin", "main"], "forbidden"),
            (["git", "push", "origin", "main", "--force"], "forbidden"),
            (["git", "reset", "--hard"], "forbidden"),
            (["git", "reset", "HEAD", "--hard"], "forbidden"),
            (["npm", "ci"], None),
            (["python", "-m", "pip", "install", "ruff"], None),
            (["git", "branch", "-D", "topic"], None),
            (["git", "restore", "file.txt"], None),
            (["Remove-Item", "-LiteralPath", "D:\\work\\tmp"], None),
            (
                [
                    "Set-Content",
                    "-LiteralPath",
                    "$USERPROFILE/.codex/plugins/cache",
                    "changed",
                ],
                "forbidden",
            ),
            (
                ["sqlite3", "$USERPROFILE/.codex/state_5.sqlite", "VACUUM"],
                "forbidden",
            ),
        ]
        for arguments, expected in policy_cases:
            try:
                actual = run_execpolicy(arguments)
                if actual != expected:
                    failures.append(
                        f"execpolicy {arguments!r}: expected {expected}, found {actual}"
                    )
            except Exception as exc:
                failures.append(str(exc))
    else:
        print("codex-policy: static PASS; live execpolicy unavailable", file=sys.stderr)

    if failures:
        for failure in failures:
            print(f"codex-policy: {failure}", file=sys.stderr)
        return 1
    print(
        f"codex-policy: PASS rules={len(blocks)} hooks={len(hook_sets)} "
        f"live_execpolicy={'yes' if codex else 'no'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
