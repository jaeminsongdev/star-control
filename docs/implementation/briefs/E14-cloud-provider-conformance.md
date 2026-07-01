# E14 Cloud Provider Conformance

## 목표

M6 cloud provider가 생성하는 provider output artifact를 provider-neutral conformance checker로 검증한다. 이 단계는 실제 cloud API/CLI 호출을 늘리지 않고, M6a/M6b가 만든 `response.json`, stdout/stderr, privacy handoff, cost metric 경로와 파일 존재를 계약으로 고정한다.

## 선행 문서

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
testing-ci-release.md
artifact-layout.md
```

## 허용 파일

```text
packages/star-control-provider/**
필요한 최소 docs 업데이트
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
schema field 변경
```

## 입력 artifact

```text
M6a CloudProviderPreflightAdapter
M6b CloudCliProviderAdapter
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
provider-output/{provider_instance_id}/response.json
provider-output/{provider_instance_id}/stdout.txt
provider-output/{provider_instance_id}/stderr.txt
provider-output/{provider_instance_id}/privacy-handoff.json
provider-output/{provider_instance_id}/cost-metric.json
```

## 출력 artifact

```text
ProviderConformanceChecker
ProviderConformanceProfile::Basic
ProviderConformanceProfile::Cloud
cloud CLI provider conformance fixture
artifact path boundary tests
```

## 핵심 TASK

```text
provider output path boundary check
request/response/stdout/stderr artifact ref consistency check
provider-output/{provider_instance_id}/ scope enforcement
cloud privacy-handoff/cost-metric sidecar requirement
artifact file existence check via StateStore
cloud CLI execution fixture conformance assertion
```

## 완료 기준

- conformance checker가 `ProviderExecution`의 result/ref와 `.ai-runs/{job_id}/provider-output/{provider_instance_id}/` 파일 존재를 함께 검증한다.
- cloud profile은 `privacy-handoff.json`과 `cost-metric.json` artifact 누락을 실패로 처리한다.
- backslash, `..`, 다른 provider instance, provider-output 밖 경로를 거부한다.
- cloud CLI fixture가 transport 실행 후 conformance checker를 통과한다.
- schema/example/CI/workflow 계약은 변경하지 않는다.

## 다음 handoff

M6d는 cloud API transport 또는 provider-specific parser를 별도 PR로 구현한다. 실제 provider API 호출, paid usage, external account mutation은 별도 승인 전까지 실행하지 않는다.
