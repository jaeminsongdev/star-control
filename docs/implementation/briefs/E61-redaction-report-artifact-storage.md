# E61 RedactionReport Artifact Storage

## 목적

RedactionReport builder가 만든 schema-valid JSON을 StateStore artifact로 저장하는 제품화 surface를 추가한다. 이 slice는 Local/Cloud AI live connector, provider live call, credential raw value 접근을 계속 금지한다.

## 선행 확인

```text
AGENTS.md
README.md
PLANS.md
docs/implementation/security-privacy-observability-contracts.md
docs/implementation/security-cost-observability.md
docs/implementation/state-store.md
docs/implementation/codex-work-queue-current.md
```

## 구현 범위

- `CoreSchema::RedactionReport` schema mapping 추가
- `StateStore::write_redaction_report_json` 추가
- `audit/<file>` job-contained artifact path 사용
- 저장 전 `redaction-report.schema.json` validation 수행
- overwrite 없는 JSON artifact write 사용
- ArtifactRef에 `producer=star-control-security`, `schema_path=specs/schemas/redaction-report.schema.json` 기록
- state-store artifact writer regression 추가

## 제외 범위

- Local AI connector live execution
- Cloud AI connector live execution
- provider live call
- credential raw value 접근/저장/출력
- CLI/provider automatic redaction-report wiring
- hard budget enforcement
- release/deploy/publish external executor
- destructive recovery action

## 검증

```text
cargo fmt --check
cargo test -p star-control-state artifacts --locked
cargo test -p star-control-security --locked
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
python scripts/ci/productization_e2e_smoke.py
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

RedactionReport는 schema validation을 통과한 경우에만 `.ai-runs/{job_id}/audit/<file>`에 저장되어야 한다. 반환 ArtifactRef는 security producer와 RedactionReport schema path를 포함해야 하며, raw secret 값은 report나 artifact ref에 기록되지 않아야 한다.

## 다음 handoff

E62는 provider cost-metric sidecar integration을 처리한다. 그 다음 productization slice는 observability/security 자동 통합의 남은 부분, hard budget enforcement, final readiness 정리, external release/deploy/publish executor policy 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
