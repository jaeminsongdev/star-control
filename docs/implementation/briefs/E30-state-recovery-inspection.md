# E30 State Recovery Inspection

## 목표

M9e slice는 StateStore recovery의 첫 runtime surface를 inspect-only로 고정한다. 손상된 job artifact와 남은 tmp file을 식별하지만, 파일 삭제, event log trim, recovered copy 생성, artifact 교체는 수행하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
state-store.md
state-store-recovery.md
artifact-layout.md
artifact-naming.md
testing-ci-release.md
```

## 허용 파일

```text
packages/star-control-state/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
새 external dependency
release/deploy/publish automation
external account/repository settings 변경
provider live call
HTTP server 구현
browser UI app 구현
tmp file 삭제
event log trim 또는 교체
artifact 자동 복구
retention cleanup 실행
```

## 입력 artifact

```text
.ai-runs/{job_id}/job.json
.ai-runs/{job_id}/run-state.json
.ai-runs/{job_id}/events.jsonl
.ai-runs/{job_id}/tmp/**
```

## 출력 artifact

```text
RecoveryInspection inspect-only JSON value
RecoveryIssue list
StateStore recovery regression tests
```

## 핵심 TASK

```text
RecoveryIssue model 추가
RecoveryInspection model 추가
StateStore::inspect_recovery 추가
job.json missing/invalid/schema mismatch issue classification
run-state.json missing/invalid/schema mismatch issue classification
events.jsonl corrupt/missing issue classification
tmp file warning issue classification
no-delete/no-mutation regression test
path traversal/unsafe job id rejection test
```

## 완료 기준

- `StateStore::inspect_recovery(job_id)`가 `inspect_only` report를 반환해야 한다.
- report는 missing required file, invalid JSON, schema mismatch, corrupt event log, partial tmp file을 구분해야 한다.
- tmp file은 정상 artifact로 보지 않고, 검사 중 삭제하거나 승격하지 않아야 한다.
- unsafe job id나 path traversal recovery input은 거부해야 한다.
- 실제 cleanup, event log trim, recovered copy 생성, artifact 교체, CLI/API command 연결은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-state --locked -- --nocapture
cargo clippy -p star-control-state --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9f는 release readiness writer 또는 recovery command surface 중 하나로 이어간다. recovery command가 파일 삭제, log trim, copy-to-recovered-file, artifact 교체를 수행하려면 별도 승인과 더 강한 audit/report 연결이 필요하다.
