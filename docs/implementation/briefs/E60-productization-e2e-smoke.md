# E60 Productization E2E Smoke

## 목적

제품화 직전 surface가 실제 binary 기준으로 함께 동작하는지 검증하는 local smoke를 추가한다. 이 smoke는 Local AI/Cloud AI live connector와 external release action을 실행하지 않는다.

## 구현 범위

- `scripts/ci/productization_e2e_smoke.py`를 제공한다.
- smoke가 `cargo build --locked -p star-control-cli -p star-daemon`으로 binary를 준비한다.
- temp project/config에서 CLI fake run, providers offline healthcheck, daemon status, loopback-only HTTP API, static UI file surface, recovery dry-run, release rollback-checklist local executor, Sentinel selfcheck를 검증한다.
- smoke output은 JSON summary를 반환한다.

## 제외 범위

- Local AI connector live execution
- Cloud AI connector live execution
- external release/deploy/publish 실행
- remote API exposure
- persistent repo `.ai-runs/` 생성

## 검증

```text
python scripts/ci/productization_e2e_smoke.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

- smoke가 `status=success` JSON을 반환한다.
- Local/Cloud AI live connector는 disabled로 확인된다.
- release smoke는 local result artifact만 만들고 `external_release_actions_performed=false`를 유지한다.
