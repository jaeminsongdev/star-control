> 흡수 출처: `star-control_design_v3/docs/13_Capability_Feature_Registry_상세.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 13. Capability Registry / Provider Feature Matrix 상세

## 1. 목적

Provider마다 기능 이름과 구현 방식이 다르다.

예:

```text
Codex: Goals, Skills, Hooks, Rules, Subagents
Claude Code: Plan Mode, Skills, Hooks, Subagents, Plugins, Permissions, Checkpoints
Gemini CLI: Plan Mode, Extensions, MCP, Commands, Hooks, Subagents, Skills
Cursor: Agent Mode, Plan Mode, Background Agents, Rules, Memories, Search
GitHub Copilot: Custom Instructions, Agent Skills, MCP, Coding Agent, Code Review
```

Star-Control은 이를 provider 고유 기능으로 흩어두지 않고, 공통 capability로 등록한다.

---

## 2. Capability Registry 구조

```yaml
capabilities:
  plan_mode:
    category: planning
    required: true
    description: "구현 전 조사, 질문, 계획, 승인 대기 기능"
    core_engine: plan_engine

  goal_mode:
    category: autonomous_loop
    required: true
    description: "목표를 지속적으로 추진하고 증거 기반으로 완료 판단"
    core_engine: goal_engine

  skills:
    category: procedure
    required: true
    description: "재사용 가능한 작업 절차"
    core_engine: skill_engine

  hooks:
    category: lifecycle
    required: true
    description: "단계 전후 자동 실행"
    core_engine: hook_engine
```

---

## 3. Capability 상태값

Provider Feature Matrix에는 각 capability를 아래 중 하나로 표시한다.

```text
native       provider가 직접 지원
partial      일부만 지원
emulated     Star-Control이 구현
unsupported  현재 사용하지 않음
unknown      공식 확인 필요
```

---

## 4. Provider Feature Matrix 필드

```yaml
provider: codex
last_verified: "2026-06-28"
verification_sources:
  - "https://developers.openai.com/codex/skills"
  - "https://developers.openai.com/codex/hooks"

features:
  skills:
    status: native
    adapter: codex.skill_renderer

  plan_mode:
    status: emulated
    adapter: star.plan_engine
    notes: "read-only profile + plan template + approval gate"
```

---

## 5. 공통 capability 전체 목록

### 5.1 Context

```text
context_files
project_instructions
scoped_instructions
memory
context_compaction
context_pack
session_history
```

### 5.2 Planning / Goal

```text
plan_mode
plan_review
plan_approval_gate
goal_mode
goal_pause_resume
goal_budget
evidence_based_completion
fix_loop
```

### 5.3 Procedure / Commands

```text
skills
lazy_skill_loading
custom_commands
prompt_files
mode_switching
slash_commands
```

### 5.4 Lifecycle

```text
hooks
before_tool
after_tool
before_worker
after_worker
before_compact
after_compact
on_done
on_failed
```

### 5.5 Safety

```text
command_rules
permissions
sandbox
approval_prompt
checkpoints
scope_guard
secret_guard
policy_rendering
```

### 5.6 Worker / Thread

```text
subagents
worker_roles
agent_teams
background_agents
thread_resume
thread_fork
headless_exec
parallel_runs
async_runs
```

### 5.7 Tools

```text
mcp
shell_tool
file_edit_tool
git_tool
github_tool
browser_tool
computer_use
```

### 5.8 Retrieval

```text
lexical_search
semantic_search
explore_worker
file_reference_resolver
diagnostics_adapter
docs_retriever
```

### 5.9 VCS / Collaboration

```text
worktree
branch
pr_create
pr_review
diff_review
checkpoint_rollback
```

### 5.10 Quality

```text
validation_loop
code_review
critic_review
security_review
test_fix_loop
static_check
evidence_engine
```

### 5.11 Packaging / UI / Cost

```text
extension_packages
plugin_packages
provider_renderers
control_plane
approval_queue
run_dashboard
cost_budget
quota_monitoring
notifications
```

---

## 6. Native vs Emulation 판단 기준

| 기준 | Native 사용 | Emulation 사용 |
|---|---|---|
| provider 기능이 안정적이고 공식 문서화됨 | O | - |
| provider 기능이 베타/실험적임 | 선택 | O |
| provider 기능이 특정 UI에만 있음 | 선택 | O |
| provider 기능이 자동화 API를 제공함 | O | - |
| provider 기능이 비대화 CLI에서 안 됨 | - | O |
| 공통 ReportSpec이 필요함 | 후처리 필요 | O |

---

## 7. Provider Feature Matrix 갱신 절차

1. 공식 문서 확인.
2. provider CLI/API 버전 확인.
3. 기능별 native/partial/emulated 상태 갱신.
4. 테스트 명령 또는 확인 방법 기록.
5. `last_verified` 갱신.
6. 변경 사항을 `docs/99_참고자료.md`에 기록.

---

## 8. Acceptance Criteria

- provider별 기능 차이가 `ProviderAdapter` 밖으로 새지 않는다.
- `capability-registry.yaml`만 봐도 Star-Control이 구현해야 할 전체 기능을 알 수 있다.
- `provider-features/*.yaml`만 봐도 native 사용 가능 여부를 알 수 있다.
- unknown 기능은 사용하지 않고 검증 대기 상태로 둔다.
