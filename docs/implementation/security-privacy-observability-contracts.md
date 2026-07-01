# Security / Privacy / Observability Contracts

## 목적

이 문서는 redaction, privacy handoff, audit event, cost metric을 machine-readable artifact로 남기는 기준을 정의한다. 기존 `security-cost-observability.md`는 운영 원칙이고, 이 문서는 schema/example 중심 구현 계약이다.

## machine-readable contracts

```text
specs/schemas/redaction-report.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/audit-event.schema.json
specs/schemas/cost-metric.schema.json
examples/security-contracts/redaction-report.example.json
examples/security-contracts/privacy-handoff.example.json
examples/security-contracts/audit-event.example.json
examples/security-contracts/cost-metric.fake.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## RedactionReport

RedactionReport는 artifact나 report를 생성할 때 민감정보 후보가 어떻게 처리됐는지 기록한다.

M9a 구현 위치:

```text
packages/star-control-security
```

M9a는 `redact_value`, `redact_value_with_report`, RedactionReport builder를 제공하고 API/UI redaction helper가 이를 소비한다. RedactionReport artifact를 StateStore에 저장하는 작업은 audit/report hardening slice에서 별도로 연결한다.

필수 필드:

```text
schema_version
job_id
artifact_path
redacted
findings
```

규칙:

- raw value를 기록하지 않는다.
- finding에는 kind, path, action만 둔다.
- user-facing report에는 redaction이 발생했다는 사실을 숨기지 않는다.

## PrivacyHandoff

PrivacyHandoff는 provider나 tool로 context를 넘길 때 어떤 context path가 사용됐는지 기록한다.

필수 필드:

```text
schema_version
job_id
destination
context_paths
redaction_required
approved
```

규칙:

- cloud provider handoff는 WorkSpec/context_pack과 연결되어야 한다.
- redaction_required가 true이면 RedactionReport 또는 equivalent evidence가 있어야 한다.
- approved가 false이면 execution을 시작하지 않는다.

## AuditEvent

AuditEvent는 사람이 나중에 검토해야 하는 control-plane 사건을 요약한다.

필수 필드:

```text
schema_version
event_id
job_id
type
created_at
actor
summary
```

AuditEvent 후보:

```text
approval_requested
approval_recorded
provider_executed
validation_recorded
gate_decided
job_blocked
job_failed
job_cancelled
```

CoreEvent는 execution timeline이고, AuditEvent는 감사 목적의 요약 event다. 둘은 중복될 수 있지만 용도가 다르다.

M9b 구현 위치:

```text
packages/star-control-observability
```

M9b는 `AuditEventWriter`를 제공한다. writer는 AuditEvent를 저장 전 redaction한 뒤 `audit-event.schema.json`으로 검증하고, 대상 프로젝트 `.ai-runs/{job_id}/audit/audit-events.jsonl`에 append-only로 기록한다. API/CLI/daemon/provider event를 자동 연결하는 작업은 후속 observability integration slice에서 처리한다.

## CostMetric

CostMetric은 provider-neutral 비용·시간·token 측정값이다.

필수 필드:

```text
schema_version
job_id
stage
provider_instance_id
estimated_cost
currency
wall_time_ms
```

선택 필드:

```text
input_tokens
output_tokens
quota_remaining
```

FakeProvider는 estimated_cost 0, token 0을 기록한다.

## 금지 사항

- credential raw value를 artifact, report, log에 저장하지 않는다.
- redaction finding에 raw value를 넣지 않는다.
- cost metric이 없다는 이유만으로 core flow를 실패시키지 않는다.
- privacy handoff가 승인되지 않았는데 provider execution을 시작하지 않는다.
- audit event를 조용히 누락하지 않는다.

## 테스트 기준

1. RedactionReport example schema validation
2. PrivacyHandoff example schema validation
3. AuditEvent example schema validation
4. CostMetric example schema validation
5. redaction finding에 raw value가 없음
6. fake provider cost는 0
7. unapproved privacy handoff는 execution 금지
8. approval/gate/provider event는 audit 후보로 기록 가능
