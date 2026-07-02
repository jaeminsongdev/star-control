# E28 Cost Metric Budget Guard

## 목표

M9c의 목표는 provider-neutral CostMetric을 공통 observability crate에서 검증, 저장, 읽기, warning-only budget evaluation 할 수 있게 하는 것이다.

이번 slice는 `packages/star-control-observability`의 CostMetricWriter와 CostBudgetThresholds만 다룬다. provider 실행 경로 자동 연결, hard budget enforcement, billing/quota 외부 조회, retention/recovery command, release readiness automation은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
security-privacy-observability-contracts.md
security-cost-observability.md
provider-system.md
testing-ci-release.md
release-readiness.md
```

## 허용 파일

```text
packages/star-control-observability/**
packages/star-control-cli/src/lib.rs
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
hard budget enforcement
retention/recovery/release automation 구현
```

## 입력 artifact

```text
specs/schemas/cost-metric.schema.json
examples/security-contracts/cost-metric.fake.example.json
provider-output/{provider_instance_id}/cost-metric.json
```

## 출력 artifact

```text
provider-output/{provider_instance_id}/cost-metric.json
Budget evaluation JSON value
```

CostMetric은 provider output sidecar다. missing CostMetric은 core flow 실패 원인이 아니며, M9c는 warning-only evaluation으로 시작한다.

## 핵심 TASK

```text
CostMetricWriter 추가
CostMetric schema validation
provider-output/{provider_instance_id}/cost-metric.json writer/readback helper
secret-like unexpected field redaction before persist
safe provider instance path containment
CostBudgetThresholds 추가
warning-only budget evaluation
missing cost metric non-fatal read path
fake/default cost=0 regression test
budget threshold warning test
CLI test temp project path collision hardening if workspace validation exposes flake
```

## 완료 기준

- CostMetricWriter가 `cost-metric.schema.json`을 만족하는 metric만 provider output sidecar로 저장한다.
- cost metric path는 `provider-output/{provider_instance_id}/cost-metric.json` 내부로 제한한다.
- 저장 전 shared redaction utility를 적용해 unexpected secret-like field를 남기지 않는다.
- missing cost metric은 `Ok(None)`으로 표현하고 core flow 실패로 취급하지 않는다.
- Budget evaluation은 `warn_only`이고 hard enforcement나 외부 billing/quota 조회를 하지 않는다.
- schema field, workflow, package manager, release/deploy/publish automation은 변경하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-observability -- --nocapture
cargo clippy -p star-control-observability --all-targets -- -D warnings
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다. 반복되면 현재 PowerShell 명령 범위에서만 `CARGO_INCREMENTAL=0`을 사용한다.

## 다음 EPIC handoff

```text
M9d는 provider conformance hardening, retention/recovery, release readiness 중 하나로 이어간다. provider execution path가 CostMetricWriter/Budget evaluation을 자동 호출하는 작업은 별도 작은 PR에서 처리한다.
```
