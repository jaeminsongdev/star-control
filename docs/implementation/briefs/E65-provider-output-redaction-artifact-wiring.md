# E65 Provider Output Redaction Artifact Wiring

## 목적

fake/local/cloud provider output artifact가 secret-like 값을 raw로 저장하지 않도록 provider 저장 경로에 redaction과 RedactionReport artifact 생성을 자동 연결한다.

## 범위

포함:

- `packages/star-control-provider/src/provider_redaction.rs` provider-specific redaction helper
- fake provider request/stdout/stderr/response artifact redaction
- local-process stdout/stderr file post-process redaction
- cloud preflight/CLI/API offline/live approval artifact redaction
- `audit/provider-redaction-<provider>-<artifact>.json` 저장
- productization E2E smoke provider request redaction assertion

제외:

- schema field 변경
- 새 external dependency
- provider live call
- Local AI live connector 실행
- Cloud AI live connector 실행
- external billing/quota 조회
- external release/deploy/publish 실행

## 완료 기준

provider artifact 저장 경로는 secret-like string을 raw로 보존하지 않고 `[REDACTED]`로 저장해야 한다. redaction finding이 있으면 schema-valid `audit/provider-redaction-<provider>-<artifact>.json`을 저장해야 하며, RedactionReport에는 raw secret 값이 없어야 한다.

## 검증

```text
cargo test -p star-control-provider --locked
python scripts/ci/productization_e2e_smoke.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 final readiness 정리 또는 external release/deploy/publish executor policy 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
