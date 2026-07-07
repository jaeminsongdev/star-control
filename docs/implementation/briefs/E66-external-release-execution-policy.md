# E66 External Release Execution Policy

## 목적

release automation dry-run/blocked/approved output이 external release/deploy/publish live execution을 수행하지 않는 이유와 blocked operation을 machine-readable policy로 노출한다.

## 범위

포함:

- `ReleaseAutomationPlanner::plan`의 `external_execution_policy`
- `ReleaseAutomationPlanner::execute` result의 `external_execution_policy`
- CLI `release` output의 top-level `external_execution_policy`
- approved deploy/package-publish 계열의 `local_plan_record_only` 유지
- productization E2E smoke의 external release policy assertion

제외:

- 실제 signing
- package registry publish
- deploy/infrastructure mutation
- repository settings 변경
- external account 변경
- Local/Cloud AI live connector 실행

## 완료 기준

release automation output은 `external_execution_policy.live_execution_enabled=false`, `external_actions_allowed=false`, `status=reserved`를 반환해야 한다. external-effect step은 approved execution에서도 local result artifact만 기록하고 `external_actions_performed=false`를 유지해야 한다.

## 검증

```text
cargo test -p star-control-cli --locked release
python scripts/ci/productization_e2e_smoke.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 final readiness 정리를 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
