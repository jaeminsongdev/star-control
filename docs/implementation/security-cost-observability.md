# Security / Cost / Observability 구현 계약

## 목적

Star-Control은 여러 provider와 장시간 작업을 관제하므로 보안, 비용, 관측성 기준을 처음부터 분리해 둬야 한다. 이 문서는 구현자가 provider 실행, artifact 저장, approval, logging, budget 추적을 만들 때 지켜야 할 공통 운영 계약을 정의한다.

## 핵심 원칙

- secret raw value를 repository, artifact, log, report에 저장하지 않는다.
- provider credential은 reference로만 다룬다.
- dependency, workflow, release, external account 변경은 승인 없이 진행하지 않는다.
- 비용과 quota는 가능한 한 job/run/provider 단위로 기록한다.
- 모든 자동 판단은 event와 report로 추적 가능해야 한다.
- 검증 실패를 숨기기 위해 CI, test, policy를 약화하지 않는다.

## security boundary

Star-Control의 security boundary는 다음 계층에 걸친다.

```text
User request
RouterEngine risk classification
WorkSpec allowed_scope / forbidden_actions
ProviderAdapter execution boundary
ValidationEngine / Star Sentinel policy
Approval / Review Flow
Artifact redaction
Report generation
```

## credential policy

금지:

```text
- credential raw value를 config에 저장
- token을 events.jsonl에 기록
- provider stdout/stderr에서 secret을 그대로 report에 인용
- UI에 secret raw value 표시
- PR 본문에 secret 포함
```

허용:

```text
- credential reference id
- environment variable name
- OS keychain reference
- external secret manager reference
```

예시:

```json
{
  "credential_ref": "env:STAR_CONTROL_PROVIDER_TOKEN"
}
```

## secret redaction

Artifact와 report를 생성할 때 secret 후보는 redaction해야 한다.

Redaction placeholder:

```text
[REDACTED]
```

초기 redaction 후보:

```text
API key pattern
Bearer token
private key block
password assignment
.env file changed line
```

Star Sentinel security profile은 secret exposure 후보를 block으로 처리할 수 있다.

Productization E64는 `star-control report --json`이 report artifact를 외부 출력하기 전에 shared redaction utility를 적용하도록 연결한다. redaction finding이 있으면 `StateStore::write_redaction_report_json`으로 job 내부 `audit/redaction-report-<stage>.json`을 저장하고, 반복 조회 시 기존 RedactionReport artifact 때문에 report command가 실패하지 않도록 유지한다. 이 경로는 credential raw value 접근, provider live call, external billing/quota 조회를 수행하지 않는다.

Productization E65는 fake/local/cloud provider artifact 저장 경로에 provider-specific redaction helper를 연결한다. provider request/stdout/stderr/response 계열 artifact는 secret-like string을 `[REDACTED]`로 저장하고, finding이 있으면 `audit/provider-redaction-<provider_instance_id>-<artifact>.json` RedactionReport를 기록한다. local-process stdout/stderr는 process 종료 후 redaction post-process를 거치며, cloud live approval 경로도 approval artifact만 redaction하고 live call은 수행하지 않는다.

## dangerous action guard

다음 action은 approval 없이 자동 진행하면 안 된다.

```text
dependency_install
package_manager_init
workflow_change
release_publish
deploy
external_account_change
file_delete
bulk_move
credential_change
permission_change
validator_policy_change
```

WorkSpec의 `forbidden_actions`와 RouterEngine의 approval 판단에 반영한다.

## dependency policy

의존성 추가/변경은 approval required다.

감시 후보 파일:

```text
package.json
package-lock.json
pnpm-lock.yaml
yarn.lock
Cargo.toml
Cargo.lock
pyproject.toml
requirements.txt
uv.lock
poetry.lock
go.mod
go.sum
```

초기 단계에서는 package manager 도입 자체를 별도 승인 전 금지한다.

## workflow policy

`.github/workflows/` 변경은 high-risk로 본다.

확인 항목:

- permissions 상승
- external action 추가
- pull_request_target 사용
- secret 접근 범위 변경
- deploy/release step 추가
- CI 검사 삭제/약화

workflow 변경은 Star Sentinel validator/security profile 후보로 본다.

## test weakening policy

다음은 위험 변경이다.

```text
테스트 파일 삭제
assertion 삭제 또는 약화
skip/only/ignore 추가
테스트 command 삭제
CI required check 삭제
```

정당한 테스트 수정이라도 review pack에 이유와 evidence를 남긴다.

## cost tracking

Cost tracking은 provider-neutral하게 기록한다.

후보 필드:

```text
provider_instance_id
job_id
stage
input_tokens
output_tokens
wall_time_ms
estimated_cost
quota_remaining
rate_limit_remaining
```

FakeProvider와 local process provider는 비용을 0으로 기록한다.

M9c 구현은 `packages/star-control-observability`의 CostMetricWriter가 담당한다. writer는 schema-valid CostMetric을 provider output sidecar로 저장/읽기하고, missing metric은 core flow 실패로 취급하지 않는다.

## budget guard

Budget guard는 다음 레벨에서 적용될 수 있다.

```text
per job
per provider instance
per day
per project
per user approval session
```

M9c budget evaluation은 `warn_only`다. threshold 초과는 status `warning`과 reasons로 표현하지만 provider execution, validation, report generation을 직접 중단하지 않는다.

E63 cloud hard budget enforcement는 provider instance의 `budget.max_estimated_cost`를 hard limit으로 사용한다. `budget.estimated_cost`가 이 limit을 초과하면 cloud CLI process, cloud API offline fixture, cloud API live approval path는 transport 준비나 process 실행 전에 `cloud_budget_estimated_cost_exceeded` blocked result로 정규화한다. 이 경로는 credential raw value 접근, live HTTP call, 외부 billing/quota 조회를 수행하지 않는다.

초과 후보 상태:

```text
BudgetExceeded
QuotaExceeded
RateLimited
```

## observability model

관측성은 다음 세 축으로 나눈다.

```text
events
logs
metrics
```

### events

`events.jsonl`에 append되는 구조화 event다. 사람이 작업 흐름을 추적할 수 있어야 한다.

### logs

provider stdout/stderr, daemon log, tool log처럼 디버깅용 상세 기록이다. secret redaction 대상이다.

### metrics

duration, count, cost, token, failure rate 같은 집계 가능한 값이다.

## trace ids

장기적으로 다음 id를 사용한다.

```text
job_id
run_id
stage_id
provider_run_id
validation_id
event_id
trace_id
```

초기 구현은 `job_id`, `validation_id`, `event_id`부터 충분하다.

## logging policy

로그는 다음을 지킨다.

- secret redaction
- provider raw output은 provider-output 아래 저장
- user-facing report에는 핵심만 요약
- debug log와 report를 분리
- error는 재현에 필요한 path와 state를 포함

## audit policy

감사 추적이 필요한 이벤트:

```text
job created
route decided
approval requested
approval responded
gate decided
provider executed
validation recorded
report generated
job completed
job blocked
job failed
job cancelled
```

## privacy policy

- 사용자의 source code를 불필요하게 Star-Control repo에 복사하지 않는다.
- provider에 전달한 context pack은 artifact로 남기되 민감정보 redaction을 고려한다.
- cloud provider 호출 시 전달 범위는 WorkSpec과 context pack에 명시한다.

## external account policy

외부 계정 수정은 자동 진행 금지다.

예시:

```text
GitHub repo setting 변경
release publish
package registry publish
cloud resource 생성/삭제
billing setting 변경
```

이런 작업은 `WAITING_APPROVAL` 또는 `BLOCKED`로 처리한다.

## report risk section

ReportSpec과 user-facing report에는 risks를 숨기지 않는다.

포함 후보:

```text
risk_level
risk_reasons
approval_required
security_findings
cost_estimate
validation_gaps
```

## minimum security tests

초기 테스트 후보:

1. credential raw value redaction
2. dependency file change -> approval required
3. workflow change -> approval required
4. test deletion -> block diagnostic
5. secret candidate -> block diagnostic
6. forbidden action -> blocked
7. provider output path traversal 차단
8. report에 secret raw value 없음

## minimum cost tests

1. fake provider cost 0
2. provider run duration 기록
3. budget exceeded 상태 표현
4. cost field가 없어도 core flow 실패하지 않음

## minimum observability tests

1. job 생성 event 기록
2. provider 실행 event 기록
3. validation event 기록
4. gate decision event 기록
5. failure event 기록
6. event_id 중복 없음

## Codex 구현 지시

보안/비용/관측성 구현은 한 PR에 모두 넣지 않는다.

권장 순서:

1. event id / event append 강화
2. secret redaction utility
3. forbidden action guard
4. provider run metrics
5. budget warning
6. audit report section
7. security profile hardening

의존성 추가가 필요한 보안 스캐너는 별도 승인 전까지 사용하지 않는다.
