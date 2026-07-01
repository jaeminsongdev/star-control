# Repository Layout

## 목적

이 문서는 Star-Control repository의 목표 구조와 package 경계를 정의한다. 현재 repository는 스캐폴드와 설계 문서 단계이므로 모든 package가 즉시 구현되어야 하는 것은 아니다. Codex가 장시간 구현을 진행할 때 책임 경계를 임의로 섞지 않도록 최종 구조를 먼저 고정한다.

현재 실제 경로의 상태는 `current-repository-map.md`를 우선 확인한다. 이 문서는 목표 구조와 package 책임을 설명한다.

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. 이 문서의 package 순서나 장기 구조 설명이 현재 작업 큐와 다르게 보이면, 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 한다.

## 현재 정본 경로

```text
README.md
AGENTS.md
.github/workflows/
docs/
specs/
configs/
builtin-providers/
builtin-tools/star-sentinel/
examples/
scripts/ci/
```

현재 scaffold 또는 reserved 경로:

```text
apps/
packages/
integrations/
```

- `apps/`는 CLI, daemon, UI entrypoint 후보를 표시하는 scaffold다.
- `packages/`는 목표 구현 package 경계다.
- `integrations/`는 장기 연동 산출물 후보이며 초기 구현 대상이 아니다.

## 목표 package 경계

```text
packages/
  star-control-core/
  star-control-state/
  star-control-schema/
  star-control-router/
  star-control-execution/
  star-control-provider/
  star-control-validation/
  star-control-report/
  star-control-cli/
  star-control-daemon/
  star-control-api/
  star-control-ui/
  star-control-security/
  star-control-release/
  star-sentinel/
```

위 구조는 목표 core 구조다. package manager 도입 전에는 문서와 스캐폴드만 둘 수 있다. Core runtime crate namespace는 `docs/decisions/0005-full-implementation-defaults.md`에 따라 `star-control-*`로 통일한다.

## provider / transport / adapter extension 경계

기존 scaffold 중 아래 계열은 core namespace가 아니라 core 안정화 이후 확장 package 후보로 분류한다.

```text
packages/star-provider-api
packages/star-provider-host
packages/star-transport-cli
packages/star-transport-http
packages/star-transport-process
packages/star-adapter-code-agent
packages/star-adapter-chat-model
packages/star-adapter-openai-compatible
```

해당 package는 provider manifest, provider instance, capability profile, ProviderAdapter interface가 안정화된 뒤 실제 구현 대상으로 삼는다. E01~E11 core fake flow를 구현할 때 `star-control-provider`를 대체하지 않는다.

## package 책임

### `star-control-core`

- job lifecycle 조정
- module orchestration
- state transition 정책 적용
- provider/tool 직접 구현 금지

### `star-control-state`

- file-based StateStore
- job.json, run-state.json, events.jsonl 관리
- atomic write와 append 규칙
- Star-Control repository 내부 `.ai-runs` 사용 금지
- inspect-only recovery report for missing/corrupt/tmp artifacts

### `star-control-schema`

- JSON schema loading
- schema subset validation
- schema validation error model
- 외부 `jsonschema` dependency 도입은 승인 전 금지

### `star-control-router`

- request 분석
- size/risk/stage 산출
- provider assignment
- approval 필요 여부 판단

### `star-control-execution`

- WorkSpec 실행
- ProviderAdapter 호출
- timeout, cancel, retry 처리
- provider output 저장

### `star-control-provider`

- provider registry
- provider manifest loading
- provider instance loading
- ProviderAdapter interface
- fake, human, local, cloud provider adapter 경계
- ProviderConformanceChecker result/ref/file/schema consistency hardening

### `star-control-validation`

- validation requirement 실행
- Star Sentinel tool invocation
- approval gate 반영
- validation/validation-decision.json 생성
- validation_runs.json 관리

### `star-control-report`

- ReportSpec 생성
- user-facing report 생성
- changed_files, risks, validation, artifacts 정리

### `star-control-cli`

- `run`, `status`, `report`, `approve`, `cancel`, `resume` 명령
- stdout/stderr/exit code 계약
- daemon 없이도 file-based flow 실행 가능해야 함

### `star-control-daemon`

- file-based daemon queue state
- StateStore job queue reference 등록
- terminal/approval/duplicate queue guard
- RESERVED: background runner, socket, API server, provider session scheduling

### `star-control-api`

- UI와 외부 도구가 사용하는 API
- read-only request/router service
- approve/cancel/resume in-process control mutation service
- api-response envelope validation
- RESERVED: HTTP server, remote exposure, auth/session, provider scheduling

### `star-control-ui`

- API read-only service를 소비하는 UI read-only view model
- API control service를 소비하는 browser-oriented control shell model
- job list, job detail, timeline, provider output, validation, approval, review pack viewer data
- approve/cancel/resume action panel과 mutation result view
- RESERVED: browser UI app, TypeScript/Node package manager, HTTP server, remote UI runtime

### `star-control-security`

- shared redaction utility
- RedactionReport builder
- secret-like key/string detection without storing raw values
- RESERVED: RedactionReport artifact storage, retention/recovery command, release readiness automation

### `star-control-observability`

- AuditEventWriter
- schema-valid `audit/audit-events.jsonl` append/readback helper
- StateStore job directory containment for audit log paths
- shared redaction utility application before audit persistence
- CostMetricWriter
- schema-valid provider output `cost-metric.json` write/readback helper
- warning-only CostBudgetThresholds evaluation
- RESERVED: API/CLI/daemon/provider automatic audit/cost integration, hard budget enforcement, retention/recovery command, release readiness automation

### `star-control-release`

- ReleaseReadinessWriter
- schema-valid `release/release-readiness.json` write/readback helper
- ReleaseConsistencyChecker for version/changelog checks
- ReleaseEvidenceFileChecker for read-only version/changelog evidence files
- reserved/not_ready readiness artifact generation
- RESERVED: signing, publish, deploy automation, repository/package registry settings changes

### `star-sentinel`

- Star Sentinel builtin tool 구현
- policy, diagnostics, approval, review pack, selfcheck
- core와 직접 결합 금지

## apps 경계

```text
apps/
  starctl/
  star-daemon/
  star-control-ui/
```

- `apps/starctl/`은 CLI entrypoint 후보다.
- `apps/star-daemon/`은 daemon entrypoint 후보이며 초기 구현 대상이 아니다.
- `apps/star-control-ui/`는 browser UI app 후보이며 초기 구현 대상이 아니다. M8a/M8b의 library-level view/control shell model은 `packages/star-control-ui/`에 둔다.
- app layer는 core logic을 직접 소유하지 않고, 안정화된 package API를 호출하는 얇은 표면이어야 한다.

## builtin 경계

```text
builtin-providers/
  test/
  local-process/
  local-server/
  cloud-cli/
  cloud-api/

builtin-tools/
  star-sentinel/
```

- `builtin-providers/`는 provider manifest와 capability profile을 둔다.
- `builtin-tools/star-sentinel/`은 tool manifest, policy, schema, fixture, example을 둔다.
- 구현 코드는 `packages/` 아래에 둔다.

## docs 경계

```text
docs/
  implementation/
  operations/
  providers/
  tools/
  decisions/
```

- `docs/implementation/`: Codex와 구현자가 따를 전체 구현 계약.
- `docs/operations/`: ChatGPT, GitHub, CI, Codex 운영 기준.
- `docs/providers/`: provider 개념, registry, capability 문서.
- `docs/tools/`: builtin tool 개요.
- `docs/decisions/`: 장기 결정 기록.

## specs 경계

```text
specs/
  schemas/
```

`specs/schemas/`는 Star-Control core-level schema를 둔다.

예시:

- `job.schema.json`
- `run-state.schema.json`
- `route.schema.json`
- `workspec.schema.json`
- `report.schema.json`

Star Sentinel 전용 schema는 `builtin-tools/star-sentinel/schemas/`에 둔다.

## configs 경계

`configs/`는 runtime default, template, role, policy, registry 후보를 둔다. implementation package가 생기기 전까지는 정본 설정과 template 중심으로 유지한다.

## examples 경계

`examples/`는 Star-Control core-level sample artifact를 둔다. Star Sentinel 전용 example은 `builtin-tools/star-sentinel/examples/`에 둔다.

`examples/runs/`는 실제 실행 결과 저장 위치가 아니라 schema와 문서 검증을 위한 sample이다.

## scripts 경계

`scripts/ci/`는 현재 repository의 계약 검증 스크립트를 둔다.

현재 검사 후보:

- repository policy
- data format
- manifest contract
- Star Sentinel naming policy
- schema example
- implementation documentation

후속 PR에서 provider contract, config contract, policy fixture, work queue consistency 검사를 추가할 수 있다.

## 금지되는 구조

```text
packages/codex-provider-core/       # 특정 제품명이 core package에 들어감
packages/star-control-star-sentinel # core와 tool 경계 혼동
.ai-runs/                           # Star-Control repo 내부 실행 산출물
```

## 현재 구현 순서 기준

실제 착수 순서는 `docs/implementation/codex-work-queue-current.md`를 우선한다. 현재 v0 구현 순서는 다음과 같다.

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

장기 구현 흐름은 아래 원칙을 따른다. 전체 milestone은 `complete-implementation-roadmap.md`를 기준으로 한다.

1. schema/runtime validator를 먼저 안정화한다.
2. file-based StateStore와 artifact layout을 안정화한다.
3. provider registry와 fake provider를 붙인다.
4. router와 execution engine을 fake flow 기준으로 연결한다.
5. CLI read-only와 fake run을 안정화한다.
6. Star Sentinel P0와 ValidationEngine을 연결한다.
7. fake provider 기반 integration smoke를 만든다.
8. local process provider를 먼저 붙인다.
9. local model/server provider와 cloud CLI/API provider를 순차 확장한다.
10. daemon, API, UI는 CLI file-based flow와 approval flow가 안정화된 뒤 확장한다.
11. release automation은 release readiness와 approval flow가 안정화된 뒤 별도 승인으로만 구현한다.

## PR 경계 원칙

- 한 PR은 하나의 계약 또는 하나의 package 목적만 다룬다.
- 문서, schema, example, CI 검증이 함께 움직여야 하는 계약은 같은 PR에 둔다.
- StateStore, Router, ProviderAdapter, ExecutionEngine, ValidationEngine, CLI 구현을 한 PR에 섞지 않는다.
- schema 변경 PR은 example과 schema-example-check 영향을 함께 검토한다.
