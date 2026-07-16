from __future__ import annotations

import argparse
import importlib.metadata
import json
import re
import sys
import tomllib
import urllib.parse
from pathlib import Path
from typing import Any


TEXT_SUFFIXES = {
    ".json",
    ".jsonl",
    ".lock",
    ".md",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".toml",
    ".txt",
    ".yaml",
    ".yml",
}
YAML_SUFFIXES = {".yaml", ".yml"}
LINK_PATTERN = re.compile(r"(?<!!)\[[^\]]*]\(([^)]+)\)|!\[[^\]]*]\(([^)]+)\)")
FENCE_PATTERN = re.compile(r"^\s{0,3}(\x60{3,}|~{3,})")


class DuplicateKeyError(ValueError):
    pass


def object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateKeyError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_text(path: Path) -> str:
    data = path.read_bytes()
    if b"\x00" in data:
        raise ValueError("NUL byte in tracked text file")
    text = data.decode("utf-8-sig")
    if "\ufffd" in text:
        raise ValueError("Unicode replacement character in tracked text file")
    return text


def validate_json(path: Path, text: str) -> None:
    if path.suffix.lower() == ".jsonl":
        for line_number, line in enumerate(text.splitlines(), start=1):
            if not line.strip():
                continue
            try:
                json.loads(line, object_pairs_hook=object_without_duplicates)
            except Exception as exc:
                raise ValueError(f"JSONL line {line_number}: {exc}") from exc
        return
    json.loads(text, object_pairs_hook=object_without_duplicates)


def validate_toml(text: str) -> None:
    tomllib.loads(text)


def normalize_link_target(raw_target: str) -> str | None:
    target = raw_target.strip()
    if target.startswith("<") and ">" in target:
        target = target[1 : target.index(">")]
    elif " " in target:
        target = target.split(maxsplit=1)[0]
    if not target or target.startswith("#"):
        return None
    lowered = target.lower()
    if lowered.startswith(("http://", "https://", "mailto:", "data:", "javascript:")):
        return None
    if re.match(r"^[a-zA-Z]:[\\/]", target):
        return None
    target = target.split("#", maxsplit=1)[0].split("?", maxsplit=1)[0]
    if not target or any(token in target for token in ("*", "{", "}", "$")):
        return None
    return urllib.parse.unquote(target)


def validate_markdown(root: Path, path: Path, text: str) -> list[str]:
    findings: list[str] = []
    open_fence: tuple[str, int] | None = None
    for line in text.splitlines():
        match = FENCE_PATTERN.match(line)
        if not match:
            continue
        marker = match.group(1)
        marker_kind = marker[0]
        if open_fence is None:
            open_fence = (marker_kind, len(marker))
        elif marker_kind == open_fence[0] and len(marker) >= open_fence[1]:
            open_fence = None
    if open_fence is not None:
        findings.append("unclosed Markdown fence")

    for match in LINK_PATTERN.finditer(text):
        raw_target = match.group(1) or match.group(2) or ""
        target = normalize_link_target(raw_target)
        if target is None:
            continue
        candidate = (root / target.lstrip("/")) if target.startswith("/") else (path.parent / target)
        try:
            candidate.resolve().relative_to(root)
        except ValueError:
            findings.append(f"link escapes repository: {raw_target}")
            continue
        if not candidate.exists():
            line_number = text.count("\n", 0, match.start()) + 1
            findings.append(f"missing local link at line {line_number}: {raw_target}")
    return findings


def load_yaml() -> tuple[Any | None, str | None]:
    try:
        installed = importlib.metadata.version("PyYAML")
        if installed != "6.0.3":
            return None, f"PyYAML 6.0.3 required, found {installed}"
        import yaml

        return yaml, None
    except importlib.metadata.PackageNotFoundError:
        return None, "PyYAML 6.0.3 is unavailable"
    except Exception as exc:
        return None, f"PyYAML environment error: {exc}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--paths-file", required=True)
    args = parser.parse_args()

    root = Path(args.root).resolve()
    raw_paths = json.loads(Path(args.paths_file).read_text(encoding="utf-8"))
    if not isinstance(raw_paths, list) or not all(isinstance(item, str) for item in raw_paths):
        raise ValueError("paths file must contain a JSON string array")

    paths: list[tuple[str, Path]] = []
    for relative in sorted(set(raw_paths)):
        candidate = (root / relative).resolve()
        try:
            candidate.relative_to(root)
        except ValueError as exc:
            raise ValueError(f"path escapes repository: {relative}") from exc
        if candidate.is_file() and candidate.suffix.lower() in TEXT_SUFFIXES:
            paths.append((relative.replace("\\", "/"), candidate))

    failures: list[str] = []
    yaml_items: list[tuple[str, Path, str]] = []
    checked = 0
    for relative, path in paths:
        try:
            text = read_text(path)
            path_parts = Path(relative).parts
            if "invalid" in path_parts and ("fixtures" in path_parts or "examples" in path_parts):
                checked += 1
                continue
            suffix = path.suffix.lower()
            if suffix in {".json", ".jsonl"}:
                validate_json(path, text)
            elif suffix in {".toml", ".lock"}:
                validate_toml(text)
            elif suffix == ".md":
                failures.extend(f"{relative}: {finding}" for finding in validate_markdown(root, path, text))
            elif suffix in YAML_SUFFIXES:
                yaml_items.append((relative, path, text))
            checked += 1
        except Exception as exc:
            failures.append(f"{relative}: {exc}")

    unverified: list[str] = []
    if yaml_items:
        yaml, reason = load_yaml()
        if yaml is None:
            unverified.append(reason or "YAML parser unavailable")
        else:
            for relative, _, text in yaml_items:
                try:
                    list(yaml.safe_load_all(text))
                except Exception as exc:
                    failures.append(f"{relative}: YAML parse error: {exc}")

    if failures:
        status = "fail"
        exit_code = 1
    elif unverified:
        status = "unverified"
        exit_code = 3
    else:
        status = "pass"
        exit_code = 0
    print(
        json.dumps(
            {
                "status": status,
                "checked": checked,
                "failures": failures,
                "unverified": unverified,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return exit_code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(json.dumps({"status": "fail", "error": str(exc)}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(1)
