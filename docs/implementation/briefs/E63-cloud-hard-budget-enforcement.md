# E63 Cloud Hard Budget Enforcement

## 목적

cloud provider의 paid/live 성격 transport가 hard budget limit을 넘을 때 실행 전에 차단되도록 한다. 이 slice는 `budget.max_estimated_cost`와 `budget.estimated_cost`만 비교하며, 외부 billing/quota 조회나 live connector 실행은 하지 않는다.

## 선행 확인

```text
AGENTS.md
README.md
PLANS.md
docs/implementation/security-cost-observability.md
docs/implementation/security-privacy-observability-contracts.md
docs/implementation/provider-system.md
docs/implementation/cloud-provider-policy.md
docs/implementation/testing-ci-release.md
docs/implementation/codex-work-queue-current.md
```

## 구현 범위

- `CloudProviderPolicyDecision`에서 `budget.max_estimated_cost` hard limit 평가
- `budget.estimated_cost > budget.max_estimated_cost`이면 `cloud_budget_estimated_cost_exceeded` blocked result 반환
- cloud CLI transport process launch 전 budget block
- cloud API offline fixture/HTTP request artifact 생성 전 budget block
- CostMetric sidecar는 유지
- credential raw value, live call, external billing/quota 조회 미수행 유지

## 제외 범위

- external billing/quota 조회
- dynamic provider pricing 계산
- Local AI connector live execution
- Cloud AI connector live execution
- credential raw value 접근/저장/출력
- external release/deploy/publish action
- repository settings 변경

## 검증

```text
cargo fmt --check
cargo test -p star-control-provider --locked cloud
cargo test -p star-control-execution --locked cloud
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
python scripts/ci/run_all.py
python scripts/ci/productization_e2e_smoke.py
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

cloud provider instance의 `budget.max_estimated_cost` hard limit이 `budget.estimated_cost`보다 낮으면 cloud transport는 시작되지 않아야 한다. provider result는 `blocked`, error kind는 `cloud_budget_estimated_cost_exceeded`여야 하며, Local/Cloud AI live connector는 disabled/reserved 상태로 유지한다.

## 다음 handoff

다음 productization slice는 observability/security 자동 통합의 남은 부분, final readiness 정리, external release/deploy/publish executor policy 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
