# E58 Release Automation Executor

## 목적

E54에서 dry-run과 approval gate까지만 제공하던 release automation surface를 approval-gated local executor로 확장한다. 이 executor는 release 준비 결과를 대상 project `.ai-runs/` 아래 artifact로 기록하지만, 실제 signing, package publish, deploy, repository settings 변경, external account 변경은 수행하지 않는다.

## 구현 범위

- `ReleaseAutomationPlanner::execute(store, job_id, readiness, action, approval_token)`을 제공한다.
- approval이 필요한 action은 `approve:{action}:{job_id}` token이 일치할 때만 실행한다.
- `rollback-checklist`처럼 approval이 필요 없는 action은 token 없이 local result artifact를 기록한다.
- `review-pack` action은 기존 `ReleaseReviewPackWriter`를 통해 release review pack artifact 준비를 시도한다.
- executor 결과는 `release/{action}-automation-result.json`에 기록한다.
- CLI `release --action <name> --approve-release-action <token> --json`은 executor 결과를 envelope으로 반환한다.

## 제외 범위

- external signing 실행
- package registry publish 실행
- deploy 실행
- repository settings mutation
- external account 변경
- provider live call
- credential raw value 접근/출력

## 검증

```text
cargo test -p star-control-cli release --locked
cargo test -p star-control-release automation --locked
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

- wrong approval token은 mutation 없이 blocked 상태를 반환하고 result artifact를 만들지 않는다.
- approved deploy action은 `release/deploy-automation-result.json`을 기록한다.
- `rollback-checklist` action은 approval 없이 local result artifact를 기록한다.
- 모든 execution result는 `external_actions_performed=false`를 유지한다.
