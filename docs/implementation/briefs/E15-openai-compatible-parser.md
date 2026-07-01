# E15 OpenAI-Compatible API Parser

## 목표

M6d는 `openai_compatible` adapter가 사용할 cloud API response parser를 구현한다. 이 단계는 실제 HTTP transport, credential lookup, paid API call 없이 Responses API와 Chat Completions JSON response fixture를 provider-neutral summary/token metric으로 정규화한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
```

Provider-specific parser 구현 직전 공식 문서를 최신 확인한다. M6d 기준 refresh는 OpenAI 공식 Responses API, Chat Completions API, Text generation guide다.

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
builtin-providers/cloud-api/openai/provider.yaml outputs.parser=openai-compatible-chat
Responses API JSON response fixture
Chat Completions JSON response fixture
usage token fields
```

## 출력 artifact

```text
OpenAiCompatibleResponseParser
OpenAiCompatibleParsedResponse
Responses API parser fixture
Chat Completions parser fixture
missing text failure fixture
```

## 핵심 TASK

```text
Responses API output_text shortcut parse
Responses API output[] message content aggregation without output[0] assumption
Chat Completions choices[].message.content parse
token usage mapping
unsupported/missing text error model
official doc refresh notes
```

## 완료 기준

- parser가 Responses API `output_text`를 우선 사용한다.
- `output_text`가 없으면 `output[]` 전체를 순회해 `type=output_text` content를 집계한다.
- parser가 Chat Completions `choices[].message.content`와 usage를 정규화한다.
- text가 없는 supported response shape는 실패로 반환한다.
- 실제 외부 API 호출, credential lookup, HTTP transport는 구현하지 않는다.

## 다음 handoff

M6e는 cloud API request builder 또는 HTTP transport boundary를 별도 PR로 구현한다. 실제 cloud API call, paid usage, credential raw value access는 별도 승인 전까지 실행하지 않는다.
