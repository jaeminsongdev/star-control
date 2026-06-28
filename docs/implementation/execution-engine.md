# ExecutionEngine 구현 계약

## 목적

ExecutionEngine은 WorkSpec을 실제 provider 실행으로 연결하는 계층이다. 이 계층은 provider-neutral하게 동작해야 하며, provider-specific 세부사항은 ProviderAdapter 내부에 격리한다.

## 함께 읽을 문서

```text
state-store.md
artifact-layout.md
artifact-naming.md
provider-system.md
router-engine.md
schema-validator.md
```

## machine-readable contracts

```text
specs/schemas/execution-request.schema.json
specs/schemas/execution-attempt.schema.json
specs/schemas/provider-run-result.schema.json
examples/execution-contracts/execution-request.fake.example.json
examples/execution-contracts/execution-attempt.success.example.json
examples/execution-contracts/fake-provider-response.success.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 책임

ExecutionEngine이 담당하는 것:

- WorkSpec 로딩
- ProviderInstance 선택 결과 확인
- ProviderAdapter 호출
- provider output directory 준비
- ExecutionRequest 작성
- ProviderRunResult 검증
- attempt 기록
- timeout/cancel/retry 정책 적용
- provider result를 ReportSpec과 RunState에 반영
- event ledger append

ExecutionEngine이 담당하지 않는 것:

- route 판단
- validation rule 판정
- Star Sentinel 내부 구현
- provider-specific credential 관리
- UI 표시
- 직접 shell command 실행

## 입력

```text
JobSpec
RunState
WorkSpec
ProviderInstance
ProviderAdapter
StateStore
SchemaValidator
```

## 출력

```text
ExecutionRequest
ExecutionAttempt
ProviderRunResult
provider-output/{provider-instance-id}/ artifacts
updated RunState
CoreEvent
stage ReportSpec 후보
```

## 실행 흐름

```text
1. WorkSpec 읽기
2. provider assignment 확인
3. provider capability 확인
4. output directory 준비
5. ExecutionRequest 생성
6. request.json 작성
7. ExecutionAttempt 생성 또는 갱신
8. ProviderAdapter.execute 호출
9. response.json / stdout / stderr / artifacts 수집
10. ProviderRunResult 검증
11. ExecutionAttempt 완료 처리
12. RunState 업데이트
13. CoreEvent append
14. ReportSpec 초안 생성
```

## output directory

초기 기본 layout:

```text
.ai-runs/J-0001/provider-output/{provider-instance-id}/
  request.json
  response.json
  stdout.txt
  stderr.txt
  logs/
  artifacts/
```

장기 attempt layout 후보:

```text
.ai-runs/J-0001/provider-output/{provider-instance-id}/attempt-0001/
  request.json
  response.json
  stdout.txt
  stderr.txt
  logs/
  artifacts/
```

초기 구현은 attempt directory 없이 기본 layout을 사용할 수 있다. 단, 기존 output이 있으면 조용히 덮어쓰지 않는다.

## ExecutionRequest

ExecutionRequest는 ProviderAdapter에 전달한 요청 snapshot이다.

저장 위치:

```text
provider-output/{provider-instance-id}/request.json
```

필수 필드:

```text
schema_version
request_id
job_id
stage
provider_instance_id
workspec_path
created_at
goal
allowed_scope
forbidden_actions
required_outputs
```

선택 필드:

```text
attempt_id
validation_requirements
context_pack
```

ExecutionRequest는 WorkSpec의 내용을 provider가 읽을 수 있는 실행 요청 형태로 정규화한 것이다.

## ProviderRunResult

ProviderRunResult는 ProviderAdapter가 반환한 normalized result다.

저장 위치:

```text
provider-output/{provider-instance-id}/response.json
```

필수 필드:

```text
schema_version
provider_instance_id
job_id
stage
status
summary
changed_files
artifacts
```

status 후보:

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

## ExecutionAttempt

ExecutionAttempt는 한 번의 provider execution 시도를 요약한다.

초기 최소 필드:

```text
schema_version
attempt_id
job_id
stage
status
```

초기 구현에서는 attempt를 별도 artifact로 저장하지 않고 event와 RunState history에만 반영해도 된다. 다만 retry/attempt directory를 도입하는 PR에서는 `execution-attempt.schema.json`을 확장하고 example을 갱신해야 한다.

attempt id 형식 후보:

```text
attempt-0001
attempt-0002
```

## FakeProviderAdapter 계약

초기 구현 대상이다.

FakeProviderAdapter는 다음을 수행한다.

1. ExecutionRequest를 읽는다.
2. 외부 network나 package manager를 사용하지 않는다.
3. source file을 직접 수정하지 않는다.
4. deterministic ProviderRunResult를 만든다.
5. `request.json`과 `response.json`을 output directory에 둔다.
6. 비용 metric은 0으로 둔다.

성공 response example은 다음을 따른다.

```text
examples/execution-contracts/fake-provider-response.success.example.json
```

## timeout 정책

초기 구현:

- timeout 값은 provider instance 또는 default config에서 읽는다.
- timeout 발생 시 provider adapter에 cancel을 요청할 수 있다.
- timeout event를 events.jsonl에 남긴다.
- 자동 retry는 기본값으로 사용하지 않는다.

장기 구현:

- stage별 timeout
- provider별 timeout
- retry budget
- daemon-level cancellation

## retry 정책

초기 구현은 자동 retry를 하지 않아도 된다.

자동 retry를 도입할 때의 조건:

- transport-level transient error만 제한적으로 retry
- deterministic failure는 retry하지 않음
- retry 횟수와 이유를 events.jsonl에 기록
- retry로 인해 duplicate file write가 생기지 않아야 함
- attempt directory 또는 attempt artifact를 함께 도입

retry를 도입하기 전에는 같은 stage/provider output이 이미 있으면 `ProviderOutputAlreadyExists` 또는 `StageAlreadyExecuted` 계열 오류를 반환한다.

## cancel 정책

cancel 요청이 들어오면:

1. RunState next_action을 cancel로 표시
2. provider adapter cancel 지원 여부 확인
3. cancel 가능한 경우 cancel 호출
4. result를 `CANCELLED`로 반영
5. event append

Provider가 cancel을 지원하지 않으면 status와 report에 명확히 기록한다.

## idempotency

ExecutionEngine은 같은 stage를 재실행할 때 기존 artifact를 무시하거나 덮어쓰지 않는다.

금지:

- 기존 `request.json` 조용히 덮어쓰기
- 기존 `response.json` 조용히 덮어쓰기
- 이전 attempt stdout/stderr를 새 run의 evidence로 재사용
- 실패한 output을 success로 간주

초기 구현에서는 동일 stage 재실행을 명시적으로 막아도 된다.

## provider output 검증

ExecutionEngine은 provider result가 최소 계약을 만족하는지 확인한다.

검증 후보:

- required fields 존재
- status enum 확인
- changed_files 배열 확인
- artifacts 배열 확인
- artifact path traversal 차단
- stdout/stderr path가 output directory 내부인지 확인
- response job_id/stage/provider_instance_id가 request와 일치

## event 기록

권장 event:

```text
PROVIDER_STARTED
PROVIDER_FINISHED
ARTIFACT_WRITTEN
ERROR_RECORDED
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

## forbidden action guard

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
ProviderOutputAlreadyExists
StageAlreadyExecuted
ArtifactWriteFailed
ForbiddenActionDetected
```

## 테스트 기준

최소 테스트:

1. FakeProvider WorkSpec 실행 성공
2. provider output directory 생성
3. request.json 작성
4. response.json 작성
5. ExecutionRequest example schema validation
6. ProviderRunResult example schema validation
7. RunState stage 완료 반영
8. event append
9. provider failure -> FAILED
10. provider blocked -> BLOCKED
11. invalid output path traversal 차단
12. missing provider assignment 오류
13. 기존 output이 있을 때 조용히 덮어쓰지 않음

## Codex 구현 지시

ExecutionEngine 구현은 StateStore, Schema Validator, ProviderSystem, RouterEngine 기본 구현 이후 진행한다.

ExecutionEngine PR에는 다음을 섞지 않는다.

- 새 cloud provider 구현
- Star Sentinel rule engine 구현
- UI 구현
- daemon 구현
- package manager 또는 dependency 추가
