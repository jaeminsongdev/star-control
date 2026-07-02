# E18 Cloud API Transport Boundary

## 목표

M6g는 cloud API live transport를 바로 실행하지 않고, future live HTTP adapter가 사용할 transport boundary artifact를 고정한다. 이 단계는 prepared request와 credential reference policy를 `http-transport-plan.json`으로 기록하되, credential raw value lookup, Authorization header value construction, HTTP client execution, paid API call은 수행하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
E15-openai-compatible-parser.md
E16-openai-compatible-request-builder.md
E17-cloud-api-offline-fixture.md
```

Provider-specific transport boundary 구현 직전 공식 문서를 최신 확인한다. M6g 기준 refresh는 OpenAI official API overview authentication, Responses API, Chat Completions API다.

## 허용 파일

```text
packages/star-control-provider/**
packages/star-control-execution/**
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
Authorization header value construction
live HTTP transport 실행
```

## 입력 artifact

```text
OpenAiCompatiblePreparedRequest
ProviderManifest kind/transport/adapter
ProviderInstance.credential_ref prefix only
ProviderInstance.limits.timeout_seconds
provider-output/{provider_instance_id}/http-request.json
```

## 출력 artifact

```text
provider-output/{provider_instance_id}/http-transport-plan.json
```

## 핵심 TASK

```text
transport plan artifact
method/url/request API capture
request body artifact path capture
credential reference kind classification without raw value lookup
header policy declaration without Authorization value construction
timeout capture
live_api_call=false assertion
approval_required_for_live_call=true assertion
offline fixture path integration
docs handoff to live transport approval gate
```

## 완료 기준

- cloud API offline runtime path가 `http-transport-plan.json`을 provider output에 기록한다.
- transport plan은 method, URL, request API, body artifact path, raw response artifact path, timeout, header policy를 기록한다.
- credential은 required/present/reference kind/materialized=false/value_present=false만 기록하고 raw value와 full `credential_ref` 문자열은 기록하지 않는다.
- Authorization header는 `deferred_credential_reference` policy로만 기록하고 header value는 만들지 않는다.
- `live_api_call=false`, `approval_required_for_live_call=true`가 fixture와 test로 고정된다.

## 다음 handoff

M6h는 approval-gated live HTTP transport adapter를 별도 PR로 설계한다. 실제 credential lookup, Authorization header value construction, HTTP client dependency, paid usage, streaming SSE는 별도 승인 전까지 실행하지 않는다.
