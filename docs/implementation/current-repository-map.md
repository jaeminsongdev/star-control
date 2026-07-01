# Current Repository Map

## 목적

이 문서는 현재 Star-Control repository에 존재하는 경로의 의미를 고정한다. `repository-layout.md`가 목표 package 경계를 설명한다면, 이 문서는 구현자가 실제 파일을 볼 때 어떤 경로가 정본이고 어떤 경로가 예약 영역인지 판단하게 해 주는 기준표다.

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. 이 문서는 repository 경로 상태를 설명하고, 현재 EPIC/TASK의 세부 순서는 `codex-work-queue-current.md`가 결정한다.

## 상태 표기

| 상태 | 의미 |
|---|---|
| `CANONICAL` | 현재 설계와 구현 계약의 정본 경로다. |
| `SCAFFOLD` | 목표 구조를 표시하기 위한 골격이다. |
| `RESERVED` | 장기 목표로 예약했지만 초기 구현 대상은 아니다. |
| `EXAMPLE` | schema, 문서, smoke 검증을 위한 예시다. |
| `BACKLOG` | 장기 구현 후보이며 현재 착수 큐보다 우선하지 않는다. |

## 현재 정본 경로

| 경로 | 상태 | 책임 |
|---|---|---|
| `README.md` | `CANONICAL` | repository 목적, 현재 상태, 첫 읽기 경로를 설명한다. |
| `AGENTS.md` | `CANONICAL` | 이 repository에서 작업하는 AI와 구현자가 지킬 작업 경계와 검증 기준이다. |
| `.github/workflows/` | `CANONICAL` | 현재 repository의 최소 CI 검증선을 둔다. |
| `docs/` | `CANONICAL` | 설계, 구현 계약, 운영 문서, 결정 기록을 둔다. |
| `docs/implementation/` | `CANONICAL` | 구현자가 따라야 하는 책임 경계, 데이터 계약, 실행 흐름, 검증 기준을 둔다. |
| `docs/operations/` | `CANONICAL` | ChatGPT, GitHub, CI, Codex 운영 기준을 둔다. |
| `docs/providers/` | `CANONICAL` | provider 개념, registry, capability 관련 문서를 둔다. |
| `docs/tools/` | `CANONICAL` | builtin tool 개요 문서를 둔다. |
| `docs/decisions/` | `CANONICAL` | 장기 결정 기록을 둔다. |
| `specs/schemas/` | `CANONICAL` | machine-readable JSON schema를 둔다. |
| `configs/` | `CANONICAL` | default config, policy, role, skill, hook, template, registry 후보를 둔다. |
| `builtin-providers/` | `CANONICAL` | builtin provider manifest와 capability profile을 둔다. provider 구현 코드는 두지 않는다. |
| `builtin-tools/star-sentinel/` | `CANONICAL` | Star Sentinel manifest, policy, schema, fixture, example, corpus를 둔다. |
| `examples/` | `EXAMPLE` | provider instance와 sample run artifact를 둔다. 실제 run output 위치가 아니다. |
| `scripts/ci/` | `CANONICAL` | repository policy, data format, manifest, naming, schema example, implementation docs 검증 스크립트를 둔다. |

## scaffold / reserved 경로

| 경로 | 상태 | 책임 |
|---|---|---|
| `apps/starctl/` | `SCAFFOLD` | 최종 CLI entrypoint 후보. 초기 구현 전에는 문서 골격만 둔다. |
| `apps/star-daemon/` | `RESERVED` | 장시간 local daemon app entrypoint 후보. M7b는 package-level queue skeleton만 구현하며 app daemon process는 아직 구현하지 않는다. |
| `apps/star-control-ui/` | `RESERVED` | browser UI shell 후보. M8a library-level view model은 `packages/star-control-ui/`에 둔다. |
| `packages/` | `CANONICAL` / `SCAFFOLD` | `star-control-*` Cargo workspace crate와 `star-sentinel` 구현 코드를 둔다. `star-control-api`는 read-only service까지 구현하며 HTTP server/mutation은 아직 reserved다. `star-control-ui`는 read-only view model까지만 구현한다. 기존 provider/transport/adapter scaffold는 post-core 확장 후보로 남긴다. |
| `integrations/` | `RESERVED` | GitHub ruleset, workflow, 외부 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다. |

## apps와 packages의 관계

`apps/`는 사람이 실행하는 표면을 나타내는 entrypoint scaffold다. `packages/`는 재사용 가능한 구현 module 경계다.

초기 구현 원칙:

1. 구현 코어는 목표상 `packages/` 아래 package 경계로 나눈다.
2. `apps/starctl`은 CLI entrypoint 후보이며 core logic을 직접 소유하지 않는다.
3. `packages/star-control-daemon`은 M7b에서 file-based queue skeleton만 구현한다.
4. `apps/star-daemon`과 `apps/star-control-ui`는 초기 구현 대상이 아니다. UI read-only view model은 package layer에서 먼저 구현한다.
5. 새 runtime dependency와 Cargo 외 package manager는 별도 승인 전까지 추가하지 않는다.

## builtin 경계

`builtin-providers/`와 `builtin-tools/`는 구현 코드 위치가 아니다.

```text
builtin-providers/             # provider manifest, capability profile
builtin-tools/star-sentinel/    # tool manifest, policy, schema, fixture, example
packages/star-sentinel/         # Star Sentinel 구현 코드 후보
```

Core package는 provider 제품명을 직접 포함하지 않는다. 새 provider는 manifest, capability profile, adapter 경계로 추가한다.

## 실행 산출물 위치

Star-Control repository 내부에는 실제 실행 산출물을 저장하지 않는다. 실제 run artifact는 대상 프로젝트 아래에 생성한다.

```text
{target-project}/.ai-runs/J-0001/
```

`examples/runs/`는 schema와 문서 검증을 위한 예시일 뿐 실제 실행 산출물이 아니다.

## naming 기준

Star Sentinel 공식 표기는 다음만 사용한다.

```text
Star Sentinel
star-sentinel
star_sentinel
star.sentinel
```

호환 alias는 `builtin-tools/star-sentinel/tool.yaml`의 `legacy_aliases`에만 둔다.

| 목적 | 표기 |
|---|---|
| CLI command | `review-pack` |
| JSON/Markdown artifact | `review_pack.json`, `review_pack.md` |
| package 후보 | `star-sentinel` |
| python entrypoint 후보 | `star_sentinel.main` |
| tool id | `star.sentinel` |

## 현재 계약 상태

현재 repository에는 v0 구현 착수를 위한 주요 계약 문서, schema, canonical example, 최소 CI 검증선이 들어 있다.

| 계약 묶음 | 현재 위치 | 상태 |
|---|---|---|
| core artifact 계약 | `specs/schemas/`, `examples/runs/`, `docs/implementation/data-contracts.md` | `CANONICAL` |
| StateStore / artifact layout | `state-store.md`, `state-store-recovery.md`, `artifact-layout.md`, `artifact-naming.md` | `CANONICAL` |
| provider 계약 | `provider-system.md`, `docs/providers/`, `examples/provider-contracts/` | `CANONICAL` |
| config / policy / role / hook 계약 | `config-system.md`, `examples/config-contracts/` | `CANONICAL` |
| router decision 계약 | `router-decision-matrix.md`, `router-engine.md`, `examples/router-contracts/` | `CANONICAL` |
| execution 계약 | `execution-engine.md`, `examples/execution-contracts/` | `CANONICAL` |
| Star Sentinel P0 계약 | `star-sentinel-p0-contracts.md`, `builtin-tools/star-sentinel/` | `CANONICAL` |
| validation handoff 계약 | `validation-engine.md`, `validation-handoff.md`, `examples/validation-contracts/` | `CANONICAL` |
| CLI / daemon queue / API read-only / UI read-only / reserved surfaces | `cli-command-reference.md`, `daemon-contract.md`, `api-contract.md`, `ui-shell-contract.md` | `CANONICAL` / `RESERVED` |
| CI 계약 검증 | `scripts/ci/`, `.github/workflows/ci.yml`, `ci-contract-validation.md` | `CANONICAL` |
| 현재 구현 큐 | `codex-work-queue-current.md` | `CANONICAL` |
| 장기 backlog | `codex-work-queue.md` | `BACKLOG` |

## 남은 정리 대상

아래 항목은 현재 구현 착수 전후로 보강할 수 있지만, `codex-work-queue-current.md`의 순서를 앞지르지 않는다.

| 대상 | 처리 기준 |
|---|---|
| handoff schema required field 강화 | 별도 schema/example PR에서 처리한다. |
| forbidden action vocabulary 고정 | schema/example/docs를 함께 수정한다. |
| work queue consistency CI | 별도 CI PR에서 추가한다. |
| E08 CLI 세부 분할 | 현재 큐 또는 후속 consistency PR에서 명시한다. |
| E09 Star Sentinel P0 세부 분할 | P0 evaluator/gate/review/selfcheck 단위로 정리한다. |
| local/cloud provider | fake flow 안정화 전까지 `RESERVED`다. |
| daemon process/API mutation/browser UI | daemon queue skeleton, API read-only service, UI read-only view model 이후에도 process, API server/mutation, browser UI app은 별도 slice까지 `RESERVED`다. |
| release automation | 별도 승인 전까지 `RESERVED`다. |
