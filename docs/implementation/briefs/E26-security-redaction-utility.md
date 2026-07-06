# E26 Security Redaction Utility

## 목표

M9a의 목표는 secret redaction을 API/UI/reporting surface가 공유할 수 있는 core utility로 분리하고, schema-valid RedactionReport 생성 경로를 고정하는 것이다.

이번 slice는 `packages/star-control-security` crate, common redaction utility, RedactionReport builder, API/UI consumer migration을 다룬다. audit event writer, cost/budget guard, retention/recovery command, release readiness automation은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
security-privacy-observability-contracts.md
security-cost-observability.md
api-contract.md
ui-shell-contract.md
testing-ci-release.md
release-readiness.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-security/**
packages/star-control-api/**
packages/star-control-ui/**
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
audit/cost/retention/recovery/release automation 구현
```

## 입력 artifact

```text
specs/schemas/redaction-report.schema.json
examples/security-contracts/redaction-report.example.json
API response data/error envelope
UI read-only/control result view
```

## 출력 artifact

```text
RedactionReport JSON value
redacted API/UI JSON value
```

이번 slice는 대상 프로젝트 `.ai-runs/`에 새 artifact를 자동 저장하지 않는다. RedactionReport writer/storage는 audit/report hardening slice에서 별도로 연결한다.

## 핵심 TASK

```text
star-control-security crate 추가
redact_value utility 추가
redact_value_with_report utility 추가
RedactionFinding model 추가
RedactionReport builder 추가
credential-like key redaction
secret-like string redaction
private key marker redaction
raw value 없는 finding/report test
redaction-report schema validation test
ApiReadOnlyService/ApiControlService redaction utility migration
UiReadOnlyShell/UiBrowserShell redaction utility migration
```

## 완료 기준

- API/UI가 중복 redaction helper 대신 `star-control-security`를 사용한다.
- RedactionReport builder가 `redaction-report.schema.json`을 만족한다.
- redaction finding/report에는 raw secret value가 들어가지 않는다.
- API/UI 기존 redaction tests가 유지된다.
- 새 external dependency는 추가하지 않는다. `serde_json = "1"`은 기존 기본 dependency set 범위로 사용한다.
- schema field, workflow, package manager, release/deploy/publish automation은 변경하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-security -- --nocapture
cargo clippy -p star-control-security --all-targets -- -D warnings
cargo test -p star-control-api -- --nocapture
cargo test -p star-control-ui -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 병렬 Cargo 실행을 피하고 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다. 반복되면 현재 PowerShell 명령 범위에서만 `CARGO_INCREMENTAL=0`을 사용한다.

## 다음 EPIC handoff

```text
M9b는 audit event writer 또는 provider conformance hardening으로 이어간다. RedactionReport를 StateStore artifact로 저장하거나 user-facing report에 연결하는 작업은 별도 작은 PR에서 처리한다.
```
