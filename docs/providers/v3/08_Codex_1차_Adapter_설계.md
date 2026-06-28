> 흡수 출처: `star-control_design_v3/docs/08_Codex_1차_Adapter_설계.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 08. Codex 1차 Adapter 설계

Codex는 Star-Control의 1차 지원 provider다. 하지만 Codex 설정은 Star-Control 원본이 아니라 adapter 산출물이다.

## 1. Codex에서 활용할 기능

- `--profile` 기반 profile config
- `codex exec` 비대화 실행
- `--json` events
- `--output-last-message`
- `--output-schema`
- `codex exec resume`
- `codex fork`
- Skills
- Hooks
- Rules
- Goals
- Subagents
- MCP
- Plugins

공식 CLI reference는 `execpolicy`, login, MCP 등 CLI 명령을 제공하고, Advanced Config는 `~/.codex/profile-name.config.toml` profile layer를 설명한다. 출처: https://developers.openai.com/codex/cli/reference, https://developers.openai.com/codex/config-advanced.

## 2. Codex Provider Spec

`providers/codex.yaml`이 Codex 실행법을 정의한다.

```yaml
provider: codex

capabilities:
  exec: true
  resume: true
  fork: true
  json_events: true
  output_last_message: true
  output_schema: true
  skills: true
  hooks: true
  rules: true
  goals: true
  subagents: true
  mcp: true

roles:
  router-low:
    command: codex
    args:
      - exec
      - --profile
      - low-router
      - --json
      - --output-last-message
      - "{run_dir}/router/output.md"

  worker-impl:
    command: codex
    args:
      - exec
      - --profile
      - worker-impl
      - --json
      - --output-last-message
      - "{run_dir}/implement/output.md"

  worker-review:
    command: codex
    args:
      - exec
      - --profile
      - worker-review
      - --json
      - --output-last-message
      - "{run_dir}/review/output.md"
```

## 3. Codex Profiles

Star-Control renderer가 생성할 파일:

```text
%USERPROFILE%\.codex\low-router.config.toml
%USERPROFILE%\.codex\worker-impl.config.toml
%USERPROFILE%\.codex\worker-review.config.toml
```

### low-router

- `read-only`
- 직접 구현 금지
- route.json / workspec 초안만 생성
- non-interactive에서는 approval prompt가 막힐 수 있으므로 안전한 profile 권장

### worker-impl

- `workspace-write`
- WorkSpec 범위 안에서 구현
- 위험 명령/의존성/삭제/commit/push 금지
- report.json 필수

### worker-review

- `read-only`
- 수정 금지
- diff/report/validation 기준으로 리뷰
- 최종 판정 필수

## 4. Codex Skills 산출물

Star-Control 공통 skill:

```text
skills/validation.md
skills/code-review.md
skills/plan-ledger.md
skills/task-orchestrator.md
```

Codex renderer 산출물:

```text
%USERPROFILE%\.agents\skillsalidation\SKILL.md
%USERPROFILE%\.agents\skills\code-review\SKILL.md
%USERPROFILE%\.agents\skills\plan-ledger\SKILL.md
```

Codex Skills는 `SKILL.md` 기반 reusable workflow이고 plugin으로 패키징 가능하다. 출처: https://developers.openai.com/codex/skills, https://developers.openai.com/codex/plugins.

## 5. Codex Rules 산출물

원본:

```text
policies/command-policy.yaml
```

산출물:

```text
%USERPROFILE%\.codex
ules\command_block.rules
```

주의:

- `.rules`는 Starlark 형식
- `prefix_rule` 기반 prefix matching
- `forbidden > prompt > allow`
- `codex execpolicy check`로 테스트

## 6. Codex Hooks 산출물

원본:

```text
hooks/*.yaml
```

Codex 산출물:

```text
%USERPROFILE%\.codex\hooks\...
```

초기에는 hooks를 Codex native로 많이 쓰지 말고 Star-Control Hook Engine으로 처리한다. Codex Hooks는 provider-specific 최적화로 나중에 추가한다.

## 7. Codex Goals 반영

Codex `/goal`은 목표가 명확하고 validation loop가 있는 작업에 적합하다. Star-Control의 Goal Engine은 provider-neutral 원본이고, Codex 목표는 native adapter로 사용할 수 있다.

매핑:

```text
Star-Control GoalSpec → Codex /goal prompt
Goal pause/resume/clear → Codex native command 또는 Star-Control state update
```

## 8. Codex Subagents 반영

초기 MVP에서는 Codex Subagent를 쓰지 않는다. 이유:

- 토큰 추가 소모
- 상태 동기화 복잡
- Star-Control이 이미 worker/thread engine을 가짐

나중에 활용할 경우:

- 설계 검토
- 독립 리뷰
- 병렬 코드베이스 탐색

## 9. Codex App/Cloud와의 관계

Codex app은 parallel thread, review pane, worktree, automations 등을 제공하지만, Star-Control 1차 MVP는 CLI adapter 중심으로 한다. App/Cloud는 나중에 Control Plane/Background Run Adapter로 붙인다.
