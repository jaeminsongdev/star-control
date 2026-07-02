# E21 Daemon Queue Skeleton

## 목표

M7b의 목표는 daemon process를 시작하기 전에 file-based daemon queue state를 구현하고, 대상 project `.ai-runs/` job을 안전 조건 통과 시 queue entry로 참조 등록하는 것이다.

이번 slice는 daemon runtime state와 project artifact 경계를 고정한다. background process, socket, HTTP API server, UI shell, provider scheduling worker는 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
daemon-contract.md
api-contract.md
state-store.md
approval-review-flow.md
testing-ci-release.md
```

## 허용 파일

```text
Cargo.toml
packages/star-control-daemon/**
docs/implementation/**
docs/operations/**
PLANS.md
```

## 금지 파일

```text
daemon background process 구현
socket 또는 HTTP API server 구현
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
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/daemon-state.schema.json
specs/schemas/run-state.schema.json
specs/schemas/approval-response.schema.json
```

## 출력 artifact

```text
{config_root}/daemon/state.json
```

대상 project `.ai-runs/` artifact는 복사하지 않는다. queue entry는 project root와 `.ai-runs/{job_id}` 상대 경로만 참조한다.

## 핵심 TASK

```text
star-control-daemon crate 추가
DaemonConfig와 DaemonQueue 추가
config_root/daemon/state.json 생성 및 schema validation
non-terminal job queue entry 등록
terminal state queue 거부
WAITING_APPROVAL approval-response precondition
non-approved approval-response queue 거부
duplicate queue entry guard
project artifact 미복사 regression test
```

## 완료 기준

- daemon state가 Star-Control repository root나 대상 project root가 아니라 caller가 넘긴 config root 아래에 생성된다.
- queue entry가 `job_id`, `project_root`, `run_dir`, `run_state`, `current_stage`, `priority`, `state`를 포함한다.
- terminal job은 queue에 등록되지 않는다.
- `WAITING_APPROVAL` job은 approved `approval-response.json` 없이는 queue에 등록되지 않는다.
- 대상 project `.ai-runs/` artifact를 daemon directory로 복사하지 않는다.

## 검증 명령

```text
cargo fmt --check
cargo test -p star-control-daemon -- --nocapture
cargo clippy -p star-control-daemon --all-targets -- -D warnings
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

## 다음 EPIC handoff

```text
M7c API read-only endpoint를 별도 PR로 설계한다. API는 daemon queue state와 StateStore artifact를 read-only로 노출하고, mutation endpoint는 이후 slice까지 구현하지 않는다.
```
