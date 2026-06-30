# Codex Work Queue Current Order

## 목적

이 문서는 계약 고정 이후 Codex가 실제 구현에 들어갈 때 우선 따를 현재 구현 순서를 정리한다. 긴 전체 큐는 `codex-work-queue.md`에 두고, 실제 착수 순서는 이 문서를 우선한다.

## 공통 완료 기준

- 관련 구현 문서를 먼저 읽는다.
- 허용 파일만 수정한다.
- 새 의존성/package manager는 승인 없이 추가하지 않는다.
- schema/example 계약을 약화하지 않는다.
- tests/CI/policy를 약화하지 않는다.
- 현재 CI를 통과한다.
- approval required 변경은 사람이 승인하기 전까지 실행하지 않는다.

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

## E01 Schema / Runtime Validator

선행 문서:

```text
schema-validator.md
data-contracts.md
ci-contract-validation.md
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

## E02 File-based StateStore

선행 문서:

```text
state-store.md
state-store-recovery.md
artifact-layout.md
artifact-naming.md
run-lifecycle.md
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

## E03 Artifact Layout Writer

선행 문서:

```text
artifact-layout.md
artifact-naming.md
security-privacy-observability-contracts.md
release-readiness.md
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

## E04 Provider Registry

선행 문서:

```text
provider-system.md
repository-layout.md
security-cost-observability.md
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

## E05 FakeProviderAdapter

선행 문서:

```text
provider-system.md
execution-engine.md
artifact-layout.md
security-privacy-observability-contracts.md
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

## E06 RouterEngine

선행 문서:

```text
router-decision-matrix.md
router-engine.md
provider-system.md
policy-profiles.md
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

## E07 ExecutionEngine

선행 문서:

```text
execution-engine.md
provider-system.md
state-store.md
artifact-layout.md
security-privacy-observability-contracts.md
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

## E08 CLI

선행 문서:

```text
cli-command-reference.md
validation-handoff.md
state-store.md
execution-engine.md
ci-contract-validation.md
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

## E09 Star Sentinel P0

선행 문서:

```text
star-sentinel-p0-contracts.md
star-sentinel-full-spec.md
policy-profiles.md
approval-review-flow.md
security-cost-observability.md
```

핵심 TASK:

```text
task input reader
changed lines reader
p0 rule registry loader
scope/dependency/test/secret rules
diagnostics writer
gate writer
review pack writer
ledger writer
fixture outcome tests
selfcheck
```

완료 기준: P0 fixtures가 expected decision을 생성해야 한다.

## E10 ValidationEngine

선행 문서:

```text
validation-engine.md
validation-handoff.md
star-sentinel-p0-contracts.md
approval-review-flow.md
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

## E11 Integration Smoke

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

## RESERVED

아래는 fake flow 안정화 전까지 구현하지 않는다.

```text
Local Process Provider
Local Model Provider
Cloud CLI Provider
Cloud API Provider
Daemon
API
UI Shell
Security / Cost / Observability Hardening
Release Readiness Automation
```

release/deploy/publish, repository settings 변경, package registry 변경은 별도 승인 전까지 구현하지 않는다.
