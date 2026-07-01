# Provider Reference Snapshots

## 목적

이 문서는 builtin provider manifest에 적힌 capability가 어떤 외부 자료에 근거하는지 추적하기 위한 snapshot이다. provider 문서는 자주 바뀌므로, manifest에 적힌 값과 공식 문서 확인 상태를 분리한다.

## 상태 표기

| 상태 | 의미 |
|---|---|
| `verified` | 공식 문서에서 현재 확인한 내용이다. |
| `reserved` | 장기 구현을 위해 예약했지만 아직 구현 계약으로 확정하지 않는다. |
| `assumed` | 현재 manifest 또는 제품 성격상 추정했지만 공식 근거를 더 확인해야 한다. |
| `needs_refresh` | 공식 문서 재확인이 필요하다. |

## 공통 원칙

- 이 문서는 구현 근거를 남기는 snapshot이다.
- 이 문서만 보고 provider adapter를 구현하지 않는다.
- adapter 구현 전에는 해당 provider의 최신 공식 문서를 다시 확인한다.
- credential raw value, 개인 token, 실제 계정 설정은 이 문서에 쓰지 않는다.
- manifest 변경은 provider schema, example, CI 검증과 함께 진행한다.

## 공식 자료 snapshot

| provider | manifest id | 현재 kind | transport | adapter | 확인 상태 | 참고 |
|---|---|---|---|---|---|---|
| OpenAI API | `provider.openai` | `cloud_api_model` | `http` | `openai_compatible` | `verified` | Responses API 기준. |
| Anthropic API | `provider.anthropic` | `cloud_api_model` | `http` | `chat_model` | `needs_refresh` | Messages API URL만 확인. 세부 필드는 adapter 전 재확인. |
| Google Gemini API | `provider.google-gemini` | `cloud_api_model` | `http` | `chat_model` | `verified` | Gemini API text/streaming 기준. |
| Codex CLI | `provider.codex-cli` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | CLI flags/session/output은 adapter 전 재확인. |
| Claude Code | `provider.claude-code` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | CLI flags/session/output은 adapter 전 재확인. |
| Gemini CLI | `provider.gemini-cli` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | CLI flags/session/output은 adapter 전 재확인. |
| Cursor | `provider.cursor` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | CLI/agent 기능은 제품 변화가 크므로 보수적으로 둔다. |
| GitHub Copilot | `provider.github-copilot` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | CLI/agent 기능은 별도 확인 필요. |
| Jules | `provider.jules` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | 원격 agent 성격은 별도 확인 필요. |
| Devin | `provider.devin` | `cloud_cli_agent` | `cli` | `code_agent` | `needs_refresh` | 원격 agent 성격은 별도 확인 필요. |
| Ollama | `provider.ollama` | `local_openai_compatible_server` | `http` | `openai_compatible` | `verified` | local API, streaming, JSON/schema output 기준. |
| vLLM | `provider.vllm` | `local_openai_compatible_server` | `http` | `openai_compatible` | `needs_refresh` | 공식 문서 URL은 확인했으나 이 snapshot에서는 세부 capability 보류. |
| LM Studio | `provider.lm-studio` | `local_openai_compatible_server` | `http` | `openai_compatible` | `verified` | OpenAI compatibility endpoint 기준. |
| llama.cpp server | `provider.llama-cpp-server` | `local_openai_compatible_server` | `http` | `openai_compatible` | `verified` | server README의 OpenAI-compatible routes 기준. |
| Local process | `provider.local-process` | `local_process_model` | `process` | `chat_model` | `reserved` | Star-Control 자체 policy가 우선. |
| llama.cpp process | `provider.llama-cpp` | `local_process_model` | `process` | `chat_model` | `reserved` | process adapter 구현 전 command contract 필요. |
| Custom runner | `provider.custom-runner` | `local_process_model` | `process` | `chat_model` | `reserved` | 사용자 정의 runner contract 필요. |
| Fake Provider | `provider.fake` | `fake_provider` | `manual` | `code_agent` | `verified` | repository 내부 smoke용 provider. |
| Human Handoff | `provider.human-handoff` | `human_handoff` | `manual` | `code_agent` | `reserved` | approval/review flow와 함께 구현. |

## Source notes

### OpenAI API

확인 URL:

```text
https://developers.openai.com/api/reference/resources/responses/methods/create/
https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/
https://developers.openai.com/api/docs/guides/text
https://developers.openai.com/api/reference/overview/
```

확인한 내용:

- 2026-07-01 기준 공식 문서를 refresh했다.
- Responses API는 text/image input과 text/JSON output을 다룬다.
- `POST /v1/responses` endpoint가 있다.
- streaming은 `stream: true`로 server-sent events를 사용한다.
- tools, function calling, structured outputs, conversation state가 문서화되어 있다.
- Responses API text output은 SDK convenience field인 `output_text`로 집계될 수 있지만, `output[]`는 여러 item을 포함할 수 있으므로 first item에 text가 있다고 가정하지 않는다.
- Chat Completions response는 `choices[].message.content`와 usage token fields를 제공한다.
- Request builder는 Responses API `model` + `input`, Chat Completions `model` + `messages`를 fixture 기준으로 사용한다.
- M6f offline fixture integration은 `transport_config.offline_response_fixture`에서 읽은 raw response fixture를 `raw-response.json`으로 복사하고 parser 결과를 normalized `response.json`과 `cost-metric.json` token usage에 반영한다.
- M6g transport boundary는 bearer credential requirement를 credential reference kind와 deferred Authorization header policy로만 표현하며 raw credential value나 full credential reference string을 materialize하지 않는다.
- Star-Control manifest의 `transport: http`, `adapter: openai_compatible`, `structured_output: true`는 현재 snapshot에서 타당하다.

보류:

- 실제 model list, price, rate limit, background response, cancel behavior는 adapter 구현 직전 재확인한다.
- 실제 HTTP transport, credential lookup, request signing/header construction, streaming SSE parser는 별도 M6 slice로 구현한다.

### Anthropic API

확인 URL:

```text
https://platform.claude.com/docs/en/api/messages
```

확인한 내용:

- Messages API 문서 URL은 확인했다.

보류:

- streaming, tool use, usage fields, response schema는 adapter 구현 직전 다시 확인한다.
- 현재 manifest는 `adapter: chat_model`로 유지한다.

### Google Gemini API

확인 URL:

```text
https://ai.google.dev/gemini-api/docs/text-generation
```

확인한 내용:

- Gemini API 문서에는 REST example이 있다.
- streaming은 `stream: true` 또는 REST `alt=sse` 방식으로 설명되어 있다.
- multimodal input과 multi-turn interaction도 문서화되어 있다.
- Star-Control manifest의 `transport: http`는 타당하다.

보류:

- structured output, tool use, exact response schema는 adapter 구현 직전 재확인한다.

### Ollama

확인 URL:

```text
https://github.com/ollama/ollama/blob/main/docs/api.md
```

확인한 내용:

- API endpoint 목록이 있다.
- `/api/generate`는 streaming endpoint다.
- `stream: false`로 single response를 받을 수 있다.
- `format: json`과 JSON schema 기반 structured output이 문서화되어 있다.
- token/duration metrics가 final response에 포함된다.

보류:

- OpenAI-compatible endpoint 사용 여부와 path는 adapter 구현 직전 최신 문서로 확인한다.

### LM Studio

확인 URL:

```text
https://lmstudio.ai/docs/developer/openai-compat
```

확인한 내용:

- OpenAI-compatible endpoints가 문서화되어 있다.
- `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/completions`, `/v1/models`가 명시되어 있다.
- OpenAI client의 base URL을 local LM Studio server로 바꿔 재사용할 수 있다고 설명한다.
- Codex support도 `/v1/responses` 구현을 근거로 언급된다.

보류:

- tool use, structured output, token metrics의 세부 field는 adapter 구현 직전 재확인한다.

### llama.cpp server

확인 URL:

```text
https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
```

확인한 내용:

- llama.cpp HTTP server는 LLM REST APIs와 web UI를 제공한다.
- OpenAI-compatible chat completions, responses, embeddings routes가 기능 목록에 있다.
- Anthropic Messages API compatible chat completions도 언급된다.
- schema-constrained JSON response format과 tool use가 기능 목록에 있다.

보류:

- endpoint별 exact request/response schema는 adapter 구현 직전 재확인한다.

### vLLM

확인 URL:

```text
https://docs.vllm.ai/en/latest/serving/openai_compatible_server/
```

확인한 내용:

- OpenAI-compatible server 문서 URL은 확인했다.

보류:

- 현재 브라우저 결과에서는 상세 본문을 충분히 확보하지 못했다.
- adapter 구현 전 endpoint, streaming, tool call, structured output support를 다시 확인한다.

## manifest 반영 원칙

Provider manifest에 반영할 수 있는 값:

- 공식 문서에서 확인한 protocol/endpoint family
- repository 내부 fake/human provider 계약
- local process처럼 Star-Control이 직접 통제하는 reserved contract

Provider manifest에 바로 반영하지 않는 값:

- 가격
- quota
- rate limit
- model list
- CLI flag
- session persistence
- hidden output format
- undocumented feature

## 구현 전 refresh checklist

Adapter 구현 PR 전에는 다음을 확인한다.

```text
1. 공식 문서 URL 접근 가능 여부
2. endpoint path
3. auth 방식과 credential reference 형태
4. streaming 지원 여부
5. structured output 또는 JSON mode 지원 여부
6. tool/function call 지원 여부
7. usage/cost/token metric field
8. cancel/resume/session 지원 여부
9. known limitations
10. manifest와 schema/example 갱신 필요 여부
```

## 후속 작업

- `check_provider_contracts.py`에서 registry, manifest, capability profile cross-reference를 검증한다.
- provider별 README에 snapshot status를 링크한다.
- adapter 구현 직전 provider별 detailed conformance fixture를 추가한다.
