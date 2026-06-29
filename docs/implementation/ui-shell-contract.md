# UI Shell Reserved Contract

## 목적

UI shell은 장시간 작업 관제를 사람이 쉽게 볼 수 있게 하는 장기 surface다. 초기에는 read-only view model 계약만 고정하고, 실제 UI 구현은 CLI/API 안정화 이후 진행한다.

## machine-readable contracts

```text
specs/schemas/ui-job-view.schema.json
examples/surface-contracts/ui-job-view.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 초기 화면 후보

```text
Job list
Job detail
Run timeline
Provider output viewer
Validation result viewer
Approval request viewer
Review pack viewer
Settings / provider registry
```

초기 UI는 read-only view부터 시작한다. 승인/취소/재개 mutation은 API와 CLI 안정화 이후 추가한다.

## UI Job View

필수 필드:

```text
schema_version
job_id
title
state
current_stage
approval_required
next_action
```

선택 필드:

```text
latest_event
artifacts
```

## long-running UX

UI는 장시간 작업에서 다음 정보를 보여야 한다.

```text
job_id
state
current_stage
active_provider
latest_event
approval_required
blocked_reason
next_action
```

## approval UX

승인 화면 후보:

```text
summary
decision
changed_files
risks
diagnostics
review_pack
questions_for_human
approval buttons
constraints input
```

Approval mutation은 `ApprovalResponse` 계약을 통해 API 또는 CLI로 전달한다.

## 금지 사항

- UI가 provider process를 직접 실행하지 않는다.
- UI가 Star Sentinel rule을 직접 구현하지 않는다.
- UI가 StateStore 파일을 임의로 수정하지 않는다.
- UI가 secret raw value를 표시하지 않는다.
- UI가 approval response 없이 `WAITING_APPROVAL` job을 진행시키지 않는다.

## 테스트 기준

1. UI job view example schema validation
2. UI view model은 secret raw value를 포함하지 않음
3. approval_required true이면 review/approval path를 노출할 수 있음
4. read-only 화면은 StateStore artifact를 수정하지 않음
5. mutation은 API/CLI contract를 통해서만 수행
