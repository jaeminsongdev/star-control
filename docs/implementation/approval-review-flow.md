# Approval / Review Flow 구현 계약

## 목적

Approval / Review Flow는 자동 진행이 위험한 작업을 멈추고 사람이 판단할 수 있도록 만드는 흐름이다. 이 문서는 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK` decision과 RunState 전이, approval artifact, review pack 생성 기준을 정의한다.

## decision

Star Sentinel gate와 ValidationEngine은 다음 decision을 사용한다.

```text
AUTO_PASS
HUMAN_REVIEW
BLOCK
```

## decision 의미

### AUTO_PASS

자동 진행 가능하다.

조건 후보:

- block diagnostic 없음
- required validation 충족
- approval required change 없음
- changed files와 report 일치
- unverified claims 없음

### HUMAN_REVIEW

사람 확인이 필요하다.

조건 후보:

- dependency change
- schema change
- public API change
- validation evidence 부족
- report와 diff 불일치 의심
- high risk path 변경
- release/profile 관련 변경

### BLOCK

자동 진행 금지다.

조건 후보:

- out-of-scope change
- secret exposure
- test deletion
- validator policy self-change without approval
- dangerous action
- invalid or contradictory tool output

## RunState mapping

```text
AUTO_PASS -> VALIDATED 또는 다음 stage
HUMAN_REVIEW -> WAITING_APPROVAL
BLOCK -> BLOCKED
```

`WAITING_APPROVAL` 상태에서는 새 provider execution을 시작하지 않는다.

## approval artifact layout

```text
.ai-runs/J-0001/approvals/
  approval-request.json
  approval-response.json
```

Star Sentinel output에도 gate decision을 남긴다.

```text
.ai-runs/J-0001/tool-output/star-sentinel/approval.json
```

## approval-request.json

후보 필드:

```text
schema_version
job_id
task_id
decision
reasons
changed_files
risks
diagnostics
review_pack_path
requested_at
requested_by
```

## approval-response.json

후보 필드:

```text
schema_version
job_id
task_id
response
reviewer
responded_at
reason
allowed_next_stage
constraints
```

response 후보:

```text
approved
rejected
needs_changes
cancelled
```

## approval response 처리

- `approved`: constraints를 반영하고 다음 stage로 진행 가능
- `rejected`: `BLOCKED` 또는 `CANCELLED`
- `needs_changes`: `REVIEWING` 또는 `POLISHING` 후보
- `cancelled`: `CANCELLED`

## approval 없는 진행 금지

다음 조건에서는 approval response 없이 진행하면 안 된다.

- RunState가 `WAITING_APPROVAL`
- approval decision이 `HUMAN_REVIEW`
- route가 `requires_user_approval: true`
- approval_required_changes가 존재
- policy profile이 human review를 요구

## ReviewPack 생성 기준

ReviewPack은 다음 경우 생성한다.

- decision이 `HUMAN_REVIEW`
- decision이 `BLOCK`
- user가 review를 요구
- risk가 HIGH 이상
- validation evidence가 부족
- provider output이 불완전
- changed_files가 많거나 scope가 불명확

## review pack 구조

JSON:

```text
review_pack.json
```

Markdown:

```text
review_pack.md
```

JSON은 도구 간 계약이고 Markdown은 사람이 읽는 산출물이다.

## review pack 내용

필수 후보:

```text
summary
decision
changed_files
risks
validations
diagnostics
unverified_claims
questions_for_human
recommended_next_action
```

Markdown에는 다음 섹션을 권장한다.

```text
# Review Pack

## Summary
## Decision
## Changed Files
## Risks
## Validation Evidence
## Diagnostics
## Questions for Human
## Recommended Next Action
```

## human review 질문 작성 원칙

질문은 구체적이어야 한다.

좋은 예:

```text
Was this dependency addition explicitly approved?
Is this schema change backward-compatible with existing examples?
Should this workflow permission change be allowed?
```

나쁜 예:

```text
Looks okay?
Review this.
Any thoughts?
```

## approval constraints

승인자가 조건부 승인을 줄 수 있다.

예시:

```text
approved, but do not modify workflow files
approved, but keep dependency versions unchanged
approved, but split public API change into a separate PR
```

ValidationEngine은 approval constraints를 다음 WorkSpec의 `forbidden_actions` 또는 `allowed_scope`에 반영해야 한다.

## audit trail

approval과 review 관련 event는 `events.jsonl`과 Star Sentinel `ledger.jsonl`에 남긴다.

권장 event:

```text
GATE_DECIDED
REVIEW_PACK_CREATED
ARTIFACT_WRITTEN
ERROR_RECORDED
```

## 위험 변경 유형

approval required 후보:

```text
public_api_change
schema_change
dependency_addition
dependency_version_change
validator_config_change
risk_path_change
file_deletion
workflow_change
release_change
credential_change
```

## BLOCK 우선 원칙

BLOCK 조건이 있으면 HUMAN_REVIEW보다 BLOCK을 우선한다.

예시:

- secret exposure + dependency change -> BLOCK
- out-of-scope file + review question -> BLOCK
- validator policy self-change without approval -> BLOCK

## user-facing report

최종 report는 approval/review 상태를 숨기면 안 된다.

포함 후보:

```text
decision
approval_required
approval_response
review_pack_path
blocked_reason
next_step
```

## 테스트 기준

최소 테스트:

1. HUMAN_REVIEW decision -> approval-request.json 생성
2. approval response 없음 -> 다음 stage 진입 금지
3. approved response -> 다음 stage 가능
4. rejected response -> BLOCKED 또는 CANCELLED
5. BLOCK decision -> review pack 생성
6. AUTO_PASS decision -> approval request 없음
7. approval constraints가 다음 WorkSpec에 반영
8. approval event가 events.jsonl에 기록

## Codex 구현 지시

Approval / Review Flow 구현은 ValidationEngine과 Star Sentinel gate writer가 안정화된 뒤 진행한다. CLI approval command와 UI approval screen은 별도 PR에서 구현한다.
