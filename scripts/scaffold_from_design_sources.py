from __future__ import annotations

import json
import shutil
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
V3 = Path(r"D:\개발\관제\star-control_design_v3")
V4 = Path(r"D:\개발\관제\custom_dev_verification_platform_design_v4_curated")

LEGACY_DASH = "auto" + "code-guard"
LEGACY_TITLE = "Auto" + "code Guard"
LEGACY_PACKAGE = "star-" + LEGACY_DASH
LEGACY_MODULE = "star_" + "autocode_guard"
OLD_TASK_SCHEMA = "task" + "_spec.schema.json"


def read_text(path: Path) -> str:
    for encoding in ("utf-8-sig", "utf-8", "cp949"):
        try:
            return path.read_text(encoding=encoding)
        except UnicodeDecodeError:
            continue
    return path.read_text(encoding="utf-8", errors="replace")


def write(path: str | Path, text: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text.rstrip() + "\n", encoding="utf-8")


def copy_binary_or_text(src: Path, dest: str | Path, *, transform: bool = True, header: str | None = None) -> None:
    target = ROOT / dest
    target.parent.mkdir(parents=True, exist_ok=True)
    if src.suffix.lower() in {".md", ".yaml", ".yml", ".json", ".toml", ".csv", ".txt"}:
        text = read_text(src)
        if transform:
            text = normalize_design_text(text)
        if header:
            text = header.rstrip() + "\n\n" + text.lstrip()
        target.write_text(text.rstrip() + "\n", encoding="utf-8")
    else:
        shutil.copyfile(src, target)


def touch(path: str | Path) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.touch()


def dump_json(path: str | Path, data: dict) -> None:
    write(path, json.dumps(data, ensure_ascii=False, indent=2))


def normalize_design_text(text: str) -> str:
    replacements = {
        "builtin-tools/" + LEGACY_DASH: "builtin-tools/star-sentinel",
        "packages/" + LEGACY_PACKAGE: "packages/star-sentinel",
        "tool-output/" + LEGACY_DASH: "tool-output/star-sentinel",
        "docs/tools/" + LEGACY_DASH + ".md": "docs/tools/star-sentinel.md",
        LEGACY_PACKAGE: "star-sentinel",
        LEGACY_MODULE: "star_sentinel",
        LEGACY_TITLE: "Star Sentinel",
        LEGACY_DASH: "star-sentinel",
        OLD_TASK_SCHEMA: "sentinel-task.schema.json",
        "task-spec.example.yaml": "sentinel-task.example.yaml",
        "task_spec": "sentinel_task",
        "Task Spec": "Star Sentinel Task",
        "task spec": "sentinel task",
        "AutocodePolicy": "StarSentinelPolicy",
        "review_pack.schema.json": "review-pack.schema.json",
        "corpus_case.schema.json": "corpus-case.schema.json",
    }
    for old, new in replacements.items():
        text = text.replace(old, new)
    return text


def source_header(source_label: str, note: str) -> str:
    return (
        f"> 흡수 출처: `{source_label}`\n"
        f"> 정리 상태: {note}\n"
    )


def provider_slug_from_v3(name: str) -> tuple[str, str]:
    stem = Path(name).stem
    mapping = {
        "codex": ("cloud-cli", "codex-cli"),
        "claude-code": ("cloud-cli", "claude-code"),
        "gemini-cli": ("cloud-cli", "gemini-cli"),
        "openai-api": ("cloud-api", "openai"),
        "anthropic-api": ("cloud-api", "anthropic"),
        "gemini-api": ("cloud-api", "google-gemini"),
        "local-ollama": ("local-server", "ollama"),
        "local-vllm": ("local-server", "vllm"),
        "local-lmstudio": ("local-server", "lm-studio"),
        "cursor": ("cloud-cli", "cursor"),
        "github-copilot": ("cloud-cli", "github-copilot"),
        "jules": ("cloud-cli", "jules"),
        "devin": ("cloud-cli", "devin"),
    }
    return mapping.get(stem, ("cloud-cli", stem))


PROVIDERS = [
    {
        "group": "cloud-cli",
        "slug": "codex-cli",
        "id": "provider.codex-cli",
        "name": "Codex CLI",
        "kind": "cloud_cli_agent",
        "transport": "cli",
        "adapter": "code_agent",
        "executable": "codex",
        "parser": "codex-cli-default",
        "capabilities": {
            "edit_files": True,
            "run_shell": True,
            "read_repo": True,
            "apply_patch": True,
            "structured_output": "partial",
            "offline": False,
            "requires_login_session": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "claude-code",
        "id": "provider.claude-code",
        "name": "Claude Code",
        "kind": "cloud_cli_agent",
        "transport": "cli",
        "adapter": "code_agent",
        "executable": "claude",
        "parser": "claude-code-default",
        "capabilities": {
            "edit_files": True,
            "run_shell": True,
            "read_repo": True,
            "apply_patch": "partial",
            "structured_output": "partial",
            "offline": False,
            "requires_login_session": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "gemini-cli",
        "id": "provider.gemini-cli",
        "name": "Gemini CLI",
        "kind": "cloud_cli_agent",
        "transport": "cli",
        "adapter": "code_agent",
        "executable": "gemini",
        "parser": "gemini-cli-default",
        "capabilities": {
            "edit_files": True,
            "run_shell": True,
            "read_repo": True,
            "apply_patch": "partial",
            "structured_output": "partial",
            "offline": False,
            "requires_login_session": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "cursor",
        "id": "provider.cursor",
        "name": "Cursor",
        "kind": "cloud_cli_agent",
        "transport": "cli",
        "adapter": "code_agent",
        "executable": "cursor",
        "parser": "cursor-default",
        "capabilities": {
            "edit_files": True,
            "run_shell": "partial",
            "read_repo": True,
            "apply_patch": "partial",
            "structured_output": "partial",
            "offline": False,
            "requires_login_session": True,
            "native_agent_mode": True,
            "native_plan_mode": True,
            "native_background_agents": True,
            "native_rules": True,
            "native_memories": True,
            "native_mcp": True,
            "native_semantic_search": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "github-copilot",
        "id": "provider.github-copilot",
        "name": "GitHub Copilot",
        "kind": "cloud_cli_agent",
        "transport": "manual",
        "adapter": "code_agent",
        "parser": "github-copilot-report",
        "capabilities": {
            "edit_files": "manual",
            "run_shell": "manual",
            "read_repo": True,
            "apply_patch": "manual",
            "structured_output": "manual",
            "offline": False,
            "requires_login_session": True,
            "native_cloud_agent": True,
            "native_custom_instructions": True,
            "native_path_specific_instructions": True,
            "native_custom_agents": True,
            "native_agent_skills": True,
            "native_prompt_files": True,
            "native_mcp": True,
            "native_code_review": True,
            "native_branch_pr_workflow": True,
            "native_budgets": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "jules",
        "id": "provider.jules",
        "name": "Jules",
        "kind": "cloud_cli_agent",
        "transport": "manual",
        "adapter": "code_agent",
        "parser": "jules-report",
        "capabilities": {
            "edit_files": "manual",
            "run_shell": "manual",
            "read_repo": True,
            "apply_patch": "manual",
            "structured_output": "manual",
            "offline": False,
            "requires_login_session": True,
            "native_async_tasks": True,
            "native_github_integration": True,
            "native_branch_changes": True,
            "native_diff_review": True,
            "native_pr_creation": True,
            "native_test_suite": True,
        },
    },
    {
        "group": "cloud-cli",
        "slug": "devin",
        "id": "provider.devin",
        "name": "Devin",
        "kind": "cloud_cli_agent",
        "transport": "manual",
        "adapter": "code_agent",
        "parser": "devin-report",
        "capabilities": {
            "edit_files": "manual",
            "run_shell": "manual",
            "read_repo": True,
            "apply_patch": "manual",
            "structured_output": "manual",
            "offline": False,
            "requires_login_session": True,
            "native_autonomous_software_engineer": True,
            "native_cloud_workspace": True,
            "native_saved_machine_state": True,
            "native_long_running_sessions": True,
            "native_team_workflow": True,
        },
    },
    {
        "group": "cloud-api",
        "slug": "openai",
        "id": "provider.openai",
        "name": "OpenAI API",
        "kind": "cloud_api_model",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "https://api.openai.com/v1",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": True,
            "offline": False,
            "requires_login_session": False,
        },
    },
    {
        "group": "cloud-api",
        "slug": "anthropic",
        "id": "provider.anthropic",
        "name": "Anthropic API",
        "kind": "cloud_api_model",
        "transport": "http",
        "adapter": "chat_model",
        "base_url": "https://api.anthropic.com",
        "parser": "anthropic-messages",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": False,
            "requires_login_session": False,
        },
    },
    {
        "group": "cloud-api",
        "slug": "google-gemini",
        "id": "provider.google-gemini",
        "name": "Google Gemini API",
        "kind": "cloud_api_model",
        "transport": "http",
        "adapter": "chat_model",
        "base_url": "https://generativelanguage.googleapis.com",
        "parser": "gemini-api",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": True,
            "offline": False,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-server",
        "slug": "generic-openai-compatible",
        "id": "provider.local-openai-compatible",
        "name": "Local OpenAI-Compatible Server",
        "kind": "local_openai_compatible_server",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "{{base_url}}",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": True,
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-server",
        "slug": "ollama",
        "id": "provider.ollama",
        "name": "Ollama",
        "kind": "local_openai_compatible_server",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "http://127.0.0.1:11434/v1",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-server",
        "slug": "vllm",
        "id": "provider.vllm",
        "name": "vLLM Server",
        "kind": "local_openai_compatible_server",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "http://127.0.0.1:8000/v1",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": True,
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-server",
        "slug": "lm-studio",
        "id": "provider.lm-studio",
        "name": "LM Studio",
        "kind": "local_openai_compatible_server",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "http://127.0.0.1:1234/v1",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-server",
        "slug": "llama-cpp-server",
        "id": "provider.llama-cpp-server",
        "name": "llama.cpp Server",
        "kind": "local_openai_compatible_server",
        "transport": "http",
        "adapter": "openai_compatible",
        "base_url": "http://127.0.0.1:8080/v1",
        "parser": "openai-compatible-chat",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-process",
        "slug": "generic-local-process",
        "id": "provider.local-process",
        "name": "Generic Local Process Runner",
        "kind": "local_process_model",
        "transport": "process",
        "adapter": "chat_model",
        "executable": "{{runner_path}}",
        "parser": "plain-text-or-json",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-process",
        "slug": "llama-cpp",
        "id": "provider.llama-cpp",
        "name": "llama.cpp Binary",
        "kind": "local_process_model",
        "transport": "process",
        "adapter": "chat_model",
        "executable": "llama-cli",
        "parser": "plain-text-or-json",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "local-process",
        "slug": "custom-runner",
        "id": "provider.custom-runner",
        "name": "Custom Local Runner",
        "kind": "local_process_model",
        "transport": "process",
        "adapter": "chat_model",
        "executable": "{{runner_path}}",
        "parser": "plain-text-or-json",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": False,
            "apply_patch": False,
            "structured_output": "partial",
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "test",
        "slug": "fake-provider",
        "id": "provider.fake",
        "name": "Fake Provider",
        "kind": "fake_provider",
        "transport": "manual",
        "adapter": "code_agent",
        "parser": "fake-provider-json",
        "capabilities": {
            "edit_files": False,
            "run_shell": False,
            "read_repo": True,
            "apply_patch": False,
            "structured_output": True,
            "offline": True,
            "requires_login_session": False,
        },
    },
    {
        "group": "test",
        "slug": "human-handoff",
        "id": "provider.human-handoff",
        "name": "Human Handoff",
        "kind": "human_handoff",
        "transport": "manual",
        "adapter": "code_agent",
        "parser": "manual-report",
        "capabilities": {
            "edit_files": "manual",
            "run_shell": "manual",
            "read_repo": "manual",
            "apply_patch": "manual",
            "structured_output": "manual",
            "offline": True,
            "requires_login_session": False,
        },
    },
]


def yaml_scalar(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "null"
    if isinstance(value, (int, float)):
        return str(value)
    s = str(value)
    if s in {"true", "false", "null"} or s.startswith("{") or ":" in s:
        return json.dumps(s, ensure_ascii=False)
    return s


def yaml_mapping(data: dict, indent: int = 0) -> str:
    lines: list[str] = []
    prefix = " " * indent
    for key, value in data.items():
        if isinstance(value, dict):
            lines.append(f"{prefix}{key}:")
            lines.append(yaml_mapping(value, indent + 2))
        elif isinstance(value, list):
            lines.append(f"{prefix}{key}:")
            for item in value:
                if isinstance(item, dict):
                    lines.append(f"{prefix}  -")
                    lines.append(yaml_mapping(item, indent + 4))
                else:
                    lines.append(f"{prefix}  - {yaml_scalar(item)}")
        else:
            lines.append(f"{prefix}{key}: {yaml_scalar(value)}")
    return "\n".join(lines)


def provider_manifest(provider: dict) -> str:
    data: dict[str, object] = {
        "id": provider["id"],
        "name": provider["name"],
        "kind": provider["kind"],
        "transport": provider["transport"],
        "adapter": provider["adapter"],
    }
    if provider["transport"] in {"cli", "process"}:
        data["command"] = {
            "executable": provider.get("executable", "{{executable}}"),
            "args_template": ["{{rendered_input}}"],
        }
    elif provider["transport"] == "http":
        data["endpoint"] = {
            "base_url": provider.get("base_url", "{{base_url}}"),
            "model": "{{model}}",
        }
    else:
        data["handoff"] = {"mode": "manual", "expected_report": "normalized-provider-result.json"}
    data["capabilities"] = provider["capabilities"]
    data["risk"] = {
        "can_modify_workspace": bool(provider["capabilities"].get("edit_files") is True),
        "can_run_commands": bool(provider["capabilities"].get("run_shell") is True),
        "requires_sandbox": provider["transport"] in {"cli", "process"},
    }
    data["outputs"] = {"parser": provider["parser"]}
    return yaml_mapping(data) + "\n"


def provider_capabilities(provider: dict) -> str:
    caps = provider["capabilities"]
    data = {
        "provider": provider["id"],
        "capability_profile": {
            "can": {
                "edit_files": caps.get("edit_files", False),
                "run_shell": caps.get("run_shell", False),
                "read_repo": caps.get("read_repo", False),
                "apply_patch": caps.get("apply_patch", False),
                "return_json": caps.get("structured_output", False),
                "work_offline": caps.get("offline", False),
                "use_tools": provider["transport"] == "cli",
            },
            "routing_tags": [
                provider["group"],
                provider["kind"],
                provider["transport"],
            ],
        },
    }
    return yaml_mapping(data) + "\n"


def write_root_files() -> None:
    write(
        "AGENTS.md",
        """# Star-Control AGENTS.md

## 기본 원칙

- 기본 응답과 문서는 한국어를 우선한다.
- 설정 키, 명령어, 코드, 파일명, 로그 원문은 원문 표기를 유지한다.
- Star-Control은 provider-neutral 관제/라우팅/실행/상태관리 본체다.
- Star Sentinel은 Star-Control 기본 탑재 검증 도구이며, 코어에 직접 결합하지 않는다.

## 작업 경계

- 원본 설계 폴더는 삭제하지 않는다.
- 원격 저장소 push, 외부 계정 수정, 의존성 설치, 패키지 매니저 도입은 명시 승인 전까지 하지 않는다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트의 `.ai-runs/` 아래에 둔다.
- provider 구현은 제품명 package가 아니라 transport, adapter, capability 중심으로 분리한다.

## 검증 기준

- JSON schema는 파싱 가능해야 한다.
- 정식 Star Sentinel 명칭은 `Star Sentinel`, `star-sentinel`, `star_sentinel`, `star.sentinel`만 사용한다.
- 과거 이름은 `builtin-tools/star-sentinel/tool.yaml`의 `legacy_aliases`와 원본 흡수 맵의 출처 표기에만 남긴다.
""",
    )
    write(
        "README.md",
        """# Star-Control

Star-Control은 여러 AI coding agent, cloud API model, local model server, local process runner, fake provider, human handoff를 공통 규격으로 다루는 provider-neutral 작업 관제 시스템이다.

## 핵심 구성

- `docs/`: Star-Control 정본 설계와 운영 문서.
- `specs/`: JSON schema와 provider/tool 계약.
- `configs/`: 기본 설정, 정책, 역할, hook, template, registry.
- `packages/`: 구현 예정 package 경계. 현재는 스캐폴드만 둔다.
- `builtin-providers/`: 구체 provider manifest와 capability profile.
- `builtin-tools/star-sentinel/`: Star Sentinel 내장 도구 manifest, policy, schema, template, corpus.
- `examples/`: provider instance, sample project, sample run artifact.

## Provider 원칙

Star-Control은 provider를 이름이 아니라 다음 축으로 판단한다.

- provider kind
- transport
- adapter
- capability profile
- provider instance

구체 provider는 `builtin-providers/` 아래 manifest로 등록하고, core package 이름에는 특정 회사나 제품명을 넣지 않는다.

## Star Sentinel

Star Sentinel은 AI가 만든 변경사항을 diff, policy, evidence, validation 기반으로 검증하고 review pack과 approval gate를 생성하는 내장 도구다.

구현 코드는 `packages/star-sentinel/`, 등록정보와 정책은 `builtin-tools/star-sentinel/`에 둔다.

## 실행 결과 위치

Star-Control repository에는 실행 결과를 저장하지 않는다. 대상 프로젝트에 다음 형태로 저장한다.

```text
대상 프로젝트/.ai-runs/J-0001/provider-output/{provider-instance-id}/
대상 프로젝트/.ai-runs/J-0001/tool-output/star-sentinel/
```

## 원본 설계 흡수 상태

두 원본 설계 폴더는 `docs/decisions/source-absorption-map.md`에 파일별 흡수 위치를 기록했다. 원본 삭제는 이 문서와 검증 결과를 확인한 뒤 별도 승인으로 처리한다.
""",
    )
    write(
        "LICENSE",
        """MIT License

Copyright (c) 2026 Star-Control contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
""",
    )
    write(
        "CHANGELOG.md",
        """# Changelog

## 0.1.0-scaffold - 2026-06-28

- Star-Control monorepo 스캐폴드 생성.
- v3 Star-Control 설계와 v4 curated 검증 플랫폼 설계를 정규 구조로 흡수.
- Star Sentinel 명칭, provider-neutral 구조, builtin provider/tool manifest 체계 반영.
- 원본 폴더 삭제 없이 파일별 흡수 맵 추가.
""",
    )
    write(
        ".gitignore",
        """.DS_Store
Thumbs.db

# Local runtime artifacts
.star-control/cache/
.star-control/logs/
.ai-runs/

# Secrets
*.secret
*.key
.env
.env.*
!.env.example

# Build/cache outputs
dist/
build/
target/
node_modules/
__pycache__/
.pytest_cache/
.mypy_cache/
.ruff_cache/
""",
    )
    write(
        "PLANS.md",
        """# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control 설계 흡수 스캐폴드는 완료됨.
- `PLANS.md`는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- 원본 설계 폴더는 별도 승인 없이 삭제하지 않는다.
- 의존성 추가, 패키지 매니저 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.

### 이미 끝난 것

- v3/v4 원본 237개 파일을 정규 구조로 흡수했다.
- 흡수 감사에서 mapped target 누락 0개, content absorption failure 0개를 확인했다.
- GitHub `origin/main`에 설계 흡수 커밋까지 반영했다.

### 아직 남은 것

- 실제 구현 언어와 패키지 매니저 결정.
- 원본 설계 폴더 삭제 여부는 사용자 별도 승인 필요.

### 건드리면 안 되는 것

- `D:/개발/관제/star-control_design_v3`
- `D:/개발/관제/custom_dev_verification_platform_design_v4_curated`
- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.

### 먼저 확인할 파일

- `README.md`
- `docs/decisions/source-absorption-map.md`
- `docs/decisions/source-absorption-audit.md`
- `configs/registries/builtin-provider-registry.yaml`

### 먼저 실행할 명령

```powershell
powershell -ExecutionPolicy Bypass -File ./scripts/test.ps1
python scripts/audit_source_absorption.py
```

### 현재 차단 요소

- 없음.

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | 실제 구현 언어와 패키지 매니저가 미정 | 다음 구현 단계의 build/test 전략 미확정 | 구현 착수 전 결정 |
| R-0002 | 원본 설계 폴더 삭제 보류 | 디스크에는 중복 원본이 남음 | 삭제 요청 시 흡수 감사 문서 확인 후 별도 처리 |

## Archive References

| 항목 | 위치 |
|---|---|
| 원본 파일별 흡수 위치 | `docs/decisions/source-absorption-map.md` |
| 흡수 감사 결과 | `docs/decisions/source-absorption-audit.md` |
| 설계 흡수 보강 커밋 | `c321f11` |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | v3/v4 설계를 Star-Control monorepo 스캐폴드와 정본 문서로 흡수 | `7ccdce5` |
| P-0002 | 2026-06-28 | 원본 237개 파일 흡수 누락 재검토 및 provider/source 보존 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
""",
    )


def write_canonical_docs() -> None:
    write(
        "docs/00_개요.md",
        """# 00. Star-Control 개요

## 정의

Star-Control은 사용자 요청을 `Job`, `Route`, `WorkSpec`, `RunState`, `Report`로 정규화하고 여러 AI 실행자를 provider-neutral 방식으로 제어하는 관제 시스템이다.

## 핵심 원칙

- 특정 AI 회사, 특정 CLI, 특정 로컬 런너에 종속되지 않는다.
- provider 선택은 이름이 아니라 capability와 policy로 결정한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트의 `.ai-runs/`에 저장한다.
- Star Sentinel은 기본 탑재 검증 도구지만 Star-Control core에 직접 결합하지 않는다.

## 주요 구성

```text
사용자 요청
  -> Star-Control Router
  -> Job / Route / WorkSpec
  -> Provider Host + Transport + Adapter
  -> Provider Result
  -> Star Sentinel / Quality Gate
  -> Final Report
```

## 삭제 가능 판단 기준

원본 설계 폴더 삭제는 다음이 모두 참일 때 별도 승인으로 진행한다.

- `docs/decisions/source-absorption-map.md`가 두 원본 폴더의 모든 파일을 포함한다.
- 정본 문서, schema, provider manifest, Star Sentinel 문서가 검증을 통과한다.
- 원본에만 남은 결정이나 스키마가 없음을 사람이 확인한다.
""",
    )
    write(
        "docs/01_아키텍처.md",
        """# 01. Star-Control 아키텍처

## Provider 계층

```text
Provider Manifest
  -> Provider Instance
  -> Transport
  -> Adapter
  -> Normalized Provider Result
```

- `Provider Manifest`: provider 종류, 기본 command/API 형태, 출력 parser, 위험도.
- `Provider Instance`: 사용자 PC나 프로젝트에서 실제 쓰는 실행 경로, URL, model, limit.
- `Transport`: CLI, HTTP, local process, manual handoff.
- `Adapter`: Star-Control `WorkSpec`을 provider 입력으로 바꾸고 결과를 표준화한다.

## Package 경계

```text
packages/star-provider-api             # provider 공통 인터페이스
packages/star-provider-host            # provider 실행 호스트
packages/star-transport-cli            # CLI transport
packages/star-transport-http           # HTTP transport
packages/star-transport-process        # local process transport
packages/star-adapter-code-agent       # coding-agent adapter
packages/star-adapter-chat-model       # chat/model adapter
packages/star-adapter-openai-compatible# OpenAI-compatible adapter
```

## Star Sentinel 경계

```text
packages/star-sentinel/          # 실제 구현 코드
builtin-tools/star-sentinel/     # manifest, policy, schema, template, corpus
```

Star Sentinel validator는 diagnostic을 만들고, gate는 diagnostic과 policy를 합쳐 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`을 결정한다.

## 실행 결과 구조

```text
대상 프로젝트/.ai-runs/J-0001/
  provider-output/{provider-instance-id}/
  tool-output/star-sentinel/
  events.jsonl
  final-report.md
```
""",
    )
    write(
        "docs/02_구현로드맵.md",
        """# 02. 구현 로드맵

## Phase 0. 스캐폴드와 정본 고정

- monorepo 디렉터리 구조 생성.
- schema, contract, policy, registry 정리.
- Star Sentinel 명칭과 provider-neutral 원칙 확정.

## Phase 1. Core Schema / Registry

- `Job`, `Route`, `WorkSpec`, `Report`, `RunState` schema 구현.
- provider kind, manifest, instance, capability schema 구현.
- builtin provider registry와 capability registry 구현.

## Phase 2. Provider Host MVP

- Fake provider로 run lifecycle을 검증한다.
- Codex CLI는 1차 cloud CLI provider instance로 다룬다.
- local OpenAI-compatible server는 draft/review 보조 provider로 다룬다.

## Phase 3. Star Sentinel P0

- repo map, file classifier, git diff parser.
- scope validator, test weakening detector, secret detector.
- dependency approval validator, AI claim verifier, review pack, approval gate.

## Phase 4. 운영 확장

- adapter conformance test.
- sandbox validation runner.
- observability, budget, cost, recovery.
- control plane UI와 daemon은 후순위로 둔다.
""",
    )
    write(
        "docs/operations/run-artifacts.md",
        """# 실행 결과 저장 규칙

Star-Control은 실행 결과를 repo 루트에 저장하지 않는다. 대상 프로젝트에 `.star-control/` 설정과 `.ai-runs/` 실행 결과를 둔다.

```text
대상 프로젝트/
  .star-control/
    project.yaml
    context.yaml
    approvals/
    rendered/
    cache/

  .ai-runs/
    J-0001/
      job.json
      effective-config.yaml
      route.json
      workspecs/
      provider-output/
      tool-output/
        star-sentinel/
      events.jsonl
      final-report.md
```

`tool-output/star-sentinel/`은 Star Sentinel의 공식 산출 경로다.
""",
    )


def write_provider_docs() -> None:
    write(
        "docs/providers/provider-model.md",
        """# Provider Model

Star-Control provider는 제품명이 아니라 실행 능력과 연결 방식으로 모델링한다.

## 계층

- Provider Manifest: 종류와 기본 실행 형태.
- Provider Instance: 사용자 또는 프로젝트별 구체 설정.
- Transport: CLI, HTTP, process, manual.
- Adapter: WorkSpec과 provider 입출력 변환.
- Capability Profile: router가 provider를 선택할 때 쓰는 능력 선언.
""",
    )
    write(
        "docs/providers/provider-capability.md",
        """# Provider Capability

Router는 provider 이름이 아니라 capability 조건으로 실행자를 선택한다.

```text
needs.file_edit
needs.shell_command
needs.structured_output
needs.private_code
needs.offline
needs.cost_cap
```

Provider manifest는 `can.edit_files`, `can.run_shell`, `can.read_repo`, `can.apply_patch`, `can.return_json`, `can.work_offline` 같은 능력을 선언한다.
""",
    )
    write(
        "docs/providers/provider-registry.md",
        """# Provider Registry

`configs/registries/builtin-provider-registry.yaml`은 builtin provider의 색인이다.

구체 provider 정의는 `builtin-providers/{group}/{provider}/provider.yaml`에 두고, capability profile은 같은 디렉터리의 `capabilities.yaml`에 둔다.
""",
    )
    write(
        "docs/providers/cloud-cli-providers.md",
        """# Cloud CLI Providers

Cloud CLI provider는 파일 수정과 shell 실행 능력을 가질 수 있으므로 sandbox, approval, command policy 검토가 필요하다.

초기 builtin 대상:

- Codex CLI
- Claude Code
- Gemini CLI
""",
    )
    write(
        "docs/providers/local-ai-providers.md",
        """# Local AI Providers

Local AI provider는 offline/private/draft 작업에 유리하지만 직접 파일 수정 권한은 기본적으로 주지 않는다.

초기 builtin 대상:

- local OpenAI-compatible server
- Ollama
- vLLM
- LM Studio
- llama.cpp server
- llama.cpp binary
""",
    )


def write_contracts() -> None:
    contracts = {
        "provider-adapter.md": (
            "Provider Adapter Contract",
            """`ProviderAdapter`는 Star-Control `WorkSpec`을 provider 실행 요청으로 바꾸고, provider 출력을 `ProviderResult`와 `ReportSpec`으로 정규화한다.

```text
prepare(workspec) -> ProviderRunRequest
start(request) -> ProviderRunHandle
poll(handle) -> ProviderRunStatus
collect(handle) -> ProviderRunResult
normalize(result) -> ReportSpec
cancel(handle) -> CancelResult
```
""",
        ),
        "provider-transport.md": (
            "Provider Transport Contract",
            """Transport는 실행 방식만 담당한다. CLI, HTTP, local process, manual handoff transport는 provider policy를 직접 결정하지 않는다.
""",
        ),
        "provider-capability.md": (
            "Provider Capability Contract",
            """Capability profile은 router가 provider를 선택하는 표준 입력이다. provider 이름 기반 분기보다 capability 조건을 우선한다.
""",
        ),
        "tool-adapter.md": (
            "Tool Adapter Contract",
            """Builtin tool은 `tool.yaml` manifest, command, input schema, output schema를 제공한다. Star Sentinel은 `star.sentinel` tool id를 사용한다.
""",
        ),
        "quality-gate.md": (
            "Quality Gate Contract",
            """Quality gate는 diagnostic, policy, validation evidence만 읽고 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`을 결정한다.
""",
        ),
        "diagnostic-model.md": (
            "Diagnostic Model Contract",
            """Diagnostic은 `rule_id`, `severity`, `message`, `location`, `evidence`, `fingerprint`를 중심으로 작성하고 SARIF export가 가능해야 한다.
""",
        ),
        "capability-registry.md": (
            "Capability Registry Contract",
            """Capability registry는 provider, tool, role, route policy가 공유하는 능력 명세의 정본이다.
""",
        ),
    }
    for filename, (title, body) in contracts.items():
        write(f"specs/contracts/{filename}", f"# {title}\n\n{body}")


def schema_defs() -> dict[str, dict]:
    provider_kinds = [
        "cloud_cli_agent",
        "cloud_api_model",
        "local_openai_compatible_server",
        "local_anthropic_compatible_server",
        "local_process_model",
        "remote_self_hosted_model",
        "fake_provider",
        "human_handoff",
    ]
    return {
        "provider-kind.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/provider-kind.schema.json",
            "title": "Star-Control Provider Kind",
            "type": "string",
            "enum": provider_kinds,
        },
        "provider-manifest.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/provider-manifest.schema.json",
            "title": "Star-Control Provider Manifest",
            "type": "object",
            "required": ["id", "name", "kind", "transport", "adapter", "capabilities", "risk", "outputs"],
            "properties": {
                "id": {"type": "string", "pattern": "^provider\\.[a-z0-9][a-z0-9.-]*$"},
                "name": {"type": "string"},
                "kind": {"$ref": "provider-kind.schema.json"},
                "transport": {"enum": ["cli", "http", "process", "manual"]},
                "adapter": {"enum": ["code_agent", "chat_model", "openai_compatible"]},
                "command": {"type": "object"},
                "endpoint": {"type": "object"},
                "capabilities": {"type": "object"},
                "risk": {"type": "object"},
                "outputs": {"type": "object"},
            },
        },
        "provider-instance.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/provider-instance.schema.json",
            "title": "Star-Control Provider Instance",
            "type": "object",
            "required": ["id", "provider", "enabled", "limits", "routing_tags"],
            "properties": {
                "id": {"type": "string"},
                "provider": {"type": "string", "pattern": "^provider\\."},
                "enabled": {"type": "boolean"},
                "command": {"type": "object"},
                "endpoint": {"type": "object"},
                "limits": {"type": "object"},
                "routing_tags": {"type": "array", "items": {"type": "string"}},
            },
        },
        "provider-capability.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/provider-capability.schema.json",
            "title": "Star-Control Provider Capability",
            "type": "object",
            "required": ["provider", "capability_profile"],
            "properties": {
                "provider": {"type": "string"},
                "capability_profile": {"type": "object"},
            },
        },
        "provider-result.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/provider-result.schema.json",
            "title": "Star-Control Provider Result",
            "type": "object",
            "required": ["provider_instance_id", "status", "artifacts"],
            "properties": {
                "provider_instance_id": {"type": "string"},
                "status": {"enum": ["DONE", "FAILED", "BLOCKED", "CANCELLED"]},
                "summary": {"type": "string"},
                "artifacts": {"type": "array", "items": {"type": "string"}},
                "raw_output_path": {"type": "string"},
                "normalized_at": {"type": "string"},
            },
        },
        "model-profile.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/model-profile.schema.json",
            "title": "Star-Control Model Profile",
            "type": "object",
            "required": ["id", "provider_kind", "context_window", "routing_tags"],
            "properties": {
                "id": {"type": "string"},
                "provider_kind": {"$ref": "provider-kind.schema.json"},
                "context_window": {"type": "integer", "minimum": 1},
                "routing_tags": {"type": "array", "items": {"type": "string"}},
            },
        },
        "validation-run.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/validation-run.schema.json",
            "title": "Star-Control Validation Run",
            "type": "object",
            "required": ["id", "command", "status", "started_at"],
            "properties": {
                "id": {"type": "string"},
                "command": {"type": "string"},
                "status": {"enum": ["PASS", "FAIL", "SKIPPED", "ERROR"]},
                "exit_code": {"type": "integer"},
                "started_at": {"type": "string"},
                "finished_at": {"type": "string"},
                "log_path": {"type": "string"},
            },
        },
        "tool-manifest.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/tool-manifest.schema.json",
            "title": "Star-Control Builtin Tool Manifest",
            "type": "object",
            "required": ["id", "name", "kind", "package", "entrypoint", "commands"],
            "properties": {
                "id": {"type": "string"},
                "name": {"type": "string"},
                "kind": {"type": "string"},
                "package": {"type": "string"},
                "entrypoint": {"type": "string"},
                "legacy_aliases": {"type": "array", "items": {"type": "string"}},
                "commands": {"type": "array"},
            },
        },
        "tool-result.schema.json": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://star-control.local/schemas/tool-result.schema.json",
            "title": "Star-Control Tool Result",
            "type": "object",
            "required": ["tool_id", "status", "artifacts"],
            "properties": {
                "tool_id": {"type": "string"},
                "status": {"enum": ["AUTO_PASS", "HUMAN_REVIEW", "BLOCK", "FAILED"]},
                "artifacts": {"type": "array", "items": {"type": "string"}},
                "diagnostics": {"type": "array"},
            },
        },
    }


def write_schemas() -> None:
    for src_name in [
        "job.schema.json",
        "route.schema.json",
        "workspec.schema.json",
        "report.schema.json",
        "run-state.schema.json",
        "event.schema.json",
        "approval.schema.json",
        "capability.schema.json",
        "policy.schema.json",
        "hook.schema.json",
    ]:
        src = V3 / "schemas" / src_name
        if src.exists():
            copy_binary_or_text(src, f"specs/schemas/{src_name}", transform=False)
    copy_binary_or_text(V4 / "schemas" / "diagnostic.schema.json", "specs/schemas/diagnostic.schema.json", transform=True)
    copy_binary_or_text(V4 / "schemas" / "policy.schema.json", "builtin-tools/star-sentinel/schemas/star-sentinel-policy.schema.json", transform=True)
    for name, data in schema_defs().items():
        dump_json(f"specs/schemas/{name}", data)
    copy_binary_or_text(V4 / "schemas" / OLD_TASK_SCHEMA, "builtin-tools/star-sentinel/schemas/sentinel-task.schema.json", transform=True)
    copy_binary_or_text(V4 / "schemas" / "review_pack.schema.json", "builtin-tools/star-sentinel/schemas/review-pack.schema.json", transform=True)
    copy_binary_or_text(V4 / "schemas" / "corpus_case.schema.json", "builtin-tools/star-sentinel/schemas/corpus-case.schema.json", transform=True)


def write_configs() -> None:
    write(
        "configs/defaults/star-control.yaml",
        """schema_version: 0.1.0
run_artifact_root: ".ai-runs"
project_config_dir: ".star-control"
default_quality_tool: "star.sentinel"
provider_selection:
  strategy: capability_first
  fallback_provider: provider.fake
""",
    )
    write(
        "configs/defaults/router.yaml",
        """schema_version: 0.1.0
route_policy:
  default_stages:
    - route
    - plan
    - implement
    - validate
    - review
    - report
  provider_selection:
    use_capabilities: true
    require_policy_check: true
""",
    )
    for src in sorted((V3 / "policies").glob("*.yaml")):
        copy_binary_or_text(src, f"configs/policies/{src.name}", transform=True)
    write(
        "configs/policies/tool-policy.yaml",
        """schema_version: 0.1.0
tools:
  star.sentinel:
    required_for:
      - validate
      - review
    default_profile: quick
""",
    )
    write(
        "configs/policies/provider-policy.yaml",
        """schema_version: 0.1.0
provider_policy:
  select_by_capability: true
  deny_core_product_named_packages: true
  require_sandbox_for:
    - cli
    - process
""",
    )
    copy_binary_or_text(V3 / "capabilities" / "capability-registry.yaml", "configs/registries/capability-registry.yaml", transform=True)
    write(
        "configs/registries/builtin-tool-registry.yaml",
        """schema_version: 0.1.0
tools:
  - id: star.sentinel
    manifest: builtin-tools/star-sentinel/tool.yaml
    package: packages/star-sentinel
""",
    )
    provider_entries = "\n".join(
        f"  - id: {p['id']}\n    manifest: builtin-providers/{p['group']}/{p['slug']}/provider.yaml\n    capabilities: builtin-providers/{p['group']}/{p['slug']}/capabilities.yaml"
        for p in PROVIDERS
    )
    write("configs/registries/builtin-provider-registry.yaml", f"schema_version: 0.1.0\nproviders:\n{provider_entries}\n")
    write(
        "configs/registries/model-registry.yaml",
        """schema_version: 0.1.0
models:
  - id: example-cloud-coding-agent
    provider_kind: cloud_cli_agent
    routing_tags: [cloud, coding-agent]
  - id: example-local-draft-model
    provider_kind: local_openai_compatible_server
    routing_tags: [local, private, draft]
""",
    )
    for src in sorted((V3 / "roles").glob("*.md")):
        copy_binary_or_text(src, f"configs/roles/{src.name}", transform=True)
    for src in sorted((V3 / "skills").glob("*.md")):
        copy_binary_or_text(src, f"configs/skills/{src.name}", transform=True)
    for src in sorted((V3 / "hooks").glob("*.yaml")):
        copy_binary_or_text(src, f"configs/hooks/{src.name}", transform=True)
    for src in sorted((V3 / "templates").glob("*")):
        if src.is_file():
            copy_binary_or_text(src, f"configs/templates/{src.name}", transform=True)
    write(
        "configs/skills/plan-ledger.md",
        """# plan-ledger

## 목적

`PLANS.md` 또는 RunState를 생성, 갱신, 압축, 인계한다.

`PLANS.md`는 append-only 로그가 아니라 현재 작업 상태 snapshot이다. 현재 판단에 필요한 목표, 활성 작업, 열린 리스크, 다음 명령, Context Pack만 남긴다.

## 운영 원칙

- 완료 작업은 1행 요약으로 압축한다.
- 상세 로그, 전체 diff, 반복 검증 출력은 남기지 않는다.
- 상세 근거는 `docs/decisions/*`, `reports/*`, changelog, commit history를 참조한다.
- 새 작업 시작 전, 작업 완료 후, handoff 전, 파일이 약 120줄을 넘을 때 압축한다.

## 갱신 대상

- 현재 목표
- 활성 작업 상태
- 열린 이슈와 리스크
- 다음 실행 명령
- 다음 스레드 인계 내용
- 상세 근거 문서 링크

## 금지 대상

- 전체 diff
- 긴 로그
- 완료 작업별 상세 파일 목록
- 반복 검증 출력
- 현재 판단과 무관한 과거 진행 과정
""",
    )
    write(
        "configs/templates/plans-template.md",
        """# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그 저장소가 아니며, 완료된 세부 과정은 `docs/decisions/*`, `reports/*`, changelog, commit history로 보낸다.

## Context Pack

### 현재 목표

-

### 반드시 지켜야 할 제약

-

### 이미 끝난 것

-

### 아직 남은 것

-

### 건드리면 안 되는 것

-

### 먼저 확인할 파일

-

### 먼저 실행할 명령

-

### 현재 차단 요소

-

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|

## 다음 실행 명령

```bash
# 필요한 명령만 남긴다.
```

## Archive References

| 항목 | 위치 |
|---|---|

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
""",
    )
    for src in sorted((V3 / "renderers").rglob("*.yaml")):
        rel = src.relative_to(V3 / "renderers")
        copy_binary_or_text(src, f"configs/renderers/{rel}", transform=True)
    write(
        "configs/provider-instances/README.md",
        """# Provider Instance Examples

이 디렉터리는 사용자별 실제 provider 설정 예시를 담는다. API key, token, password는 여기에 평문 저장하지 않는다.
""",
    )
    provider_instance_examples = {
        "codex-cli.example.yaml": ("my-codex-cli", "provider.codex-cli", {"executable": "codex"}, ["cloud", "coding-agent", "file-edit"]),
        "claude-code.example.yaml": ("my-claude-code", "provider.claude-code", {"executable": "claude"}, ["cloud", "coding-agent"]),
        "gemini-cli.example.yaml": ("my-gemini-cli", "provider.gemini-cli", {"executable": "gemini"}, ["cloud", "coding-agent"]),
        "openai-api.example.yaml": ("my-openai-api", "provider.openai", {"base_url": "https://api.openai.com/v1", "model": "gpt-example"}, ["cloud", "api"]),
        "anthropic-api.example.yaml": ("my-anthropic-api", "provider.anthropic", {"base_url": "https://api.anthropic.com", "model": "claude-example"}, ["cloud", "api"]),
        "google-gemini-api.example.yaml": ("my-google-gemini-api", "provider.google-gemini", {"base_url": "https://generativelanguage.googleapis.com", "model": "gemini-example"}, ["cloud", "api"]),
        "local-openai-compatible.example.yaml": ("my-local-openai-compatible", "provider.local-openai-compatible", {"base_url": "http://127.0.0.1:8000/v1", "model": "local-coder"}, ["local", "private"]),
        "local-process.example.yaml": ("my-local-process", "provider.local-process", {"executable": "runner", "model_path": "model.gguf"}, ["local", "process"]),
        "fake-provider.example.yaml": ("my-fake-provider", "provider.fake", {}, ["test", "offline"]),
    }
    for filename, (id_, provider, connection, tags) in provider_instance_examples.items():
        data = {
            "id": id_,
            "provider": provider,
            "enabled": True,
            "limits": {"timeout_seconds": 300, "max_parallel_jobs": 1},
            "routing_tags": tags,
        }
        if "base_url" in connection:
            data["endpoint"] = connection
        elif connection:
            data["command"] = connection
        write(f"configs/provider-instances/{filename}", yaml_mapping(data))


def write_providers() -> None:
    for provider in PROVIDERS:
        base = f"builtin-providers/{provider['group']}/{provider['slug']}"
        write(f"{base}/provider.yaml", provider_manifest(provider))
        write(f"{base}/capabilities.yaml", provider_capabilities(provider))
        touch(f"{base}/templates/.gitkeep")
        if provider["group"] == "cloud-cli":
            touch(f"{base}/parsers/.gitkeep")
        write(
            f"{base}/docs/README.md",
            f"""# {provider['name']}

Builtin provider manifest for `{provider['id']}`.

이 provider는 core package가 아니라 manifest/capability로 등록된다.
""",
        )
    for src in sorted((V3 / "providers").glob("*.yaml")):
        group, slug = provider_slug_from_v3(src.name)
        copy_binary_or_text(
            src,
            f"builtin-providers/{group}/{slug}/docs/v3-provider-source.yaml",
            transform=True,
        )
    for src in sorted((V3 / "provider-features").glob("*.features.yaml")):
        group, slug = provider_slug_from_v3(src.name.replace(".features", ""))
        copy_binary_or_text(
            src,
            f"builtin-providers/{group}/{slug}/docs/v3-features-source.yaml",
            transform=True,
        )


def write_star_sentinel() -> None:
    legacy_alias = LEGACY_DASH
    write(
        "builtin-tools/star-sentinel/tool.yaml",
        f"""id: star.sentinel
name: Star Sentinel
kind: builtin
package: star-sentinel
entrypoint: star_sentinel.main

legacy_aliases:
  - {legacy_alias}

description: >
  AI가 생성한 코드 변경사항을 diff, policy, evidence, validation 기준으로 검증하고
  review pack과 approval gate를 생성하는 Star-Control 기본 탑재 검증 도구.

commands:
  - name: check
    description: AI 작업 결과를 diff, log, policy 기준으로 검증한다.
  - name: review-pack
    description: 사람이 읽을 review pack을 생성한다.
  - name: gate
    description: AUTO_PASS / HUMAN_REVIEW / BLOCK 판정을 생성한다.
  - name: selfcheck
    description: Star Sentinel의 정책, 검증기, corpus, CI 변경을 자기검증한다.

profiles:
  - quick
  - near
  - full
  - security
  - release
  - validator

outputs:
  - repo_map.json
  - changed_lines.json
  - diagnostics.json
  - validation_runs.json
  - review_pack.md
  - approval.json
  - ledger.jsonl
""",
    )
    write(
        "builtin-tools/star-sentinel/README.md",
        """# Star Sentinel

Star Sentinel은 Star-Control에 기본 탑재되는 AI 코드 변경 검증 도구다.

## 책임

- AI가 만든 변경사항을 diff, policy, evidence, validation 기준으로 검증한다.
- 테스트 삭제/약화, secret, dependency 변경, scope 위반, validator self-bypass를 탐지한다.
- review pack과 approval gate를 생성한다.

## 경계

- 구현 코드는 `packages/star-sentinel/`에 둔다.
- manifest, policy, schema, template, corpus는 `builtin-tools/star-sentinel/`에 둔다.
""",
    )
    copy_binary_or_text(V4 / "templates" / "autocode-policy.example.yaml", "builtin-tools/star-sentinel/policies/star-sentinel-policy.default.yaml", transform=True)
    write(
        "builtin-tools/star-sentinel/policies/risk-paths.default.yaml",
        """risk_paths:
  critical:
    - "**/.github/**"
    - "**/ci/**"
    - "**/security/**"
    - "**/auth/**"
    - "**/policies/**"
    - "**/validators/**"
    - "**/secrets/**"
  high:
    - "**/package.json"
    - "**/Cargo.toml"
    - "**/pyproject.toml"
    - "**/migrations/**"
""",
    )
    write(
        "builtin-tools/star-sentinel/policies/test-protection.default.yaml",
        """test_protection:
  block:
    - test_file_deleted
    - assertion_removed
    - skip_only_added
    - validator_rule_deleted
  human_review:
    - snapshot_updated
    - fixture_changed
    - dependency_manifest_changed
""",
    )
    copy_binary_or_text(V4 / "templates" / "task-spec.example.yaml", "builtin-tools/star-sentinel/templates/sentinel-task.example.yaml", transform=True)
    copy_binary_or_text(V4 / "templates" / "review-pack.example.md", "builtin-tools/star-sentinel/templates/review-pack.example.md", transform=True)
    copy_binary_or_text(V4 / "templates" / "rejection-prompt.example.md", "builtin-tools/star-sentinel/templates/rejection-prompt.example.md", transform=True)
    copy_binary_or_text(V4 / "templates" / "corpus-case.example.yaml", "builtin-tools/star-sentinel/templates/corpus-case.example.yaml", transform=True)
    for corpus_dir in ["positive", "negative", "regression"]:
        touch(f"builtin-tools/star-sentinel/corpus/{corpus_dir}/.gitkeep")
    star_docs = {
        "00_개요.md": """# Star Sentinel 개요

Star Sentinel은 AI가 구현한 변경을 자동으로 심사하고, 사람이 최종 승인할 수 있는 review pack과 approval gate를 만든다.
""",
        "01_검증프로파일.md": """# Star Sentinel 검증 프로파일

- `quick`: diff, scope, secret, test weakening, AI claim, review pack.
- `near`: quick + near tests + architecture-lite + dependency diff.
- `full`: near + P1/P2 validator.
- `security`: secret + security-lite + dependency/license + auth risk.
- `release`: full + SBOM-lite + artifact hash + release manifest.
- `validator`: validator self-test + corpus + golden diagnostics.
""",
        "02_진단모델.md": """# Star Sentinel 진단 모델

진단은 `rule_id`, `severity`, `confidence`, `location`, `evidence`, `remediation`, `fingerprint`를 포함한다.
""",
        "03_리뷰팩.md": """# Star Sentinel 리뷰팩

리뷰팩은 작업 요약, diff 요약, 자동 검증 요약, 위험 요약, 미검증 항목, 사람 확인 질문, gate 판정을 담는다.
""",
    }
    for filename, body in star_docs.items():
        write(f"builtin-tools/star-sentinel/docs/{filename}", body)
    for src in sorted(V4.glob("*.md")):
        target_name = src.name
        copy_binary_or_text(
            src,
            f"builtin-tools/star-sentinel/docs/curated/{target_name}",
            transform=True,
            header=source_header(f"{V4.name}/{src.name}", "Star Sentinel 정규 명칭으로 변환해 흡수한 상세 설계 문서."),
        )
    for src in sorted((V4 / "checklists").glob("*.md")):
        copy_binary_or_text(
            src,
            f"builtin-tools/star-sentinel/docs/checklists/{src.name}",
            transform=True,
            header=source_header(f"{V4.name}/checklists/{src.name}", "Star Sentinel 구현 체크리스트로 흡수."),
        )
    copy_binary_or_text(V4 / "references.md", "builtin-tools/star-sentinel/docs/references.md", transform=True)
    copy_binary_or_text(V4 / "feature_inventory_curated.csv", "builtin-tools/star-sentinel/docs/feature-inventory-curated.csv", transform=True)
    copy_binary_or_text(V4 / "zip_manifest.json", "builtin-tools/star-sentinel/docs/source-zip-manifest.json", transform=True)


def classify_v3_doc_target(src: Path) -> str:
    rel = src.relative_to(V3)
    if rel.parts[0] == "docs":
        name = rel.name
        provider_prefixes = ("03_", "08_", "09_", "11_", "13_", "14_", "19_", "21_", "28_", "30_", "31_", "36_")
        operation_prefixes = ("06_", "12_", "15_", "16_", "18_", "20_", "24_", "25_", "26_", "27_", "29_", "32_", "34_", "35_")
        decision_prefixes = ("10_", "23_", "99_")
        if name.startswith(provider_prefixes):
            return f"docs/providers/v3/{name}"
        if name.startswith(operation_prefixes):
            return f"docs/operations/v3/{name}"
        if name.startswith(decision_prefixes):
            return f"docs/decisions/v3/{name}"
        return f"docs/architecture/v3/{name}"
    if rel.parts[0] == "operations":
        return f"docs/operations/{rel.name}"
    if rel.name == "README.md":
        return "docs/architecture/v3/README.md"
    return f"docs/architecture/v3/{rel.as_posix()}"


def write_v3_docs() -> None:
    copy_binary_or_text(V3 / "README.md", "docs/architecture/v3/README.md", transform=True, header=source_header(f"{V3.name}/README.md", "v3 설계 패키지 개요를 정규 문서로 흡수."))
    for src in sorted((V3 / "docs").glob("*.md")):
        target = classify_v3_doc_target(src)
        copy_binary_or_text(src, target, transform=True, header=source_header(f"{V3.name}/docs/{src.name}", "v3 상세 설계를 Star-Control 정규 문서 트리로 흡수."))
    for src in sorted((V3 / "operations").glob("*.md")):
        copy_binary_or_text(src, f"docs/operations/{src.name}", transform=True, header=source_header(f"{V3.name}/operations/{src.name}", "운영 runbook으로 흡수."))
    copy_binary_or_text(
        V3 / "provider-features" / "CHANGELOG.md",
        "docs/providers/v3/provider-feature-matrix-changelog.md",
        transform=True,
        header=source_header(f"{V3.name}/provider-features/CHANGELOG.md", "Provider feature matrix 변경 로그로 흡수."),
    )


def write_apps_packages_examples_tests() -> None:
    apps = {
        "starctl": "Star-Control CLI entrypoint scaffold.",
        "star-daemon": "Long-running local daemon scaffold. 실제 구현은 후순위.",
        "star-control-ui": "Control plane UI scaffold. 실제 구현은 후순위.",
    }
    for app, desc in apps.items():
        write(f"apps/{app}/README.md", f"# {app}\n\n{desc}\n")
        touch(f"apps/{app}/src/.gitkeep")
    packages = [
        "star-core",
        "star-config",
        "star-state",
        "star-policy",
        "star-capability",
        "star-router",
        "star-provider-api",
        "star-provider-host",
        "star-transport-cli",
        "star-transport-http",
        "star-transport-process",
        "star-adapter-code-agent",
        "star-adapter-chat-model",
        "star-adapter-openai-compatible",
        "star-renderer",
        "star-hooks",
        "star-vcs",
        "star-context",
        "star-observability",
        "star-tool-api",
        "star-tool-host",
        "star-quality",
        "star-sentinel",
    ]
    descriptions = {
        "star-provider-api": "Provider 공통 인터페이스.",
        "star-provider-host": "Provider 실행 호스트.",
        "star-transport-cli": "CLI transport.",
        "star-transport-http": "HTTP/API transport.",
        "star-transport-process": "Local process transport.",
        "star-adapter-code-agent": "Coding-agent provider adapter.",
        "star-adapter-chat-model": "Chat/completion model adapter.",
        "star-adapter-openai-compatible": "OpenAI-compatible endpoint adapter.",
        "star-sentinel": "Star Sentinel 구현 코드 package.",
    }
    for package in packages:
        write(f"packages/{package}/README.md", f"# {package}\n\n{descriptions.get(package, 'Star-Control package scaffold.')}\n")
        touch(f"packages/{package}/src/.gitkeep")
    for src in sorted((V3 / "quality").rglob(".gitkeep")):
        rel = src.parent.relative_to(V3 / "quality")
        touch(f"packages/star-quality/src/{rel}/.gitkeep")
    for src in sorted((V3 / "retrieval").rglob(".gitkeep")):
        rel = src.parent.relative_to(V3 / "retrieval")
        touch(f"packages/star-context/src/{rel}/.gitkeep")
    for src in sorted((V3 / "vcs").rglob(".gitkeep")):
        rel = src.parent.relative_to(V3 / "vcs")
        touch(f"packages/star-vcs/src/{rel}/.gitkeep")
    for src in sorted((V3 / "tools").rglob(".gitkeep")):
        rel = src.parent.relative_to(V3 / "tools")
        touch(f"packages/star-tool-host/src/{rel}/.gitkeep")
    for src in sorted((V3 / "control-plane").rglob(".gitkeep")):
        rel = src.parent.relative_to(V3 / "control-plane")
        touch(f"apps/star-control-ui/src/{rel}/.gitkeep")
    for sample in ["rust-sample", "python-sample", "node-sample"]:
        touch(f"examples/projects/{sample}/.gitkeep")
    for src in sorted((V3 / "examples").rglob("*")):
        if src.is_file():
            rel = src.relative_to(V3 / "examples")
            copy_binary_or_text(src, f"examples/{rel}", transform=True)
    touch("examples/runs/.gitkeep")
    for src in sorted((V3 / "codex-output").rglob("*")):
        if src.is_file():
            rel = src.relative_to(V3 / "codex-output")
            copy_binary_or_text(src, f"examples/rendered-provider-artifacts/{rel}", transform=True)
    for name in [
        "codex-cli.personal.example.yaml",
        "local-vllm.dgxspark.example.yaml",
        "lm-studio.desktop.example.yaml",
        "llama-cpp.gpu.example.yaml",
    ]:
        write(
            f"examples/provider-instances/{name}",
            """id: example-instance
provider: provider.local-openai-compatible
enabled: false
limits:
  timeout_seconds: 300
  max_parallel_jobs: 1
routing_tags:
  - example
""",
        )
    for path in [
        "tests/unit/.gitkeep",
        "tests/integration/.gitkeep",
        "tests/fixtures/.gitkeep",
        "tests/conformance/providers/.gitkeep",
        "tests/conformance/tools/.gitkeep",
        "tests/conformance/transports/.gitkeep",
        "tests/conformance/adapters/.gitkeep",
        "integrations/github/workflows/.gitkeep",
        "integrations/github/rulesets/.gitkeep",
    ]:
        touch(path)
    write(
        "scripts/dev.ps1",
        """Write-Host "Star-Control scaffold only. No dev server is defined yet."
""",
    )
    write(
        "scripts/test.ps1",
        """$ErrorActionPreference = "Stop"
Get-ChildItem -Recurse -Filter *.json | ForEach-Object {
  Get-Content -Raw $_.FullName | ConvertFrom-Json | Out-Null
}
Write-Host "JSON files parsed successfully."
""",
    )
    write(
        "scripts/build.ps1",
        """Write-Host "No build target yet. This repository currently contains scaffold, schemas, and documentation."
""",
    )
    write(
        "scripts/package.ps1",
        """Write-Host "No package target yet. Package manager selection is intentionally deferred."
""",
    )


def source_files(root: Path) -> list[Path]:
    return sorted([p for p in root.rglob("*") if p.is_file()], key=lambda p: p.as_posix().lower())


def map_v3_source(rel: Path) -> tuple[str, str, str]:
    rel_posix = rel.as_posix()
    first = rel.parts[0]
    if rel_posix == "README.md":
        return "docs/architecture/v3/README.md", "흡수됨", "v3 설계 패키지 개요"
    if first == "docs":
        return classify_v3_doc_target(V3 / rel), "흡수됨", "v3 상세 문서"
    if first == "operations":
        return f"docs/operations/{rel.name}", "흡수됨", "운영 runbook"
    if first == "schemas":
        if rel.name == "provider.schema.json":
            return "specs/schemas/provider-*.schema.json", "분리 흡수", "provider schema를 manifest/instance/capability/result로 분리"
        return f"specs/schemas/{rel.name}", "흡수됨", "공통 schema"
    if first == "policies":
        return f"configs/policies/{rel.name}", "흡수됨", "정책 설정"
    if first == "roles":
        return f"configs/roles/{rel.name}", "흡수됨", "역할 지침"
    if first == "skills":
        if rel.name == "plan-ledger.md":
            return "configs/skills/plan-ledger.md", "정책 갱신 흡수", "bounded snapshot 운영 원칙으로 갱신"
        return f"configs/skills/{rel.name}", "흡수됨", "provider-neutral skill 원본"
    if first == "hooks":
        return f"configs/hooks/{rel.name}", "흡수됨", "lifecycle hook"
    if first == "templates":
        if rel.name == "plans-template.md":
            return "configs/templates/plans-template.md", "정책 갱신 흡수", "bounded snapshot template으로 갱신"
        return f"configs/templates/{rel.name}", "흡수됨", "공통 template"
    if first == "capabilities":
        return "configs/registries/capability-registry.yaml", "흡수됨", "capability registry"
    if first == "renderers":
        return f"configs/renderers/{Path(*rel.parts[1:]).as_posix()}", "흡수됨", "provider renderer 설정"
    if first == "providers":
        group, slug = provider_slug_from_v3(rel.name)
        return f"builtin-providers/{group}/{slug}/provider.yaml", "정규화 흡수", "제품별 provider 초안을 builtin manifest로 흡수하고 docs/v3-provider-source.yaml에 원본 세부 보존"
    if first == "provider-features":
        if rel.name == "CHANGELOG.md":
            return "docs/providers/v3/provider-feature-matrix-changelog.md", "흡수됨", "provider feature matrix 변경 로그"
        group, slug = provider_slug_from_v3(rel.name.replace(".features", ""))
        return f"builtin-providers/{group}/{slug}/capabilities.yaml", "정규화 흡수", "provider feature matrix를 capability profile로 흡수하고 docs/v3-features-source.yaml에 원본 세부 보존"
    if first == "examples":
        return f"examples/{Path(*rel.parts[1:]).as_posix()}", "흡수됨", "예시 artifact"
    if first == "codex-output":
        return f"examples/rendered-provider-artifacts/{Path(*rel.parts[1:]).as_posix()}", "흡수됨", "렌더링 산출 예시"
    if first == "quality":
        return f"packages/star-quality/src/{Path(*rel.parts[1:-1]).as_posix()}/.gitkeep", "스캐폴드 흡수", "quality package 경계"
    if first == "retrieval":
        return f"packages/star-context/src/{Path(*rel.parts[1:-1]).as_posix()}/.gitkeep", "스캐폴드 흡수", "context/retrieval package 경계"
    if first == "vcs":
        return f"packages/star-vcs/src/{Path(*rel.parts[1:-1]).as_posix()}/.gitkeep", "스캐폴드 흡수", "vcs package 경계"
    if first == "tools":
        return f"packages/star-tool-host/src/{Path(*rel.parts[1:-1]).as_posix()}/.gitkeep", "스캐폴드 흡수", "tool host package 경계"
    if first == "control-plane":
        return f"apps/star-control-ui/src/{Path(*rel.parts[1:-1]).as_posix()}/.gitkeep", "스캐폴드 흡수", "control plane UI 경계"
    if first == "runs":
        return f"examples/runs/{Path(*rel.parts[1:]).as_posix()}", "스캐폴드 흡수", "run artifact 예시"
    return f"docs/decisions/source-absorption-map.md", "기록됨", "별도 파일 없는 빈 디렉터리 또는 보류 항목"


def map_v4_source(rel: Path) -> tuple[str, str, str]:
    first = rel.parts[0]
    if rel.suffix == ".md" and first != "templates" and first != "checklists":
        if rel.name == "references.md":
            return "builtin-tools/star-sentinel/docs/references.md", "흡수됨", "Star Sentinel 참고자료"
        return f"builtin-tools/star-sentinel/docs/curated/{rel.name}", "흡수됨", "Star Sentinel 상세 설계"
    if first == "checklists":
        return f"builtin-tools/star-sentinel/docs/checklists/{rel.name}", "흡수됨", "Star Sentinel checklist"
    if first == "schemas":
        mapping = {
            OLD_TASK_SCHEMA: "builtin-tools/star-sentinel/schemas/sentinel-task.schema.json",
            "review_pack.schema.json": "builtin-tools/star-sentinel/schemas/review-pack.schema.json",
            "corpus_case.schema.json": "builtin-tools/star-sentinel/schemas/corpus-case.schema.json",
            "diagnostic.schema.json": "specs/schemas/diagnostic.schema.json",
            "policy.schema.json": "builtin-tools/star-sentinel/schemas/star-sentinel-policy.schema.json",
        }
        return mapping.get(rel.name, f"builtin-tools/star-sentinel/schemas/{rel.name}"), "정규화 흡수", "Star Sentinel schema"
    if first == "templates":
        mapping = {
            "autocode-policy.example.yaml": "builtin-tools/star-sentinel/policies/star-sentinel-policy.default.yaml",
            "task-spec.example.yaml": "builtin-tools/star-sentinel/templates/sentinel-task.example.yaml",
            "review-pack.example.md": "builtin-tools/star-sentinel/templates/review-pack.example.md",
            "rejection-prompt.example.md": "builtin-tools/star-sentinel/templates/rejection-prompt.example.md",
            "corpus-case.example.yaml": "builtin-tools/star-sentinel/templates/corpus-case.example.yaml",
        }
        return mapping.get(rel.name, f"builtin-tools/star-sentinel/templates/{rel.name}"), "정규화 흡수", "Star Sentinel template/policy"
    if rel.name == "feature_inventory_curated.csv":
        return "builtin-tools/star-sentinel/docs/feature-inventory-curated.csv", "흡수됨", "75개 기능 인벤토리"
    if rel.name == "zip_manifest.json":
        return "builtin-tools/star-sentinel/docs/source-zip-manifest.json", "흡수됨", "원본 zip manifest"
    return "docs/decisions/source-absorption-map.md", "기록됨", "흡수 맵에서 추적"


def write_source_absorption_map() -> None:
    rows: list[tuple[str, str, str, str]] = []
    for src in source_files(V3):
        rel = src.relative_to(V3)
        target, status, note = map_v3_source(rel)
        rows.append((f"{V3.name}/{rel.as_posix()}", target, status, note))
    for src in source_files(V4):
        rel = src.relative_to(V4)
        target, status, note = map_v4_source(rel)
        rows.append((f"{V4.name}/{rel.as_posix()}", target, status, note))
    lines = [
        "# Source Absorption Map",
        "",
        f"- v3 source files: {len(source_files(V3))}",
        f"- v4 curated source files: {len(source_files(V4))}",
        f"- total source files: {len(rows)}",
        "- 원본 폴더는 삭제하지 않았다.",
        "- 이 표는 원본 폴더 삭제 승인 전 확인할 흡수 근거다.",
        "",
        "| 원본 | 흡수 대상 | 상태 | 비고 |",
        "|---|---|---|---|",
    ]
    for source, target, status, note in rows:
        lines.append(f"| `{source}` | `{target}` | {status} | {note} |")
    write("docs/decisions/source-absorption-map.md", "\n".join(lines))


def main() -> None:
    if not V3.exists():
        raise SystemExit(f"missing v3 source: {V3}")
    if not V4.exists():
        raise SystemExit(f"missing v4 source: {V4}")
    write_root_files()
    write_canonical_docs()
    write_provider_docs()
    write_contracts()
    write_schemas()
    write_configs()
    write_providers()
    write_star_sentinel()
    write_v3_docs()
    write_apps_packages_examples_tests()
    write_source_absorption_map()


if __name__ == "__main__":
    main()
