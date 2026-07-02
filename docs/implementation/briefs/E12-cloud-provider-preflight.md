# E12 Cloud Provider Preflight

## 목표

M6 cloud provider 구현을 시작하기 전에 credential, privacy handoff, cost metric 계약을 runtime 경로에 연결한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
security-cost-observability.md
security-privacy-observability-contracts.md
artifact-layout.md
```

## 허용 파일

```text
packages/star-control-provider/**
packages/star-control-execution/**
configs/provider-instances/*.example.yaml
docs/implementation/**
PLANS.md
```

## 금지 파일

```text
Cargo 외 package manager
새 dependency
GitHub workflow
release/deploy/publish automation
실제 cloud API 호출
실제 paid CLI/API 실행
credential raw value 저장
```

## 입력 artifact

```text
specs/schemas/provider-instance.schema.json
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
configs/provider-instances/*api.example.yaml
configs/provider-instances/*cli.example.yaml
```

## 출력 artifact

```text
CloudProviderPreflightAdapter
privacy-handoff.json
cost-metric.json
cloud provider BLOCKED response for unsafe/preflight-only states
execution-level preflight fixture
```

## 핵심 TASK

```text
cloud manifest kind/transport detection
raw credential field guard
cloud API credential_ref required check
cloud CLI credential_ref or login_session check
privacy handoff approval check
provider-output sidecar artifact writer
ExecutionEngine cloud provider selection
unit and execution fixture tests
```

## 완료 기준

- cloud provider instance가 raw credential field를 포함하면 raw value echo 없이 `BLOCKED` 처리한다.
- cloud API provider는 `credential_ref` 없이는 실행되지 않는다.
- cloud CLI provider는 `credential_ref` 또는 login session auth 선언 없이는 실행되지 않는다.
- cloud provider preflight는 `privacy-handoff.json`과 `cost-metric.json`을 provider output directory에 저장한다.
- `ExecutionEngine` 경로에서 cloud preflight 결과가 RunState `BLOCKED`, artifact refs, `PROVIDER_FINISHED` event로 이어진다.

## 다음 handoff

M6b는 실제 cloud CLI transport execution을 별도 PR로 구현한다. 구현 전 해당 CLI 공식 문서를 최신 확인하고, 실제 외부 서비스 호출이 필요한 검증은 사용자 승인을 받아야 한다.

