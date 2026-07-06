# E24 API Control Mutations

## 목표

M7d의 목표는 HTTP server 없이 `packages/star-control-api`에 in-process control mutation service를 추가해 UI와 외부 도구가 CLI와 같은 approval/cancel/resume 계약을 사용할 수 있게 하는 것이다.

이번 slice는 API response envelope, approval response artifact writer, cancel/resume state transition, approval precondition, mutation structured error를 고정한다. HTTP server, socket, auth/session, remote exposure, daemon background worker, provider execution scheduling은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
api-contract.md
approval-review-flow.md
daemon-contract.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-api/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
HTTP server 구현
socket listener 구현
remote API exposure
auth/session 시스템 구현
daemon background worker 변경
provider process 실행 구현
UI browser app 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

## 입력 artifact

```text
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/approvals/approval-request.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/api-response.schema.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
```

## 출력 artifact

```text
대상 project .ai-runs/{job_id}/approvals/approval-response.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
```

API control mutation은 Star-Control repository에 실행 산출물을 쓰지 않는다.

## 핵심 TASK

```text
ApiControlService 추가
POST /projects/{project_id}/jobs/{job_id}/approve
POST /projects/{project_id}/jobs/{job_id}/cancel
POST /projects/{project_id}/jobs/{job_id}/resume
approval request presence check
approval response schema validation
approval response artifact writer
approved response resume precondition
terminal cancel guard
StateStore run-state update
events.jsonl audit event append
structured error envelope tests
secret-like response redaction 유지
ApiReadOnlyService non-GET rejection 유지
```

## 완료 기준

- `ApiControlService`가 GET read-only endpoint와 POST approve/cancel/resume control endpoint를 in-process로 처리한다.
- `ApiReadOnlyService`는 계속 non-GET mutation을 거부한다.
- `approve`는 `WAITING_APPROVAL`와 approval request artifact를 요구하고 `approval-response.json`을 schema-valid로 쓴다.
- `cancel`은 non-terminal job만 `CANCELLED`로 전이한다.
- `resume`은 `WAITING_APPROVAL` job에서 matching approved response가 있을 때만 `VALIDATED`로 전이한다.
- 모든 response는 `api-response.schema.json` envelope을 만족한다.
- HTTP server, socket, auth/session, remote exposure는 구현하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-api -- --nocapture
cargo clippy -p star-control-api --all-targets -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 병렬 Cargo 실행을 피하고 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다. 반복되면 현재 PowerShell 명령 범위에서만 `CARGO_INCREMENTAL=0`을 사용한다.

## 다음 EPIC handoff

```text
M8b browser UI shell은 ApiReadOnlyService와 ApiControlService를 함께 소비하도록 설계한다. browser UI package manager, network server, remote API exposure는 별도 승인 전까지 구현하지 않는다.
```
