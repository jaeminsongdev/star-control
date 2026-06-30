# ValidationEngine 구현 계약

## 목적

ValidationEngine은 provider 실행 결과가 정책, schema, evidence, 사용자 승인 기준을 만족하는지 검증하는 Star-Control core 계층이다. ValidationEngine은 Star Sentinel rule을 직접 구현하지 않고 builtin tool 계약을 통해 호출한다.

## 함께 읽을 문서

```text
validation-handoff.md
approval-review-flow.md
star-sentinel-p0-contracts.md
star-sentinel-full-spec.md
policy-profiles.md
schema-validator.md
```

## machine-readable contracts

```text
specs/schemas/validation-decision.schema.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
specs/schemas/review-pack-handoff.schema.json
examples/validation-contracts/validation-decision.human-review.example.json
examples/validation-contracts/approval-request.example.json
examples/validation-contracts/approval-response.example.json
examples/validation-contracts/review-pack-handoff.example.json
builtin-tools/star-sentinel/schemas/approval.schema.json
builtin-tools/star-sentinel/schemas/review-pack.schema.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 책임

ValidationEngine이 담당하는 것:

- validation requirement 수집
- provider output과 changed files 확인
- Star Sentinel task artifact 생성
- Star Sentinel `check`, `gate`, `review-pack` 호출 조정
- validation_runs.json 수집
- diagnostics와 approval decision 반영
- ValidationDecision 생성
- ApprovalRequest 생성
- ApprovalResponse 읽기
- ReviewPackHandoff 생성
- RunState를 `VALIDATING`, `VALIDATED`, `WAITING_APPROVAL`, `BLOCKED`, `FAILED`로 전이
- report에 validation 결과와 risks 연결

ValidationEngine이 담당하지 않는 것:

- provider 실행
- source file 직접 수정
- Star Sentinel rule 내부 판정 로직
- 사람이 승인할지 여부 결정
- CI 검사 삭제 또는 약화
- UI approval screen 구현

## 입력

```text
JobSpec
RunState
RouteSpec
WorkSpec
ProviderRunResult
changed files
provider artifacts
validation requirements
Star Sentinel manifest
StateStore
```

## 출력

```text
validation_runs.json
approval.json
validation-decision.json
approval-request.json
approval-response.json
review_pack.json
review_pack.md
handoff.json
updated RunState
CoreEvent
ReportSpec validation section
```

## 기본 흐름

```text
1. RunState를 VALIDATING으로 전이
2. provider output과 changed files 확인
3. Star Sentinel task.json 생성
4. repo_map.json과 changed_lines.json 준비 또는 요청
5. Star Sentinel check 실행
6. diagnostics.json 수집
7. validation_runs.json 기록
8. Star Sentinel gate 실행
9. approval.json 수집
10. ValidationDecision 생성
11. decision에 따라 상태 전이
12. 필요한 경우 review-pack 생성
13. 필요한 경우 ApprovalRequest 생성
14. approval response가 있으면 constraints 반영
15. report에 validation 결과 연결
```

## Star Sentinel 호출 원칙

- core 내부에 Star Sentinel rule을 직접 구현하지 않는다.
- Star Sentinel policy file을 core가 직접 해석하지 않는다.
- core는 tool manifest, command, input artifact, output artifact 계약을 통해 호출한다.
- Star Sentinel output이 schema와 맞지 않으면 validation failure로 처리한다.

## validation requirement

WorkSpec의 `validation_requirements` 또는 RouteSpec의 risk 판단에서 validation requirement가 나온다.

예시:

```text
policy:p0
schema-example-check
unit-test
integration-smoke
manual-review
```

초기 구현은 `policy:p0`와 schema/example 검증 연결부터 안정화한다.

## decision mapping

Star Sentinel approval decision은 ValidationDecision으로 정규화한 뒤 상태로 mapping한다.

```text
AUTO_PASS -> VALIDATED
HUMAN_REVIEW -> WAITING_APPROVAL
BLOCK -> BLOCKED
invalid output -> FAILED
```

ValidationEngine은 route decision 또는 Star Sentinel decision을 더 낮은 위험으로 낮추면 안 된다.

## diagnostics 처리

Diagnostic severity:

```text
info
warn
block
```

처리 기준:

- `info`: report에 기록, 자동 진행 가능
- `warn`: report에 기록, profile에 따라 review 필요 가능
- `block`: gate decision이 BLOCK이면 자동 진행 금지

`block` diagnostic이 있는데 gate가 AUTO_PASS를 반환하면 tool output inconsistency로 보고 `FAILED` 또는 `BLOCKED` 처리한다.

## approval 처리

`HUMAN_REVIEW` 또는 approval required change가 있으면 다음을 생성한다.

```text
.ai-runs/J-0001/approvals/approval-request.json
```

approval response가 없으면 다음 stage를 시작하지 않는다.

ApprovalResponse mapping:

```text
approved -> allowed_next_stage로 진행 가능
rejected -> BLOCKED 또는 CANCELLED
needs_changes -> REVIEWING 또는 POLISHING 후보
cancelled -> CANCELLED
```

approval constraints는 다음 WorkSpec의 `allowed_scope` 또는 `forbidden_actions`에 반영한다.

## review pack handoff

Star Sentinel 원본 output:

```text
tool-output/star-sentinel/review_pack.json
tool-output/star-sentinel/review_pack.md
```

Core canonical copy:

```text
review-packs/review_pack.json
review-packs/review_pack.md
review-packs/handoff.json
```

ValidationEngine은 handoff artifact에 source path와 canonical path를 모두 기록한다.

## validation_runs.json

ValidationEngine은 실행한 validation을 구조화해 기록한다.

각 validation run에는 다음이 포함되어야 한다.

```text
schema_version
validation_id
task_id
command
profile
status
exit_code
started_at
finished_at
evidence
diagnostics
```

## error model

ValidationEngine 오류 후보:

```text
ValidationRequirementUnknown
ProviderOutputMissing
ChangedLinesMissing
StarSentinelUnavailable
StarSentinelOutputInvalid
ApprovalDecisionInvalid
ApprovalResponseInvalid
ReviewPackGenerationFailed
ReviewPackHandoffFailed
ValidationCommandFailed
HumanApprovalRequired
PolicyBlocked
```

## event 기록

권장 event type:

```text
VALIDATION_RECORDED
GATE_DECIDED
APPROVAL_REQUESTED
APPROVAL_RECORDED
REVIEW_PACK_CREATED
ERROR_RECORDED
```

모든 gate decision은 events.jsonl에 남긴다.

## forbidden behavior

ValidationEngine은 다음을 하면 안 된다.

- 실패한 검사를 삭제하거나 성공으로 위장
- test 삭제를 정상 변경으로 처리
- assertion weakening을 무시
- dependency 추가를 approval 없이 통과
- secret exposure diagnostic을 warn으로 낮춤
- Star Sentinel output schema 오류를 무시
- approval response 없이 `WAITING_APPROVAL`에서 다음 stage로 진행

## 테스트 기준

최소 테스트:

1. AUTO_PASS decision -> VALIDATED
2. HUMAN_REVIEW decision -> WAITING_APPROVAL
3. BLOCK decision -> BLOCKED
4. invalid approval.json -> FAILED
5. block diagnostic과 AUTO_PASS 불일치 감지
6. validation_runs.json 생성
7. review_pack 생성
8. review-pack handoff 생성
9. approval-request 생성
10. approval-response approved constraints 반영
11. missing provider output 오류
12. approval response 없이는 다음 stage 진입 금지
13. events.jsonl에 gate decision 기록

## Codex 구현 지시

ValidationEngine 구현은 StateStore, Schema Validator, ProviderSystem, ExecutionEngine 기본 구현 이후 진행한다.

한 PR에 다음을 섞지 않는다.

- Star Sentinel 전체 rule engine 구현
- cloud provider 구현
- daemon 구현
- UI 구현
- package manager 도입
