# Star-Control

Star-Control은 여러 AI coding agent, cloud API model, local model server, local process runner, fake provider, human handoff를 공통 규격으로 다루는 provider-neutral 작업 관제 시스템이다.

현재 repository는 설계 계약과 Rust + Cargo workspace 기반 core runtime을 함께 키우는 단계다. CLI fake/local/cloud approval-gated flow, daemon queue skeleton, API read-only/control mutation service, UI read-only view model, browser-oriented control shell model, security/observability hardening, provider conformance, state recovery inspection/recovery command surface, release readiness writer/API read surface/version checker/evidence file reader/profile readiness builder/UI read surface/CLI read surface/review pack foundation/final M9 readiness audit/final completion audit은 작은 PR 단위로 구현 중이며, 실제 daemon process, HTTP API server, browser UI app, destructive recovery action, release/deploy/publish automation은 아직 별도 slice까지 RESERVED다.

v0 fake flow는 최종 목표가 아니라 첫 번째 반복 가능한 검증 마일스톤이다. 완전 구현은 `docs/decisions/0005-full-implementation-defaults.md`와 `docs/implementation/complete-implementation-roadmap.md`를 기준으로 local provider, cloud provider, daemon/API/UI, 운영 안정화까지 확장한다.

## 핵심 구성

- `.github/workflows/`: 현재 repository의 최소 CI 검증선.
- `docs/`: Star-Control 정본 설계, 구현 계약, 운영 문서, 결정 기록.
- `specs/`: JSON schema와 provider/tool 계약 후보.
- `configs/`: 기본 설정, 정책, 역할, hook, template, registry.
- `builtin-providers/`: 구체 provider manifest와 capability profile. 구현 코드는 두지 않는다.
- `builtin-tools/star-sentinel/`: Star Sentinel 내장 도구 manifest, policy, schema, template, corpus.
- `packages/`: `star-control-*` core Cargo workspace crate와 `packages/star-sentinel/` 구현 코드. 기존 provider/transport/adapter scaffold는 post-core 확장 후보로 둔다.
- `apps/`: CLI, daemon, UI 같은 사용자 표면 scaffold. daemon app process와 browser UI app은 아직 RESERVED다.
- `examples/`: provider instance, sample project, sample run artifact. 실제 실행 결과가 아니다.
- `scripts/ci/`: 문서, schema, manifest, naming policy 검증 스크립트.
- `integrations/`: 외부 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다.

상세한 현재 경로 상태와 목표 경계는 `docs/implementation/current-repository-map.md`와 `docs/implementation/repository-layout.md`를 기준으로 한다.

## 구현 전 읽을 문서

Codex 또는 다른 구현자는 구현 전에 아래 문서를 먼저 읽는다.

```text
AGENTS.md
README.md
docs/implementation/README.md
docs/implementation/current-repository-map.md
docs/implementation/repository-layout.md
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

현재 단계는 설계 문서와 core Cargo workspace 구현이 병행되는 상태다. CLI는 file-based flow를 우선하고, daemon은 M7b 기준 queue skeleton까지만 구현한다. API는 M7c/M7d 기준 HTTP server 없는 read-only/control mutation service까지만 구현한다. UI는 M8a/M8b 기준 API read-only/control service를 소비하는 library-level view/control shell model까지만 구현한다. Security hardening은 M9a 기준 shared redaction utility와 RedactionReport builder부터 구현했고, observability hardening은 M9b AuditEventWriter와 M9c CostMetricWriter/warn-only budget evaluation부터 구현한다. M9d provider conformance hardening은 ProviderConformanceChecker가 provider result/ref/file/schema 일치를 검증하도록 확장한다. M9e state recovery inspection은 StateStore가 missing/corrupt/tmp artifact를 inspect-only report로 분류하도록 확장한다. M9n CLI recovery command surface는 `star-control recover --list`로 inspection 결과를 표시한다. M9f release readiness writer는 `release/release-readiness.json`을 schema-valid artifact로 쓰고 읽는 수준까지 제공한다. M9g API read surface는 existing readiness artifact를 HTTP server 없는 `ApiReadOnlyService` response로 조회한다. M9h release version checker는 caller-provided version/changelog text로 readiness checks/blockers를 만든다. M9i release evidence file reader는 project root 내부 version/changelog file을 read-only로 읽는다. M9j release profile readiness builder는 profile pass/fail evidence와 version/changelog result를 같은 readiness artifact에 병합한다. M9k UI read surface는 release readiness API response를 `UiReadOnlyShell` job detail에 표시한다. M9l CLI read surface는 `star-control report --release-readiness`로 existing readiness artifact를 읽는다. M9m release review pack foundation은 existing readiness value를 Markdown review pack으로 렌더링한다. M9o final readiness audit은 M9 필수 hardening/recovery/release-readiness check를 schema-valid readiness value로 조립하되 all-pass도 `reserved`로 둔다. M9p final completion audit은 M0~M9 milestone, validation, CI, stacked PR, reserved action confirmation을 schema-valid readiness value로 조립하되 all-pass도 `reserved`로 둔다. `ready` status, destructive recovery action, 실제 release/deploy/publish automation은 아직 추가하지 않는다.

현재 문서상 core runtime crate namespace는 `star-control-*`로 통일한다. 기존 `star-provider-*`, `star-transport-*`, `star-adapter-*` scaffold는 core가 안정화된 뒤 provider/transport/adapter extension package로 분류한다.
