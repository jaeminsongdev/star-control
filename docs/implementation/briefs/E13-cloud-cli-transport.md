# E13 Cloud CLI Transport

## 목표

M6a preflight를 통과한 `cloud_cli_agent` + `cli` provider instance를 실제 CLI process/file handoff transport로 실행한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
security-cost-observability.md
security-privacy-observability-contracts.md
artifact-layout.md
```

Provider-specific behavior를 하드코딩하기 전에 해당 CLI 공식 문서를 최신 확인한다. M6b의 기준 refresh는 OpenAI Codex CLI 공식 문서다.

## 허용 파일

```text
packages/star-control-provider/**
packages/star-control-execution/**
docs/implementation/**
PLANS.md
```

## 금지 파일

```text
Cargo 외 package manager
새 dependency
GitHub workflow
release/deploy/publish automation
실제 paid CLI/API 호출 검증
credential raw value 저장
credential_ref env raw value passthrough
```

## 입력 artifact

```text
M6a CloudProviderPreflightAdapter
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
provider instance command.executable
provider instance command.args
```

## 출력 artifact

```text
CloudCliProviderAdapter
provider-output/{provider_instance_id}/request.json
provider-output/{provider_instance_id}/privacy-handoff.json
provider-output/{provider_instance_id}/stdout.txt
provider-output/{provider_instance_id}/stderr.txt
provider-output/{provider_instance_id}/cost-metric.json
provider-output/{provider_instance_id}/response.json
execution-level cloud CLI fixture
```

## 핵심 TASK

```text
cloud CLI manifest detection
preflight block reuse for unsafe provider instances
command executable/args vector execution
shell wrapper denial
timeout handling
stdout/stderr capture
cost metric wall_time_ms recording
ExecutionEngine cloud CLI selection
provider and execution fixture tests
```

## 완료 기준

- cloud CLI provider가 preflight 통과 시 shell 없이 executable/args vector로 실행된다.
- stdout/stderr가 provider output directory에 저장된다.
- timeout은 `ProviderRunResult.status=timeout`과 RunState `FAILED`로 이어진다.
- successful command는 `ProviderRunResult.status=success`와 RunState `IMPLEMENTED`로 이어진다.
- `credential_ref` env raw value를 command env allowlist로 전달하지 않는다.
- 실제 외부 CLI 호출 검증 없이 local test executable fixture로 transport contract를 검증한다.

## 다음 handoff

M6c는 cloud provider output conformance를 별도 PR로 구현한다. 실제 provider 호출, paid usage, external account mutation은 별도 승인 전까지 실행하지 않는다.
