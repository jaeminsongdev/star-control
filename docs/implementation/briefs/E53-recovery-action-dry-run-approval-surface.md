# E53 Recovery Action Dry-run Approval Surface

## 목표

Productization recovery slice는 inspect-only recovery를 action-oriented dry-run/approval surface로 확장한다. 이 slice는 tmp cleanup, recovered copy, event log trim, artifact replace, retention cleanup 계획을 표시하고 non-dry-run action을 approval gate로 막되, 실제 destructive mutation executor는 후속 slice로 남긴다.

## 선행 문서

```text
state-store-recovery.md
cli-command-reference.md
security-cost-observability.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-state/**
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
release/deploy/publish automation
repository settings 변경
destructive recovery mutation
remote API exposure
browser UI 변경
```

## 입력

```text
StateStore::inspect_recovery(job_id)
star-control recover --action <name> --dry-run --json
star-control recover --action <name> --json
```

## 출력

```text
StateStore::plan_recovery_action(job_id, action, mode)
RecoveryActionPlan JSON value
schema-valid CLI envelope
supported actions: tmp-cleanup, recovered-copy, event-log-trim, artifact-replace, retention-cleanup
approval gate token for non-dry-run action
destructive_actions_performed = false
```

## 핵심 TASK

```text
recovery action plan model 추가
recover --action parser option 추가
dry-run action plan output 추가
non-dry-run action approval gate blocked output 추가
tmp file no-delete regression 유지
unsupported action/non-recovery option rejection 유지
```

## 완료 기준

- `star-control recover --action <name> --dry-run --json`은 tmp cleanup, recovered copy, event log trim, artifact replace, retention cleanup 계획을 CLI envelope으로 반환해야 한다.
- dry-run 없는 action은 destructive mutation 없이 `status=blocked`, `approval_gate.approval_token`, `destructive_actions_performed=false`를 반환해야 한다.
- 실제 file delete, event log trim replacement, artifact replacement, retention deletion, provider output mutation, release/deploy/publish automation은 수행하지 않는다.

## 검증

```text
cargo test -p star-control-cli --all-targets --locked recover -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 recovery action executor, release automation surface, daemon queue loop/provider scheduling integration, productization E2E smoke, final readiness 정리 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
