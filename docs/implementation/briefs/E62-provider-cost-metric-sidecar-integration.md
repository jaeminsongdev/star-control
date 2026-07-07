# E62 Provider Cost-Metric Sidecar Integration

## 목적

fake/local-process provider execution path가 cloud provider와 같은 provider output `cost-metric.json` sidecar를 남기도록 연결한다. 이 slice는 비용 관측성 표면을 채우지만 hard budget enforcement, 외부 billing/quota 조회, live connector 실행은 계속 금지한다.

## 선행 확인

```text
AGENTS.md
README.md
PLANS.md
docs/implementation/provider-system.md
docs/implementation/local-process-provider-policy.md
docs/implementation/security-privacy-observability-contracts.md
docs/implementation/security-cost-observability.md
docs/implementation/testing-ci-release.md
docs/implementation/codex-work-queue-current.md
```

## 구현 범위

- provider-neutral zero-cost metric helper 추가
- `FakeProviderAdapter` cost metric sidecar write
- `LocalProcessProviderAdapter` cost metric sidecar write
- fake/local-process provider response artifact list에 `cost-metric.json` 포함
- local-process conformance fixture expected artifact 갱신
- productization E2E smoke에서 fake provider cost sidecar 확인

## 제외 범위

- hard budget enforcement
- 외부 billing/quota 조회
- provider live call
- Local AI connector live execution
- Cloud AI connector live execution
- credential raw value 접근/저장/출력
- release/deploy/publish external action
- repository settings 변경

## 검증

```text
cargo fmt --check
cargo test -p star-control-provider --locked fake
cargo test -p star-control-provider --locked local_process
cargo test -p star-control-execution --locked fake
cargo test -p star-control-execution --locked local_process
cargo test -p star-control-provider --locked conformance
python scripts/ci/productization_e2e_smoke.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

fake/local-process provider execution path는 `.ai-runs/{job_id}/provider-output/{provider_instance_id}/cost-metric.json`을 schema-valid CostMetric으로 남겨야 한다. `estimated_cost`, `input_tokens`, `output_tokens`는 0으로 기록하고, Local/Cloud AI live connector는 disabled/reserved 상태로 유지한다.

## 다음 handoff

다음 productization slice는 observability/security 자동 통합의 남은 부분, hard budget enforcement, final readiness 정리, external release/deploy/publish executor policy 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
