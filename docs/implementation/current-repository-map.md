# Current Repository Map

## 목적

이 문서는 현재 Star-Control repository에 존재하는 경로의 의미를 고정한다. `repository-layout.md`가 목표 package 경계를 설명한다면, 이 문서는 구현자가 실제 파일을 볼 때 어떤 경로가 정본이고 어떤 경로가 예약 영역인지 판단하게 해 주는 기준표다.

## 상태 표기

| 상태 | 의미 |
|---|---|
| `CANONICAL` | 현재 설계와 구현 계약의 정본 경로다. |
| `SCAFFOLD` | 목표 구조를 표시하기 위한 골격이다. |
| `RESERVED` | 장기 목표로 예약했지만 초기 구현 대상은 아니다. |
| `EXAMPLE` | schema, 문서, smoke 검증을 위한 예시다. |

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
| `scripts/ci/` | `CANONICAL` | repository policy, data format, manifest, naming, schema example 검증 스크립트를 둔다. |

## scaffold / reserved 경로

| 경로 | 상태 | 책임 |
|---|---|---|
| `apps/starctl/` | `SCAFFOLD` | 최종 CLI entrypoint 후보. 초기 구현 전에는 문서 골격만 둔다. |
| `apps/star-daemon/` | `RESERVED` | 장시간 local daemon 후보. CLI file-based flow가 안정화된 뒤 구현한다. |
| `apps/star-control-ui/` | `RESERVED` | UI shell 후보. API와 read-only state view가 안정화된 뒤 구현한다. |
| `packages/` | `SCAFFOLD` | 목표 implementation package 경계. package manager 도입 전에는 실제 runtime package를 추가하지 않는다. |
| `integrations/` | `RESERVED` | GitHub ruleset, workflow, 외부 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다. |

## apps와 packages의 관계

`apps/`는 사람이 실행하는 표면을 나타내는 entrypoint scaffold다. `packages/`는 재사용 가능한 구현 module 경계다.

초기 구현 원칙:

1. 구현 코어는 목표상 `packages/` 아래 package 경계로 나눈다.
2. `apps/starctl`은 CLI entrypoint 후보이며 core logic을 직접 소유하지 않는다.
3. `apps/star-daemon`과 `apps/star-control-ui`는 초기 구현 대상이 아니다.
4. package manager와 runtime dependency는 별도 승인 전까지 추가하지 않는다.

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

## 후속 계약 정리 대상

이 문서는 현재 repository map만 고정한다. 아래 항목은 별도 PR에서 schema, example, CI 검증과 함께 닫는다.

| 후속 PR | 대상 |
|---|---|
| PR-02 | core artifact schema/example |
| PR-04 | provider manifest, instance, capability, registry schema |
| PR-06 | config, policy, hook, renderer, role, skill schema |
| PR-07 | router risk, approval, policy profile decision matrix |
| PR-11 | CLI command reference와 JSON/error output |
| PR-12 | daemon, API, UI reserved contract |
| PR-14 | docs, provider, config, policy contract validator |
