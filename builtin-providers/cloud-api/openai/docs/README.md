# OpenAI API

Builtin provider manifest for `provider.openai`.

이 provider는 core package가 아니라 manifest/capability로 등록된다.

## M6d parser 기준

- 2026-07-01 기준 OpenAI official Responses API, Chat Completions API, Text generation guide를 확인했다.
- `openai_compatible` parser는 Responses API `output_text`를 우선 사용하고, 없으면 `output[]` 전체를 순회해 `type=output_text` content를 집계한다.
- Chat Completions는 `choices[].message.content`와 usage token fields를 정규화한다.
- 이 README는 parser 기준만 기록한다. HTTP transport, credential lookup, live API call, streaming parser는 별도 M6 slice에서 다룬다.
