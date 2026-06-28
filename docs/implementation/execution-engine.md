# ExecutionEngine 구현 계약

## 목적

ExecutionEngine은 WorkSpec을 실제 provider 실행으로 연결하는 계층이다. 이 계층은 provider-neutral하게 동작해야 하며, provider-specific 세부사항은 ProviderAdapter 내부에 격리한다.

## 책임

ExecutionEngine이 담당하는 것:

- WorkSpec 로딩
- ProviderInstance 선택 결과 확인
- ProviderAdapter 호출
- provider output directory 준비
- request/response artifact 저장 확인
- timeout/cancel/retry 정책 적용
- provider result를 ReportSpec과 RunState에 반영
- event ledger append

ExecutionEngine이 담당하지 않는 것:

- route 판단
- validation rule 판정
- Star Sentinel 내부 구현
- provider-specific credential 관리
- UI 표시

## 입력

```text
JobSpec
RunState
WorkSpec
ProviderInstance
ProviderAdapter
StateStore
```

## 출력

```text
ProviderRunResult
provider-output/{provider-instance-id}/ artifacts
updated RunState
LedgerEvent
stage ReportSpec 후보
```

## 실행 흐름

```text
1. WorkSpec 읽기
2. provider assignment 확인
3. provider capability 확인
4. output directory 준비
5. request.json 작성
6. ProviderAdapter.execute 호출
7. response.json / stdout / stderr / artifacts 수집
8. ProviderRunResult 검증
9. RunState 업데이트
10. LedgerEvent append
11. ReportSpec 초안 생성
```

## output directory

```text
.ai-runs/J-0001/provider-output/{provider-instance-id}/
  request.json
  response.json
  stdout.txt
  stderr.txt
  logs/
  artifacts/
```

ExecutionEngine은 provider output을 이 경로로 제한한다.

## request.json

ProviderAdapter에 전달한 요청 snapshot을 저장한다.

후보 필드:

```text
schema_version
job_id
stage
provider_instance_id
workspec
created_at
```

## response.json

ProviderAdapter가 반환한 결과 summary를 저장한다.

후보 필드:

```text
schema_version
job_id
stage
provider_instance_id
status
summary
changed_files
artifacts
started_at
finished_at
error
```

## status

Provider run status 후보:

```text
success
failed
blocked
cancelled
timeout
error
```

상태 mapping:

- `success` -> 해당 stage 완료 상태
- `failed` -> `FAILED`
- `blocked` -> `BLOCKED`
- `cancelled` -> `CANCELLED`
- `timeout` -> `FAILED` 또는 retry 후보
- `error` -> `FAILED`

## timeout 정책

초기 구현:

- timeout 값은 provider instance 또는 default config에서 읽는다.
- timeout 발생 시 provider adapter에 cancel을 요청할 수 있다.
- timeout event를 events.jsonl에 남긴다.

장기 구현:

- stage별 timeout
- provider별 timeout
- retry budget
- daemon-level cancellation

## retry 정책

초기 구현은 자동 retry를 하지 않아도 된다.

장기 구현 시 조건:

- provider transport error만 제한적으로 retry
- deterministic failure는 retry하지 않음
- retry 횟수와 이유를 events.jsonl에 기록
- retry로 인해 duplicate file write가 생기지 않아야 함

## cancel 정책

cancel 요청이 들어오면:

1. RunState next_action을 cancel로 표시
2. provider adapter cancel 지원 여부 확인
3. cancel 가능한 경우 cancel 호출
4. result를 `CANCELLED`로 반영
5. event append

Provider가 cancel을 지원하지 않으면 status에 명확히 기록한다.

## idempotency

ExecutionEngine은 같은 stage를 재실행할 때 기존 artifact를 무시하거나 덮어쓰지 않는다.

권장 방식:

```text
provider-output/{provider-instance-id}/attempt-0001/
provider-output/{provider-instance-id}/attempt-0002/
```

초기 구현에서는 attempt directory를 RESERVED로 두고, 동일 stage 재실행을 명시적으로 막아도 된다.

## provider output 검증

ExecutionEngine은 provider result가 최소 계약을 만족하는지 확인한다.

검증 후보:

- required fields 존재
- status enum 확인
- changed_files 배열 확인
- artifact path traversal 차단
- stdout/stderr path가 output directory 내부인지 확인

## event 기록

권장 event:

```text
ARTIFACT_WRITTEN
ERROR_RECORDED
VALIDATION_RECORDED
```

ExecutionEngine은 provider run 시작/완료/실패를 event로 남긴다.

## ReportSpec 연동

ExecutionEngine은 stage 완료 후 ReportSpec 초안을 생성할 수 있다.

포함 후보:

```text
job_id
stage
status
changed_files
commands_run
validation
risks
next_step
artifacts
```

ValidationEngine이 별도 검증을 수행하면 validation 필드는 나중에 보강될 수 있다.

## dangerous action guard

ExecutionEngine은 WorkSpec의 `forbidden_actions`를 확인해야 한다.

금지 후보:

```text
dependency_install
file_delete
test_delete
assertion_weakening
workflow_change
secret_print
external_account_change
release_publish
```

ProviderAdapter가 이를 시도했다는 evidence가 있으면 `BLOCKED` 또는 `FAILED`로 전환한다.

## local command 실행 주의

LocalProcessProvider가 생기기 전까지 ExecutionEngine은 직접 shell command를 실행하지 않는다. 모든 실행은 ProviderAdapter를 통한다.

## error model

ExecutionEngine 오류 후보:

```text
WorkSpecNotFound
ProviderAssignmentMissing
ProviderCapabilityMissing
ProviderExecutionFailed
ProviderTimedOut
ProviderCancelled
ProviderOutputInvalid
ArtifactWriteFailed
ForbiddenActionDetected
```

## 테스트 기준

최소 테스트:

1. FakeProvider WorkSpec 실행 성공
2. provider output directory 생성
3. request.json 작성
4. response.json 작성
5. RunState stage 완료 반영
6. event append
7. provider failure -> FAILED
8. provider blocked -> BLOCKED
9. invalid output path traversal 차단
10. missing provider assignment 오류

## Codex 구현 지시

ExecutionEngine 구현은 StateStore, Schema Validator, ProviderSystem, RouterEngine 기본 구현 이후 진행한다.

ExecutionEngine PR에는 다음을 섞지 않는다.

- 새 cloud provider 구현
- Star Sentinel rule engine 구현
- UI 구현
- daemon 구현
- package manager 또는 dependency 추가
