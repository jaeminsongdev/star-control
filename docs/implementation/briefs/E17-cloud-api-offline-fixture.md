# E17 Cloud API Offline Fixture Integration

## 목표

M6f는 `cloud_api_model` + `http` provider가 실제 HTTP client, credential lookup, paid API call 없이 request builder와 response parser를 하나의 runtime path에서 검증하게 한다. 이 단계는 `transport_config.offline_response_fixture`가 명시된 provider instance에서만 실행되며, fixture가 없으면 기존 cloud preflight `BLOCKED` 흐름을 유지한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
E15-openai-compatible-parser.md
E16-openai-compatible-request-builder.md
```

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
live HTTP transport 실행
```

## 입력 artifact

```text
ExecutionRequest.goal
ProviderInstance.endpoint.base_url
ProviderInstance.endpoint.model
ProviderInstance.endpoint.api optional responses/chat_completions selector
ProviderInstance.transport_config.offline_response_fixture project-relative JSON path
OpenAI-compatible response fixture JSON
```

## 출력 artifact

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

## 핵심 TASK

```text
CloudApiOfflineProviderAdapter
ExecutionEngine cloud API provider selection
offline_response_fixture project-relative path guard
prepared request artifact write
raw response fixture artifact write
OpenAI-compatible parser integration
cost metric token mapping from parsed usage
provider conformance fixture
no live API call / no credential raw value assertion
```

## 완료 기준

- `transport_config.offline_response_fixture`가 있는 cloud API provider는 live HTTP 호출 없이 request builder와 response parser를 같은 runtime path에서 실행한다.
- fixture path는 project-relative path만 허용하고 absolute path, `..`, `.git`, drive prefix를 거부한다.
- `http-request.json`에는 method/url/body만 기록하고 credential reference/raw value를 포함하지 않는다.
- `raw-response.json`, normalized `response.json`, privacy handoff, cost metric이 provider output에 기록된다.
- parsed usage token은 `cost-metric.json`과 provider result metrics에 반영된다.
- fixture가 없는 cloud API provider는 기존 preflight `BLOCKED` 흐름을 유지한다.

## 다음 handoff

M6g는 cloud API transport boundary를 별도 PR로 설계한다. 실제 credential lookup, request signing/header construction, live API call, streaming SSE, paid usage는 별도 승인 전까지 실행하지 않는다.
