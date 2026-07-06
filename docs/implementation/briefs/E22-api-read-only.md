# E22 API Read-Only

## 목표

M7c의 목표는 HTTP server나 remote exposure 없이 read-only API request/router library를 구현해 UI shell과 외부 도구가 StateStore artifact를 같은 response envelope으로 읽을 수 있게 하는 것이다.

이번 slice는 API response envelope, read-only endpoint path dispatch, StateStore read-only 보장, missing artifact structured error, 최소 redaction을 고정한다. mutation endpoint, socket, HTTP server, auth, remote exposure는 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
api-contract.md
daemon-contract.md
state-store.md
security-cost-observability.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-api/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
HTTP server 구현
socket listener 구현
remote API exposure
auth/session 시스템 구현
mutation endpoint 구현
daemon background worker 변경
UI 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

## 입력 artifact

```text
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/reports/{stage}-report.json
{config_root}/daemon/state.json
specs/schemas/api-response.schema.json
```

## 출력 artifact

```text
없음
```

Read-only API는 대상 project `.ai-runs/` artifact를 수정하거나 API response artifact를 쓰지 않는다.

## 핵심 TASK

```text
star-control-api crate 추가
ApiReadOnlyService 추가
GET /daemon/state
GET /projects
GET /projects/{project_id}/jobs
GET /projects/{project_id}/jobs/{job_id}
GET /projects/{project_id}/jobs/{job_id}/events
GET /projects/{project_id}/jobs/{job_id}/report?stage={stage}
api-response schema validation
missing project/job/report structured error
mutation method rejection
read-only no-write regression test
secret-like response redaction test
```

## 완료 기준

- 모든 API response가 `api-response.schema.json`을 만족한다.
- daemon queue state를 read-only로 조회할 수 있다.
- read-only endpoint가 StateStore artifact를 직접 변형하지 않는다.
- missing project/job/report는 structured error envelope으로 반환한다.
- mutation method와 mutation-like path는 구현되지 않는다.
- response에 secret-like raw value가 그대로 포함되지 않는다.
- HTTP server, socket, auth, remote exposure는 구현하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-api -- --nocapture
cargo clippy -p star-control-api --all-targets -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Cargo incremental finalize 경고가 재발하면, 병렬 Cargo 실행을 피하고 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다.

## 다음 EPIC handoff

```text
M8 UI shell read-only view를 별도 PR로 설계한다. UI는 API read-only service 계약을 소비하고 provider process, Star Sentinel rule, StateStore file mutation을 직접 구현하지 않는다.
```
