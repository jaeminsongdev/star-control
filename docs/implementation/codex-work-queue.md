# Codex Work Queue

## 목적

이 문서는 Codex가 Star-Control 전체 구현을 장시간 목표추진으로 진행할 때 따를 EPIC/TASK 큐다. 큐는 전체 완성형 시스템을 목표로 하지만, 실제 작업은 검증 가능한 작은 PR로 나눈다.

## 공통 완료 기준

모든 TASK는 다음을 만족해야 한다.

- 문서 계약을 위반하지 않는다.
- 허용 파일만 수정한다.
- 새 의존성이나 package manager는 승인 없이 추가하지 않는다.
- 관련 테스트 또는 fixture를 추가한다.
- 현재 CI를 통과한다.
- PR 본문에 검증 결과를 기록한다.

## EPIC 01: Schema / Runtime Validator

목표:

- repository 내부 JSON schema subset validator를 구현한다.
- CI validator와 runtime validator의 의미 차이를 최소화한다.

선행 문서:

```text
schema-validator.md
data-contracts.md
testing-ci-release.md
```

TASK 후보:

```text
E01-T01: schema package scaffold
E01-T02: JSON loader and parse errors
E01-T03: const/enum/type validation
E01-T04: required/properties/items validation
E01-T05: additionalProperties/minLength/pattern validation
E01-T06: validation error model
E01-T07: schema/example fixture tests
E01-T08: CI script reuse plan
```

완료 기준:

- core schema와 Star Sentinel schema examples를 runtime validator로 검증할 수 있다.
- invalid JSON, missing required, enum mismatch가 테스트로 잡힌다.

## EPIC 02: File-based StateStore

목표:

- 대상 프로젝트 `.ai-runs/` 아래 job state를 저장하고 읽는다.

선행 문서:

```text
state-store.md
artifact-layout.md
data-contracts.md
run-lifecycle.md
```

TASK 후보:

```text
E02-T01: StateStore package scaffold
E02-T02: project root and .ai-runs directory handling
E02-T03: job id allocation
E02-T04: job.json create/load
E02-T05: run-state.json save/load
E02-T06: events.jsonl append/read
E02-T07: route/workspec/report save/load
E02-T08: path traversal guard
E02-T09: atomic write helper
E02-T10: list_jobs
```

완료 기준:

- fake project에서 `J-0001` 생성 가능
- job/state/event roundtrip 가능
- path traversal 차단 테스트 성공

## EPIC 03: Artifact Layout Writer

목표:

- provider-output, tool-output, approvals, review-packs 경로를 안정적으로 생성한다.

선행 문서:

```text
artifact-layout.md
state-store.md
security-cost-observability.md
```

TASK 후보:

```text
E03-T01: artifact path resolver
E03-T02: provider output directory creation
E03-T03: tool output directory creation
E03-T04: approvals and review-packs directory creation
E03-T05: tmp cleanup policy hook
E03-T06: relative artifact registry
```

완료 기준:

- 모든 artifact path가 job directory 내부로 제한된다.
- absolute/path traversal 입력이 차단된다.

## EPIC 04: Provider Registry

목표:

- provider manifest와 instance를 로드하고 capability 기반 조회를 제공한다.

선행 문서:

```text
provider-system.md
repository-layout.md
security-cost-observability.md
```

TASK 후보:

```text
E04-T01: ProviderManifest schema draft
E04-T02: fake provider manifest
E04-T03: ProviderInstance schema draft
E04-T04: registry loader
E04-T05: capability lookup
E04-T06: invalid manifest errors
```

완료 기준:

- fake provider instance가 registry에서 조회된다.
- capability missing error가 테스트된다.

## EPIC 05: FakeProviderAdapter

목표:

- 외부 호출 없이 deterministic provider output을 생성한다.

선행 문서:

```text
provider-system.md
execution-engine.md
artifact-layout.md
```

TASK 후보:

```text
E05-T01: ProviderAdapter interface
E05-T02: FakeProviderAdapter execute
E05-T03: request.json writer
E05-T04: response.json writer
E05-T05: success simulation
E05-T06: failure/block simulation
E05-T07: provider output tests
```

완료 기준:

- FakeProviderAdapter가 request/response artifact를 생성한다.
- 동일 입력에 deterministic output을 반환한다.

## EPIC 06: RouterEngine

목표:

- JobSpec을 RouteSpec과 WorkSpec 후보로 변환한다.

선행 문서:

```text
router-engine.md
provider-system.md
data-contracts.md
policy-profiles.md
```

TASK 후보:

```text
E06-T01: RouterEngine scaffold
E06-T02: size/risk heuristic
E06-T03: stage selection
E06-T04: approval reason detection
E06-T05: provider capability assignment
E06-T06: RouteSpec validation
E06-T07: WorkSpec generation handoff
```

완료 기준:

- LOW/SMALL 문서 요청이 fake provider route를 만든다.
- dependency/workflow/schema risk가 approval required로 표시된다.

## EPIC 07: ExecutionEngine

목표:

- WorkSpec을 ProviderAdapter 실행으로 연결한다.

선행 문서:

```text
execution-engine.md
provider-system.md
state-store.md
artifact-layout.md
```

TASK 후보:

```text
E07-T01: ExecutionEngine scaffold
E07-T02: WorkSpec load and provider lookup
E07-T03: provider output directory preparation
E07-T04: FakeProvider execution path
E07-T05: RunState update
E07-T06: provider event append
E07-T07: provider failure mapping
E07-T08: stage report draft
```

완료 기준:

- fake provider WorkSpec 실행이 provider-output artifact와 RunState 변경을 만든다.

## EPIC 08: CLI

목표:

- daemon 없이 file-based flow를 사용할 수 있는 기본 CLI를 만든다.

선행 문서:

```text
cli-daemon-api-ui.md
state-store.md
execution-engine.md
testing-ci-release.md
```

TASK 후보:

```text
E08-T01: CLI scaffold
E08-T02: status command
E08-T03: report command
E08-T04: run command with fake provider
E08-T05: --json output
E08-T06: approve command
E08-T07: cancel command
E08-T08: resume command
```

완료 기준:

- `run`, `status`, `report`가 fake project에서 동작한다.
- `--json` 출력이 parse 가능하다.

## EPIC 09: Star Sentinel P0 Implementation

목표:

- Star Sentinel P0 rule evaluator와 output writers를 구현한다.

선행 문서:

```text
star-sentinel-full-spec.md
star-sentinel-p0-contracts.md
star-sentinel-p0-implementation-split.md
docs/decisions/0004-star-sentinel-p0-scope.md
approval-review-flow.md
policy-profiles.md
```

TASK 후보:

```text
E09a-T01: task input reader
E09a-T02: changed lines reader
E09a-T03: P0 rule registry loader
E09a-T04: 5개 P0 rule evaluator
E09b-T01: diagnostics writer
E09b-T02: gate decision writer
E09c-T01: review pack writer
E09d-T01: ledger writer
E09d-T02: selfcheck
```

완료 기준:

- P0 fixtures produce expected decision.
- Star Sentinel outputs validate against schemas.

## EPIC 10: ValidationEngine

목표:

- provider output을 Star Sentinel tool contract와 연결한다.

선행 문서:

```text
validation-engine.md
validation-handoff.md
star-sentinel-p0-contracts.md
approval-review-flow.md
```

TASK 후보:

```text
E10-T01: ValidationEngine scaffold
E10-T02: SentinelTask writer
E10-T03: validation_runs writer
E10-T04: approval decision reader
E10-T05: decision -> RunState mapping
E10-T06: review pack handoff
E10-T07: validation report section
```

완료 기준:

- AUTO_PASS/HUMAN_REVIEW/BLOCK decision이 RunState에 반영된다.

## EPIC 11: Integration Smoke

목표:

- fake project 기준 end-to-end smoke를 만든다.

TASK 후보:

```text
E11-T01: fake project fixture
E11-T02: run -> route -> execute -> validate -> report flow
E11-T03: AUTO_PASS smoke
E11-T04: HUMAN_REVIEW smoke
E11-T05: BLOCK smoke
E11-T06: final report smoke
```

완료 기준:

- fake provider 기반 전체 흐름이 CI에서 반복 가능하다.

## EPIC 12: Local Process Provider

상태: RESERVED until fake flow stable.

Milestone: M5 Local Provider.

목표:

- 허용된 로컬 명령만 실행하는 provider를 구현한다.

승인 필요 후보:

- shell execution policy
- timeout/cancel behavior
- dangerous action guard

## EPIC 13: Local Model Provider

상태: RESERVED.

Milestone: M5 Local Provider.

목표:

- 로컬 모델 서버 provider를 provider-neutral interface로 연결한다.

## EPIC 14: Cloud CLI Provider

상태: RESERVED.

Milestone: M6 Cloud CLI / Cloud API Provider.

목표:

- cloud CLI agent를 process/stdio/file handoff 방식으로 연결한다.

주의:

- 특정 제품명은 adapter package에만 제한적으로 사용한다.
- core package에는 제품명을 넣지 않는다.

## EPIC 15: Codex Provider Adapter Smoke

상태: RESERVED.

Milestone: M6 Cloud CLI / Cloud API Provider.

목표:

- Codex CLI 또는 Codex-like provider adapter를 smoke 수준으로 연결한다.

주의:

- 외부 계정 변경 또는 인증 설정은 명시 승인 필요.
- 자동 비용 발생 가능성은 budget guard로 기록한다.

## EPIC 16: Daemon

상태: RESERVED.

Milestone: M7 Daemon / API Control Plane.

목표:

- 장시간 queue와 provider session을 관리한다.

선행 조건:

- CLI fake flow 안정화
- StateStore resume/cancel 안정화
- approval flow 안정화

## EPIC 17: API

상태: RESERVED.

Milestone: M7 Daemon / API Control Plane.

목표:

- UI와 외부 도구가 상태를 읽고 제한된 mutation을 수행하게 한다.

초기 read-only API부터 시작한다.

## EPIC 18: UI Shell

상태: RESERVED.

Milestone: M8 UI Shell.

목표:

- job list, job detail, run timeline, approval/review 화면을 제공한다.

초기 read-only UI부터 시작한다. 현재 착수 큐에서는 `UiReadOnlyShell` read-only view model과 `UiBrowserShell` browser-oriented control shell model을 package layer에서 먼저 구현하고, 실제 browser UI app/runtime은 별도 승인 전까지 RESERVED로 둔다.

## EPIC 19: Security / Cost / Observability Hardening

목표:

Milestone: M9 Hardening / Conformance / Release Readiness.

- secret redaction, budget warning, audit log, metrics를 강화한다.

TASK 후보:

```text
E19-T01: redaction utility
E19-T02: forbidden action guard hardening
E19-T03: provider metrics
E19-T04: budget warning
E19-T05: audit report section
E19-T06: security profile fixtures
```

현재 착수 큐의 M9a는 E19-T01을 `packages/star-control-security`의 shared redaction utility와 RedactionReport builder로 구현한다. audit log, cost/budget guard, retention/recovery, release readiness는 후속 M9 slice로 남긴다.

## EPIC 20: Release Readiness

상태: RESERVED.

Milestone: M9 Hardening / Conformance / Release Readiness.

목표:

- release profile, changelog, versioning, publishing policy를 준비한다.

주의:

- release/deploy/publish는 명시 승인 전까지 구현하지 않는다.

## 다음 EPIC 진입 규칙

다음 EPIC으로 넘어가려면:

- 현재 EPIC의 필수 TASK가 완료되어야 한다.
- 관련 CI가 통과해야 한다.
- 문서와 schema 계약이 어긋나지 않아야 한다.
- 남은 TODO가 다음 EPIC을 막지 않아야 한다.
- approval required 변경이 있으면 사람이 승인해야 한다.
