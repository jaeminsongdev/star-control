# PLANS.md

## 목적

이 문서는 Star-Control `v0.1.0` 완성·출시 작업의 현재 판단에 필요한 bounded snapshot만 소유한다. 상세 계약은 [문서 읽는 순서](docs/README.md), 단계 정의는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 실측·감사는 `docs/testing/`, `benchmarks/`, Git history가 소유한다. 과거 P-ID의 `DONE`은 해당 bounded Slice의 완료만 뜻하며 1~11단계 Master Checklist 전체 완료를 뜻하지 않는다.

## 확정 결정

- 첫 언어는 Rust다. syntax는 private `tree-sitter-rust`, semantic은 exact pinned external `rust-analyzer`다.
- 공개 채널은 GitHub Releases `v0.1.0`이며 별도 서버 deploy는 없다.
- x64는 signed Stable, ARM64는 cross-build·simulation 기반 signed Preview와 `native_unverified`다.
- Authenticode 인증서가 없으면 unsigned Stable로 낮추지 않고 `blocked_external`을 유지한다.
- Rust 1.96과 `rustfmt`·Clippy·`rust-analyzer`·`rust-src`, Runtime EXE 4개, required core action 17개가 current inventory다.
- 과거 13-action·현재 설치본 6-action 감사는 해당 snapshot만 설명하며 source 17/17과 섞지 않는다.

## 핵심 불변식

- v1은 migration input으로만 읽고 v2만 쓴다. path·remote URL·HEAD 유사성으로 `CheckoutId`를 추측하지 않는다.
- source/manifest가 canonical이고 DB/index/cache는 derived다. Controller만 current projection을 쓴다.
- `partial`, `stale`, `unsupported`, `unverified`, `not_run`, `flaky`, unknown을 pass나 `confirmed_empty`로 승격하지 않는다.
- required core action은 owning handler와 generated input/output Schema가 모두 있을 때만 `ready`다.
- signed byte는 새 candidate다. publish timeout은 write 재시도 없이 read-only reconcile하고 `publish_outcome_unknown`을 유지한다.
- local AI, 다른 AI provider, OpenAI API 직접 호출, browser/HTTP control UI, 자체 scheduler·compiler·CI·signer를 추가하지 않는다.
- 다른 `D:\개발` 저장소, linked worktree, `legacy/`, `target/`, Codex runtime state를 임의 정리하지 않는다.
- Runtime generation ID는 source revision이 아니라 canonical Runtime payload file-set digest에 결박하며 stage·reseal·verify 모두 같은 산식을 사용한다.

## 현재 상태

| 범위 | 상태 | bounded seal / 현재 판정 |
|---|---|---|
| P-0039~P-0052 | historical `DONE` | 각 P-ID에서 명시한 bounded Slice만 봉인. Master Checklist의 계약·실어댑터·CLI·E2E 완성 판정으로 승격하지 않음 |
| P-0053 local | historical `DONE` | source 17/17 MCP readiness, x64 isolated lifecycle, ARM64 simulation, pre-sign supply chain과 signing-negative audit |
| P-0053 public | `blocked_external` | trusted signing, signed clean install, signed final provenance와 public remote publish/reconcile 필요. current Codex 17/17과 unpublished draft readback은 P-0055에서 완료 |
| P-0054 | `DONE / internal product seal` | 최신 `main` 기준 Recovery Slice, M1~M11, 최종 16 Profile의 내부 contract→engine→repository→Controller→CLI를 구현하고 requested TARGET→effective FULL 10/10을 통과. 외부·물리 Gate는 별도 상태 유지 |
| P-0055 | `DONE / non-signing external seal` | exact `0d0eca9a`에서 payload-content Runtime identity, FULL 10/10, RELEASE 14/15(서명·공개만 unverified), x64 lifecycle, ARM64 cross simulation, SBOM/audit/provenance, current host 무재시작 17/17과 GitHub unpublished draft digest/cleanup을 봉인했다. Authenticode와 서명 필수 공개 Stable만 별도 `blocked_external`이다. |

P-0041~P-0053 implementation·Schema·fixture·문서 snapshot은 `b29c178..ac3ca70` commit chain으로 보존한다. P-0054 기준선은 `main` `a93de7e68aff3ac02315d3a324aeaa497e1ede38`이다. 문서의 단계 설명이나 Rust type 존재만으로 완료를 판정하지 않고 Controller 경유 실제 경로, 실어댑터, stable JSON CLI, 저장·복구, negative corpus와 disposable E2E가 함께 닫혀야 완료다.

## P-0054 기능 감사 기준선

- **복구 P0:** backend-neutral recovery 계약 14개, active-set startup, online backup manifest-last, recovery-only Controller·CLI, side-by-side restore/rebuild·원자 활성화, verified ArtifactRef reindex, local-state export/import와 disposable 16-scenario Corpus를 구현했다.
- **M1~M4:** explicit multi-root·current index에서 revisioned planning, real process/rule/evidence Gate와 typed Recipe/PatchSetV2 apply·recovery까지 공통 lineage를 연결했다. M11 pre/post Gate도 같은 경로를 사용한다.
- **M5~M8:** Managed Registry·compatibility와 failure/security/dependency/migration/performance/language의 contract·persistence·Controller/CLI에 registered Tool terminal Operation과 `DevelopmentEffectReceiptV1`을 연결했다. exact subject·tool·arguments·executable·permission·approval·Gate를 검증하며 partial/unknown을 성공으로 승격하지 않는다. canonical source mutation은 계속 M4 PatchSet을 사용한다.
- **M9:** 실제 local Git worktree·merge·remote observation/push adapter와 exact durable approval를 연결했다. P-0055는 remote recovery provider Operation을 exact 영수증으로 봉인하고 plan/permission/Gate를 검증한 뒤에만 apply를 기록한다.
- **M10:** Controller/CLI가 `star-release`의 build-once candidate, byte verify, M3 evidence, promote/lifecycle, EvaluationRun/Catalog와 exact `ReleaseAssetBindingV1`을 사용한다. GitHub publisher는 draft-first/no-clobber/readback/reconcile을 구현했고, signer가 없으면 unsigned Stable publish apply는 fail-closed다.
- **M11:** owned isolated preview, pinned rustfmt/Clippy, candidate build/test, exact durable `personal_auto`, M2 Profile→M4 PatchSetV2→M3 pre/post Gate와 recovery를 연결했다.
- **16 Profile:** `catalog/profiles`의 정확한 16개 release source, strict descriptor/loader/resolver, parent closure·strict floor merge·fingerprint, `TaskSpec`/`ValidationPlan`/Evidence binding과 `star profile list|show|resolve`를 구현했다.
- **공통:** P-0054 기준 generated Schema manifest 186개에 P-0055 `DevelopmentEffectReceiptV1`·`ReleaseAssetBindingV1`을 더해 현재 188개와 해당 fixture를 생성·검사했다. exact `0d0eca9a` FULL은 10/10 PASS다. RELEASE는 source code·clean worktree·x64·ARM64·lifecycle 14 PASS이고 서명/publication만 1 unverified다.

감사 상세와 항목별 구현 증거는 [P-0054 실제 기능 완성 감사](docs/testing/p0054-functional-completion-audit-2026-07-23.md)에 유지한다. 미구현 항목은 실제 코드·테스트가 닫히기 전 `DONE`으로 바꾸지 않는다.

## 과거 검증과 로컬 출시 증거

- clean FULL: `target/validation/20260722T183736373Z-23820/report.json`, source `ac3ca70`, 10/10 complete PASS, report `sha256:059466b8c24911c70640192af8aed995933e0cde62840fc7e096fdc2050a4df4`.
- clean release: `target/validation/20260722T183855005Z-11592/report.json`, source `ac3ca70`, 14/15 PASS, failed 0. 유일한 non-pass는 external signing/publication `unverified/not_run`이다.
- PR 전체 변경 Gate: `target/validation/20260722T184037675Z-24808/report.json`, 11/11 PASS. GitHub Actions run `29947378549`, job `89016087886`도 최종 candidate source에서 PASS했다.
- M1 x64 reference: `benchmarks/m1-code-index-x64-reference.json`, 10,000-file corpus와 cache profile 5회 반복.
- P-0053 audit: `benchmarks/p53-release-audit-x64-arm64.json`, clean candidate `p0053-final-ac3ca70`, source `ac3ca70`.
- x64/ARM64 stage는 manifest 279파일, 네 EXE PE machine·digest와 Inno installer model을 검증했다.
- ARM64 workspace·explicit-feature Rust corpus cross-check/Clippy는 PASS지만 native execution은 `native_unverified`다.
- unsigned signing-negative stage는 `seal-signed` Authenticode Gate에서 manifest mutation 없이 거부됐다.
- official Inspector 0.22.0은 candidate stage의 fixed 12 tools, required core ready search 17/17과 describe 17/17을 통과했다.
- x64·ARM64 SPDX SBOM, cargo-audit와 provenance는 pre-sign evidence로 생성했으며 signing 뒤 반드시 재생성한다.

## P-0054 완료 Gate와 외부 잔여

1. `TARGET` clean-workspace package normalization 회귀를 수정하고 계약 테스트를 고정했다. 수정 직후 영향 승격 FULL 10/10은 통과했다.
2. 복구 P0를 계약 → port → state/evidence adapter → recovery application → Controller IPC → CLI → disposable Corpus 순서로 구현하고 정본 문서에 동기화했다.
3. M1~M11과 16 Profile의 공용 contract·lineage·Controller/CLI·negative E2E 구현과 generated Schema/fixture 갱신을 완료했다.
4. 정본 status와 P-0054 감사 문서를 실제 코드·focused 증거에 맞게 동기화했다.
5. code review와 `git diff --check`, format, Schema check, requested `TARGET`→effective `FULL` 10/10을 통과했다. 최종 report는 `target/validation/20260723T113308437Z-12820/report.json`, duration 122,292 ms다.
6. 실제 Authenticode signing, signed 설치, Codex runtime 변경, authenticated remote와 GitHub publish는 별도 승인·외부 Gate로 유지한다.

## P-0055 비서명 외부·복구 Slice 완료 증거

1. 최종 비서명 artifact source는 commit `0d0eca9a0fc441eb3cedb0d044608c3393222f07`, tree `3f33005b0ff4a159560d0f87500c3b41a2ff09a9`이며 `origin/main=a93de7e`를 포함한다. 봉인 시 origin branch와 GitHub API readback도 exact commit/tree와 일치했으며 후속 docs-only commit은 stage source revision을 바꾸지 않는다.
2. exact FULL은 `target/validation/20260724T112510915Z-3852/report.json`, 10/10 complete·stable PASS, `sha256:3f3ada0647e577455283769e15c3eed2583cfa9bf4a29c8ec8610aa7c759633b`다. exact RELEASE는 `target/validation/20260724T112310049Z-10640/report.json`, 14/15 PASS, failed 0, unverified 1, `sha256:73abe9690a8c8e0b1cbdc643e90ea7ec7b2bb25038f5c449e970d8aec4277e9b`이며 유일한 non-pass는 signing/publication이다.
3. x64/ARM64 stage는 각각 473파일, set digest `5344c92e…97ff`/`b165fb75…be12`, manifest `0a446f2f…9834`/`217e3db6…738a`다. content-addressed generation은 `rt_23cd8e31911f8415`/`rt_5913080cde8a516b`; PE machine은 `0x8664`/`0xaa64`이고 ARM64는 `native_unverified`다.
4. Inno Setup 6.7.3 설치기는 x64 24,180,687 bytes `396cbb29…b268`, ARM64 20,181,633 bytes `0822f557…66c4`이며 둘 다 `NotSigned`다. x64 isolated finalize·Bridge v2·status는 installation `ins_01KY9YCDXRZGNXVBPMNN12D12F`, activation revision 1, generation `rt_23cd8e31911f8415`, `verified=true`로 통과했다.
5. SPDX SBOM은 각 7 packages, RustSec `1abf7a8` 기준 223 dependencies·vulnerability 0·warning 0이다. pre-sign provenance는 `dist/release-evidence/p0055-0d0eca9a/provenance.pre-sign.json`, `sha256:cbba5c53…67dc`이며 signing 뒤 재생성해야 한다.
6. current installed payload는 Desktop 재시작 없이 activation revision 5·`rt_c569d8e23ed61e8e`·integration verified·registry revision 7 release 17/17로 복구했다. exact packaged CLI의 후속 Operation `upd_Ns0vvX…`도 `activation_changed=false`, 종료 PID 0으로 idempotence를 증명했다. fixed EXE maintenance install은 이 Slice 완료 조건이 아니다.
7. disposable GitHub draft release `359263161`은 exact `0d0eca9a`를 target으로 했고 1,261-byte asset의 local/provider/download SHA가 모두 `fd4a5bf3…bfaf3`로 일치했다. release ID/tag/tag ref는 cleanup 후 absent이며 evidence는 `dist/release-evidence/p0055-0d0eca9a/github-draft-roundtrip.json`, `sha256:3b623692…8606`다.
8. `dist/release-evidence/p0055-e248efe4`와 `p0055-e248efe4-r2`는 stale explicit-target binary와 source-derived Runtime generation collision을 드러낸 비채택 증거다. 삭제하지 않지만 최종 candidate나 완료 근거로 사용하지 않는다.
9. 비서명 제품·복구·unpublished remote seal은 `DONE`이다. 남은 외부 Gate는 Authenticode certificate/private key/trusted timestamp, signed clean lifecycle, signed provenance와 public Stable publish/readback뿐이며 unsigned Stable은 fail-closed다.

## 현재 Context Pack

- repo: `D:\개발\관제\Star-Control`
- branch: `codex/p0055-nonsigning-external-seal`
- base / 비서명 artifact source: `a93de7e68aff3ac02315d3a324aeaa497e1ede38` / `0d0eca9a0fc441eb3cedb0d044608c3393222f07`
- 현재 Slice: P-0055 비서명 외부·복구 seal `DONE`. Runtime content identity, exact package/lifecycle/supply-chain, current host no-restart와 unpublished GitHub 왕복을 봉인했다.
- 다음 Gate: 서명 자료가 공급될 때 signed byte를 새 candidate로 만들어 content ID·SBOM·provenance·설치·Codex·GitHub public readback을 재실행한다. 그 전에는 `blocked_external`이다.
- 먼저 읽기: `README.md`, `docs/README.md`, `docs/contracts/development-management.md`, `docs/contracts/events-and-state.md`, `docs/contracts/validation-and-evidence.md`, `docs/contracts/versioning-and-migrations.md`, `docs/architecture/state-and-artifacts.md`, `docs/architecture/repository-layout.md`, `docs/roadmap/final-implementation.md`, ADR-0006~0008
- 승인됨: package/dependency 설치, network·외부 도구, disposable install/update/repair/uninstall, GitHub draft·push·tag·remote readback. 각 effect는 exact target을 재검증하고 증거를 남긴다.
- 금지: Authenticode signing, unsigned Stable 공개 publish, `legacy/`·`target/` 정리, 실제 사용자 project/data 손상, installer·AI/browser/scheduler 재개방
- ARM64 결정: native 장비 Gate를 재요구하지 않고 cross-build·PE 검증·simulation corpus로 `native_unverified` Preview를 봉인한다.

## Archive References

- updater/lifecycle: `4f01948`, [계약](docs/contracts/codex-lifecycle-and-updater.md), [E2E](docs/testing/star-updater-restart-e2e-2026-07-18.md)
- MCP 감사: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- 단계 정본: [M1](docs/contracts/project-catalog-and-code-index.md), [M10](docs/contracts/ci-release-evaluation-and-product-completion.md), [M11](docs/features/rust-code-style-auto-fix.md)
