# UI Shell Read-Only Contract

## 목적

UI shell은 장시간 작업 관제를 사람이 쉽게 볼 수 있게 하는 장기 surface다. M8a에서는 read-only view model을 `packages/star-control-ui`에 library-level로 구현하고, M8b에서는 browser-oriented control shell model을 같은 crate에 구현한다. 실제 browser app은 별도 승인 전까지 후속 slice로 남긴다.

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

초기 UI는 read-only view model부터 시작한다. 승인/취소/재개 mutation은 API와 CLI 안정화 이후 `UiBrowserShell`이 `ApiControlService`를 통해 수행한다.

## M8a 구현 범위

구현함:

```text
packages/star-control-ui
UiReadOnlyShell
job_list view model
job_detail view model
timeline event view
provider output path viewer data
validation result path viewer data
approval request viewer data
review pack viewer data
ui-job-view schema validation
secret-like value redaction
read-only no-write regression test
```

아직 구현하지 않음:

```text
browser UI app
TypeScript/Node package manager
HTTP server
browser UI mutation wiring
provider process 실행
Star Sentinel rule 직접 구현
StateStore file mutation
```

## M8b 구현 범위

구현함:

```text
UiBrowserShell
browser_control_shell action panel
approve/cancel/resume action surface
ApiControlService consumer wiring
control mutation result view
terminal cancel disabled surface
approved response 이후 resume enabled surface
HTTP/server/package-manager 미도입 regression test
```

아직 구현하지 않음:

```text
browser UI app
TypeScript/Node package manager
HTTP server
socket listener
remote API exposure
auth/session
provider process 실행
Star Sentinel rule 직접 구현
StateStore file 직접 mutation
```

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

Approval mutation은 `ApprovalResponse` 계약을 통해 API 또는 CLI로 전달한다. M8b library-level browser shell은 API control service endpoint와 body contract를 노출하고, 실제 mutation은 `ApiControlService`를 통해서만 수행한다.

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
6. missing report 같은 선택 artifact는 read-only error surface로 표시함
7. browser control shell은 HTTP server나 package manager 없이 approve/cancel/resume action result를 생성함
8. terminal job cancel은 disabled로 표시되고 structured failure result를 유지함
