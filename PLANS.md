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
| installed P-0057 update | `held / approval_required` | 실행 중 source `b20d234`는 수정 전 byte. 실제 install/system state와 Codex restart는 별도 사용자 승인 전까지 변경하지 않음 |
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

## 현재 Context Pack

- repo/branch: `D:\개발\관제\Star-Control`, `main`; 시작 HEAD와 `origin/main`은 exact `816be57`, 시작 worktree clean
- current Slice: P-0057 current-system audit source seal. 상세 증거와 미배포 경계는 `docs/testing/p0057-current-system-audit-2026-07-25.md`
- 완료: 세 기준선, live install/restart/lease, 17 action E2E, recovery/approval/Registry 수정, M1~M6 negative test, 13개 repo, unsigned x64 lifecycle/installer, RustSec, ARM64 cross-build
- live held: installed management `recovery_only`, watcher unavailable 중복, destructive core approval 결함은 source fix 전 Runtime에 남음. update/restart 전 해당 mutation을 사용하지 않음
- final validation: current HEAD FULL 10/10 `complete/stable/pass`; RELEASE 14/15, failed 0, external signing/publication 1 `unverified`. exact report hash는 source에 되먹임하지 않고 최종 handoff에 고정
- 금지: 실제 사용자 management/project data 변경, `legacy/`·`target/` 정리, Codex runtime DB/cache 직접 수정, 불필요한 Desktop restart, 승인 없는 dependency/system setting/push/publish/signing

## 현재 Gate

1. live source·P-0056 artifact·installed Runtime exact subject 분리: **DONE**.
2. installed Runtime 기본 verify/doctor와 current HEAD FULL 10/10: **DONE**.
3. restart receipt·management recovery·Registry optional root 분류와 core 17 action E2E: **DONE**.
4. persisted recovery·권한/trust·M1~M6·13개 cross-repo·비서명 설치 lifecycle·ARM64 cross 검증: **DONE**. 실제 13 repo workload는 `not_run`, ARM64 native는 `native_unverified`.
5. P-0057 source fix를 실행 중 설치본에 update/restart: **`approval_required`**. 승인 전 installed mutation action 사용 금지.
6. Authenticode·trusted timestamp, signed-byte lifecycle·provenance와 public Stable publish/readback: **`blocked_external`**. 별도 승인과 외부 signer 없이는 실행하지 않고 unsigned Stable로 우회하지 않는다.
