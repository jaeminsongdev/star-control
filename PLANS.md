# PLANS.md

## 목적

이 문서는 Star-Control `v0.1.0`의 현재 판단에 필요한 bounded snapshot만 소유한다. 계약은 [문서 읽는 순서](docs/README.md), 단계 정의는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 실측은 `docs/testing/`과 생성 evidence가 소유한다. 과거 P-ID `DONE`은 그 exact source/Slice의 historical seal이며 현재 source 완료 근거로 자동 승격하지 않는다.

## 확정 불변식

- source/manifest/Catalog가 canonical이고 DB/index/cache는 derived 또는 명시적 local-only state다. Controller만 persisted current projection을 쓴다.
- `partial|stale|unsupported|unverified|not_run|flaky|outcome_unknown`을 pass로 바꾸지 않는다.
- CLI-only core에는 local/other AI, OpenAI API 직접 호출, browser/HTTP control UI와 자체 scheduler가 없다.
- x64 공개 Stable은 Authenticode와 trusted timestamp가 필수다. unsigned Stable publish는 fail-closed다.
- ARM64는 Rust 1.96 cross-build·PE·cfg/target simulation 기반 Preview이며 `native_unverified`다.
- `legacy/`, `target/`, Codex runtime DB/cache와 실제 사용자 management/project data를 임의 정리하거나 손상시키지 않는다.

## 현재 상태

| 범위 | 상태 | 현재 판정 |
|---|---|---|
| P-0039~P-0053 | historical `DONE` | 각 문서의 bounded Slice만 보존 |
| P-0054 | historical internal seal | `a93de7e` 기준 구현/검증. 현재 source status로 재사용하지 않음 |
| P-0055 | historical non-signing seal | exact `0d0eca9a` package/lifecycle/GitHub evidence. 현재 source artifact로 재사용하지 않음 |
| P-0056 internal | `DONE / exact FULL PASS` | 최신 main 재감사, v2 validation/planning·EffectiveConfig·recovery bundle v2, M7/M8 receipt/current evidence, M9 handoff, M10 case/policy·cost/budget·actual finding/suppression/Radar·23/16 audit와 M11 공통 Patch/Gate 경로 구현. generated 202 Schema, workspace test·all-feature Clippy·170/170 matrix와 TARGET effective FULL 10/10 PASS |
| P-0056 external | `DONE / non-signing unpublished seal` | exact `1bce4724` RELEASE 비서명 범위, x64 격리 lifecycle·rollback, ARM64 cross simulation, installer·SBOM·RustSec·pre-sign provenance, GitHub draft byte 왕복·cleanup과 remote commit/tree readback PASS |
| P-0057 current-system audit | `DONE / source fixed` | 세 기준선 분리, live lifecycle·17 action E2E, recovery·trust/Registry·13개 repo·비서명 install/supply-chain·ARM64 경계 감사. empty-v1 migration/active-set, watcher root dedupe, destructive action approval 결함은 source에서 수정 |
| P-0058 feature/Profile independent audit | historical `SOURCE_PASS / SIGNED_LIVE_PASS / pushed` | 독립 감사와 발견 결함 수정은 `02f815a`, signed Codex App Server lifecycle 보강은 `728c662`로 `origin/main`에 반영. 23/23·16/16·Runtime 4/4·Schema 213·error 528·MCP 170/170 당시 evidence이며 current source 판정으로 자동 상속하지 않음 |
| P-0059 current feature implementation audit | `SOURCE_PASS / FULL_PASS / STRICT_APPROVE_WITH_NOTES` | current `728c662`에서 공통 기반, A01~D03 23개 기능과 C01 16개 Profile을 여섯 층으로 재감사해 23/23·16/16·Runtime 4/4·Schema 213·error 528·MCP 170/170 PASS. pre-seal FULL 11/11 PASS, BLOCKER·MAJOR 없음. 설치 `b20d234` recovery-only와 외부 Gate는 분리하며 final current-byte FULL·commit/push identity는 handoff가 소유 |
| installed P-0057 update | `DONE / verified` | 설치 source `f496f6e732cb9692da7716807b724a75c6ca4d05`, `D:\도구\Star-Control` installation verified, Codex integration registered. management는 별도 `recovery_only` 상태로 보존 |
| P-0060 Codex 전체 기능 routing·delivery | `STATUS_REPAIR_IN_PROGRESS` | 중앙 Skill·23/16 routing·7-file installer/updater delivery와 forced-close 재실행은 구현·검증됨. `6430219` x64 설치 및 자동 재기동은 관찰됐으나 7-file render를 status가 5-file로 재수집해 receipt가 `partially_applied`; 공통 component closure와 disk round-trip 회귀 test를 수정하고 최종 재설치 준비 중 |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish 필요 |

## P-0056 현재 구현 범위

- Rule·Diagnostic·Baseline·Suppression·Disposition v2, deterministic migration/projection와 revisioned repository
- ChangePlan/PlanningBundle v2 및 v1→v2 plan/apply/rollback
- sealed Validator Guard corpus와 528-code closed stable error catalog
- `EffectiveConfigV1` User→Project→Goal→Command merge, provenance/fingerprint, typed CLI/MCP override와 runtime 변경 허용 policy materialization; fixed/reserved product key는 shipped invariant만 허용
- `LocalStateBundle` v2의 v1/v2 decision round-trip, legacy v1 compatibility, stale/duplicate/tamper/current-target admission
- normal backup의 global PlanningBundle 보존과 portable bundle의 cross-store exclusion/loss-report 경계
- source rebuild의 selected checkout별 exact config 재해석, 한 checkout 또는 유일한 main worktree 선택과 ambiguous clone fail-closed
- M7 reproduction semantic observation·terminal effect receipt와 current regression evidence
- M8 effectful phase receipt·current validation/restore/equivalence, M9 merge/remote/release handoff current evidence
- M10 precommitted evaluation case/policy, provider-verified cost/budget, current Gate·DiagnosticEvaluation 기반 Finding/Suppression metric, Radar exact EvaluationRun ref
- `star release audit`의 A01~D03 23개 handler, exact 16 Profile, M11 closure와 release/lifecycle evidence 분리 판정
- generated Schema manifest 202 files와 minimal/full/invalid/future fixture, 528-code closed stable error catalog

상세와 항목별 증거는 [P-0056 최신 기능·복구 감사](docs/testing/p0056-current-functional-recovery-audit-2026-07-24.md)가 소유한다.

## P-0057 revision·byte 기준선

| 표면 | exact subject | 현재 증거와 판정 |
|---|---|---|
| audit start source | `main`/`origin/main` commit `816be57b4de21c0da937d2500686fc8395293ef6`, tree `c0339f71e6f4d985bcfacb515917c3e899044f53` | pre-change FULL 10/10 `complete/stable/pass`; `target/validation/20260724T224815675Z-9068/report.json`, `sha256:cd64d3773bd732e7a4cb2ff823bbe032a0fe31838ed972d20cc637476106c438` |
| P-0057 fixed source | 이 bounded snapshot을 포함하는 final local `main` HEAD; exact commit/tree/report hash는 생성 evidence와 최종 handoff가 고정 | current HEAD FULL 10/10 `complete/stable/pass`; RELEASE source-owned 14 PASS·failed 0, external signing/publication 1 `unverified` |
| P-0056 artifact | commit `1bce4724c34414cef74862dbe9bf9de1f094ad2f`, tree `4e1c3b1d55bfbe35eb7eaf4455c02bde711bcac4` | `dist/release-evidence/p0056-1bce4724`; 비서명 RELEASE·package·lifecycle·공급망·unpublished provider evidence의 historical exact seal이며 live source/설치 Runtime에 상속하지 않음 |
| installed Runtime | release source `b20d234b38a7dcb347049b6b95aff3407c5dedc9`, active generation `rt_c569d8e23ed61e8e` | root manifest `sha256:d96015a0bdb6f2fc437e0251a87266acecb20b12b79d629af6486df606edbe0c`; Runtime manifest `sha256:634a36495a2fe51937e0ba6369f30287021bcfa036fd7c5e1de8cef566e36ae9`; install/update verify와 doctor PASS, Registry revision 7 |

## P-0059 현재 Context Pack

- repo/branch: `D:\개발\관제\Star-Control`, `main`; 시작 HEAD/tree `728c66297bfbfdbd425723b016435a0182933543` / `dc29567a14a66135277427c1f46ba1b5c0fc55da`, 시작 worktree clean, `origin/main`과 동기화
- current Slice: 공통 기반과 A01~D03 23개 기능, C01 16개 Profile의 actual implementation 전수 감사 및 current-byte 결함 수정. P-0058 결과는 retrieval handle일 뿐 완료 증거로 상속하지 않음
- audit order: source/install/evidence identity → 공통 evidence acceptance → 상태/저장 → Registry/안전 → 이해/계획/배정/실행 → 공통 validation → mutation/contract/failure/security → multi-project/Profile/release/evaluation/lifecycle
- completion criterion: 각 기능의 정본 owner, generated contract/Schema, repository·Controller handler, CLI/MCP/Codex 경로, positive·negative·failure·recovery test, current fingerprint를 대조하고 FULL Gate와 STRICT self-review를 통과
- completed: machine inventory와 owning surface를 current byte에서 대조했고 제품 로직의 새 결함은 발견하지 않음. pre-seal FULL 11/11 PASS, STRICT `APPROVE_WITH_NOTES`; 상세 근거는 `docs/testing/p0059-current-feature-implementation-audit-2026-07-26.md`
- handoff: 최종 source byte의 FULL report와 commit/push exact identity는 task final response가 소유하며 PLANS에 자기참조 SHA를 쓰지 않음
- 금지: 실제 사용자 management/project data 변경, `legacy/`·`target/` 정리, Codex runtime DB/cache 직접 수정, dependency/system setting·signing·publication. 최종 source commit/push만 사용자가 이번 목표에서 명시적으로 승인

## P-0060 현재 Context Pack

- 목표: Codex가 Star-Control 23개 기능과 16개 Profile을 고정 MCP와 CLI-only surface까지 함께 사용할 수 있게 하고, installer/updater가 동일 Plugin 자산을 자동 생성·교체·rollback 검증한다.
- 범위: `AGENTS.md`, Codex Plugin Skill·metadata·reference, fixed MCP/SessionStart 문구, Codex adapter closed asset set, Windows updater 회귀 검증, 설치 계약·inventory와 package artifact.
- 유지: `.mcp.json` fixed Gateway, Hook 7종 topology, Codex 전역 config/rules, Feature/Profile Catalog 정본, `legacy/`, `target/`, 사용자 cache/runtime DB 직접 수정 금지.
- current source: local `main` `6430219f6d8e255dfb9b20f7d51bdc83cea5381e`, `origin/main` `07dc4396ad2fdfad26c61fabb4ccc3f3117055c0`; Codex status 재수집 회귀 fix, regenerated product source evidence와 `PLANS.md`가 working diff다.
- live blocker: 설치 파일은 `6430219` byte로 교체됐고 새 Plugin `0.1.0+codex.925d62b13ac4`가 로드됐다. 그러나 active Runtime selector는 실패 복구로 이전 generation이며 management는 `recovery_only`; `management rebuild plan --json`은 `MANAGEMENT_IDENTITY_CONFLICT`로 fail-closed다. 최종 updater 거래가 `exited`하고 새 Runtime이 활성화된 뒤 read-only rebuild plan을 재생성한다.
- 검증: `target/validation/20260726T032045633Z-34388/report.json`은 status 회귀 fix·source evidence·원장 diff를 MCP `star.core.validation.run`으로 실행해 FULL 11/11 PASS, stable/complete, 139861ms. Profile resolve는 management recovery 때문에 `MANAGEMENT_RECOVERY_REQUIRED`로 차단되어 fingerprint는 아직 `unverified`; live 설치 후 다시 고정한다.
- package evidence: `dist/release-evidence/p0060-6430219/`; x64 stage set `sha256:4508b15f745d3bfa31a8dc68ccf646f750fb637ad4814ce858df775ea9a962d4`, installer `sha256:7a7c56eae73800f2f075b75f432e191fa15f760c6163b3ad12b0ceb165792b70`, updater `sha256:fe63175da86193295de66215cdb35fdac5570cfeba21bf496162f4604a67c4fe`; ARM64 stage set `sha256:6fa5ec3ecf2e922d71a4ec7e19c62f41e71523c6688ebd5eaa72bb57f4cf79fd`, installer `sha256:7d332f9d9d07c1c6c6aa20e78ae5c0b84ffd7409301523124ccf9916bfec3f1b`, updater `sha256:83fc9a64d4d43e5a5a30af1306a612b8fa2e9c940e4df28d0ed52b09d017759d`. 두 package 모두 531 files·필수 Codex 자산 7/7, `unsigned_local`/`NotSigned`다.
- restart evidence: `upd_frw...`는 forced-close 직후 exit wait 누락으로 `aborted`; `upd_cQi...`는 사용자가 installer post-step 전에 Codex를 수동 기동해 `ActiveCodexDesktop`(exit 7)로 `rollback_required`; `upd_xHB...`는 자동 재기동과 새 Plugin load까지 관찰됐지만 status의 5/7 hash mismatch로 `partially_applied`. 이 상태를 성공으로 승격하지 않고 component closure 수정 후 exact final installer로 재시도한다.
- 외부 Gate: Authenticode/trusted timestamp/publication은 `blocked_external`; unsigned local artifact를 Stable 또는 published로 승격하지 않는다.

## 현재 Gate

1. live source·P-0056 artifact·installed Runtime exact subject 분리: **DONE**.
2. installed Runtime 기본 verify/doctor와 current HEAD FULL 10/10: **DONE**.
3. restart receipt·management recovery·Registry optional root 분류와 core 17 action E2E: **DONE**.
4. persisted recovery·권한/trust·M1~M6·13개 cross-repo·비서명 설치 lifecycle·ARM64 cross 검증: **DONE**. 실제 13 repo workload는 `not_run`, ARM64 native는 `native_unverified`.
5. P-0057 source fix의 설치/update와 수동 Codex restart: **`DONE`**. source `f496f6e`, installation verified, integration registered.
6. Authenticode·trusted timestamp, signed-byte lifecycle·provenance와 public Stable publish/readback: **`blocked_external`**. 별도 승인과 외부 signer 없이는 실행하지 않고 unsigned Stable로 우회하지 않는다.
7. P-0059 23개 기능·16개 Profile current source surface와 pre-seal FULL: **`PASS`**. STRICT review는 `APPROVE_WITH_NOTES`; 최종 current-byte FULL identity는 handoff가 소유한다.
8. installed file tree `6430219`, 이전 active Runtime generation의 management recovery-only 상태: **`blocked_operational`**. rebuild dry-run이 `MANAGEMENT_IDENTITY_CONFLICT`로 닫혔으며 exact plan fingerprint가 생성되기 전 apply하지 않는다.
9. P-0060 source·installer·updater 구현과 `6430219` x64·ARM64 package: **`SOURCE_PASS / LIVE_PARTIALLY_APPLIED`**. 자동 재기동은 관찰됐지만 7-file status closure 결함 때문에 receipt가 terminal success가 아니며, 회귀 fix commit·FULL·최종 package·재설치·live integration/Runtime/management 검증은 **`IN_PROGRESS`**.
