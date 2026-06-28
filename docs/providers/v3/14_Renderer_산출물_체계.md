> 흡수 출처: `star-control_design_v3/docs/14_Renderer_산출물_체계.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 14. Provider Renderer / 산출물 체계

## 1. 목적

Star-Control의 원본 규격은 provider-neutral하다.

하지만 실제 Provider는 각자 다른 파일 구조를 요구한다.

예:

```text
Codex     → .codex/*.config.toml, .agents/skills/*/SKILL.md, .codex/rules/*.rules
Claude    → CLAUDE.md, .claude/skills, .claude/agents, settings/permissions/hooks
Gemini    → GEMINI.md, extensions, commands, hooks, subagents
Cursor    → rules, skills, CLI/headless settings
GitHub    → .github/copilot-instructions.md, .github/instructions, Agent Skills
```

Renderer는 Star-Control 원본을 Provider별 산출물로 변환한다.

---

## 2. 원본과 산출물

```text
Star-Control 원본
  roles/*.md
  skills/*.md
  policies/*.yaml
  hooks/*.yaml
  providers/*.yaml
  capabilities/*.yaml

↓ renderer

Provider 산출물
  Codex config/skills/rules/hooks
  Claude settings/skills/hooks/agents
  Gemini extensions/commands/hooks
  Cursor rules/skills
  GitHub instructions/skills
```

---

## 3. Renderer 원칙

1. 원본은 하나만 유지한다.
2. provider 산출물은 언제든 재생성 가능해야 한다.
3. 산출물을 직접 수정한 내용은 원본에 반영하지 않으면 사라질 수 있다.
4. renderer는 provider 기능 버전과 feature matrix를 확인해야 한다.
5. 산출물 생성 전 dry-run diff를 제공해야 한다.

---

## 4. Codex Renderer

### 4.1 Profile Renderer

입력:

```text
roles/router-low.md
providers/codex.yaml
policies/approval-policy.yaml
policies/sandbox-policy.yaml
```

출력:

```text
%USERPROFILE%\.codex\low-router.config.toml
%USERPROFILE%\.codex\worker-impl.config.toml
%USERPROFILE%\.codex\worker-review.config.toml
```

예시:

```toml
model = "gpt-5.5"
model_reasoning_effort = "low"
approval_policy = "never"
sandbox_mode = "read-only"

developer_instructions = """
Star-Control router-low 역할로 실행한다.
직접 구현하지 않는다.
route.schema.json에 맞는 결과를 생성한다.
"""
```

### 4.2 Skill Renderer

입력:

```text
skills/code-review.md
skills/validation.md
skills/plan-ledger.md
```

출력:

```text
%USERPROFILE%\.agents\skills\code-review\SKILL.md
%USERPROFILE%\.agents\skills\validation\SKILL.md
%USERPROFILE%\.agents\skills\plan-ledger\SKILL.md
```

필수 변환:

```yaml
---
name: code-review
description: Use when reviewing code changes...
---
```

### 4.3 Rules Renderer

입력:

```text
policies/command-policy.yaml
```

출력:

```text
%USERPROFILE%\.codex\rules\command_block.rules
```

공통 policy의 `forbidden`과 `prompt`를 Codex `prefix_rule()`로 변환한다.

---

## 5. Claude Renderer

출력 후보:

```text
CLAUDE.md
.claude/settings.json
.claude/skills/*/SKILL.md
.claude/agents/*.md
.claude/plugins/*
```

변환 규칙:

- `contexts/project.md` → `CLAUDE.md`
- `skills/*.md` → `.claude/skills/*/SKILL.md`
- `roles/worker-review.md` → `.claude/agents/reviewer.md`
- `policies/permission-policy.yaml` → Claude permissions config
- `hooks/*.yaml` → Claude hook config

---

## 6. Gemini Renderer

출력 후보:

```text
GEMINI.md
.gemini/extensions/star-control/
commands/
hooks/
subagents/
skills/
```

변환 규칙:

- Star-Control extension pack을 Gemini extension으로 렌더링한다.
- MCP 설정은 provider feature matrix와 tools/mcp 설정을 합쳐 생성한다.
- Plan Mode는 native 지원 여부에 따라 Gemini plan customization으로 연결한다.

---

## 7. Cursor Renderer

출력 후보:

```text
.cursor/rules
.cursor/skills
cursor headless run config
```

변환 규칙:

- `scope-policy.yaml` → Cursor rules
- `skills/*.md` → Cursor-compatible skill pack
- `Plan Engine` → Cursor Plan Mode adapter
- `Background Run Engine` → Cursor background agent adapter

---

## 8. GitHub Renderer

출력 후보:

```text
.github/copilot-instructions.md
.github/instructions/*.instructions.md
.github/skills/*/SKILL.md
.github/prompts/*.prompt.md
```

변환 규칙:

- project context → copilot instructions
- scoped context → path-specific instructions
- skills → Agent Skills
- code-review policy → Copilot code review custom instructions

---

## 9. Local Renderer

출력 후보:

```text
local prompts
ollama prompt templates
lmstudio request payloads
vLLM OpenAI-compatible payloads
```

로컬 renderer는 직접 수정 권한을 주지 않는 초안용 prompt를 기본으로 생성한다.

---

## 10. Renderer CLI

권장 명령:

```text
star-control render codex --dry-run
star-control render codex --apply
star-control render claude --dry-run
star-control render all --dry-run
```

출력:

```text
created:
  - %USERPROFILE%\.codex\low-router.config.toml
updated:
  - %USERPROFILE%\.agents\skills\validation\SKILL.md
unchanged:
  - %USERPROFILE%\.codex\rules\command_block.rules
```

---

## 11. Acceptance Criteria

- Codex 산출물을 재생성할 수 있다.
- renderer dry-run이 diff를 보여준다.
- provider feature matrix의 unsupported 기능은 렌더링하지 않는다.
- 산출물이 원본보다 권한을 넓히지 않는다.
