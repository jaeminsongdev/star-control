# Repository Layout

## 목적

이 문서는 Star-Control repository의 목표 구조와 package 경계를 정의한다. 현재 repository는 스캐폴드와 설계 문서 단계이므로 모든 package가 즉시 구현되어야 하는 것은 아니다. Codex가 장시간 구현을 진행할 때 책임 경계를 임의로 섞지 않도록 최종 구조를 먼저 고정한다.

현재 실제 경로의 상태는 `current-repository-map.md`를 우선 확인한다. 이 문서는 목표 구조와 package 책임을 설명한다.

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
  star-sentinel/
```

위 구조는 목표 구조다. package manager 도입 전에는 문서와 스캐폴드만 둘 수 있다.

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

### `star-control-validation`

- validation requirement 실행
- Star Sentinel tool invocation
- approval gate 반영
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

- 장시간 queue 처리
- background runner
- provider session 관리
- RESERVED: 초기 구현 전 문서 계약만 둔다

### `star-control-api`

- UI와 외부 도구가 사용하는 API
- RESERVED: 초기 구현 전 문서 계약만 둔다

### `star-control-ui`

- 작업 생성, 진행 상태, 승인, 리뷰 확인 UI
- RESERVED: 초기 구현 전 문서 계약만 둔다

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
- `apps/star-control-ui/`는 UI shell 후보이며 초기 구현 대상이 아니다.
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

후속 계약 PR에서 config, policy, hook, renderer, role, skill schema를 별도로 고정한다.

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

후속 PR에서 문서 index, provider contract, config contract, policy fixture 검사를 추가할 수 있다.

## 금지되는 구조

```text
packages/codex-provider-core/       # 특정 제품명이 core package에 들어감
packages/star-control-star-sentinel # core와 tool 경계 혼동
.ai-runs/                           # Star-Control repo 내부 실행 산출물
```

## 구현 순서 기준

1. 문서와 schema 계약 고정
2. file-based state package
3. schema validator package
4. provider registry와 fake provider
5. router와 execution engine
6. validation engine과 Star Sentinel P0 implementation
7. CLI
8. integration smoke
9. local/cloud provider 확장
10. daemon/API/UI 확장

## PR 경계 원칙

- 한 PR은 하나의 계약 또는 하나의 package 목적만 다룬다.
- 문서, schema, example, CI 검증이 함께 움직여야 하는 계약은 같은 PR에 둔다.
- StateStore, Router, ProviderAdapter, ExecutionEngine, ValidationEngine, CLI 구현을 한 PR에 섞지 않는다.
- schema 변경 PR은 example과 schema-example-check 영향을 함께 검토한다.
