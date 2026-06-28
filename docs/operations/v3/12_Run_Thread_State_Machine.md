> 흡수 출처: `star-control_design_v3/docs/12_Run_Thread_State_Machine.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 12. Run / Thread 상태머신 설계

## 1. 목적

Star-Control은 “새 채팅방/쓰레드 생성 → 작업 → 완료 보고 → 다음 단계 재개” 흐름을 provider와 독립적으로 관리해야 한다.

따라서 `Thread`와 `Run`을 분리한다.

```text
Thread = provider 내부 대화/세션/작업 컨텍스트
Run    = Star-Control이 관리하는 실행 단위
Job    = 사용자 요청 전체
Stage  = route / plan / implement / validate / review / polish / report
```

---

## 2. 핵심 개념

### 2.1 Job

사용자 요청 하나.

예:

```text
J-0001: 스톱워치 만들어줘
```

### 2.2 Stage

Job 내부 단계.

```text
route → plan → design → implement → validate → review → polish → report
```

### 2.3 Worker Run

특정 stage를 특정 provider/role로 실행한 단위.

예:

```text
W-0001: J-0001 implement stage, Codex worker-impl
```

### 2.4 Thread

Provider가 제공하는 세션 단위.

Codex 예:

```text
codex exec session id
codex resume session id
codex fork session id
```

Local model은 thread 개념이 없을 수 있으므로 Star-Control이 가상 thread id를 만든다.

---

## 3. Job 상태값

```text
REQUESTED
ROUTING
ROUTED
PLANNING
PLANNED
WAITING_APPROVAL
IMPLEMENTING
IMPLEMENTED
VALIDATING
VALIDATED
REVIEWING
REVIEWED
POLISHING
POLISHED
REPORTING
DONE
FAILED
BLOCKED
CANCELLED
```

---

## 4. Worker Run 상태값

```text
PENDING
STARTING
RUNNING
COLLECTING
DONE
FAILED
BLOCKED
NEEDS_APPROVAL
TIMEOUT
CANCELLED
REPORT_INVALID
```

---

## 5. 상태 전이 규칙

### 5.1 기본 흐름

```text
REQUESTED
  → ROUTING
  → ROUTED
  → PLANNING
  → PLANNED
  → IMPLEMENTING
  → IMPLEMENTED
  → VALIDATING
  → VALIDATED
  → REVIEWING
  → REVIEWED
  → POLISHING
  → POLISHED
  → REPORTING
  → DONE
```

### 5.2 승인 필요 흐름

```text
PLANNED
  → WAITING_APPROVAL
  → APPROVED
  → IMPLEMENTING
```

### 5.3 실패 흐름

```text
IMPLEMENTING
  → FAILED
  → route repair worker
  → IMPLEMENTING
```

또는:

```text
FAILED
  → BLOCKED
  → user action required
```

---

## 6. 상태 전이표

| 현재 상태 | 이벤트 | 다음 상태 | 처리 |
|---|---|---|---|
| REQUESTED | route_start | ROUTING | router-low 실행 |
| ROUTING | route_done | ROUTED | route.json 검증 |
| ROUTED | plan_required | PLANNING | plan worker 실행 |
| ROUTED | plan_not_required | IMPLEMENTING | WorkSpec 생성 |
| PLANNING | plan_done | PLANNED | 승인 필요 판단 |
| PLANNED | approval_required | WAITING_APPROVAL | approval request 생성 |
| WAITING_APPROVAL | approved | IMPLEMENTING | worker 실행 |
| IMPLEMENTING | worker_done | IMPLEMENTED | report 수집 |
| IMPLEMENTING | worker_failed | FAILED | retry 정책 적용 |
| IMPLEMENTED | validation_required | VALIDATING | validation 실행 |
| VALIDATING | validation_passed | VALIDATED | review로 이동 |
| VALIDATING | validation_failed | FAILED | fix loop |
| REVIEWING | review_block | BLOCKED | 사용자 보고 |
| REVIEWING | review_pass | REVIEWED | polish로 이동 |
| POLISHED | final_report_done | DONE | 완료 |

---

## 7. Run directory layout

```text
runs/
  J-0001/
    request.md
    job.json
    route.json
    run-state.json

    route/
      input.md
      output.md
      events.jsonl
      report.json

    plan/
      workspec.md
      output.md
      report.json

    implement/
      workspec.md
      output.md
      events.jsonl
      stdout.log
      stderr.log
      report.json
      changed-files.json

    validation/
      validation.json
      stdout.log
      stderr.log

    review/
      workspec.md
      output.md
      report.json

    final-report.md
```

---

## 8. Controller resume 설계

Provider가 thread resume을 native 지원하면 Adapter가 사용한다.

Codex:

```text
codex exec resume <SESSION_ID> "worker report를 읽고 다음 단계를 판단해"
```

Provider가 resume을 지원하지 않으면 Star-Control은 새 router run을 시작하되, `Context Pack + run-state.json + reports`를 넣어 controller continuation을 emulation한다.

```text
native_resume_supported: true  → provider resume
native_resume_supported: false → fresh controller run + context pack
```

---

## 9. 병렬 실행 정책

초기 MVP에서는 병렬 실행 금지.

```yaml
parallelism:
  enabled: false
  max_workers: 1
```

2단계부터 stage별 병렬화 허용.

허용 가능한 병렬화:

- review 후보 2개 병렬 생성
- local draft + cloud design 병렬 생성
- docs update + test generation 병렬 생성

금지:

- 동일 파일을 수정하는 implement worker 병렬 실행
- 같은 worktree에서 여러 writer 실행
- approval 없이 remote mutation 병렬 실행

---

## 10. Worktree 정책

초기 MVP:

```text
single workspace, sequential execution
```

고급 모드:

```text
job별 git worktree 생성
worker별 branch 생성
final merge는 사용자 승인 후 진행
```

---

## 11. Timeout / Retry

기본값:

```yaml
retry:
  max_attempts: 2
  retryable_status:
    - TIMEOUT
    - REPORT_INVALID
    - TRANSIENT_PROVIDER_ERROR
  non_retryable_status:
    - NEEDS_APPROVAL
    - BLOCKED
    - POLICY_VIOLATION
```

---

## 12. Acceptance Criteria

Run Engine MVP 완료 기준:

- `ai-router run "스톱워치 만들어줘"`가 `runs/J-0001`을 만든다.
- `run-state.json`이 모든 stage 전이를 기록한다.
- worker 실패 시 상태가 `FAILED`로 바뀐다.
- `report.json` 불일치 시 `REPORT_INVALID`가 된다.
- 최종 `final-report.md`가 생성된다.
