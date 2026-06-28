# CLI / Daemon / API / UI 구현 계약

## 목적

이 문서는 Star-Control의 사용자 표면을 정의한다. Star-Control은 장시간 작업 관제 시스템이므로, 최종적으로 CLI, daemon, API, UI shell이 같은 데이터 계약과 StateStore를 공유해야 한다.

초기 구현은 CLI와 file-based StateStore를 우선한다. Daemon, API, UI는 전체 목표 구조를 먼저 고정하고 실제 구현은 후속 단계에서 진행한다.

## 공통 원칙

- 모든 user surface는 같은 JobSpec, RunState, RouteSpec, WorkSpec, ReportSpec 계약을 사용한다.
- 실행 산출물은 대상 프로젝트 `.ai-runs/` 아래에 저장한다.
- CLI/daemon/API/UI는 provider-specific 세부 구현에 직접 의존하지 않는다.
- Star Sentinel은 builtin tool로 호출하며 UI 또는 CLI가 rule을 직접 구현하지 않는다.
- 사람이 승인해야 하는 작업은 `WAITING_APPROVAL` 상태에서 멈춘다.

## 구현 우선순위

```text
1. CLI run/status/report
2. CLI approve/cancel/resume
3. daemon queue skeleton
4. API read-only endpoints
5. API mutation endpoints
6. UI shell read-only view
7. UI approval/review flow
8. provider session dashboard
```

## CLI 목표

CLI는 daemon 없이도 file-based flow를 실행할 수 있어야 한다.

초기 명령:

```text
star-control run
star-control status
star-control report
star-control approve
star-control cancel
star-control resume
star-control providers
star-control sentinel check
star-control sentinel gate
star-control sentinel review-pack
```

## CLI 공통 규칙

- stdout은 사람이 읽는 요약을 기본으로 한다.
- `--json` 옵션이 있으면 machine-readable JSON을 출력한다.
- stderr에는 오류와 경고만 출력한다.
- exit code는 명확해야 한다.
- 명령 실행 중 만든 artifact 경로를 출력한다.
- secret 또는 credential raw value를 출력하지 않는다.

## CLI exit code

권장 exit code:

```text
0: success
1: validation failed or policy blocked
2: invalid user input
3: missing artifact or state error
4: provider execution error
5: internal error
```

## `star-control run`

목적:

- 새 job 생성
- route 생성
- 필요한 WorkSpec 생성
- provider 실행 또는 queue 등록
- validation/report까지 실행할 수 있음

후보 옵션:

```text
--project <path>
--request <text>
--entrypoint <path-or-command>
--provider <instance-id>
--profile <profile>
--dry-run
--json
```

초기 구현은 `--project`와 `--request`를 필수로 한다.

출력 후보:

```text
job_id
state
current_stage
run_dir
next_action
```

## `star-control status`

목적:

- job의 현재 RunState 표시

후보 옵션:

```text
--project <path>
--job <job-id>
--json
```

표시 항목:

```text
job_id
state
current_stage
next_action
latest_event
artifacts
```

## `star-control report`

목적:

- 최종 또는 stage report 표시

후보 옵션:

```text
--project <path>
--job <job-id>
--stage <stage>
--json
--markdown
```

## `star-control approve`

목적:

- `WAITING_APPROVAL` job에 대해 approval response 작성

후보 옵션:

```text
--project <path>
--job <job-id>
--response approved|rejected|needs_changes|cancelled
--reason <text>
--constraint <text>
--json
```

규칙:

- approval-request.json이 없으면 실패한다.
- response는 approvals/approval-response.json에 저장한다.
- 승인 이후 즉시 실행할지 여부는 별도 옵션 또는 resume 명령으로 처리한다.

## `star-control cancel`

목적:

- running 또는 waiting job을 취소한다.

규칙:

- terminal state job은 cancel하지 않는다.
- provider cancel 지원 여부를 확인한다.
- 취소 event를 events.jsonl에 기록한다.

## `star-control resume`

목적:

- 중단되거나 approval 이후 대기 중인 job을 이어서 진행한다.

진입 조건:

- RunState가 terminal state가 아니다.
- 필요한 artifact가 존재한다.
- approval required 상태라면 approval-response.json이 존재한다.

## `star-control providers`

목적:

- 사용 가능한 provider instance와 capability 확인

후보 하위 명령:

```text
list
show
healthcheck
```

## `star-control sentinel ...`

Star Sentinel builtin tool을 CLI에서 직접 실행하는 명령이다.

후보:

```text
star-control sentinel check --project <path> --job <job-id>
star-control sentinel gate --project <path> --job <job-id>
star-control sentinel review-pack --project <path> --job <job-id>
```

CLI는 Star Sentinel rule을 직접 구현하지 않고 tool command를 호출한다.

## Daemon 목표

Daemon은 장시간 작업과 provider session을 관리한다.

역할:

- job queue 관리
- provider execution scheduling
- cancellation propagation
- resume orchestration
- status watch
- API server host 후보

초기에는 RESERVED다. CLI file-based flow가 안정화된 뒤 구현한다.

## daemon state

Daemon은 Star-Control repository가 아니라 사용자 machine의 config/cache 영역을 사용해야 한다. job artifact는 여전히 대상 프로젝트 `.ai-runs/`에 둔다.

후보:

```text
~/.star-control/daemon/state.json
~/.star-control/daemon/logs/
```

이 위치는 OS별 config policy 문서가 생긴 뒤 확정한다.

## API 목표

API는 UI와 외부 도구가 Star-Control state를 읽고 제한된 mutation을 수행하게 한다.

초기 read-only endpoint 후보:

```text
GET /projects
GET /projects/{project_id}/jobs
GET /projects/{project_id}/jobs/{job_id}
GET /projects/{project_id}/jobs/{job_id}/events
GET /projects/{project_id}/jobs/{job_id}/report
```

mutation endpoint 후보:

```text
POST /projects/{project_id}/jobs
POST /projects/{project_id}/jobs/{job_id}/approve
POST /projects/{project_id}/jobs/{job_id}/cancel
POST /projects/{project_id}/jobs/{job_id}/resume
```

API는 local-first를 기본으로 하고, remote exposure는 별도 보안 문서와 승인 이후 구현한다.

## API 응답 규칙

- JSON만 반환한다.
- error는 structured error로 반환한다.
- secret raw value를 반환하지 않는다.
- artifact path는 필요한 경우 상대 경로로 반환한다.

## UI shell 목표

UI shell은 장시간 작업 관제를 사람이 쉽게 볼 수 있게 하는 계층이다.

초기 화면 후보:

```text
Job list
Job detail
Run timeline
Provider output viewer
Validation result viewer
Approval request viewer
Review pack viewer
Settings / provider registry
```

초기 UI는 read-only view부터 시작한다. 승인/취소/재개 mutation은 API와 CLI 안정화 이후 추가한다.

## UI 금지 사항

- UI가 직접 provider process를 실행하지 않는다.
- UI가 Star Sentinel rule을 직접 구현하지 않는다.
- UI가 StateStore 파일을 임의로 수정하지 않는다.
- UI가 secret raw value를 표시하지 않는다.

## long-running UX

장시간 작업은 다음 정보를 보여야 한다.

```text
job_id
state
current_stage
active_provider
latest_event
elapsed_time
approval_required
blocked_reason
next_action
```

## approval UX

승인 화면에는 다음을 표시한다.

```text
summary
decision
changed_files
risks
diagnostics
review_pack
questions_for_human
approval buttons
constraints input
```

## Codex 구현 지시

Codex는 CLI를 먼저 구현한다. Daemon, API, UI는 문서 계약만 보고 임의로 앞당겨 구현하지 않는다.

권장 구현 순서:

1. CLI status read-only
2. CLI report read-only
3. CLI run with fake provider
4. CLI approve/cancel/resume
5. daemon skeleton
6. API read-only
7. UI shell read-only

각 단계는 별도 PR로 진행한다.
