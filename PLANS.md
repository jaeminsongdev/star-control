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

## 현재 Context Pack

- repo: `D:\개발\관제\Star-Control`
- branch: `codex/p0056-current-functional-recovery`
- base: `origin/main a93de7e68aff3ac02315d3a324aeaa497e1ede38`
- artifact source: commit `1bce4724c34414cef74862dbe9bf9de1f094ad2f`, tree `4e1c3b1d55bfbe35eb7eaf4455c02bde711bcac4`
- remote artifact-source readback: origin branch exact `1bce4724` PASS; `origin/main a93de7e` 포함
- evidence root: `dist/release-evidence/p0056-1bce4724`; pre-sign provenance `sha256:3bde861329cff0cb8f6a8bbae12a1e40391275e7614f3eedbbd67442ff97d226`
- current Slice: P-0056 최신 main 기능 전수 감사 + 복구 Slice + 서명 제외 external reseal
- 승인됨: dependency/toolchain, network, disposable lifecycle, GitHub draft/tag/asset cleanup, commit/push/readback
- 금지: Authenticode signing, unsigned Stable publish, 실제 사용자 data 손상, `legacy/`·`target/` 정리, 불필요한 Desktop restart

## 닫힌 Gate와 남은 외부 Gate

1. source commit과 remote exact commit/tree readback: **DONE**.
2. exact source RELEASE, x64/ARM64 package·simulation·격리 lifecycle·SBOM/RustSec/pre-sign provenance: **DONE**.
3. authenticated GitHub disposable draft digest round-trip과 release/tag cleanup: **DONE / unpublished**.
4. P-0056 audit/PLANS exact evidence 동기화: **DONE**. 후속 docs-only seal commit은 artifact source와 byte set을 바꾸지 않는다.
5. Authenticode·trusted timestamp, signed-byte lifecycle·provenance와 public Stable publish/readback: **`blocked_external`**. unsigned Stable로 우회하지 않는다.
