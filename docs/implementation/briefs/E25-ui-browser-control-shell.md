# E25 UI Browser Control Shell

## 목표

M8b의 목표는 실제 browser app을 만들기 전에 `packages/star-control-ui`에 browser-oriented control shell model을 추가해 UI approval/review flow가 `ApiControlService` 계약을 안전하게 소비할 수 있게 하는 것이다.

이번 slice는 action panel, approve/cancel/resume mutation result view, action enable/disable rule, control API wiring을 library-level로 고정한다. TypeScript/Node package manager, HTTP server, socket listener, remote exposure, auth/session, provider process 실행은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
ui-shell-contract.md
api-contract.md
approval-review-flow.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

## 허용 파일

```text
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
socket listener 구현
remote API exposure
auth/session 시스템 구현
daemon background worker 변경
provider process 실행 구현
Star Sentinel rule 직접 구현
StateStore file 직접 mutation 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

## 입력 artifact

```text
ApiControlService GET read-only endpoint
ApiControlService POST approve/cancel/resume endpoint
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/approvals/approval-request.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/ui-job-view.schema.json
specs/schemas/api-response.schema.json
```

## 출력 artifact

```text
없음
```

UI shell은 Star-Control repository에 실행 산출물을 쓰지 않는다. approve/cancel/resume으로 필요한 `.ai-runs/` mutation은 `ApiControlService`를 통해서만 수행한다.

## 핵심 TASK

```text
UiBrowserShell 추가
browser_control_shell action panel 추가
approve action surface 추가
cancel action surface 추가
resume action surface 추가
ApiControlService handle_get/handle_post 소비
approval response body builder
control mutation result view
terminal cancel disabled surface
approved response 이후 resume enabled surface
ui-job-view schema validation 유지
secret-like result redaction 유지
HTTP/server/package-manager 미도입 regression test
```

## 완료 기준

- `UiBrowserShell`이 `ApiControlService`를 소비해 browser-oriented action panel을 만든다.
- action panel은 approve/cancel/resume endpoint, method, body contract, enable/disable reason을 노출한다.
- approve/cancel/resume은 `ApiControlService`를 통해서만 실행되고 structured result view를 반환한다.
- approval response 이후 resume action이 enabled로 전환된다.
- terminal job cancel은 UI action에서 disabled로 보이고 API structured failure도 result view로 표시한다.
- 모든 job view는 `ui-job-view.schema.json`을 만족한다.
- TypeScript/Node package manager, HTTP server, socket, auth/session, remote exposure는 구현하지 않는다.

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

Cargo incremental finalize 경고가 재발하면, 병렬 Cargo 실행을 피하고 경고 package만 `cargo clean -p <package>`로 정리한 뒤 같은 명령을 순차 재실행한다. 반복되면 현재 PowerShell 명령 범위에서만 `CARGO_INCREMENTAL=0`을 사용한다.

## 다음 EPIC handoff

```text
M9 hardening은 UI/API/CLI/daemon/provider 흐름의 security, audit, conformance, release readiness 검증을 작은 PR로 확장한다. 실제 browser app, HTTP server, auth/session, remote exposure, package manager 도입은 별도 승인 전까지 RESERVED다.
```
