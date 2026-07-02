# Cloud Provider Policy

## 목적

M6 cloud provider 구현은 실제 외부 호출보다 먼저 credential, privacy handoff, cost metric 계약을 강제한다.

이 문서는 `cloud_cli_agent` + `cli`, `cloud_api_model` + `http` provider의 공통 안전 기준이다. 특정 provider 공식 API/CLI 세부 동작은 실제 transport 구현 PR에서 최신 문서를 다시 확인한다.

## 기본 원칙

- credential raw value는 provider instance, artifact, log, report에 저장하지 않는다.
- cloud API provider는 `credential_ref`로만 인증 정보를 참조한다.
- cloud CLI provider는 `credential_ref` 또는 `transport_config.auth_mode: login_session`을 명시해야 한다.
- cloud handoff는 실행 전에 `privacy-handoff.json`으로 context 전달 범위를 남긴다.
- cost/budget은 provider-neutral `cost-metric.json`으로 남긴다.
- 실제 외부 API 호출, 유료 사용, 외부 계정 변경은 사용자 승인 없이 실행하지 않는다.

## credential policy

허용되는 credential reference 후보:

```text
env:NAME
keychain:NAME
secret-manager:NAME
login-session:NAME
```

금지되는 raw field 후보:

```text
api_key
token
access_token
refresh_token
secret
password
credential
credentials
bearer_token
```

Adapter는 provider instance에서 위 raw field가 문자열 값으로 발견되면 `ProviderRunResult.status=blocked`로 정규화하고 raw 값을 response, stderr, event에 복사하지 않는다.

## privacy handoff

Cloud provider는 실행 전 다음 artifact를 provider output directory에 남긴다.

```text
provider-output/{provider_instance_id}/privacy-handoff.json
```

`privacy-handoff.json`은 `specs/schemas/privacy-handoff.schema.json`을 따른다.

초기 M6a preflight는 `transport_config.privacy_handoff_approved`가 true가 아니면 transport 실행을 차단한다. 이후 approval flow가 cloud handoff approval을 생성하면 이 값을 approval artifact에서 유도할 수 있다.

## cost metric

Cloud provider는 다음 artifact를 provider output directory에 남긴다.

```text
provider-output/{provider_instance_id}/cost-metric.json
```

`cost-metric.json`은 `specs/schemas/cost-metric.schema.json`을 따른다.

M6a preflight는 실제 provider 호출이 없으므로 token과 wall time을 0으로 기록한다. 실제 CLI/API transport 구현 PR은 provider가 반환한 usage, rate limit, quota 정보를 가능한 범위에서 이 계약에 매핑한다.

## M6a preflight scope

M6a는 다음만 구현한다.

- cloud manifest kind/transport를 ExecutionEngine provider selection에 연결한다.
- cloud provider instance raw credential field를 차단한다.
- cloud API `credential_ref` 누락을 차단한다.
- cloud CLI `credential_ref` 또는 login session 선언 누락을 차단한다.
- privacy handoff approval 누락을 차단한다.
- `privacy-handoff.json`, `cost-metric.json`, `response.json`, `stdout.txt`, `stderr.txt`를 `.ai-runs/` 아래에 쓴다.

M6a는 다음을 구현하지 않는다.

- 실제 cloud CLI process 실행
- 실제 HTTP API 호출
- provider별 parser
- paid usage
- external account 변경
- credential raw value 조회

Preflight가 통과해도 M6a에서는 `cloud_provider_transport_not_implemented`로 `BLOCKED` 처리한다. 실제 transport 실행은 다음 M6 slice에서 별도 conformance fixture와 함께 구현한다.

## M6b cloud CLI transport scope

M6b는 `cloud_cli_agent` + `cli` provider만 실행한다.

실행 규칙:

- M6a preflight를 통과하지 못하면 transport를 시작하지 않고 기존 `BLOCKED` response를 쓴다.
- command는 shell 없이 `command.executable`과 `command.args` vector로만 실행한다.
- `cmd`, `powershell`, `pwsh`, `sh`, `bash`, `zsh` 같은 shell wrapper executable은 거부한다.
- cwd는 대상 프로젝트 root다.
- stdout/stderr는 provider output directory의 `stdout.txt`, `stderr.txt`에 capture한다.
- timeout은 `limits.timeout_seconds`를 사용하며, 최대값은 adapter가 제한한다.
- `credential_ref: env:NAME`의 raw value를 `command_policy.env_allowlist`로 그대로 넘기지 않는다.
- `command.args`는 `{{request_path}}`, `{{job_id}}`, `{{stage}}`, `{{goal}}` placeholder를 지원할 수 있다.

M6b 검증은 local test executable fixture만 사용한다. 실제 Codex CLI, Claude Code, Gemini CLI 같은 외부 CLI 호출 검증은 사용자가 승인한 별도 환경에서만 수행한다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI Codex CLI docs를 확인했다.
- 확인 URL: `https://developers.openai.com/codex/cli/reference`, `https://developers.openai.com/codex/cli/features`, `https://developers.openai.com/codex/concepts/sandboxing`
- Codex CLI는 prompt를 command line 인자로 받을 수 있고, sandbox/approval 경계가 별도 설정으로 존재한다.
- Star-Control은 provider-specific flag를 core에 하드코딩하지 않고 provider instance `command` 설정으로 격리한다.

## conformance 방향

M6 전체 exit criteria에는 다음 fixture가 필요하다.

- cloud CLI preflight success + transport execution fixture
- cloud API preflight success + offline request/response parser fixture
- missing credential_ref -> `BLOCKED`
- raw credential field -> `BLOCKED` and no raw value echo
- unapproved privacy handoff -> `BLOCKED`
- cost/rate/budget metric report mapping

## M6c provider output conformance scope

M6c는 외부 provider 호출을 늘리지 않고 provider output 계약을 runtime fixture로 검증한다.

검증 규칙:

- `ProviderExecution`의 `request_ref`, `response_ref`, `stdout_ref`, optional `stderr_ref`가 `provider-output/{provider_instance_id}/` 아래 canonical `/` path를 가리킨다.
- `response.json`의 `stdout_path`, `stderr_path`, `artifacts[]`는 같은 provider instance output directory 안에 있어야 한다.
- backslash, `..`, absolute path, 다른 provider instance path, `tool-output/` 같은 provider-output 밖 경로는 거부한다.
- cloud profile은 `privacy-handoff.json`과 `cost-metric.json` artifact가 누락되면 실패한다.
- checker는 `StateStore`를 통해 대상 프로젝트 `.ai-runs/{job_id}/` 안의 실제 파일 존재를 확인한다.

M6c는 다음을 구현하지 않는다.

- 실제 cloud API 호출
- provider별 stdout/stderr semantic parser
- token usage parser
- live credential lookup
- paid usage validation

## M6d OpenAI-compatible API parser scope

M6d는 `openai_compatible` adapter가 사용할 response parser를 구현한다. 이 단계는 실제 HTTP request/response transport가 아니라 fixture JSON parsing만 다룬다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI official API docs를 확인했다.
- 확인 URL: `https://developers.openai.com/api/reference/resources/responses/methods/create/`, `https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/`, `https://developers.openai.com/api/docs/guides/text`
- Responses API는 text output shortcut인 `output_text`를 제공하지만, `output[]`는 여러 item을 포함할 수 있으므로 `output[0].content[0].text`를 가정하지 않는다.
- Chat Completions response는 `choices[].message.content`와 `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`를 기준으로 파싱한다.

Parser 규칙:

- Responses API는 `output_text`가 있으면 우선 사용한다.
- `output_text`가 없으면 `output[]` 전체를 순회해 `type=output_text` content를 집계한다.
- Chat Completions는 `choices[].message.content`를 집계하고 usage token field를 provider-neutral `input_tokens`, `output_tokens`, `total_tokens`로 매핑한다.
- supported response shape지만 text output이 없으면 parser error로 처리한다.

M6d는 다음을 구현하지 않는다.

- HTTP client
- live API call
- credential lookup
- request signing
- streaming SSE parser
- cost price calculation

## M6e OpenAI-compatible request builder scope

M6e는 `openai_compatible` adapter가 사용할 request builder boundary를 구현한다. 이 단계는 HTTP transport를 실행하지 않고, target endpoint URL과 JSON request body만 만든다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI official Responses API와 Chat Completions API reference를 확인했다.
- 확인 URL: `https://developers.openai.com/api/reference/resources/responses/methods/create/`, `https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/`
- Responses API request는 `model`과 `input`을 기준으로 한다.
- Chat Completions request는 `model`과 `messages`를 기준으로 한다.

Builder 규칙:

- 기본 API는 Responses API이며 `POST {base_url}/responses` body를 만든다.
- `endpoint.api=chat_completions` 또는 `chat-completions`이면 `POST {base_url}/chat/completions` body를 만든다.
- request body는 `model`, prompt input/messages, `stream=false`만 포함한다.
- `credential_ref`, raw credential-like field, environment variable name/value는 request body에 포함하지 않는다.
- `endpoint.model` 누락과 unknown API selector는 builder error로 처리한다.

M6e는 다음을 구현하지 않는다.

- HTTP client
- request signing/header construction
- live credential lookup
- live API call
- streaming SSE parser
- price/cost calculation

## M6f cloud API offline fixture integration scope

M6f는 `cloud_api_model` + `http` provider가 실제 외부 HTTP 호출 없이 request builder와 response parser를 같은 runtime path에서 검증하게 한다. 이 단계는 `transport_config.offline_response_fixture`가 있는 provider instance에서만 실행된다. fixture가 없으면 기존 M6a preflight `cloud_provider_transport_not_implemented` `BLOCKED` 흐름을 유지한다.

실행 규칙:

- `CloudApiOfflineProviderAdapter`는 M6a credential/privacy/cost preflight를 먼저 재사용한다.
- `transport_config.offline_response_fixture`는 대상 프로젝트 root 기준 상대 JSON path만 허용한다.
- absolute path, `..`, `.git`, drive prefix, 빈 path는 거부한다.
- adapter는 `OpenAiCompatibleRequestBuilder`로 prepared request를 만들고 `provider-output/{provider_instance_id}/http-request.json`에 쓴다.
- adapter는 fixture JSON을 `provider-output/{provider_instance_id}/raw-response.json`에 복사하고 `OpenAiCompatibleResponseParser`로 normalized provider `response.json`을 만든다.
- request body, stdout/stderr, response, cost metric에는 credential raw value나 `credential_ref` 값을 포함하지 않는다.
- parsed usage token은 `cost-metric.json`과 provider result metrics에 매핑한다.

M6f output artifact:

```text
provider-output/{provider_instance_id}/request.json
provider-output/{provider_instance_id}/http-request.json
provider-output/{provider_instance_id}/raw-response.json
provider-output/{provider_instance_id}/response.json
provider-output/{provider_instance_id}/stdout.txt
provider-output/{provider_instance_id}/stderr.txt
provider-output/{provider_instance_id}/privacy-handoff.json
provider-output/{provider_instance_id}/cost-metric.json
```

M6f는 다음을 구현하지 않는다.

- live HTTP client execution
- request signing/header construction
- live credential lookup
- streaming SSE parser
- paid API call
- external account mutation

## M6g cloud API transport boundary scope

M6g는 live HTTP transport adapter를 바로 실행하지 않고 future live adapter가 사용할 transport boundary artifact를 고정한다. 이 단계는 M6f offline fixture runtime path에 `http-transport-plan.json`을 추가한다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI official API overview authentication, Responses API, Chat Completions API reference를 확인했다.
- 확인 URL: `https://developers.openai.com/api/reference/overview/`, `https://developers.openai.com/api/reference/resources/responses/methods/create/`, `https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/`
- OpenAI API는 bearer credential을 사용하지만, M6g는 credential raw value를 조회하지 않고 header value를 만들지 않는다.

Transport plan artifact:

```text
provider-output/{provider_instance_id}/http-transport-plan.json
```

Transport plan 규칙:

- method, URL, request API, request body artifact path, raw response artifact path를 기록한다.
- credential은 required/present/reference kind/materialized/value_present 상태만 기록한다.
- full `credential_ref` 문자열과 credential raw value는 기록하지 않는다.
- `Content-Type: application/json`은 literal header policy로 기록할 수 있다.
- `Authorization`은 `deferred_credential_reference` policy로만 기록하고 header value는 만들지 않는다.
- `live_api_call=false`, `approval_required_for_live_call=true`를 기록한다.
- timeout은 provider instance `limits.timeout_seconds` policy를 따른다.

M6g는 다음을 구현하지 않는다.

- live HTTP client execution
- credential raw value lookup
- Authorization header value construction
- retry/rate limit policy
- streaming SSE parser
- paid API call

## M6h cloud API live approval gate scope

M6h는 live HTTP transport adapter를 실행하지 않고, provider instance가 live call 의도를 명시했을 때 approval gate artifact와 `BLOCKED` runtime state를 남긴다. `transport_config.live_api_call_requested=true`는 외부 API 호출 승인이 아니라 approval-required flow를 시작하는 입력 flag다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI official API overview authentication, Responses API, Chat Completions API reference를 확인했다.
- 확인 URL: `https://developers.openai.com/api/reference/overview/`, `https://developers.openai.com/api/reference/resources/responses/methods/create/`, `https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/`
- OpenAI API는 bearer credential을 사용하지만, M6h도 credential raw value를 조회하지 않고 header value를 만들지 않는다.

Live approval artifact:

```text
provider-output/{provider_instance_id}/live-transport-approval.json
```

M6h runtime 규칙:

- `transport_config.live_api_call_requested=true`이고 `offline_response_fixture`가 없으면 provider result는 `blocked`다.
- `http-request.json`, `http-transport-plan.json`, `live-transport-approval.json`, `privacy-handoff.json`, `cost-metric.json`을 기록한다.
- `raw-response.json`은 생성하지 않는다.
- `live-transport-approval.json`은 `credential_lookup`, `authorization_header_value_construction`, `live_http_request`, `paid_api_call`을 approval-required action으로 기록한다.
- full `credential_ref` 문자열과 credential raw value는 기록하지 않는다.
- transport plan은 `execution_mode=live_approval_required`, `raw_response_expected=false`, `live_api_call=false`, `approval_required_for_live_call=true`를 기록한다.
- ExecutionEngine은 provider `blocked` result를 RunState `BLOCKED`로 전이한다.

M6h는 다음을 구현하지 않는다.

- live HTTP client execution
- credential raw value lookup
- Authorization header value construction
- retry/rate limit policy
- streaming SSE parser
- paid API call
