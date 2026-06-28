> 흡수 출처: `star-control_design_v3/docs/11_Provider_Adapter_계약.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 11. Provider Adapter 계약

## 1. 목적

이 문서는 Star-Control이 Codex, Claude Code, Gemini CLI, Cursor, GitHub Copilot, Jules, Devin, 로컬 모델 등을 **동일한 실행 단위**로 다루기 위한 Provider Adapter 계약을 정의한다.

핵심 원칙은 다음이다.

```text
Star-Control Core는 provider 고유 명령을 직접 알지 않는다.
Star-Control Core는 Provider Adapter 계약만 호출한다.
CodexAdapter, ClaudeAdapter, GeminiAdapter, LocalAdapter가 provider별 차이를 흡수한다.
```

즉 Star-Control 내부에서는 항상 아래 공통 명령으로만 작업한다.

```text
start_run(workspec) -> RunHandle
poll_run(run_id) -> RunStatus
collect_artifacts(run_id) -> ArtifactBundle
cancel_run(run_id) -> CancelResult
resume_run(run_id, prompt) -> RunHandle | Unsupported
```

---

## 2. Provider Adapter의 책임

Provider Adapter는 다음 책임을 가진다.

1. 공통 `WorkSpec`을 provider별 입력 프롬프트/명령/설정으로 변환한다.
2. provider 프로세스 또는 API를 실행한다.
3. stdout, stderr, 이벤트 스트림, 최종 메시지, 산출물을 수집한다.
4. provider 고유 결과를 공통 `ReportSpec`으로 정규화한다.
5. 실패, 중단, 승인 필요, timeout을 표준 상태로 변환한다.
6. provider가 native 기능을 지원하면 native 기능을 사용하고, 아니면 Star-Control emulation을 사용한다.

Provider Adapter는 다음을 하면 안 된다.

- Star-Control 정책을 우회해서 명령을 실행하면 안 된다.
- `WorkSpec` 범위를 임의로 넓히면 안 된다.
- provider별 성공 메시지를 곧바로 `DONE`으로 신뢰하면 안 된다.
- 검증하지 않은 결과를 `passed`로 변환하면 안 된다.

---

## 3. 표준 Adapter 인터페이스

언어와 상관없이 최소 인터페이스는 아래 형태를 만족해야 한다.

```python
class ProviderAdapter:
    def prepare(self, provider_config: dict) -> None: ...
    def validate_capabilities(self, role: str, workspec: dict) -> CapabilityCheck: ...
    def render_input(self, workspec: dict) -> RenderedInput: ...
    def start_run(self, rendered_input: RenderedInput) -> RunHandle: ...
    def poll_run(self, run_id: str) -> RunStatus: ...
    def collect_artifacts(self, run_id: str) -> ArtifactBundle: ...
    def normalize_report(self, artifacts: ArtifactBundle) -> dict: ...
    def cancel_run(self, run_id: str) -> CancelResult: ...
```

### 3.1 `prepare`

Provider 실행 전 환경을 확인한다.

확인 항목:

- CLI/API 사용 가능 여부
- 인증 상태
- 모델/프로필 존재 여부
- 작업 디렉터리 접근 가능 여부
- provider feature matrix와 실제 설정의 불일치 여부

### 3.2 `validate_capabilities`

해당 provider/role이 `WorkSpec`을 수행할 수 있는지 확인한다.

예시:

```yaml
role: worker-review
requires:
  edit_files: false
  read_files: true
  run_shell: false
  output_schema: report.schema.json
```

Provider가 `output_schema`를 native 지원하지 않으면 Adapter가 후처리 검증을 수행해야 한다.

### 3.3 `render_input`

공통 `WorkSpec`을 provider별 실행 입력으로 변환한다.

Codex 예:

```text
codex exec --profile worker-impl --json --output-last-message runs/J-0001/implement/output.md < workspec.md
```

Local Ollama 예:

```text
ollama run qwen2.5-coder:14b < workspec.md > output.md
```

API provider 예:

```json
{
  "model": "...",
  "system": "role prompt",
  "messages": [...],
  "response_format": "report.schema.json"
}
```

### 3.4 `start_run`

실행 단위를 시작한다.

반드시 반환해야 할 정보:

```json
{
  "run_id": "W-0001",
  "provider": "codex",
  "role": "worker-impl",
  "pid": 1234,
  "started_at": "...",
  "artifact_dir": "runs/J-0001/implement"
}
```

### 3.5 `poll_run`

실행 상태를 확인한다.

표준 상태값:

```text
PENDING
RUNNING
DONE
FAILED
BLOCKED
NEEDS_APPROVAL
TIMEOUT
CANCELLED
UNKNOWN
```

### 3.6 `collect_artifacts`

표준 산출물:

```text
output.md
report.json
events.jsonl
stdout.log
stderr.log
changed-files.json
validation.json
```

Provider가 일부 산출물을 native로 만들지 못하면 Adapter가 생성해야 한다.

### 3.7 `normalize_report`

provider 고유 응답을 공통 `ReportSpec`으로 변환한다.

정규화 규칙:

- 명령 실행 실패는 `FAILED` 또는 `BLOCKED`로 변환한다.
- 검증 미실행은 `validation.status = not_run`으로 기록한다.
- provider가 “완료”라고 말해도 검증 증거가 없으면 `DONE_WITH_UNVERIFIED_RISK` 같은 내부 risk flag를 붙인다.
- report schema 검증 실패 시 `FAILED`로 기록하고 raw output을 보존한다.

---

## 4. Provider Capability 요구사항

Provider는 역할별로 요구 capability를 충족해야 한다.

```yaml
roles:
  router-low:
    required:
      - read_context
      - output_schema
    forbidden:
      - edit_files
      - run_destructive_commands

  worker-impl:
    required:
      - read_files
      - edit_files
      - run_validation
      - output_report
    optional:
      - structured_output
      - json_events

  worker-review:
    required:
      - read_diff
      - read_validation_result
      - output_review
    forbidden:
      - edit_files
```

---

## 5. Provider별 구현 방향

### 5.1 CodexAdapter

CodexAdapter는 1차 기준 adapter다.

주요 native 기능:

- profile config
- `codex exec`
- `--json`
- `--output-last-message`
- `--output-schema`
- `resume`, `fork`
- skills
- rules
- hooks
- MCP

CodexAdapter는 provider 설정을 다음 파일로 렌더링한다.

```text
%USERPROFILE%\.codex\low-router.config.toml
%USERPROFILE%\.codex\worker-impl.config.toml
%USERPROFILE%\.codex\worker-review.config.toml
%USERPROFILE%\.codex\rules\command_block.rules
%USERPROFILE%\.agents\skills\...
```

### 5.2 LocalModelAdapter

로컬 모델은 처음에는 직접 수정 금지다.

기본 정책:

```yaml
can_edit_files: false
can_run_shell: false
allowed_outputs:
  - draft.md
  - summary.md
  - review-notes.md
```

로컬 모델은 다음 작업에만 사용한다.

- 파일 요약
- 초안 작성
- 테스트 케이스 후보
- 실패 로그 요약
- diff 1차 리뷰
- 반복적 단순 변환

### 5.3 ClaudeCodeAdapter

Claude Code adapter는 Plan Mode, permissions, hooks, checkpoints, skills, subagents, plugins, MCP를 변환 대상으로 둔다.

Star-Control 원본 정책은 다음으로 렌더링될 수 있다.

```text
CLAUDE.md
.claude/settings.json
.claude/skills/*/SKILL.md
.claude/agents/*.md
.claude/hooks config
```

### 5.4 GeminiCLIAdapter

Gemini CLI adapter는 Plan Mode, extensions, MCP, commands, hooks, subagents, skills를 변환 대상으로 둔다.

Star-Control extension pack은 Gemini extension으로 렌더링 가능해야 한다.

### 5.5 CursorAdapter

Cursor adapter는 Plan Mode, background agents, rules, memories, MCP, headless CLI, semantic search를 변환 대상으로 둔다.

### 5.6 GitHubCopilotAdapter

GitHub adapter는 custom instructions, path-specific instructions, agent skills, MCP, code review, coding agent workflow를 변환 대상으로 둔다.

---

## 6. Adapter 실패 처리

Provider Adapter는 아래 실패를 표준화해야 한다.

| Provider 실패 | 표준 상태 | 처리 |
|---|---|---|
| 인증 실패 | BLOCKED | 사용자 인증 필요 |
| 모델 미지원 | BLOCKED | fallback provider 선택 |
| 명령 실패 | FAILED | stderr 저장, retry 정책 적용 |
| report schema 불일치 | FAILED | raw output 보존, repair worker 가능 |
| timeout | TIMEOUT | retry 또는 escalation |
| 승인 필요 | NEEDS_APPROVAL | approval queue에 등록 |
| provider quota 초과 | BLOCKED | fallback 또는 대기 |

---

## 7. Acceptance Criteria

Provider Adapter MVP 완료 기준:

- `WorkSpec` 하나를 받아 provider worker를 실행할 수 있다.
- stdout/stderr/final output이 run directory에 저장된다.
- `ReportSpec`으로 정규화된다.
- 실패 시 `RunState`가 갱신된다.
- provider가 바뀌어도 Star-Control Core 코드는 바뀌지 않는다.
