# P-0059 전체 기능 구현 상태 재감사

## 판정 기준과 현재 결론

감사 시작 subject는 `main`/`origin/main` commit `728c66297bfbfdbd425723b016435a0182933543`, tree `dc29567a14a66135277427c1f46ba1b5c0fc55da`다. P-0058의 결론은 탐색 경로로만 사용하고 현재 byte에서 다음 여섯 층을 다시 대조했다.

1. 정본 owner 문서
2. 실제 contract/type와 generated Schema hash
3. repository·Controller owning handler
4. CLI/Controller 및 적용되는 MCP·Codex 경로
5. 서로 다른 positive·negative·failure·recovery executable test
6. 위 입력 전체의 current source fingerprint

현재 source surface 판정은 `23/23 기능 PASS`, `16/16 Profile PASS`다. pre-seal FULL Gate는 `complete/stable/pass` 11/11이고 STRICT 자체 리뷰에는 BLOCKER·MAJOR가 없었다. 최종 current-byte FULL은 이 문서와 원장 byte를 고정한 뒤 다시 실행한다. 이 판정은 설치 Runtime update, 실제 외부 provider workload, ARM64 native 실행, Authenticode signing 또는 public publication 완료를 뜻하지 않는다.

상태 용어는 다음처럼 분리한다.

| 상태 | 의미 |
|---|---|
| `SOURCE_SURFACE_PASS` | 여섯 층의 선언·소유·경로·test가 current source byte에 있고 machine audit가 일치 |
| `LIVE_AVAILABLE` | 현재 설치 Runtime에서 해당 read-only 표면을 실제 호출해 확인 |
| `BLOCKED_OPERATIONAL` | 구현은 있으나 현재 설치/management 상태 때문에 live 제품 호출이 차단 |
| `NOT_RUN_EXTERNAL` | 외부 계정·네트워크·실효과 또는 사용자 데이터 변경이 필요한 E2E를 이번 감사에서 실행하지 않음 |
| `BLOCKED_EXTERNAL` | 제품 source로 닫을 수 없는 signer, trusted timestamp, public publication 또는 native hardware 증거가 없음 |

## 공통 시스템 기반

| 기반 | source 판정 | 현재 live/외부 판정 |
|---|---|---|
| CLI-only 본체와 application service | `SOURCE_SURFACE_PASS`; 23개 기능별 CLI/Controller command와 owning handler가 inventory에 고정됨 | 설치 `star.exe`는 실행 가능하나 설치 source가 현재 main보다 오래됨 |
| Controller single writer·durable state | `SOURCE_SURFACE_PASS`; SQLite repository, revision/CAS, backup·restore·rebuild·recovery test가 연결됨 | 설치 Runtime management는 `recovery_only`; 쓰기나 일반 management 경로를 성공으로 승격하지 않음 |
| Codex Plugin·Hook | template source와 installer/integration lifecycle이 source·test에 존재 | 설치 integration은 `verified/registered`; 새 task와 Hook trust가 필요 |
| 고정 MCP Gateway·live Registry | fixed MCP contract, core package action, search/describe/call·LKG test가 존재 | Registry revision 7, `star.core.doctor` PASS, core action 17개 ready |
| Codex App Server 연결 | capability/route와 start·resume·fork·interrupt adapter, fake-process 4종 test가 존재 | P-0058 signed-byte live evidence는 historical retrieval handle; 이번 감사에서 provider turn은 재실행하지 않아 `NOT_RUN_EXTERNAL` |
| Catalog·Schema·Corpus | 기능 23, Profile 16, Runtime 4, generated Schema 213, stable error 528, MCP matrix 170/170 exact 검사 | source Catalog가 정본이고 설치 Catalog는 별도 revision |
| 외부 도구 Adapter | Git/GitHub, validation, Windows installer/updater와 typed receipt 경로가 존재 | 실제 remote publish, signer, user-state recovery는 실행하지 않음 |
| Windows Runtime | `star`, `star-controller`, `star-mcp`, `star-updater` 4/4 source/manifest 확인 | x64 `0.1.0` 설치 verified; ARM64 native와 signed Stable은 별도 Gate |
| fail-closed 상태 원칙 | partial/stale/unverified/flaky/outcome_unknown과 recovery-only를 pass로 바꾸지 않는 test가 기능별 failure/recovery ref에 고정됨 | live management 오류도 `MANAGEMENT_RECOVERY_REQUIRED`로 보존됨 |

## A 계열 10개

| ID | current source 구현 근거 | 판정 |
|---|---|---|
| A01 | `TaskSpec`/Goal contract, `GoalStore`, goal lifecycle CLI·MCP, revision·restart tests | `SOURCE_SURFACE_PASS` |
| A02 | `StageSpecV1`/`StageGraphV1`/`StageResultV1`, DAG·replan handler와 completed-stage/recovery tests | `SOURCE_SURFACE_PASS` |
| A03 | Project/Workspace/Index/ContextPack contract, multi-root scan/index와 freshness·secret·stale tests | `SOURCE_SURFACE_PASS`; 13개 canonical root는 `LIVE_AVAILABLE` |
| A04 | planning bundle, impact/risk/affected-check handler, `validation.plan` MCP와 graph-limit/replan tests | `SOURCE_SURFACE_PASS`; current FULL plan `ready` |
| A05 | App Server Schema/model probe, `CapabilitySnapshotV1`, `RouteDecisionV1`, timeout·stale·fallback tests | `SOURCE_SURFACE_PASS`; current provider probe `NOT_RUN_EXTERNAL` |
| A06 | Context/permission binding과 Codex start·resume·fork·interrupt, approval/effect/deadline/recovery tests | `SOURCE_SURFACE_PASS`; current provider lifecycle `NOT_RUN_EXTERNAL` |
| A07 | durable management repository, checkpoint/backup/restore/rebuild와 crash/corruption tests | `SOURCE_SURFACE_PASS`; 설치 state는 `BLOCKED_OPERATIONAL` |
| A08 | `PermissionPlan`, exact approval/trust/effect receipt와 expiry/default-deny tests | `SOURCE_SURFACE_PASS` |
| A09 | worktree/merge plan·queue·run, overlap/cycle/receipt/recovery tests | `SOURCE_SURFACE_PASS`; 실제 multi-repo merge `NOT_RUN_EXTERNAL` |
| A10 | source Catalog, live Tool Registry, managed registry, Profile resolution과 LKG/fuzz recovery tests | `SOURCE_SURFACE_PASS`; live core Registry만 `LIVE_AVAILABLE` |

## B 계열 9개

모든 B 기능은 별도 성공 엔진이 아니라 공통 `ValidationPlan → ChangeSet 재수집 → Tool → Diagnostic → baseline/suppression → GateDecision → EvidenceBundle/ReviewPack` 경로의 contract·handler·test를 사용한다.

| ID | current source 구현 근거 | 판정 |
|---|---|---|
| B01 | validation runner, claim/current diff binding, evidence/review packaging, timeout/flaky/cache tests | `SOURCE_SURFACE_PASS` |
| B02 | test trust Rule, weakening·flaky·partial 거부, stable unsuppressed cache tests | `SOURCE_SURFACE_PASS` |
| B03 | sealed Validator Guard, checked-in Corpus, false-positive/weakening/expected-behavior tests | `SOURCE_SURFACE_PASS` |
| B04 | contract snapshot/compare, compatibility report, managed-registry drift와 style recovery path | `SOURCE_SURFACE_PASS` |
| B05 | dependency/supply-chain/Radar handler, provenance/freshness/partial/unknown tests | `SOURCE_SURFACE_PASS`; current external advisory refresh는 `NOT_RUN_EXTERNAL` |
| B06 | failure identity, reproduction v2, regression/recovery, exact failed-run·Diagnostic·input binding tests | `SOURCE_SURFACE_PASS` |
| B07 | documentation/config/environment/doctor, managed registry·config defaults·restart-safe read tests | `SOURCE_SURFACE_PASS` |
| B08 | migration checkpoint, performance cohort, equivalence/cutover, resume/rollback·ARM64 boundary tests | `SOURCE_SURFACE_PASS`; workload·ARM64 native는 `NOT_RUN_EXTERNAL` |
| B09 | build-once release manifest, verification/promotion, digest/signing/publish rollback tests | `SOURCE_SURFACE_PASS`; signed/public release는 `BLOCKED_EXTERNAL` |

## C01과 16개 Profile

`catalog/profiles/*.toml`의 exact 16개 descriptor는 모두 `schema_version = 2`, `profile_version = 1.1.0`이다. 공통 resolution, validation selection, permission, recovery, rollback handler/test 10개가 current-byte evidence에 포함되고 required Rule/Check/Gate/approval/effect/unknown/rollback policy가 Profile fingerprint에 고정된다.

| Profile | descriptor·공통 엔진 판정 | 실제 외부/효과 경계 |
|---|---|---|
| `change_planning` | `SOURCE_SURFACE_PASS` | live user project plan 미실행 |
| `refactor_codemod` | `SOURCE_SURFACE_PASS` | user checkout apply 미실행 |
| `api_contract_change` | `SOURCE_SURFACE_PASS` | consumer migration 미실행 |
| `docs_config_environment` | `SOURCE_SURFACE_PASS` | clean-room workload 미실행 |
| `debug_recovery` | `SOURCE_SURFACE_PASS` | actual failure recovery 미실행 |
| `security_supply_chain` | `SOURCE_SURFACE_PASS` | external refresh 미실행 |
| `dependency_upgrade` | `SOURCE_SURFACE_PASS` | package-manager apply 미실행 |
| `data_config_db_migration` | `SOURCE_SURFACE_PASS` | user data migration 미실행 |
| `performance_build` | `SOURCE_SURFACE_PASS` | opt-in workload 미실행 |
| `ci_release_deploy` | `SOURCE_SURFACE_PASS` | publish/deploy 미실행 |
| `language_platform_migration` | `SOURCE_SURFACE_PASS` | native cutover 미실행 |
| `ai_development_validation` | `SOURCE_SURFACE_PASS` | current FULL Gate가 이번 Slice의 acceptance |
| `test_correctness` | `SOURCE_SURFACE_PASS` | current FULL Gate가 이번 Slice의 acceptance |
| `architecture_quality` | `SOURCE_SURFACE_PASS` | current FULL Gate가 이번 Slice의 acceptance |
| `project_understanding` | `SOURCE_SURFACE_PASS` | 13개 root identity만 live read; full workload scan 미실행 |
| `rust_style_auto_fix` | `SOURCE_SURFACE_PASS` | user checkout auto-apply 미실행 |

## D 계열 3개

| ID | current source 구현 근거 | 판정 |
|---|---|---|
| D01 | cross-repo bundle, remote snapshots/operations, GitHub adapter, exact approval와 timeout reconciliation tests | `SOURCE_SURFACE_PASS`; final native Git push 외 제품 remote E2E는 `NOT_RUN_EXTERNAL` |
| D02 | EvaluationRun/Case/Cost/Budget, comparable cohort와 reject/unknown/tombstone tests | `SOURCE_SURFACE_PASS`; provider cost workload 미실행 |
| D03 | 4 Runtime, installer/updater/lifecycle/source audit V2, interrupted recovery·ARM64 evidence tests | `SOURCE_SURFACE_PASS`; installed source stale/recovery-only, signed public Stable·ARM64 native는 `BLOCKED_EXTERNAL` |

## 설치 Runtime과 current source 분리

read-only live 확인 결과는 다음과 같다.

- source: `728c66297bfbfdbd425723b016435a0182933543`
- installed release source: `b20d234b38a7dcb347049b6b95aff3407c5dedc9`
- install: x64 `0.1.0`, `verified=true`, Codex integration `registered`
- active generation: `rt_c569d8e23ed61e8e`, Registry revision 7
- doctor: Controller command contract, 13개 project active set/identity/registration Gate 모두 PASS
- management: `recovery_only`; `RECOVERY_STORE_MIGRATION_REQUIRED`, `RECOVERY_ACTIVE_SET_MATERIALIZATION_MISMATCH`
- live `profile list`: `MANAGEMENT_RECOVERY_REQUIRED`

따라서 source 구현 완료를 현재 설치 Runtime의 모든 기능 live-ready로 승격하지 않는다. 설치 update/restart와 management recovery는 실제 사용자 상태와 system installation을 바꾸므로 이번 source audit·commit 범위에 포함하지 않는다.

## 이번 Slice의 수정과 남은 Gate

- P-0058의 오래된 `follow-up commit 대기` 원장을 실제 `728c662` push 상태로 정정했다.
- P-0059 bounded Context Pack을 등록했다.
- 위 문서/원장 byte 변경에 맞춰 `catalog/product-source-evidence.json`을 마지막 source byte에서 다시 생성한다.
- machine inventory는 기능 `23/23`, Profile `16/16`, Runtime `4/4`, generated Schema `213`, stable error `528`, MCP matrix `170/170`을 요구한다.
- 제품 로직의 새 결함은 source inspection과 machine inventory 단계에서 발견되지 않았다. 최종 FULL 실패가 나오면 해당 결함을 수정하고 같은 Gate를 다시 실행한다.
- Authenticode certificate/private key/trusted timestamp, public release publication/readback, ARM64 native와 실제 사용자 management recovery는 이번 완료 판정 밖에 둔다.

## pre-seal 검증과 자체 리뷰

- Star action: `star.core.validation.run`, requested/effective `full`
- operation: `opn_01KYD0NF1JV44AMD72C61J4HCQ`, `succeeded`
- report: `target/validation/20260725T161024348Z-12356/report.json`
- evidence SHA-256: `sha256:9b33662cee9421869a5cd87015bd33836c400d0b45065c902b15a3ed14009263`
- 결과: exit `0`, `complete/stable/pass`, 11/11, 131.464초
- STRICT review: `APPROVE_WITH_NOTES`; scope 밖 변경, test 약화, public Schema/handler 누락, evidence 수동 조작은 없음. 설치 Runtime의 old source/recovery-only와 외부 signing·publication·ARM64 native 미검증은 completion으로 승격하지 않는다.

최종 FULL report와 commit/push SHA는 문서 자기참조를 만들지 않도록 이 파일 수정 뒤 생성되는 handoff evidence가 소유한다.
