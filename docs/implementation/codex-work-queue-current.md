# Codex Work Queue Current Order

## 목적

이 문서는 계약 고정 이후 Codex가 실제 구현에 들어갈 때 우선 따를 현재 구현 순서를 정리한다. 긴 전체 큐는 `codex-work-queue.md`에 두고, 실제 착수 순서는 이 문서를 우선한다.

## 우선권

- 이 문서는 현재 구현 착수 순서의 최상위 기준이다.
- `codex-work-queue.md`는 장기 backlog이며 이 문서보다 우선하지 않는다.
- `repository-layout.md`는 package 경계와 장기 구조를 설명하지만, 현재 EPIC/TASK 순서는 이 문서를 따른다.
- 각 EPIC의 세부 구현 범위가 관련 구현 문서와 충돌하면, schema/example 계약을 먼저 보존하고 문서 충돌을 별도 PR로 정리한다.

## 공통 완료 기준

- 관련 구현 문서를 먼저 읽는다.
- 허용 파일만 수정한다.
- 새 의존성/package manager는 승인 없이 추가하지 않는다.
- schema/example 계약을 약화하지 않는다.
- tests/CI/policy를 약화하지 않는다.
- 현재 CI를 통과한다.
- approval required 변경은 사람이 승인하기 전까지 실행하지 않는다.
- 완료 보고에 변경 파일, 검증 명령, 검증 결과, 남은 위험, 다음 EPIC/TASK handoff를 남긴다.

## EPIC 항목 형식

각 EPIC은 아래 기준으로 수행한다.

```text
선행 문서
허용 파일
금지 파일
입력 artifact
출력 artifact
핵심 TASK
완료 기준
다음 EPIC handoff
```

## v0 구현 순서

```text
E01 Schema / Runtime Validator
E02 File-based StateStore
E03 Artifact Layout Writer
E04 Provider Registry
E05 FakeProviderAdapter
E06 RouterEngine
E07 ExecutionEngine
E08 CLI read-only + fake run
E09 Star Sentinel P0
E10 ValidationEngine
E11 Integration Smoke
```

## post-v0 현재 구현 순서

```text
E12 Cloud Provider Preflight
E13 Cloud CLI Transport
E14 Cloud Provider Conformance
E15 OpenAI-Compatible API Parser
E16 OpenAI-Compatible Request Builder
E17 Cloud API Offline Fixture Integration
E18 Cloud API Transport Boundary
E19 Cloud API Live Approval Gate
E20 CLI Control Commands
E21 Daemon Queue Skeleton
E22 API Read-Only
E23 UI Read-Only View
E24 API Control Mutations
E25 UI Browser Control Shell
E26 Security Redaction Utility
```

E22 이후 M8 UI, M9 hardening 순서로 작은 PR을 추가한다. E23은 browser app이 아니라 read-only UI view model slice이고, E24는 HTTP server 없는 in-process API control mutation slice다. E25는 browser app이 아니라 ApiControlService를 소비하는 library-level browser control shell slice다. E26은 API/UI가 공유하는 redaction utility와 schema-valid RedactionReport builder slice다. 실제 외부 provider 호출, 유료 사용, credential raw value 접근, workflow/release/deploy 변경은 별도 승인 전까지 실행하지 않는다.

## E01 Schema / Runtime Validator

선행 문서:

```text
schema-validator.md
data-contracts.md
ci-contract-validation.md
```

허용 파일:

```text
packages/star-control-schema/** 또는 선택된 schema package
관련 unit tests
필요한 최소 docs 업데이트
```

금지 파일:

```text
StateStore 구현 파일
ProviderAdapter 구현 파일
RouterEngine 구현 파일
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
Cargo 외 package manager / E01 baseline 밖의 lockfile
```

입력 artifact:

```text
specs/schemas/*.schema.json
builtin-tools/star-sentinel/schemas/*.schema.json
examples/**
scripts/ci/check_schema_examples.py
```

출력 artifact:

```text
runtime schema loader
runtime JSON validator
validation error model
validator unit tests
canonical example validation test 후보
```

핵심 TASK:

```text
schema package scaffold
JSON loader and parse errors
const/enum/type validation
required/properties/items validation
additionalProperties/minLength/pattern validation
validation error model
schema/example fixture tests
```

완료 기준: canonical examples를 runtime validator로 검증할 수 있어야 한다.

다음 EPIC handoff:

```text
E02 StateStore가 저장 전 schema validation에 사용할 validator API 이름과 오류 모델을 보고에 남긴다.
```

## E02 File-based StateStore

선행 문서:

```text
state-store.md
state-store-recovery.md
artifact-layout.md
artifact-naming.md
run-lifecycle.md
schema-validator.md
```

허용 파일:

```text
packages/star-control-state/** 또는 선택된 state package
관련 unit tests
필요한 최소 docs 업데이트
```

금지 파일:

```text
RouterEngine 구현 파일
ProviderAdapter 구현 파일
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
Star Sentinel rule 구현 파일
Cargo 외 package manager / E02 baseline 밖의 lockfile
```

입력 artifact:

```text
specs/schemas/job.schema.json
specs/schemas/run-state.schema.json
specs/schemas/route.schema.json
specs/schemas/workspec.schema.json
specs/schemas/report.schema.json
specs/schemas/event.schema.json
examples/runs/J-0001/
E01 runtime validator API
```

출력 artifact:

```text
job directory 생성 기능
job.json create/load
run-state.json save/load
events.jsonl append/read
route/workspec/report save/load
StateStore error model
```

핵심 TASK:

```text
project root and .ai-runs handling
job id allocation
job.json create/load
run-state.json save/load
events.jsonl append/read
route/workspec/report save/load
path traversal guard
atomic write helper
```

완료 기준: fake project에서 `J-0001` roundtrip이 가능해야 한다.

다음 EPIC handoff:

```text
E03이 사용할 job directory resolver, artifact path resolver, atomic write helper, path traversal guard 사용법을 보고에 남긴다.
```

## E03 Artifact Layout Writer

선행 문서:

```text
artifact-layout.md
artifact-naming.md
state-store.md
security-privacy-observability-contracts.md
release-readiness.md
```

허용 파일:

```text
packages/star-control-state/** 또는 artifact helper package
관련 unit tests
필요한 최소 docs 업데이트
```

금지 파일:

```text
ProviderAdapter 실행 구현
RouterEngine 구현
ExecutionEngine 구현
ValidationEngine 구현
CLI 구현
local/cloud provider 구현
```

입력 artifact:

```text
E02 StateStore path helpers
specs/schemas/artifact-ref.schema.json
examples/core/artifact-ref.example.json
artifact-layout.md
artifact-naming.md
```

출력 artifact:

```text
artifact path resolver
provider-output directory resolver
tool-output directory resolver
approvals/review-packs/tmp writer helpers
relative ArtifactRef registry helper
```

핵심 TASK:

```text
artifact path resolver
provider-output writer
tool-output writer
approvals/review-packs writer
security/release artifact writer
relative artifact registry
```

완료 기준: 모든 artifact path가 job directory 내부로 제한되어야 한다.

다음 EPIC handoff:

```text
E04/E05가 provider registry와 fake provider output 저장에 사용할 provider-output path helper를 보고에 남긴다.
```

## E04 Provider Registry

선행 문서:

```text
provider-system.md
repository-layout.md
security-cost-observability.md
```

허용 파일:

```text
packages/star-control-provider/** 또는 선택된 provider package
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
FakeProviderAdapter 실행 로직
local/cloud provider 실제 연결
network 호출
provider session 관리
ExecutionEngine 구현
RouterEngine 구현
```

입력 artifact:

```text
builtin-providers/**/provider.yaml
builtin-providers/**/capabilities.yaml
configs/registries/builtin-provider-registry.yaml
specs/schemas/provider-*.schema.json
examples/provider-contracts/
```

출력 artifact:

```text
provider manifest loader
provider instance loader
builtin provider registry loader
capability profile loader
capability lookup API
provider registry errors
```

핵심 TASK:

```text
provider manifest loader
provider instance loader
builtin provider registry loader
capability profile loader
capability lookup
invalid manifest errors
fake provider registry test
```

완료 기준: fake provider instance가 registry에서 조회되어야 한다.

다음 EPIC handoff:

```text
E05가 사용할 ProviderAdapter interface 후보와 fake provider instance lookup 방법을 보고에 남긴다.
```

## E05 FakeProviderAdapter

선행 문서:

```text
provider-system.md
execution-engine.md
artifact-layout.md
security-privacy-observability-contracts.md
```

허용 파일:

```text
packages/star-control-provider/**
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
실제 source file 수정
local/cloud provider 실제 연결
network 호출
shell command 실행
ExecutionEngine orchestration 구현
CLI 구현
```

입력 artifact:

```text
E03 artifact helpers
E04 provider registry / capability lookup
specs/schemas/execution-request.schema.json
specs/schemas/provider-run-result.schema.json
examples/execution-contracts/execution-request.fake.example.json
examples/execution-contracts/fake-provider-response.success.example.json
```

출력 artifact:

```text
ProviderAdapter interface
FakeProviderAdapter deterministic execute
request.json writer 후보
response.json writer 후보
fake cost metric 후보
```

핵심 TASK:

```text
ProviderAdapter interface
ExecutionRequest reader
FakeProviderAdapter execute
request.json writer
response.json writer
success/failure/block simulation
fake cost metric writer
```

완료 기준: 동일 입력에 deterministic output을 반환하고 fake cost는 0이어야 한다.

다음 EPIC handoff:

```text
E06/E07이 사용할 fake provider id, capability profile, normalized ProviderRunResult shape를 보고에 남긴다.
```

## E06 RouterEngine

선행 문서:

```text
router-decision-matrix.md
router-engine.md
provider-system.md
policy-profiles.md
```

허용 파일:

```text
packages/star-control-router/** 또는 선택된 router package
관련 unit tests
필요한 최소 route schema/example/docs 업데이트
```

금지 파일:

```text
ProviderAdapter 실행
ExecutionEngine 구현
StateStore 저장 구현 변경
ValidationEngine 구현
CLI 구현
local/cloud provider 활성화
```

입력 artifact:

```text
JobSpec
provider registry / capability lookup
policy profile docs
specs/schemas/route.schema.json
specs/schemas/workspec.schema.json
examples/router-contracts/
```

출력 artifact:

```text
RouteSpec generator
size/risk/change_type classifier
policy profile selector
approval reason detector
stage list generator
WorkSpec handoff metadata
```

핵심 TASK:

```text
size/risk heuristic
change type detection
policy profile selection
approval reason detection
provider capability assignment
RouteSpec validation
WorkSpec generation handoff
```

완료 기준: dependency/workflow/schema risk가 approval required로 표시되어야 한다.

다음 EPIC handoff:

```text
E07이 사용할 RouteSpec fields, assignments, workspecs path, provider instance mapping을 보고에 남긴다.
```

## E07 ExecutionEngine

선행 문서:

```text
execution-engine.md
provider-system.md
state-store.md
artifact-layout.md
security-privacy-observability-contracts.md
```

허용 파일:

```text
packages/star-control-execution/** 또는 선택된 execution package
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
새 cloud provider 구현
Star Sentinel rule engine 구현
ValidationEngine 구현
UI 구현
daemon 구현
package manager 또는 dependency 추가
```

입력 artifact:

```text
WorkSpec
ProviderRegistry
FakeProviderAdapter
StateStore
artifact helpers
specs/schemas/execution-request.schema.json
specs/schemas/provider-run-result.schema.json
```

출력 artifact:

```text
ExecutionRequest writer
provider output directory preparation
FakeProvider execution path
RunState update
provider event append
provider failure mapping
stage report draft 후보
```

핵심 TASK:

```text
WorkSpec load and provider lookup
provider output directory preparation
ExecutionRequest writer
FakeProvider execution path
RunState update
provider event append
provider failure mapping
privacy handoff check
stage report draft
```

완료 기준: fake provider WorkSpec 실행이 provider-output artifact와 RunState 변경을 만들어야 한다.

다음 EPIC handoff:

```text
E08 CLI가 호출할 execution entrypoint, required project/job inputs, JSON output 후보를 보고에 남긴다.
```

## E08 CLI read-only + fake run

선행 문서:

```text
cli-command-reference.md
validation-handoff.md
state-store.md
execution-engine.md
ci-contract-validation.md
```

허용 파일:

```text
packages/star-control-cli/** 또는 선택된 CLI package
apps/starctl/** scaffold 범위
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
daemon 구현
API 구현
UI 구현
local/cloud provider 실제 연결
release automation
package manager 또는 dependency 추가
```

입력 artifact:

```text
E02 StateStore
E07 ExecutionEngine entrypoint
specs/schemas/cli-output.schema.json
specs/schemas/cli-error.schema.json
examples/cli-contracts/
```

출력 artifact:

```text
status command
report command
run dry-run
run with fake provider
--json output envelope
approve/cancel/resume 후보
```

핵심 TASK:

```text
status command
report command
run dry-run
run with fake provider
--json output envelope
approve/cancel/resume
```

완료 기준: `run`, `status`, `report`가 fake project에서 동작하고 JSON output이 schema를 만족해야 한다.

다음 EPIC handoff:

```text
E09/E10 smoke에서 사용할 CLI command shape, exit code, sample fake run project를 보고에 남긴다.
```

주의: CLI command가 커지면 `status/report`, `run dry-run`, `run with fake provider`, `approve/cancel/resume`을 별도 TASK/PR로 나눈다.

## E09 Star Sentinel P0

선행 문서:

```text
star-sentinel-p0-contracts.md
star-sentinel-full-spec.md
policy-profiles.md
approval-review-flow.md
security-cost-observability.md
```

허용 파일:

```text
packages/star-sentinel/** 또는 선택된 Star Sentinel package
builtin-tools/star-sentinel/policies/**
builtin-tools/star-sentinel/examples/**
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
Star-Control core에 rule 직접 구현
cloud/local provider 구현
ExecutionEngine 구현
ValidationEngine 구현
CLI 구현
release profile automation
```

입력 artifact:

```text
builtin-tools/star-sentinel/schemas/sentinel-task.schema.json
builtin-tools/star-sentinel/schemas/changed-lines.schema.json
builtin-tools/star-sentinel/schemas/diagnostic.schema.json
builtin-tools/star-sentinel/schemas/p0-rule-registry.schema.json
builtin-tools/star-sentinel/schemas/fixture-outcome.schema.json
builtin-tools/star-sentinel/policies/p0-rule-registry.json
builtin-tools/star-sentinel/examples/p0/
```

출력 artifact:

```text
changed-lines reader
p0 rule registry loader
P0 rule evaluator
diagnostics writer
gate writer 후보
review pack writer 후보
ledger writer 후보
selfcheck 후보
```

핵심 TASK:

```text
task input reader
changed lines reader
p0 rule registry loader
scope/dependency/test/sensitive-data/validator rules
diagnostics writer
gate writer
review pack writer
ledger writer
fixture outcome tests
selfcheck
```

완료 기준: P0 fixtures가 expected decision을 생성해야 한다.

다음 EPIC handoff:

```text
E10 ValidationEngine이 호출할 Star Sentinel command, required input artifact, output artifact, decision mapping을 보고에 남긴다.
```

주의: 이 EPIC은 커질 수 있으므로 `P0 evaluator`, `gate writer`, `review-pack writer`, `selfcheck`를 별도 TASK/PR로 나눌 수 있다. 세부 분리는 `star-sentinel-p0-contracts.md`의 PR 분리 원칙을 따른다.

## E10 ValidationEngine

선행 문서:

```text
validation-engine.md
validation-handoff.md
star-sentinel-p0-contracts.md
approval-review-flow.md
```

허용 파일:

```text
packages/star-control-validation/** 또는 선택된 validation package
관련 unit tests
필요한 최소 docs/example 업데이트
```

금지 파일:

```text
Star Sentinel 전체 rule engine 구현
cloud provider 구현
daemon 구현
UI 구현
package manager 도입
```

입력 artifact:

```text
ProviderRunResult
changed files 후보
Star Sentinel command/output contract
specs/schemas/validation-decision.schema.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
specs/schemas/review-pack-handoff.schema.json
examples/validation-contracts/
```

출력 artifact:

```text
SentinelTask writer
validation_runs writer
approval decision reader
ValidationDecision writer
approval request writer
approval response reader
review pack handoff writer
RunState decision mapping
validation report section
```

핵심 TASK:

```text
SentinelTask writer
validation_runs writer
approval decision reader
ValidationDecision writer
decision -> RunState mapping
approval request writer
approval response reader
review pack handoff writer
validation report section
```

완료 기준: AUTO_PASS/HUMAN_REVIEW/BLOCK decision이 RunState에 반영되어야 한다.

다음 EPIC handoff:

```text
E11 integration smoke가 사용할 validate entrypoint, approval artifact paths, review pack paths, state transition evidence를 보고에 남긴다.
```

## E11 Integration Smoke

선행 문서:

```text
codex-long-run-workflow.md
testing-ci-release.md
ci-contract-validation.md
run-lifecycle.md
artifact-layout.md
cli-command-reference.md
validation-engine.md
```

허용 파일:

```text
integration smoke tests
examples/projects/** 또는 dedicated smoke fixture
필요한 최소 docs 업데이트
```

금지 파일:

```text
local/cloud provider 실제 연결
daemon 구현
API 구현
UI 구현
release/deploy/publish automation
workflow permission 확대
```

입력 artifact:

```text
E01 runtime validator
E02 StateStore
E03 artifact helpers
E04 provider registry
E05 FakeProviderAdapter
E06 RouterEngine
E07 ExecutionEngine
E08 CLI
E09 Star Sentinel P0
E10 ValidationEngine
```

출력 artifact:

```text
fake project fixture
run -> route -> execute -> validate -> report smoke
AUTO_PASS smoke
HUMAN_REVIEW smoke
BLOCK smoke
final report smoke
CLI JSON smoke
```

핵심 TASK:

```text
fake project fixture
run -> route -> execute -> validate -> report flow
AUTO_PASS smoke
HUMAN_REVIEW smoke
BLOCK smoke
final report smoke
CLI JSON smoke
```

완료 기준: fake provider 기반 전체 흐름이 CI에서 반복 가능해야 한다.

다음 EPIC handoff:

```text
v0 fake flow 완료 보고를 남기고, local/cloud provider 확장 전 approval 필요 항목과 남은 위험을 정리한다.
```

## E12 Cloud Provider Preflight

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
security-cost-observability.md
security-privacy-observability-contracts.md
artifact-layout.md
```

허용 파일:

```text
packages/star-control-provider/**
packages/star-control-execution/**
configs/provider-instances/*.example.yaml
필요한 최소 docs 업데이트
PLANS.md
```

금지 파일:

```text
Cargo 외 package manager
새 dependency
GitHub workflow
release/deploy/publish automation
실제 cloud API 호출
실제 paid CLI/API 실행
credential raw value 저장
```

입력 artifact:

```text
specs/schemas/provider-instance.schema.json
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
configs/provider-instances/*api.example.yaml
configs/provider-instances/*cli.example.yaml
```

출력 artifact:

```text
CloudProviderPreflightAdapter
privacy-handoff.json
cost-metric.json
cloud provider BLOCKED response for unsafe/preflight-only states
execution-level preflight fixture
```

핵심 TASK:

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

완료 기준: cloud provider preflight가 credential/privacy/cost 계약을 artifact로 남기고, 실제 transport 실행 전 안전하지 않은 상태를 `BLOCKED`로 정규화해야 한다.

다음 EPIC handoff:

```text
M6b cloud CLI transport implementation은 provider 공식 문서 최신 확인과 실제 외부 호출 승인 조건을 보고에 남긴다.
```

## E13 Cloud CLI Transport

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
security-cost-observability.md
security-privacy-observability-contracts.md
artifact-layout.md
```

허용 파일:

```text
packages/star-control-provider/**
packages/star-control-execution/**
필요한 최소 docs 업데이트
PLANS.md
```

금지 파일:

```text
Cargo 외 package manager
새 dependency
GitHub workflow
release/deploy/publish automation
실제 paid CLI/API 호출 검증
credential raw value 저장
credential_ref env raw value passthrough
```

입력 artifact:

```text
M6a CloudProviderPreflightAdapter
specs/schemas/provider-run-result.schema.json
specs/schemas/privacy-handoff.schema.json
specs/schemas/cost-metric.schema.json
provider instance command.executable
provider instance command.args
```

출력 artifact:

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

핵심 TASK:

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

완료 기준: cloud CLI provider가 preflight 통과 시 shell 없이 executable/args vector로 실행되고, success/timeout이 provider result와 RunState에 반영되어야 한다.

다음 EPIC handoff:

```text
M6c cloud provider output conformance를 별도 PR로 구현한다.
```

## E14 Cloud Provider Conformance

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
testing-ci-release.md
artifact-layout.md
```

허용 파일:

```text
packages/star-control-provider/**
필요한 최소 docs 업데이트
PLANS.md
```

금지 파일:

```text
Cargo 외 package manager
새 dependency
GitHub workflow
release/deploy/publish automation
실제 paid CLI/API 호출 검증
credential raw value 저장
schema field 변경
```

입력 artifact:

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

출력 artifact:

```text
ProviderConformanceChecker
ProviderConformanceProfile::Basic
ProviderConformanceProfile::Cloud
cloud CLI provider conformance fixture
artifact path boundary tests
```

핵심 TASK:

```text
provider output path boundary check
request/response/stdout/stderr artifact ref consistency check
provider-output/{provider_instance_id}/ scope enforcement
cloud privacy-handoff/cost-metric sidecar requirement
artifact file existence check via StateStore
cloud CLI execution fixture conformance assertion
```

완료 기준: conformance checker가 cloud provider output의 path/ref/file existence와 privacy/cost sidecar 존재를 검증해야 한다.

다음 EPIC handoff:

```text
M6d OpenAI-compatible API response parser를 별도 PR로 구현한다.
```

## E15 OpenAI-Compatible API Parser

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
```

허용 파일:

```text
packages/star-control-provider/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

금지 파일:

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

입력 artifact:

```text
builtin-providers/cloud-api/openai/provider.yaml outputs.parser=openai-compatible-chat
Responses API JSON response fixture
Chat Completions JSON response fixture
usage token fields
```

출력 artifact:

```text
OpenAiCompatibleResponseParser
OpenAiCompatibleParsedResponse
Responses API parser fixture
Chat Completions parser fixture
missing text failure fixture
```

핵심 TASK:

```text
Responses API output_text shortcut parse
Responses API output[] message content aggregation without output[0] assumption
Chat Completions choices[].message.content parse
token usage mapping
unsupported/missing text error model
official doc refresh notes
```

완료 기준: OpenAI-compatible parser가 Responses API와 Chat Completions JSON response fixture를 live API 호출 없이 정규화해야 한다.

다음 EPIC handoff:

```text
M6e OpenAI-compatible request builder를 별도 PR로 구현한다.
```

## E16 OpenAI-Compatible Request Builder

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
```

허용 파일:

```text
packages/star-control-provider/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

금지 파일:

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

입력 artifact:

```text
ExecutionRequest.goal
ProviderInstance.endpoint.base_url
ProviderInstance.endpoint.model
ProviderInstance.endpoint.api optional responses/chat_completions selector
```

출력 artifact:

```text
OpenAiCompatibleRequestBuilder
OpenAiCompatiblePreparedRequest
Responses API request body fixture
Chat Completions request body fixture
credential exclusion fixture
```

핵심 TASK:

```text
Responses API request body builder
Chat Completions request body builder
base_url + endpoint path normalization
model required validation
unsupported API selector failure
credential_ref/raw credential exclusion from request body
official doc refresh notes
```

완료 기준: request builder가 live HTTP 호출 없이 Responses API와 Chat Completions request body/URL을 만들고 credential 값을 포함하지 않아야 한다.

다음 EPIC handoff:

```text
M6f cloud API offline HTTP response fixture integration을 별도 PR로 구현한다.
```

## E17 Cloud API Offline Fixture Integration

선행 문서:

```text
complete-implementation-roadmap.md
cloud-provider-policy.md
provider-system.md
docs/providers/provider-reference-snapshots.md
testing-ci-release.md
E15-openai-compatible-parser.md
E16-openai-compatible-request-builder.md
```

허용 파일:

```text
packages/star-control-provider/**
packages/star-control-execution/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

금지 파일:

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

입력 artifact:

```text
ExecutionRequest.goal
ProviderInstance.endpoint.base_url
ProviderInstance.endpoint.model
ProviderInstance.endpoint.api optional responses/chat_completions selector
ProviderInstance.transport_config.offline_response_fixture project-relative JSON path
OpenAI-compatible response fixture JSON
```

출력 artifact:

```text
CloudApiOfflineProviderAdapter
provider-output/{provider_instance_id}/http-request.json
provider-output/{provider_instance_id}/raw-response.json
normalized response.json
privacy-handoff.json
cost-metric.json with parsed usage tokens
execution engine cloud API offline fixture path
provider conformance fixture
```

핵심 TASK:

```text
cloud API preflight reuse
offline_response_fixture project-relative path guard
OpenAI-compatible request builder integration
OpenAI-compatible parser integration
provider output artifact writes
ExecutionEngine cloud API provider selection
no live call / no credential raw value assertions
```

완료 기준: cloud API provider가 `transport_config.offline_response_fixture`가 있을 때 live HTTP 호출 없이 prepared request와 fixture response parse를 같은 runtime path에서 검증하고, fixture가 없으면 기존 preflight `BLOCKED` 흐름을 유지해야 한다.

다음 EPIC handoff:

```text
M6g cloud API transport boundary를 별도 PR로 설계한다. 실제 credential lookup, request signing/header construction, live API call, streaming SSE, paid usage는 별도 승인 전까지 실행하지 않는다.
```

## E18 Cloud API Transport Boundary

선행 문서:

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

허용 파일:

```text
packages/star-control-provider/**
packages/star-control-execution/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

금지 파일:

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

입력 artifact:

```text
OpenAiCompatiblePreparedRequest
ProviderManifest kind/transport/adapter
ProviderInstance.credential_ref prefix only
ProviderInstance.limits.timeout_seconds
provider-output/{provider_instance_id}/http-request.json
```

출력 artifact:

```text
provider-output/{provider_instance_id}/http-transport-plan.json
```

핵심 TASK:

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

완료 기준: cloud API offline runtime path가 `http-transport-plan.json`을 provider output에 기록하고, credential raw value와 full credential reference를 materialize하지 않으며, live API call과 Authorization header value construction이 approval-gated로 남아 있어야 한다.

다음 EPIC handoff:

```text
M6h cloud API live approval gate를 별도 PR로 설계한다. 실제 credential lookup, Authorization header value construction, HTTP client dependency, paid usage, streaming SSE는 별도 승인 전까지 실행하지 않는다.
```

## E19 Cloud API Live Approval Gate

선행 문서:

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

허용 파일:

```text
packages/star-control-provider/**
packages/star-control-execution/**
docs/implementation/**
docs/providers/**
builtin-providers/cloud-api/openai/docs/**
PLANS.md
```

금지 파일:

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

입력 artifact:

```text
ProviderInstance.transport_config.live_api_call_requested=true
OpenAiCompatiblePreparedRequest
ProviderManifest kind/transport/adapter
ProviderInstance.credential_ref prefix only
provider-output/{provider_instance_id}/http-request.json
provider-output/{provider_instance_id}/http-transport-plan.json
```

출력 artifact:

```text
provider-output/{provider_instance_id}/live-transport-approval.json
provider-output/{provider_instance_id}/response.json
```

핵심 TASK:

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

완료 기준: cloud API provider가 `transport_config.live_api_call_requested=true`이고 offline fixture가 없을 때 live HTTP 호출 없이 `http-request.json`, `http-transport-plan.json`, `live-transport-approval.json`, privacy/cost sidecar를 기록하고 `BLOCKED`로 전이해야 한다. full credential reference, raw credential value, `Authorization` header value, `raw-response.json`은 생성하지 않는다.

다음 EPIC handoff:

```text
M7a CLI control commands를 별도 PR로 설계한다. daemon process, API server, UI shell은 다음 M7 slice까지 구현하지 않는다.
```

## E20 CLI Control Commands

선행 문서:

```text
complete-implementation-roadmap.md
cli-command-reference.md
cli-daemon-api-ui.md
approval-review-flow.md
validation-engine.md
state-store.md
daemon-contract.md
api-contract.md
```

허용 파일:

```text
packages/star-control-cli/**
docs/implementation/**
docs/operations/**
PLANS.md
```

금지 파일:

```text
daemon process 구현
API server 구현
UI 구현
새 dependency
Cargo 외 package manager
GitHub workflow
schema field 변경
release/deploy/publish automation
```

입력 artifact:

```text
.ai-runs/{job_id}/run-state.json
.ai-runs/{job_id}/events.jsonl
.ai-runs/{job_id}/approvals/approval-request.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
specs/schemas/cli-output.schema.json
specs/schemas/cli-error.schema.json
```

출력 artifact:

```text
.ai-runs/{job_id}/approvals/approval-response.json
.ai-runs/{job_id}/run-state.json
.ai-runs/{job_id}/events.jsonl
```

핵심 TASK:

```text
CLI approve dispatch
approval request presence check
approval response schema validation
approval response artifact writer
CLI cancel dispatch
terminal state cancel guard
CLI resume dispatch
approval response precondition check
WAITING_APPROVAL -> VALIDATED state transition
schema-valid CLI output/error envelope tests
```

완료 기준: CLI `approve`, `cancel`, `resume`이 StateStore `.ai-runs/` artifact만 변경하며 schema-valid JSON output/error envelope을 반환해야 한다. approval request 누락, terminal cancel, missing approval response를 regression test로 고정한다.

다음 EPIC handoff:

```text
M7 daemon queue skeleton을 별도 PR로 설계한다. daemon runtime state는 repository root가 아니라 user config/cache 영역에 둔다.
```

## E21 Daemon Queue Skeleton

선행 문서:

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
daemon-contract.md
api-contract.md
state-store.md
approval-review-flow.md
testing-ci-release.md
```

허용 파일:

```text
Cargo.toml
packages/star-control-daemon/**
docs/implementation/**
docs/operations/**
PLANS.md
```

금지 파일:

```text
daemon background process 구현
socket 또는 HTTP API server 구현
UI 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

입력 artifact:

```text
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/daemon-state.schema.json
specs/schemas/run-state.schema.json
specs/schemas/approval-response.schema.json
```

출력 artifact:

```text
{config_root}/daemon/state.json
```

핵심 TASK:

```text
star-control-daemon crate 추가
DaemonConfig와 DaemonQueue 추가
config_root/daemon/state.json 생성 및 schema validation
non-terminal job queue entry 등록
terminal state queue 거부
WAITING_APPROVAL approval-response precondition
non-approved approval-response queue 거부
duplicate queue entry guard
project artifact 미복사 regression test
```

완료 기준: daemon state가 Star-Control repository root나 대상 project root가 아니라 caller가 넘긴 config root 아래에 생성되고, queue entry가 project `.ai-runs/{job_id}`를 참조하되 artifact를 복사하지 않아야 한다. terminal job과 approved response 없는 `WAITING_APPROVAL` job은 queue에 등록되지 않아야 한다.

다음 EPIC handoff:

```text
M7c API read-only endpoint를 별도 PR로 설계한다. API는 daemon queue state와 StateStore artifact를 read-only로 노출하고 mutation endpoint는 이후 slice까지 구현하지 않는다.
```

## E22 API Read-Only

선행 문서:

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
api-contract.md
daemon-contract.md
state-store.md
security-cost-observability.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

허용 파일:

```text
Cargo.toml
Cargo.lock
packages/star-control-api/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
HTTP server 구현
socket listener 구현
remote API exposure
auth/session 시스템 구현
mutation endpoint 구현
daemon background worker 변경
UI 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

입력 artifact:

```text
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/reports/{stage}-report.json
{config_root}/daemon/state.json
specs/schemas/api-response.schema.json
```

출력 artifact:

```text
없음
```

핵심 TASK:

```text
star-control-api crate 추가
ApiReadOnlyService 추가
GET /daemon/state
GET /projects
GET /projects/{project_id}/jobs
GET /projects/{project_id}/jobs/{job_id}
GET /projects/{project_id}/jobs/{job_id}/events
GET /projects/{project_id}/jobs/{job_id}/report?stage={stage}
api-response schema validation
missing project/job/report structured error
mutation method rejection
read-only no-write regression test
secret-like response redaction test
```

완료 기준: 모든 API response가 `api-response.schema.json`을 만족하고, read-only endpoint가 daemon queue state와 StateStore artifact를 변형하지 않아야 한다. missing artifact는 structured error envelope으로 반환하고, mutation method/path, HTTP server, socket, auth, remote exposure는 구현하지 않는다.

다음 EPIC handoff:

```text
M8 UI shell read-only view를 별도 PR로 설계한다. UI는 API read-only service 계약을 소비하고 provider process, Star Sentinel rule, StateStore file mutation을 직접 구현하지 않는다.
```

## E23 UI Read-Only View

선행 문서:

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
ui-shell-contract.md
api-contract.md
daemon-contract.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

허용 파일:

```text
Cargo.toml
Cargo.lock
packages/star-control-ui/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
browser UI app 구현
TypeScript/Node package manager 도입
HTTP server 구현
API mutation endpoint 구현
provider process 실행 구현
Star Sentinel rule 직접 구현
StateStore file mutation 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

입력 artifact:

```text
ApiReadOnlyService response envelope
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/reports/{stage}-report.json
specs/schemas/ui-job-view.schema.json
```

출력 artifact:

```text
없음
```

핵심 TASK:

```text
star-control-ui crate 추가
UiReadOnlyShell 추가
job_list view model
job_detail view model
UI job view schema validation
timeline event view
provider output path viewer data
validation result path viewer data
approval request viewer data
review pack viewer data
read-only no-write regression test
secret-like view redaction test
missing report read-only error surface test
```

완료 기준: `UiReadOnlyShell`이 `ApiReadOnlyService`를 소비해 schema-valid job list/detail view model을 만들고, StateStore artifact를 직접 수정하지 않아야 한다. approval-required job은 approval path와 API/CLI mutation surface를 노출하지만 UI view model은 mutation을 수행하지 않는다.

다음 EPIC handoff:

```text
E24 API control mutation slice를 별도 PR로 설계한다. 그 이후 M8b browser UI shell은 read-only view model과 API control service를 함께 소비하도록 설계한다.
```

## E24 API Control Mutations

선행 문서:

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
api-contract.md
approval-review-flow.md
daemon-contract.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

허용 파일:

```text
Cargo.toml
Cargo.lock
packages/star-control-api/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
HTTP server 구현
socket listener 구현
remote API exposure
auth/session 시스템 구현
daemon background worker 변경
provider process 실행 구현
UI browser app 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

입력 artifact:

```text
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/approvals/approval-request.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/api-response.schema.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
```

출력 artifact:

```text
대상 project .ai-runs/{job_id}/approvals/approval-response.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
```

핵심 TASK:

```text
ApiControlService 추가
POST /projects/{project_id}/jobs/{job_id}/approve
POST /projects/{project_id}/jobs/{job_id}/cancel
POST /projects/{project_id}/jobs/{job_id}/resume
approval request presence check
approval response schema validation
approval response artifact writer
approved response resume precondition
terminal cancel guard
StateStore run-state update
events.jsonl audit event append
structured error envelope tests
secret-like response redaction 유지
ApiReadOnlyService non-GET rejection 유지
```

완료 기준: `ApiControlService`가 GET read-only endpoint와 POST approve/cancel/resume control endpoint를 in-process로 처리하고, 모든 response가 `api-response.schema.json` envelope을 만족해야 한다. HTTP server, socket, auth/session, remote exposure는 구현하지 않는다.

다음 EPIC handoff:

```text
E25 UI browser control shell은 UiBrowserShell이 ApiControlService를 소비하도록 설계한다. browser UI package manager, network server, remote API exposure는 별도 승인 전까지 구현하지 않는다.
```

## E25 UI Browser Control Shell

선행 문서:

```text
complete-implementation-roadmap.md
cli-daemon-api-ui.md
ui-shell-contract.md
api-contract.md
approval-review-flow.md
state-store.md
security-privacy-observability-contracts.md
testing-ci-release.md
```

허용 파일:

```text
packages/star-control-ui/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
browser UI app 구현
TypeScript/Node package manager 도입
HTTP server 구현
socket listener 구현
remote API exposure
auth/session 시스템 구현
daemon background worker 변경
provider process 실행 구현
Star Sentinel rule 직접 구현
StateStore file 직접 mutation 구현
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
외부 provider live call
credential raw value lookup/materialization
```

입력 artifact:

```text
ApiControlService GET read-only endpoint
ApiControlService POST approve/cancel/resume endpoint
대상 project .ai-runs/{job_id}/job.json
대상 project .ai-runs/{job_id}/run-state.json
대상 project .ai-runs/{job_id}/events.jsonl
대상 project .ai-runs/{job_id}/approvals/approval-request.json
대상 project .ai-runs/{job_id}/approvals/approval-response.json
specs/schemas/ui-job-view.schema.json
specs/schemas/api-response.schema.json
```

출력 artifact:

```text
없음
```

핵심 TASK:

```text
UiBrowserShell 추가
browser_control_shell action panel 추가
approve/cancel/resume action surface 추가
ApiControlService handle_get/handle_post 소비
approval response body builder
control mutation result view
terminal cancel disabled surface
approved response 이후 resume enabled surface
secret-like result redaction 유지
HTTP/server/package-manager 미도입 regression test
```

완료 기준: `UiBrowserShell`이 `ApiControlService`를 소비해 browser-oriented action panel과 approve/cancel/resume result view를 만들고, mutation은 API control service를 통해서만 수행해야 한다. TypeScript/Node package manager, HTTP server, socket, auth/session, remote exposure는 구현하지 않는다.

다음 EPIC handoff:

```text
M9 hardening은 security, cost, observability, conformance, release readiness 검증을 작은 PR로 확장한다. 실제 browser app, HTTP server, auth/session, remote exposure, package manager 도입은 별도 승인 전까지 RESERVED다.
```

## E26 Security Redaction Utility

선행 문서:

```text
complete-implementation-roadmap.md
security-privacy-observability-contracts.md
security-cost-observability.md
api-contract.md
ui-shell-contract.md
testing-ci-release.md
release-readiness.md
```

허용 파일:

```text
Cargo.toml
Cargo.lock
packages/star-control-security/**
packages/star-control-api/**
packages/star-control-ui/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
external account/repository settings 변경
credential raw value lookup/materialization
provider live call
HTTP server 구현
browser UI app 구현
audit/cost/retention/recovery/release automation 구현
```

입력 artifact:

```text
specs/schemas/redaction-report.schema.json
examples/security-contracts/redaction-report.example.json
API response data/error envelope
UI read-only/control result view
```

출력 artifact:

```text
RedactionReport JSON value
redacted API/UI JSON value
```

핵심 TASK:

```text
star-control-security crate 추가
redact_value utility 추가
redact_value_with_report utility 추가
RedactionFinding model 추가
RedactionReport builder 추가
credential-like key redaction
secret-like string redaction
private key marker redaction
raw value 없는 finding/report test
redaction-report schema validation test
ApiReadOnlyService/ApiControlService redaction utility migration
UiReadOnlyShell/UiBrowserShell redaction utility migration
```

완료 기준: API/UI가 중복 redaction helper 대신 `star-control-security`를 사용하고, RedactionReport builder가 `redaction-report.schema.json`을 만족해야 한다. finding/report에는 raw secret value를 넣지 않으며, schema field, workflow, package manager, release/deploy/publish automation은 변경하지 않는다.

다음 EPIC handoff:

```text
M9b는 audit event writer로 이어간다. RedactionReport를 StateStore artifact로 저장하거나 user-facing report에 연결하는 작업은 별도 작은 PR에서 처리한다.
```

## E27 Observability Audit Event Writer

선행 문서:

```text
complete-implementation-roadmap.md
artifact-layout.md
state-store.md
security-privacy-observability-contracts.md
security-cost-observability.md
testing-ci-release.md
release-readiness.md
```

허용 파일:

```text
Cargo.toml
Cargo.lock
packages/star-control-observability/**
packages/star-control-cli/src/lib.rs
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
external account/repository settings 변경
credential raw value lookup/materialization
provider live call
HTTP server 구현
browser UI app 구현
cost/retention/recovery/release automation 구현
```

입력 artifact:

```text
specs/schemas/audit-event.schema.json
examples/security-contracts/audit-event.example.json
StateStore job directory
redacted JSON value
```

출력 artifact:

```text
audit/audit-events.jsonl
ArtifactRef(kind=log, producer=star-control-observability)
```

핵심 TASK:

```text
star-control-observability crate 추가
AuditEventWriter 추가
AuditEvent schema validation
StateStore resolve_job_path 기반 job directory containment
append-only audit/audit-events.jsonl writer
audit log readback helper
secret-like value redaction before persist
path traversal rejection test
raw secret persistence regression test
schema-valid audit event append test
```

완료 기준: AuditEventWriter가 schema-valid AuditEvent만 `.ai-runs/{job_id}/audit/audit-events.jsonl`에 append-only로 저장하고, 저장 전 shared redaction utility를 적용해야 한다. writer가 반환하는 ArtifactRef는 `kind=log`, `producer=star-control-observability`, `schema_path=specs/schemas/audit-event.schema.json`을 사용한다. API/CLI/daemon/provider 흐름 자동 연결, cost/budget guard, retention/recovery command, release readiness automation은 후속 slice로 남긴다.

다음 EPIC handoff:

```text
M9c는 cost metric budget guard로 이어간다. API/CLI/daemon/provider event를 AuditEventWriter에 연결하는 작업은 별도 작은 PR에서 처리한다.
```

## E28 Cost Metric Budget Guard

선행 문서:

```text
complete-implementation-roadmap.md
security-privacy-observability-contracts.md
security-cost-observability.md
provider-system.md
testing-ci-release.md
release-readiness.md
```

허용 파일:

```text
packages/star-control-observability/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

금지 파일:

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
release/deploy/publish automation
external account/repository settings 변경
credential raw value lookup/materialization
provider live call
HTTP server 구현
browser UI app 구현
hard budget enforcement
retention/recovery/release automation 구현
```

입력 artifact:

```text
specs/schemas/cost-metric.schema.json
examples/security-contracts/cost-metric.fake.example.json
provider-output/{provider_instance_id}/cost-metric.json
```

출력 artifact:

```text
provider-output/{provider_instance_id}/cost-metric.json
Budget evaluation JSON value
```

핵심 TASK:

```text
CostMetricWriter 추가
CostMetric schema validation
provider-output/{provider_instance_id}/cost-metric.json writer/readback helper
secret-like unexpected field redaction before persist
safe provider instance path containment
CostBudgetThresholds 추가
warning-only budget evaluation
missing cost metric non-fatal read path
fake/default cost=0 regression test
budget threshold warning test
CLI test temp project path collision hardening if workspace validation exposes flake
```

완료 기준: CostMetricWriter가 schema-valid CostMetric만 provider output sidecar로 저장하고, missing metric은 core flow 실패가 아닌 `Ok(None)`으로 표현해야 한다. Budget evaluation은 `warn_only`이며 hard enforcement, billing/quota 외부 조회, provider execution 자동 연결은 후속 slice로 남긴다.

다음 EPIC handoff:

```text
M9d는 provider conformance hardening, retention/recovery, release readiness 중 하나로 이어간다. provider execution path가 CostMetricWriter/Budget evaluation을 자동 호출하는 작업은 별도 작은 PR에서 처리한다.
```

## RESERVED

아래는 E12 이후 별도 작은 PR로 구현한다.

```text
Local Process Provider hardening / conformance extension
Local Model Provider
Cloud CLI Provider parser / conformance extension
Cloud API Provider transport execution
Cloud provider-specific parser / conformance
Daemon
HTTP server / remote API exposure
Browser UI Shell app / remote UI runtime
Observability Integration / Conformance Hardening
Release Readiness Automation
```

release/deploy/publish, repository settings 변경, package registry 변경은 별도 승인 전까지 구현하지 않는다.
