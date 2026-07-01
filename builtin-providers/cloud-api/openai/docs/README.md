# OpenAI API

Builtin provider manifest for `provider.openai`.

이 provider는 core package가 아니라 manifest/capability로 등록된다.

## M6d parser 기준

- 2026-07-01 기준 OpenAI official Responses API, Chat Completions API, Text generation guide를 확인했다.
- `openai_compatible` parser는 Responses API `output_text`를 우선 사용하고, 없으면 `output[]` 전체를 순회해 `type=output_text` content를 집계한다.
- Chat Completions는 `choices[].message.content`와 usage token fields를 정규화한다.
- 이 README는 parser 기준만 기록한다. HTTP transport, credential lookup, live API call, streaming parser는 별도 M6 slice에서 다룬다.

## M6e request builder 기준

- request builder는 기본적으로 `POST {base_url}/responses` body를 만든다.
- `endpoint.api=chat_completions` 또는 `chat-completions`이면 `POST {base_url}/chat/completions` body를 만든다.
- request body는 `model`, prompt input/messages, `stream=false`만 포함하고 credential reference/raw credential 값은 포함하지 않는다.
- 이 단계는 URL/body fixture만 만들며 HTTP transport와 live API call은 실행하지 않는다.

## M6f offline fixture 기준

- `transport_config.offline_response_fixture`가 있는 provider instance는 live API call 없이 prepared request와 fixture response parse를 같은 runtime path에서 검증한다.
- adapter는 `http-request.json`, `raw-response.json`, normalized `response.json`, `privacy-handoff.json`, `cost-metric.json`을 provider output에 기록한다.
- fixture path는 대상 프로젝트 root 기준 상대 path만 허용한다.
- 이 단계도 HTTP client, live credential lookup, request signing/header construction, paid API call은 실행하지 않는다.

## M6g transport boundary 기준

- 2026-07-01 기준 OpenAI official API overview authentication, Responses API, Chat Completions API reference를 확인했다.
- adapter는 `http-transport-plan.json`에 method, URL, request API, request body artifact path, raw response artifact path, timeout, header policy를 기록한다.
- credential은 reference kind와 materialized/value_present 상태만 기록하고 full `credential_ref` 문자열이나 raw value는 기록하지 않는다.
- `Authorization` header는 `deferred_credential_reference` policy로만 기록하며 header value는 만들지 않는다.
- `live_api_call=false`, `approval_required_for_live_call=true`를 유지한다.
