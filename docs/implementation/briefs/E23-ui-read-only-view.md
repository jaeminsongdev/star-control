# E23 UI Read-Only View

## 목표

M8a의 목표는 browser app을 만들기 전에 `packages/star-control-ui`의 read-only view model 계층을 구현해 UI shell이 API read-only service 계약을 안전하게 소비할 수 있게 하는 것이다.

이번 slice는 job list, job detail, timeline, provider output, validation result, approval request, review pack viewer가 사용할 JSON view model을 고정한다. 실제 browser UI, package manager, HTTP server, API control mutation, provider process 실행은 구현하지 않는다. API control mutation은 E24에서 별도 slice로 다룬다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
ui-shell-contract.md
api-contract.md
daemon-contract.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-ui/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
browser UI app 구현
TypeScript/Node package manager 도입
HTTP server 구현
API control mutation 구현
provider process 실행 구현
Star Sentinel rule 직접 구현
StateStore file mutation 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

## 입력 artifact

```text
ApiReadOnlyService response envelope
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/reports/{stage}-report.json
specs/schemas/ui-job-view.schema.json
```

## 출력 artifact

```text
없음
```

Read-only UI view model은 대상 project `.ai-runs/` artifact를 수정하거나 UI artifact를 쓰지 않는다.

## 핵심 TASK

```text
star-control-ui crate 추가
UiReadOnlyShell 추가
job_list view model
job_detail view model
UI job view schema validation
timeline event view
provider output path viewer data
validation result path viewer data
approval request viewer data
review pack viewer data
read-only no-write regression test
secret-like view redaction test
missing report read-only error surface test
```

## 완료 기준

- `UiReadOnlyShell`이 `ApiReadOnlyService`를 소비해 job list와 job detail view model을 만든다.
- 각 job card는 `ui-job-view.schema.json`을 만족한다.
- detail view가 timeline, provider output, validation result, approval request, review pack path를 표시할 수 있다.
- approval-required job은 approval path와 API/CLI mutation surface를 노출하지만 직접 mutation하지 않는다.
- UI view model에 secret-like raw value가 그대로 포함되지 않는다.
- read-only view model은 StateStore artifact를 수정하거나 새 UI artifact를 쓰지 않는다.
- browser UI app, HTTP API server, API control mutation, provider process, Star Sentinel rule 구현은 포함하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-ui -- --nocapture
cargo clippy -p star-control-ui --all-targets -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 병렬 Cargo 실행을 피하고 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다.

## 다음 EPIC handoff

```text
E24 API control mutation slice를 별도 PR로 설계한다. 그 이후 M8b browser UI shell은 read-only view model과 API control service를 함께 소비하도록 설계한다.
```
