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
| P-0060 Codex 전체 기능 routing·delivery | `LIVE_RESTART_PASS / FINAL_INSTALL_PENDING` | 중앙 Skill·23/16 routing·7-file installer/updater delivery, 공통 component closure와 disk round-trip 검증을 구현. 비동기 forced-close를 fresh census 반복으로 판정한 `upd_WHA...`가 `exited`하고 Codex 자동 재기동까지 실측됐으며, exact final installer 재생성·설치와 Runtime/management 후검증이 남음 |
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
- source lineage: 7-file status closure는 `db5e704d14838c8412173900e624dc637cb2470d`까지 commit됐고 `origin/main` 기준선은 `07dc4396ad2fdfad26c61fabb4ccc3f3117055c0`이다. bounded 반복 종료·원장·product source evidence가 final source candidate이며 exact commit/FULL identity는 자기참조를 피하도록 task handoff가 소유한다.
- live blocker: 설치 fixed file은 아직 `6430219` byte이고 active Runtime selector도 실패 복구된 이전 generation이라 management는 `recovery_only`다. 수정 updater의 integration repair 거래는 `exited`했지만 이전 설치 CLI가 5-file 계약으로 status를 읽어 `rendered Codex Plugin does not satisfy the integration contract`를 유지한다. exact final installer 설치 후 installation·integration·Runtime·management를 다시 검증한다.
- 검증: exact clean `db5e704`의 `target/validation/20260726T032520036Z-27888/report.json`은 FULL 11/11 PASS, stable/complete, 133549ms이며 evidence hash는 `sha256:62ededecd3edfd79872c78ff18c37647ca2b37e224b57228385dd7d58d9395e3`다. bounded 반복 종료 변경은 package unit 16/16과 live restart `exited`를 통과했고 final-byte FULL은 evidence 재생성 뒤 실행한다. Profile resolve는 management recovery 때문에 아직 `unverified`다.
- package evidence: `dist/release-evidence/p0060-db5e704/`; x64 stage set `sha256:81dcc0c769ff072c876e70e28ad8dce35a0cfecd1246eb74eaf2b8b0dad05cf7`, installer `sha256:c7441d97e66287520251d47891b9995d9e922ba03190a043e7ddadb6c52fde09`, updater `sha256:0f50093cfb1bb02c7528981d1e27323b79a168db4d0030f0cd14826f00e47a64`; ARM64 stage set `sha256:18ae6278b272856234588e4a8a391260f1c384255a662843b83e6490d6c3198d`, installer `sha256:310468de88936ff0e8b8e7bcf5289f9015545d1d52f0b2338adf23dc2f0000bd`, updater `sha256:ee9ff10e520cabd89297a9ac0334b0e475d985928b809a3b39150340465e5f46`. 두 package 모두 531 files·필수 Codex 자산 7/7, `unsigned_local`/`NotSigned`이며 bounded 종료 final source보다 이전 evidence라 최종 package로 승격하지 않는다.
- restart evidence: `upd_frw...`는 forced-close exit wait 누락으로 `aborted`; `upd_cQi...`는 installer post-step 전 수동 Codex 기동으로 `ActiveCodexDesktop`(exit 7)·`rollback_required`; `upd_xHB...`는 5/7 hash mismatch로 `partially_applied`; `upd_iEb...`는 inaccessible helper에서 종료 pass가 중단돼 `aborted`; `upd_BAb...`는 비동기 종료 직후 census를 terminal failure로 오판해 `aborted`. 이를 분리한 bounded 반복 pass의 `upd_WHAje3cdPeS--gZo2uFg3qvOkgEwJ3ox4McLpjUmTYc`는 8개 instance를 닫고 `2026-07-26T04:20:42.661184300Z`에 `exited`, updater-parent Codex 자동 재기동을 완료했다.
- 외부 Gate: Authenticode/trusted timestamp/publication은 `blocked_external`; unsigned local artifact를 Stable 또는 published로 승격하지 않는다.

## 현재 Gate

1. live source·P-0056 artifact·installed Runtime exact subject 분리: **DONE**.
2. installed Runtime 기본 verify/doctor와 current HEAD FULL 10/10: **DONE**.
3. restart receipt·management recovery·Registry optional root 분류와 core 17 action E2E: **DONE**.
4. persisted recovery·권한/trust·M1~M6·13개 cross-repo·비서명 설치 lifecycle·ARM64 cross 검증: **DONE**. 실제 13 repo workload는 `not_run`, ARM64 native는 `native_unverified`.
5. P-0057 source fix의 설치/update와 수동 Codex restart: **`DONE`**. source `f496f6e`, installation verified, integration registered.
6. Authenticode·trusted timestamp, signed-byte lifecycle·provenance와 public Stable publish/readback: **`blocked_external`**. 별도 승인과 외부 signer 없이는 실행하지 않고 unsigned Stable로 우회하지 않는다.
7. P-0059 23개 기능·16개 Profile current source surface와 pre-seal FULL: **`PASS`**. STRICT review는 `APPROVE_WITH_NOTES`; 최종 current-byte FULL identity는 handoff가 소유한다.
8. installed file tree `6430219`, 이전 active Runtime generation의 management recovery-only 상태: **`blocked_operational`**. 최종 installer update와 exact read-only rebuild plan 전에는 apply하지 않는다.
9. P-0060 source·installer·updater와 Codex 자동 재기동: **`SOURCE_PASS / LIVE_RESTART_PASS / FINAL_INSTALL_PENDING`**. `upd_WHA...` receipt는 `exited`; final-byte FULL·commit, x64/ARM64 package 재생성·설치, live integration/Runtime/management 검증과 push는 **`IN_PROGRESS`**.
