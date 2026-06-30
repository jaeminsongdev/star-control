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
ProviderRegistry
ProviderAdapter
ProviderRunResult
ProviderOutput
```

## machine-readable contracts

Provider 관련 schema와 example은 다음을 기준으로 한다.

```text
specs/schemas/provider-kind.schema.json
specs/schemas/provider-manifest.schema.json
specs/schemas/capability-profile.schema.json
specs/schemas/provider-instance.schema.json
specs/schemas/provider-registry.schema.json
specs/schemas/provider-run-result.schema.json
examples/provider-contracts/
```

YAML manifest와 capability profile은 후속 provider contract validator에서 직접 검증한다. 현재 `schema-example-check`는 JSON으로 미러링한 canonical example을 검증한다.

## ProviderManifest

ProviderManifest는 builtin provider 또는 plugin provider의 정적 정의다.

위치:

```text
builtin-providers/{group}/{provider}/provider.yaml
```

필수 필드:

```text
id
name
kind
transport
adapter
capabilities
risk
outputs
```

선택 필드:

```text
endpoint
command
handoff
```

## provider kind

현재 manifest에서 사용하는 canonical kind는 다음이다.

```text
cloud_cli_agent
cloud_api_model
local_openai_compatible_server
local_anthropic_compatible_server
local_process_model
remote_self_hosted_model
fake_provider
human_handoff
```

개념적으로는 다음 그룹으로 볼 수 있다.

| conceptual group | manifest kind 후보 |
|---|---|
| fake | `fake_provider` |
| human | `human_handoff` |
| local process | `local_process_model` |
| local model/server | `local_openai_compatible_server`, `local_anthropic_compatible_server`, `remote_self_hosted_model` |
| cloud API | `cloud_api_model` |
| cloud CLI | `cloud_cli_agent` |
| remote agent | `remote_self_hosted_model` |

RouterEngine은 가능한 한 conceptual group이 아니라 manifest capability와 routing tag를 우선 사용한다. kind는 coarse filter로만 사용한다.

## transport

현재 지원 후보:

```text
cli
http
process
manual
stdio
websocket
file_handoff
```

- `cli`: CLI command 실행 기반
- `http`: HTTP API 기반
- `process`: local process 실행 기반
- `manual`: fake 또는 human handoff 기반
- `stdio`: 장기 stdio transport 후보
- `websocket`: streaming 또는 long-running session 후보
- `file_handoff`: 파일 기반 handoff 후보

## adapter

Adapter는 provider-specific 세부사항을 격리하는 구현 경계다.

현재 manifest에 쓰는 후보:

```text
code_agent
chat_model
openai_compatible
```

장기적으로 adapter 이름은 product name보다 protocol 또는 behavior 중심으로 둔다. 특정 제품명 adapter가 필요하면 core package가 아니라 provider package 또는 manifest 경계에 둔다.

## CapabilityProfile

CapabilityProfile은 provider가 할 수 있는 일을 선언한다.

Manifest 내부 capability:

```text
edit_files
run_shell
read_repo
apply_patch
structured_output
offline
requires_login_session
```

별도 capability profile file:

```text
provider
capability_profile.can.edit_files
capability_profile.can.run_shell
capability_profile.can.read_repo
capability_profile.can.apply_patch
capability_profile.can.return_json
capability_profile.can.work_offline
capability_profile.can.use_tools
capability_profile.routing_tags
```

Capability 값은 `true`, `false`, `partial`, `manual` 같은 boolean 또는 string 값을 허용한다. RouterEngine은 단순 truthiness로만 판단하지 말고 `partial`과 `manual`을 별도로 해석해야 한다.

## ProviderInstance

ProviderInstance는 특정 실행에서 사용할 provider 설정이다.

위치 후보:

```text
configs/provider-instances/*.example.yaml
examples/provider-instances/*.yaml
```

필수 필드:

```text
id
provider
enabled
limits
routing_tags
```

선택 필드:

```text
profile
command
endpoint
transport_config
capability_overrides
credential_ref
budget
```

credential은 직접 저장하지 않는다. reference만 허용한다.

## ProviderRegistry

Builtin provider registry는 provider id와 manifest/capability file path를 연결한다.

위치:

```text
configs/registries/builtin-provider-registry.yaml
```

필수 필드:

```text
schema_version
providers[].id
providers[].manifest
providers[].capabilities
```

후속 validator는 다음을 검사해야 한다.

1. registry id와 provider manifest id가 일치한다.
2. registry capabilities path가 존재한다.
3. capability profile의 provider 값이 registry id와 일치한다.
4. manifest와 capability file이 parse 가능하다.

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

ProviderRunResult는 provider execution의 normalized summary다.

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
metrics
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

ProviderRunResult는 `provider-output/{provider-instance-id}/response.json`의 최소 계약으로 사용할 수 있다.

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
3. ProviderRegistry loader
4. CapabilityProfile loader
5. ProviderAdapter interface
6. FakeProviderAdapter
7. provider output writer
8. provider result schema/example
9. integration smoke

CodexProviderAdapter, cloud provider, local process provider는 FakeProvider flow가 통과한 뒤 별도 PR로 구현한다.
