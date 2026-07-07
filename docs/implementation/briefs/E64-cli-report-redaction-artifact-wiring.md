# E64 CLI Report Redaction Artifact Wiring

## 목적

`star-control report --json`이 report artifact를 외부 출력하기 전에 shared redaction utility를 적용하고, redaction finding이 있으면 schema-valid RedactionReport artifact를 job audit directory에 남긴다.

## 범위

포함:

- `packages/star-control-cli` report command redaction 적용
- `star-control-security` shared redaction utility 소비
- `StateStore::write_redaction_report_json`을 통한 `audit/redaction-report-<stage>.json` 저장
- 반복 report read에서 existing RedactionReport artifact 허용
- productization E2E smoke의 CLI report redaction assertion

제외:

- schema field 변경
- 새 external dependency
- provider live call
- Local AI live connector 실행
- Cloud AI live connector 실행
- external billing/quota 조회
- external release/deploy/publish 실행

## 완료 기준

`star-control report --json`은 저장된 report에 secret-like 값이 있어도 stdout에 raw value를 노출하지 않고 `[REDACTED]` 값을 반환해야 한다. redaction finding이 있으면 `audit/redaction-report-<stage>.json`을 저장하고 CLI artifacts에 포함해야 한다. 같은 report를 반복 조회해도 기존 RedactionReport artifact 때문에 실패하지 않아야 한다.

## 검증

```text
cargo test -p star-control-cli --locked report_json_redacts_sensitive_values_and_writes_redaction_report
python scripts/ci/productization_e2e_smoke.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 provider observability/security 자동 통합의 남은 부분, final readiness 정리, external release/deploy/publish executor policy 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
