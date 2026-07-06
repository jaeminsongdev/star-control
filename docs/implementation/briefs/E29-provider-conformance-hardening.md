# E29 Provider Conformance Hardening

## 목표

M9d slice는 기존 `ProviderConformanceChecker`를 provider-neutral hardening gate로 강화한다. 실제 provider 호출을 늘리지 않고, provider result와 `.ai-runs/{job_id}/provider-output/{provider_instance_id}/` artifact가 서로 일치하는지 검증한다.

## 선행 문서

```text
complete-implementation-roadmap.md
provider-system.md
security-privacy-observability-contracts.md
security-cost-observability.md
testing-ci-release.md
release-readiness.md
```

## 허용 파일

```text
packages/star-control-provider/**
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
새 external dependency
release/deploy/publish automation
external account/repository settings 변경
credential raw value lookup/materialization
provider live call
HTTP server 구현
browser UI app 구현
retention/recovery/release automation 구현
```

## 입력 artifact

```text
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
provider-output/{provider_instance_id}/request.json
provider-output/{provider_instance_id}/response.json
provider-output/{provider_instance_id}/stdout.txt
provider-output/{provider_instance_id}/stderr.txt
provider-output/{provider_instance_id}/privacy-handoff.json
provider-output/{provider_instance_id}/cost-metric.json
```

## 출력 artifact

```text
ProviderConformanceChecker hardening
ProviderConformanceReport checked_artifacts
conformance regression tests
```

## 핵심 TASK

```text
provider_instance_id safe segment check
ArtifactRef path/kind/producer consistency check
stored response.json schema validation
stored response.json equals ProviderRunResult value check
cloud privacy-handoff schema validation
cloud cost-metric schema validation
cloud sidecar job/provider/stage consistency check
unsafe provider id regression test
stored response mismatch regression test
schema-invalid cloud sidecar regression test
```

## 완료 기준

- `ProviderConformanceChecker`가 `request_ref`, `response_ref`, `stdout_ref`, optional `stderr_ref`의 path/kind/producer를 검증해야 한다.
- stored `response.json`은 `provider-run-result.schema.json`을 만족하고 `ProviderExecution.result().value()`와 일치해야 한다.
- cloud profile은 `privacy-handoff.json`과 `cost-metric.json`을 실제 schema로 검증하고 job/provider/stage 핵심 필드가 execution result와 일치해야 한다.
- provider instance id나 artifact path가 provider output directory boundary를 우회하면 실패해야 한다.
- 실제 provider live call, schema field 변경, workflow 변경, release/deploy/publish automation은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-provider --locked -- --nocapture
cargo clippy -p star-control-provider --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9e는 retention/recovery 또는 release readiness writer 중 하나로 이어간다. provider execution path가 conformance checker를 모든 provider run마다 자동 호출하는 작업은 별도 작은 PR에서 처리한다.
