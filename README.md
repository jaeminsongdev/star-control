# Star-Control

Star-Control은 여러 AI coding agent, cloud API model, local model server, local process runner, fake provider, human handoff를 공통 규격으로 다루는 provider-neutral 작업 관제 시스템이다.

현재 repository는 설계 계약과 Rust + Cargo workspace 기반 core runtime을 함께 키우는 단계다. CLI fake/local/cloud approval-gated flow, explicit provider-instance local OpenAI-compatible loopback execution, CLI providers read-only discovery/offline readiness healthcheck, CLI sentinel command group, daemon queue skeleton/app process/local HTTP API server/loopback CORS/HTTP control action audit integration/queue scheduler tick/local-process scheduler executor, API read-only/control mutation service, UI read-only view model, browser-oriented control shell model/static browser UI app, security/observability hardening/RedactionReport artifact storage, provider cost-metric sidecar integration, provider conformance, state recovery inspection/recovery command dry-run/approval surface/recovery action executor/artifact replacement source selection executor, release readiness writer/API read surface/version checker/evidence file reader/profile readiness builder/UI read surface/CLI read surface/review pack foundation/release automation dry-run/approval surface/approval-gated local executor/productization E2E smoke/final M9 readiness audit/final completion audit/final audit evidence/stacked PR readiness evidence/final evidence refresh/stacked merge procedure는 작은 PR 단위로 구현 중이며, daemon cloud/live scheduler executor, daemon Local AI live scheduler connector, external release/deploy/publish executor는 아직 별도 slice까지 RESERVED다.

v0 fake flow는 최종 목표가 아니라 첫 번째 반복 가능한 검증 마일스톤이다. 완전 구현은 `docs/decisions/0005-full-implementation-defaults.md`와 `docs/implementation/complete-implementation-roadmap.md`를 기준으로 local provider, cloud provider, daemon/API/UI, 운영 안정화까지 확장한다.

## 핵심 구성

- `.github/workflows/`: 현재 repository의 최소 CI 검증선.
- `docs/`: Star-Control 정본 설계, 구현 계약, 운영 문서, 결정 기록.
- `specs/`: JSON schema와 provider/tool 계약 후보.
- `configs/`: 기본 설정, 정책, 역할, hook, template, registry.
- `builtin-providers/`: 구체 provider manifest와 capability profile. 구현 코드는 두지 않는다.
- `builtin-tools/star-sentinel/`: Star Sentinel 내장 도구 manifest, policy, schema, template, corpus.
- `packages/`: `star-control-*` core Cargo workspace crate와 `packages/star-sentinel/` 구현 코드. 기존 provider/transport/adapter scaffold는 post-core 확장 후보로 둔다.
- `apps/`: CLI, daemon, UI 같은 사용자 표면. `apps/star-daemon`은 daemon process/API surface를 제공하고, `apps/star-control-ui`는 정적 browser UI app을 제공한다.
- `examples/`: provider instance, sample project, sample run artifact. 실제 실행 결과가 아니다.
- `scripts/ci/`: 문서, schema, manifest, naming policy 검증 스크립트와 productization E2E smoke.
- `integrations/`: 외부 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다.

상세한 현재 경로 상태와 목표 경계는 `docs/implementation/current-repository-map.md`와 `docs/implementation/repository-layout.md`를 기준으로 한다. 리팩토링과 하드코딩 정리는 `docs/implementation/refactoring-hardcoding-guidelines.md`를 기준으로 하며, 기능 추가나 approval-required action과 섞지 않는다.

## 구현 전 읽을 문서

Codex 또는 다른 구현자는 구현 전에 아래 문서를 먼저 읽는다.

```text
AGENTS.md
README.md
docs/implementation/README.md
docs/implementation/current-repository-map.md
docs/implementation/repository-layout.md
docs/implementation/refactoring-hardcoding-guidelines.md
docs/decisions/0002-runtime-stack.md
docs/decisions/0005-full-implementation-defaults.md
docs/implementation/codex-long-run-workflow.md
docs/implementation/codex-work-queue-current.md
docs/implementation/complete-implementation-roadmap.md
```

실제 구현 착수 순서는 `docs/implementation/codex-work-queue-current.md`를 우선한다. `docs/implementation/codex-work-queue.md`는 장기 backlog이며, 두 문서가 다르게 보이면 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 작업한다.

전체 완성형 milestone과 E11 이후 확장 순서는 `docs/implementation/complete-implementation-roadmap.md`를 기준으로 한다.

그 다음 작업 대상에 따라 `data-contracts.md`, `state-store.md`, `provider-system.md`, `router-engine.md`, `execution-engine.md`, `validation-engine.md`, `star-sentinel-full-spec.md` 등을 읽는다.

## Provider 원칙

Star-Control은 provider를 이름이 아니라 다음 축으로 판단한다.

- provider kind
- transport
- adapter
- capability profile
- provider instance

구체 provider는 `builtin-providers/` 아래 manifest로 등록하고, core package 이름에는 특정 회사나 제품명을 넣지 않는다.

## Star Sentinel

Star Sentinel은 AI가 만든 변경사항을 diff, policy, evidence, validation 기반으로 검증하고 review pack과 approval gate를 생성하는 내장 도구다.

구현 코드는 `packages/star-sentinel/`, 등록정보와 정책은 `builtin-tools/star-sentinel/`에 둔다.

## 실행 결과 위치

Star-Control repository에는 실행 결과를 저장하지 않는다. 대상 프로젝트에 다음 형태로 저장한다.

```text
대상 프로젝트/.ai-runs/J-0001/provider-output/{provider-instance-id}/
대상 프로젝트/.ai-runs/J-0001/tool-output/star-sentinel/
```

`examples/runs/`는 문서와 schema 검증을 위한 sample artifact이며 실제 실행 결과 저장 위치가 아니다.

## 현재 상태

이 repository의 `README.md`, `AGENTS.md`, `docs/`, `specs/`, `configs/`, `builtin-providers/`, `builtin-tools/star-sentinel/`, `examples/`, `scripts/ci/`가 Star-Control 설계 기준이다.

현재 단계는 설계 문서와 core Cargo workspace 구현이 병행되는 상태다. CLI file-based flow, daemon queue/process/API surface, loopback-only HTTP API와 CORS preflight, static browser UI app, explicit provider-instance local OpenAI-compatible loopback execution, provider read-only/offline readiness, Star Sentinel CLI, security/observability foundation, RedactionReport StateStore artifact storage, CLI report/provider output redaction artifact wiring, fake/local/cloud provider cost-metric sidecar, cloud hard budget enforcement, provider conformance, recovery action, release readiness/review/automation local executor, final audit evidence, stacked merge procedure를 작은 PR 단위로 구현 중이다.

제품화 진행 slice 기준으로 E49~E67은 daemon app, local HTTP API, static browser UI, daemon HTTP control audit, recovery dry-run/approval, release dry-run/approval, daemon scheduler tick, local-process scheduler executor, recovery action executor, release automation executor, artifact replacement source selection executor, productization E2E smoke, RedactionReport artifact storage, provider cost-metric sidecar integration, cloud hard budget enforcement, CLI report redaction artifact wiring, provider output redaction artifact wiring, external release execution policy, final readiness 정리까지 포함한다. E59는 `--recovery-artifact`와 `--recovery-source`가 current inspection issue와 일치하고 approval token이 맞을 때만 target artifact를 approved source로 교체한다. E60 `scripts/ci/productization_e2e_smoke.py`는 CLI fake run, providers offline healthcheck, daemon status/API, static UI surface, provider request redaction artifact, CLI report redaction artifact, external release policy, recovery dry-run, release local executor, Sentinel selfcheck를 Local/Cloud AI live connector disabled 상태로 검증한다. E61은 `StateStore::write_redaction_report_json`으로 schema-valid `audit/redaction-report.json` 저장과 ArtifactRef 생성을 검증한다. E62는 fake/local-process provider도 cloud provider와 같은 `provider-output/{provider_instance_id}/cost-metric.json` sidecar를 기록하도록 연결한다. E63은 cloud provider의 `budget.max_estimated_cost` hard limit이 `budget.estimated_cost`보다 낮으면 CLI/API transport를 실행하기 전에 `blocked` result로 정규화한다. E64는 `star-control report --json`이 shared redaction을 적용하고 secret-like finding이 있으면 `audit/redaction-report-<stage>.json`을 저장하도록 연결한다. E65는 fake/local/cloud provider output artifact를 저장하기 전에 secret-like string을 redaction하고 provider RedactionReport artifact를 남기도록 연결한다. E66은 release automation plan/result에 `external_execution_policy`를 포함해 external publish/deploy/signing live execution이 reserved임을 machine-readable하게 고정한다. E67은 final completion audit과 readiness example에서 구현 blocker를 Local/Cloud AI live connector 2개로 한정하고, release/deploy/destructive/main update는 approval-gated execution으로 분리한다.

아직 추가하지 않는 항목은 `ready` status, daemon scheduler 기반 Local/Cloud AI live connector, Cloud live execution, 실제 external release/deploy/publish live executor, main 병합이다. Local OpenAI-compatible server는 명시 provider-instance CLI 경로에서만 loopback live smoke가 가능하다.

현재 문서상 core runtime crate namespace는 `star-control-*`로 통일한다. 기존 `star-provider-*`, `star-transport-*`, `star-adapter-*` scaffold는 core가 안정화된 뒤 provider/transport/adapter extension package로 분류한다.
