# E27 Observability Audit Event Writer

## 목표

M9b의 목표는 AuditEvent를 StateStore job directory 내부 append-only JSONL artifact로 기록하는 writer를 추가하고, 저장 전 redaction과 schema validation boundary를 고정하는 것이다.

이번 slice는 `packages/star-control-observability` crate와 `audit/audit-events.jsonl` writer/readback contract만 다룬다. API/CLI/daemon/provider 흐름에 AuditEvent 자동 연결, cost/budget guard, retention/recovery command, release readiness automation은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
artifact-layout.md
state-store.md
security-privacy-observability-contracts.md
security-cost-observability.md
testing-ci-release.md
release-readiness.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-observability/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
external account/repository settings 변경
credential raw value lookup/materialization
provider live call
HTTP server 구현
browser UI app 구현
cost/retention/recovery/release automation 구현
```

## 입력 artifact

```text
specs/schemas/audit-event.schema.json
examples/security-contracts/audit-event.example.json
StateStore job directory
redacted JSON value
```

## 출력 artifact

```text
audit/audit-events.jsonl
ArtifactRef(kind=log, producer=star-control-observability)
```

AuditEvent log는 대상 프로젝트 `.ai-runs/{job_id}/audit/audit-events.jsonl`에 append-only로 저장한다. CoreEvent `events.jsonl`과 목적이 다르며, timeline event를 대체하지 않는다.

## 핵심 TASK

```text
star-control-observability crate 추가
AuditEventWriter 추가
AuditEvent schema validation
StateStore resolve_job_path 기반 job directory containment
append-only audit/audit-events.jsonl writer
audit log readback helper
secret-like value redaction before persist
path traversal rejection test
raw secret persistence regression test
schema-valid audit event append test
```

## 완료 기준

- AuditEventWriter가 `audit-event.schema.json`을 만족하는 event만 저장한다.
- audit log path는 `StateStore::resolve_job_path(job_id, "audit/audit-events.jsonl")`를 통해 job directory 내부로 제한한다.
- 저장 전 `star-control-security` redaction utility를 적용해 raw secret-like value를 남기지 않는다.
- writer가 반환하는 artifact ref는 `kind=log`, `producer=star-control-observability`, `schema_path=specs/schemas/audit-event.schema.json`을 사용한다.
- schema field, workflow, package manager, release/deploy/publish automation은 변경하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-observability -- --nocapture
cargo clippy -p star-control-observability --all-targets -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다. 반복되면 현재 PowerShell 명령 범위에서만 `CARGO_INCREMENTAL=0`을 사용한다.

## 다음 EPIC handoff

```text
M9c는 cost/budget guard, provider conformance hardening, retention/recovery, release readiness 중 하나로 이어간다. API/CLI/daemon/provider event를 AuditEventWriter에 연결하는 작업은 별도 작은 PR에서 처리한다.
```
