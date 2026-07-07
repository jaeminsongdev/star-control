# E52 Daemon HTTP Control Audit Integration

## 목표

Productization observability slice는 `star-daemon api` loopback HTTP control action을 `AuditEventWriter`에 자동 연결한다. 이 slice는 approve/cancel/resume POST 처리 이후 schema-valid/redacted audit event를 대상 job audit log에 append하되 provider execution, provider live call, hard budget enforcement, recovery/destructive action, release/deploy/publish automation은 수행하지 않는다.

## 선행 문서

```text
security-privacy-observability-contracts.md
security-cost-observability.md
api-contract.md
daemon-contract.md
cli-daemon-api-ui.md
complete-implementation-roadmap.md
```

## 허용 파일

```text
apps/star-daemon/**
docs/implementation/**
PLANS.md
README.md
Cargo.lock
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
새 external dependency
provider execution
provider live call
network/process probe
credential raw value 접근/출력
release/deploy/publish automation
repository settings 변경
destructive recovery action
remote API exposure
```

## 입력

```text
star-daemon api loopback endpoint
ApiControlService GET/POST response envelope
packages/star-control-observability::AuditEventWriter
StateStore project registration
```

## 출력

```text
star-daemon HTTP approve/cancel/resume audit event append
.ai-runs/{job_id}/audit/audit-events.jsonl
API response data.observability.audit_event_ref
audit omission warnings when no job/project audit target exists
cargo test -p star-daemon --all-targets --locked
```

## 핵심 TASK

```text
star-daemon HTTP wrapper를 DaemonApiService로 분리
registered project StateStore를 audit target으로 보존
approve/cancel/resume POST response 이후 AuditEventWriter append 연결
schema-valid/redacted audit event와 artifact ref 노출
audit 누락 시 API response warning으로 표시
HTTP control action audit regression test 추가
```

## 완료 기준

- `star-daemon api`가 approve/cancel/resume HTTP POST action을 처리한 뒤 대상 job의 `audit/audit-events.jsonl`에 `api_control_action` audit event를 append해야 한다.
- audit event는 `AuditEventWriter`를 통해 schema validation과 shared redaction을 거쳐야 한다.
- response에는 audit artifact ref 또는 audit 누락 warning이 표시되어야 한다.
- provider execution, provider live call, credential raw value 접근, network/process probe, hard budget enforcement, RedactionReport artifact storage, recovery/destructive action, release/deploy/publish automation은 수행하지 않는다.

## 검증

```text
cargo test -p star-daemon --all-targets --locked
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 observability/security 자동 통합의 남은 부분, recovery/retention action, release automation surface, productization E2E smoke, final readiness 정리 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
