# E19 Cloud API Live Approval Gate

## 목표

M6h는 cloud API live HTTP transport 실행을 바로 구현하지 않고, live 요청 의도를 explicit flag로 받았을 때 approval-required artifact와 `BLOCKED` runtime state를 남기는 gate를 추가한다. 이 단계는 request preparation과 approval artifact를 검증하되 credential raw value lookup, Authorization header value construction, HTTP client dependency, paid API call은 수행하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
E16-openai-compatible-request-builder.md
E17-cloud-api-offline-fixture.md
E18-cloud-api-transport-boundary.md
```

Provider-specific live transport 구현 직전 공식 문서를 최신 확인한다. M6h 기준 refresh는 OpenAI official API overview authentication, Responses API, Chat Completions API다.

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
ProviderInstance.transport_config.live_api_call_requested=true
OpenAiCompatiblePreparedRequest
ProviderManifest kind/transport/adapter
ProviderInstance.credential_ref prefix only
provider-output/{provider_instance_id}/http-request.json
provider-output/{provider_instance_id}/http-transport-plan.json
```

## 출력 artifact

```text
provider-output/{provider_instance_id}/live-transport-approval.json
provider-output/{provider_instance_id}/response.json
```

## 핵심 TASK

```text
explicit live request flag parsing
approval-required provider result
RunState BLOCKED transition through ExecutionEngine
live-transport-approval artifact
live_api_call=false assertion
credential materialized/value_present=false assertion
raw-response artifact absence assertion
provider conformance coverage
docs handoff to daemon/API control plane
```

## 완료 기준

- `transport_config.live_api_call_requested=true`이고 offline fixture가 없으면 provider result가 `blocked`가 된다.
- `http-request.json`, `http-transport-plan.json`, `live-transport-approval.json`, `privacy-handoff.json`, `cost-metric.json`이 provider output에 기록된다.
- `raw-response.json`은 생성하지 않는다.
- approval artifact는 approval-required actions를 기록하되 full `credential_ref` 문자열과 credential raw value를 기록하지 않는다.
- `live_api_call=false`, `approval_required_for_live_call=true`, credential `materialized=false`, `value_present=false`가 fixture와 test로 고정된다.
- ExecutionEngine state는 `BLOCKED`로 전이한다.

## 다음 handoff

M7 daemon/API control plane을 별도 PR로 설계한다. 실제 credential lookup, Authorization header value construction, HTTP client dependency, paid usage, streaming SSE는 별도 승인 전까지 계속 보류한다.
