# E20 CLI Control Commands

## 목표

M7a는 daemon/API control plane에 들어가기 전 CLI file-based `approve`, `cancel`, `resume` 계약을 안정화한다. 이 단계는 StateStore의 `.ai-runs/` artifact를 canonical source로 유지하면서 approval response, cancellation state, resume precondition을 검증한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cli-command-reference.md
cli-daemon-api-ui.md
approval-review-flow.md
validation-engine.md
state-store.md
daemon-contract.md
api-contract.md
```

## 허용 파일

```text
packages/star-control-cli/**
docs/implementation/**
docs/operations/**
PLANS.md
```

## 금지 파일

```text
daemon process 구현
API server 구현
UI 구현
새 dependency
Cargo 외 package manager
GitHub workflow
schema field 변경
release/deploy/publish automation
```

## 입력 artifact

```text
.ai-runs/{job_id}/run-state.json
.ai-runs/{job_id}/events.jsonl
.ai-runs/{job_id}/approvals/approval-request.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
specs/schemas/cli-output.schema.json
specs/schemas/cli-error.schema.json
```

## 출력 artifact

```text
.ai-runs/{job_id}/approvals/approval-response.json
.ai-runs/{job_id}/run-state.json
.ai-runs/{job_id}/events.jsonl
```

## 핵심 TASK

```text
CLI approve dispatch
approval request presence check
approval response schema validation
approval response artifact writer
CLI cancel dispatch
terminal state cancel guard
CLI resume dispatch
approval response precondition check
WAITING_APPROVAL -> VALIDATED state transition
schema-valid CLI output/error envelope tests
```

## 완료 기준

- `star-control approve --json`은 `WAITING_APPROVAL` job의 `approval-request.json`을 읽고 `approvals/approval-response.json`을 쓴다.
- approval request가 없으면 schema-valid CLI error를 반환한다.
- `star-control cancel --json`은 non-terminal job을 `CANCELLED`로 전이하고 terminal state cancel을 거부한다.
- `star-control resume --json`은 `WAITING_APPROVAL` job에 approved response가 있어야 gate를 통과시키며, 통과 시 `VALIDATED`와 `next_action=report`를 기록한다.
- 모든 mutation은 대상 프로젝트 `.ai-runs/` 아래 artifact만 변경한다.

## 다음 handoff

M7 daemon queue skeleton을 별도 PR로 설계한다. daemon runtime state는 repository root가 아니라 user config/cache 영역에 두고, API server와 UI 구현은 별도 slice로 유지한다.
