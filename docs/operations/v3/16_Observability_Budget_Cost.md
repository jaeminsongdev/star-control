> 흡수 출처: `star-control_design_v3/docs/16_Observability_Budget_Cost.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 16. Observability / Budget / Cost 설계

## 1. 목적

Star-Control은 여러 AI provider와 worker를 실행한다. 따라서 다음을 추적해야 한다.

- 어느 provider가 실행되었는가?
- 얼마나 걸렸는가?
- 얼마의 비용/토큰/쿼터를 썼는가?
- 어떤 명령을 실행했는가?
- 어떤 파일을 변경했는가?
- 왜 실패했는가?
- 어떤 worker가 품질 문제를 만들었는가?

---

## 2. Event Log

모든 실행은 append-only event log를 남긴다.

```json
{
  "event_id": "E-000001",
  "job_id": "J-0001",
  "run_id": "W-0001",
  "timestamp": "2026-06-28T12:00:00+09:00",
  "event_type": "worker.started",
  "provider": "codex",
  "role": "worker-impl",
  "stage": "implement"
}
```

이벤트 종류:

```text
job.created
route.started
route.completed
worker.started
worker.completed
worker.failed
validation.started
validation.completed
review.completed
approval.requested
approval.approved
approval.rejected
policy.violation
budget.warning
budget.exceeded
report.generated
```

---

## 3. Metrics

필수 metric:

```text
job_count
job_success_rate
job_failure_rate
average_job_duration
worker_duration_by_provider
validation_failure_rate
review_block_rate
policy_violation_count
approval_count
token_estimate_by_provider
cost_estimate_by_provider
local_model_usage_count
cloud_model_usage_count
```

---

## 4. Cost / Token 추적

Provider가 token usage를 제공하면 사용한다.

Provider가 제공하지 않으면 추정한다.

```yaml
cost_tracking:
  mode: native_or_estimated
  estimate_method:
    input_tokens: tokenizer_estimate
    output_tokens: tokenizer_estimate
  fallback:
    count_chars: true
    chars_per_token: 4
```

---

## 5. Budget Policy

```yaml
budget:
  per_job:
    max_cloud_runs: 5
    max_high_model_runs: 2
    max_local_runs: 10
    max_minutes: 60
    max_retry: 2

  escalation:
    if_budget_exceeded: WAITING_APPROVAL
    if_high_model_needed: approval_required_when_costly
```

---

## 6. Quota / Rate Limit

Provider Adapter는 quota 상태를 보고해야 한다.

표준 상태:

```text
AVAILABLE
LOW_QUOTA
RATE_LIMITED
EXHAUSTED
UNKNOWN
```

라우팅 정책은 quota를 고려해야 한다.

```text
A-high quota exhausted → A-low 또는 local draft로 degrade
local unavailable → cloud low fallback
all providers unavailable → BLOCKED
```

---

## 7. Trace ID

모든 파일에는 동일 job/run id를 포함한다.

```text
J-0001 = 사용자 요청 전체
W-0001 = worker 실행 단위
E-000001 = 이벤트 단위
A-0001 = artifact 단위
```

---

## 8. Artifact Index

```json
{
  "job_id": "J-0001",
  "artifacts": [
    {
      "artifact_id": "A-0001",
      "type": "report",
      "path": "runs/J-0001/implement/report.json",
      "producer": "W-0001"
    }
  ]
}
```

---

## 9. Dashboard 준비

초기에는 UI가 없어도 된다. 하지만 데이터는 UI가 읽기 좋게 남겨야 한다.

```text
runs/J-0001/events.jsonl
runs/J-0001/run-state.json
runs/J-0001/artifacts.json
runs/J-0001/final-report.md
```

---

## 10. Acceptance Criteria

- 모든 worker 실행이 `events.jsonl`에 기록된다.
- 모든 report가 artifact index에 등록된다.
- budget 초과 시 자동으로 `WAITING_APPROVAL`이 된다.
- provider quota 실패가 `BLOCKED`로 정규화된다.
- 나중에 UI 없이도 파일만 보고 작업 과정을 복원할 수 있다.
