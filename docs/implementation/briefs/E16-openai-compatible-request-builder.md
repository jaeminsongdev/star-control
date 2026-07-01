# E16 OpenAI-Compatible Request Builder

## 목표

M6e는 `openai_compatible` adapter가 사용할 cloud API request builder boundary를 구현한다. 이 단계는 실제 HTTP client, credential lookup, paid API call 없이 Star-Control `ExecutionRequest`와 `ProviderInstance`에서 OpenAI-compatible JSON request body와 endpoint URL만 만든다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
```

Provider-specific request builder 구현 직전 공식 문서를 최신 확인한다. M6e 기준 refresh는 OpenAI 공식 Responses API와 Chat Completions API다.

## 허용 파일

```text
packages/star-control-provider/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

## 금지 파일

```text
Cargo 외 package manager
새 dependency
GitHub workflow
schema field 변경
release/deploy/publish automation
실제 paid CLI/API 호출 검증
credential raw value 저장
live credential lookup
HTTP transport 실행
```

## 입력 artifact

```text
ExecutionRequest.goal
ProviderInstance.endpoint.base_url
ProviderInstance.endpoint.model
ProviderInstance.endpoint.api optional responses/chat_completions selector
```

## 출력 artifact

```text
OpenAiCompatibleRequestBuilder
OpenAiCompatiblePreparedRequest
Responses API request body fixture
Chat Completions request body fixture
credential exclusion fixture
```

## 핵심 TASK

```text
Responses API request body builder
Chat Completions request body builder
base_url + endpoint path normalization
model required validation
unsupported API selector failure
credential_ref/raw credential exclusion from request body
official doc refresh notes
```

## 완료 기준

- builder가 기본적으로 `POST {base_url}/responses` body를 만든다.
- `endpoint.api=chat_completions`이면 `POST {base_url}/chat/completions` body를 만든다.
- request body에는 `model`, prompt input/messages, `stream=false`만 포함하고 credential reference/raw value를 포함하지 않는다.
- `endpoint.model` 누락과 unknown API selector는 실패로 반환한다.
- 실제 HTTP client, credential lookup, 외부 API 호출은 구현하지 않는다.

## 다음 handoff

M6f는 cloud API transport boundary 또는 offline HTTP response fixture integration을 별도 PR로 구현한다. 실제 cloud API call, paid usage, credential raw value access는 별도 승인 전까지 실행하지 않는다.
