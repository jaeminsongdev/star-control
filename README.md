# Star-Control

Star-Control은 여러 AI coding agent, cloud API model, local model server, local process runner, fake provider, human handoff를 공통 규격으로 다루는 provider-neutral 작업 관제 시스템이다.

현재 repository는 구현 전 설계와 계약을 고정하는 단계다. 실제 앱, daemon, UI, runtime package, package manager는 별도 승인과 별도 PR에서 추가한다.

## 핵심 구성

- `.github/workflows/`: 현재 repository의 최소 CI 검증선.
- `docs/`: Star-Control 정본 설계, 구현 계약, 운영 문서, 결정 기록.
- `specs/`: JSON schema와 provider/tool 계약 후보.
- `configs/`: 기본 설정, 정책, 역할, hook, template, registry.
- `builtin-providers/`: 구체 provider manifest와 capability profile. 구현 코드는 두지 않는다.
- `builtin-tools/star-sentinel/`: Star Sentinel 내장 도구 manifest, policy, schema, template, corpus.
- `packages/`: 목표 구현 package 경계. package manager 도입 전에는 구현 코드를 추가하지 않는다.
- `apps/`: CLI, daemon, UI 같은 사용자 표면 scaffold. 초기에는 RESERVED다.
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
docs/implementation/codex-long-run-workflow.md
docs/implementation/codex-work-queue-current.md
```

실제 구현 착수 순서는 `docs/implementation/codex-work-queue-current.md`를 우선한다. `docs/implementation/codex-work-queue.md`는 장기 backlog이며, 두 문서가 다르게 보이면 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 작업한다.

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

현재 단계는 스캐폴드와 설계 문서 상태다. 실제 앱, daemon, UI, package manager는 아직 추가하지 않는다.
