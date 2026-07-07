# E54 Release Automation Dry-run Approval Surface

## 목표

Productization release slice는 release readiness artifact를 기반으로 signing/publish/deploy/rollback/review 준비 계획을 표시하는 dry-run/approval surface를 추가한다. 이 slice는 실제 signing, package publish, deploy, repository settings 변경, external account 변경을 수행하지 않는다.

## 선행 문서

```text
release-readiness.md
cli-command-reference.md
security-cost-observability.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-release/**
packages/star-control-cli/**
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
credential raw value 접근/출력
release/deploy/publish 실행
repository settings 변경
external account 변경
destructive recovery mutation
remote API exposure
browser UI 변경
```

## 입력

```text
release/release-readiness.json
star-control release --action <name> --dry-run --json
star-control release --action <name> --json
```

## 출력

```text
ReleaseAutomationPlanner
schema-valid CLI envelope
supported actions: prepare, signing-policy, package-publish, deploy, rollback-checklist, approval-record, review-pack
approval gate token for non-dry-run action
external_actions_performed = false
release_actions_performed = false
```

## 핵심 TASK

```text
release automation plan model 추가
release top-level CLI command 추가
dry-run release action plan output 추가
non-dry-run release action approval gate blocked output 추가
release readiness no-mutation regression 유지
unsupported action/non-release option rejection 유지
```

## 완료 기준

- `star-control release --action <name> --dry-run --json`은 signing policy, package publish, deploy, rollback checklist, approval record, release review pack 준비 계획을 CLI envelope으로 반환해야 한다.
- dry-run 없는 action은 external/release mutation 없이 `status=blocked`, `approval_gate.approval_token`, `external_actions_performed=false`, `release_actions_performed=false`를 반환해야 한다.
- 실제 signing, package publish, deploy, repository settings mutation, external account 변경, release readiness overwrite를 수행하지 않는다.

## 검증

```text
cargo test -p star-control-cli --all-targets --locked release -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 release/recovery action executor, daemon queue loop/provider scheduling integration, observability/security 남은 자동 통합, productization E2E smoke, final readiness 정리 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
