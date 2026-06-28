# Provider System 구현 계약

## 목적

Provider System은 Star-Control이 다양한 실행 주체를 공통 계약으로 다루게 하는 계층이다. provider는 제품명이 아니라 `kind`, `transport`, `adapter`, `capability_profile`, `provider_instance`로 판단한다.

## 핵심 원칙

- Core package 이름에는 특정 provider 제품명을 넣지 않는다.
- 새 provider 추가는 가능하면 core 수정 없이 manifest와 adapter 추가로 처리한다.
- provider credential 값은 repository나 artifact에 저장하지 않는다.
- provider output은 `.ai-runs/J-0001/provider-output/{provider-instance-id}/` 아래에 저장한다.
- 초기 구현은 FakeProviderAdapter부터 시작한다.

## provider 구성 요소

```text
ProviderManifest
CapabilityProfile
ProviderInstance
ProviderAdapter
ProviderOutput
```

## ProviderManifest

ProviderManifest는 builtin provider 또는 plugin provider의 정적 정의다.

후보 위치:

```text
builtin-providers/{provider-id}/provider.yaml
```

필드 후보:

```text
id
name
kind
transport
adapter
capability_profile
commands
inputs
outputs
risk_notes
```

## provider kind

지원 후보:

```text
fake
human
local_process
local_model
cloud_api
cloud_cli
remote_agent
```

### `fake`

테스트와 smoke용 provider다. 외부 호출 없이 deterministic output을 생성한다.

### `human`

사람 승인, 수동 리뷰, handoff를 표현한다.

### `local_process`

대상 프로젝트에서 로컬 명령을 실행한다.

### `local_model`

로컬 모델 서버 또는 로컬 AI runtime을 호출한다.

### `cloud_api`

cloud API model provider를 호출한다.

### `cloud_cli`

cloud AI CLI agent를 호출한다.

### `remote_agent`

원격 agent나 다른 machine의 worker를 호출한다.

## transport

transport 후보:

```text
none
stdio
process
http
websocket
file_handoff
human_handoff
```

- `none`: fake provider처럼 외부 transport 없음
- `stdio`: CLI와 stdin/stdout으로 통신
- `process`: command spawn 기반
- `http`: HTTP API 기반
- `websocket`: streaming 또는 long-running session
- `file_handoff`: 파일 기반 handoff
- `human_handoff`: 사람 승인/입력 기반

## CapabilityProfile

CapabilityProfile은 provider가 할 수 있는 일을 선언한다.

후보 capability:

```text
read_context
write_files
run_commands
run_tests
analyze_diff
review_code
produce_patch
produce_report
ask_human
stream_output
resume_session
```

RouterEngine은 provider 이름이 아니라 capability를 보고 assignment해야 한다.

## ProviderInstance

ProviderInstance는 특정 실행에서 사용할 provider 설정이다.

필드 후보:

```text
instance_id
provider_id
profile
transport_config
capability_overrides
budget
limits
```

credential은 직접 저장하지 않는다. credential reference만 허용한다.

## ProviderAdapter interface

권장 기능 단위:

```text
prepare(request, context) -> PreparedProviderRun
execute(prepared_run) -> ProviderRunResult
cancel(run_id) -> CancelResult
collect(run_id) -> ProviderRunResult
healthcheck(instance) -> HealthResult
```

초기 구현은 아래만 있어도 된다.

```text
execute(workspec, provider_instance, output_dir) -> ProviderRunResult
```

## ProviderRunResult

필드 후보:

```text
schema_version
provider_instance_id
job_id
stage
status
started_at
finished_at
stdout_path
stderr_path
artifacts
changed_files
summary
error
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

## FakeProviderAdapter

초기 구현 대상이다.

역할:

- WorkSpec을 읽는다.
- 외부 명령이나 network 호출 없이 deterministic response를 만든다.
- provider output directory에 `request.json`, `response.json`을 쓴다.
- 필요하면 dummy changed_files와 summary를 만든다.

금지:

- 실제 source file 수정 금지
- package manager 실행 금지
- network 호출 금지
- hidden dependency 사용 금지

테스트 기준:

1. FakeProvider 실행 시 output directory 생성
2. request.json 저장
3. response.json 저장
4. deterministic output 보장
5. 실패 simulation 가능

## LocalProcessProvider

RESERVED. 장기 구현 대상이다.

역할:

- 대상 프로젝트에서 허용된 command만 실행
- stdout/stderr capture
- exit code 기록
- timeout/cancel 지원

금지:

- approval 없이 dependency install
- approval 없이 delete/move 대량 작업
- approval 없이 test weakening

## LocalModelProvider

RESERVED. 장기 구현 대상이다.

역할:

- 로컬 model server 호출
- prompt/context pack 전달
- response artifact 저장

## CloudApiProvider

RESERVED. 장기 구현 대상이다.

역할:

- cloud API model 호출
- 비용/budget 추적
- rate limit 처리
- credential reference 사용

## CloudCliProvider

RESERVED. 장기 구현 대상이다.

역할:

- cloud AI CLI agent 실행
- session 관리
- stdout/stderr/log capture
- 작업 단위 제한

## HumanProvider

RESERVED. approval과 manual review에 사용한다.

역할:

- approval request 생성
- human response 대기
- 승인/거절 결과를 artifact로 저장

## provider output 저장 규칙

```text
provider-output/{provider-instance-id}/
  request.json
  response.json
  stdout.txt
  stderr.txt
  logs/
  artifacts/
```

ProviderAdapter는 이 경로 밖에 쓰면 안 된다.

## error 처리

Provider 오류 유형 후보:

```text
ProviderNotFound
ProviderInstanceInvalid
CapabilityMissing
TransportFailed
ProviderTimeout
ProviderCancelled
ProviderReturnedInvalidJson
ProviderOutputMissing
BudgetExceeded
ApprovalRequired
```

## budget guard

ProviderSystem은 다음 정보를 기록할 수 있어야 한다.

```text
input_tokens
output_tokens
wall_time_ms
estimated_cost
quota_remaining
```

초기 fake provider는 비용을 0으로 기록한다.

## security guard

- credential raw value 저장 금지
- provider stdout/stderr에서 secret 노출 가능성을 report에 위험으로 남김
- dangerous command는 approval 없이 실행 금지
- workflow, dependency, schema, public API 변경은 Router/Validation 계층에서 approval 후보로 표시

## Codex 구현 지시

ProviderSystem 구현은 다음 순서로 진행한다.

1. ProviderManifest loader
2. ProviderInstance loader
3. ProviderAdapter interface
4. FakeProviderAdapter
5. provider output writer
6. provider result schema/example
7. integration smoke

CodexProviderAdapter, cloud provider, local process provider는 FakeProvider flow가 통과한 뒤 별도 PR로 구현한다.
