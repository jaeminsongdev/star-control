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

## 현재 상태

| 범위 | 상태 | bounded seal / 현재 판정 |
|---|---|---|
| P-0039~P-0052 | historical `DONE` | 각 P-ID에서 명시한 bounded Slice만 봉인. Master Checklist의 계약·실어댑터·CLI·E2E 완성 판정으로 승격하지 않음 |
| P-0053 local | historical `DONE` | source 17/17 MCP readiness, x64 isolated lifecycle, ARM64 simulation, pre-sign supply chain과 signing-negative audit |
| P-0053 public | `blocked_external` | trusted signing, signed clean install, current Codex 17/17 invoke, final provenance와 remote reconcile 필요 |
| P-0054 | `DONE / internal product seal` | 최신 `main` 기준 Recovery Slice, M1~M11, 최종 16 Profile의 내부 contract→engine→repository→Controller→CLI를 구현하고 requested TARGET→effective FULL 10/10을 통과. 외부·물리 Gate는 별도 상태 유지 |
| P-0055 | `RECOVERY_RESEALING` | stale selector를 root manifest-owned generation으로 승격하고 declared/ready exact set을 검증하는 replacement reconcile에 더해, 이미 설치된 payload는 Codex 재시작 없이 복구하는 `reconcile-installed-runtime`을 구현했다. current host는 activation revision 5, `rt_c569d8e23ed61e8e`, integration verified와 MCP 17/17 search·describe·invoke를 확인했다. 새 exact source commit의 FULL/RELEASE·x64/ARM64 package·격리 lifecycle·GitHub draft/remote readback 재봉인이 남았다. Authenticode와 서명 필수 공개 Stable은 계속 별도 `blocked_external`이다. |

P-0041~P-0053 implementation·Schema·fixture·문서 snapshot은 `b29c178..ac3ca70` commit chain으로 보존한다. P-0054 기준선은 `main` `a93de7e68aff3ac02315d3a324aeaa497e1ede38`이다. 문서의 단계 설명이나 Rust type 존재만으로 완료를 판정하지 않고 Controller 경유 실제 경로, 실어댑터, stable JSON CLI, 저장·복구, negative corpus와 disposable E2E가 함께 닫혀야 완료다.

## P-0054 기능 감사 기준선

- **복구 P0:** backend-neutral recovery 계약 14개, active-set startup, online backup manifest-last, recovery-only Controller·CLI, side-by-side restore/rebuild·원자 활성화, verified ArtifactRef reindex, local-state export/import와 disposable 16-scenario Corpus를 구현했다.
- **M1~M4:** explicit multi-root·current index에서 revisioned planning, real process/rule/evidence Gate와 typed Recipe/PatchSetV2 apply·recovery까지 공통 lineage를 연결했다. M11 pre/post Gate도 같은 경로를 사용한다.
- **M5~M8:** Managed Registry·compatibility와 failure/security/dependency/migration/performance/language의 contract·persistence·Controller/CLI에 registered Tool terminal Operation과 `DevelopmentEffectReceiptV1`을 연결했다. exact subject·tool·arguments·executable·permission·approval·Gate를 검증하며 partial/unknown을 성공으로 승격하지 않는다. canonical source mutation은 계속 M4 PatchSet을 사용한다.
- **M9:** 실제 local Git worktree·merge·remote observation/push adapter와 exact durable approval를 연결했다. P-0055는 remote recovery provider Operation을 exact 영수증으로 봉인하고 plan/permission/Gate를 검증한 뒤에만 apply를 기록한다.
- **M10:** Controller/CLI가 `star-release`의 build-once candidate, byte verify, M3 evidence, promote/lifecycle, EvaluationRun/Catalog와 exact `ReleaseAssetBindingV1`을 사용한다. GitHub publisher는 draft-first/no-clobber/readback/reconcile을 구현했고, signer가 없으면 unsigned Stable publish apply는 fail-closed다.
- **M11:** owned isolated preview, pinned rustfmt/Clippy, candidate build/test, exact durable `personal_auto`, M2 Profile→M4 PatchSetV2→M3 pre/post Gate와 recovery를 연결했다.
- **16 Profile:** `catalog/profiles`의 정확한 16개 release source, strict descriptor/loader/resolver, parent closure·strict floor merge·fingerprint, `TaskSpec`/`ValidationPlan`/Evidence binding과 `star profile list|show|resolve`를 구현했다.
- **공통:** P-0054 기준 generated Schema manifest 186개에 P-0055 `DevelopmentEffectReceiptV1`·`ReleaseAssetBindingV1`을 더해 현재 188개와 해당 fixture를 생성·검사했다. exact `b20d234` final FULL은 10/10 PASS다. RELEASE는 source code·clean worktree·x64·ARM64·lifecycle 14 PASS이고 서명/publication만 1 unverified다.

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

## P-0055 exact local candidate 증거와 남은 순서

1. 구현 530 paths는 implementation commit `4554c4a56564ecea800a335dfbf4bb82d546e299`, tree `2eb3680b3f0cf5a8ae6b0daadff6fe54f003e067`로 봉인했다. 그 위의 제품 증거 기준 docs commit은 `b20d234b38a7dcb347049b6b95aff3407c5dedc9`, tree `ea4407eab1a782fcd94ff671686cdedf952b44e6`다.
2. official Inspector 0.22.0과 실제 Codex Hook 순서를 포함한 final FULL Operation `opn_01KY7WS7Y2ZEXB10WPKR2D583X`는 `target/validation/20260723T162621247Z-36040/report.json`, 10/10 complete·stable PASS, report `sha256:26c029c48f4ec2374906310edf4ffdc656b778aeda174797308ea578079e5b32`다. requested·required·selected profile은 모두 `full`이다.
3. final RELEASE는 `target/validation/20260723T154718929Z-29140/report.json`, 14/15 PASS, failed 0, unverified 1이다. 유일한 non-pass는 승인된 서명·공개 제외를 나타내는 `release-external-signing-publication`이다.
4. exact `b20d234` stage는 x64/ARM64 각각 473파일이며 set digest는 `20ae1b66…2eaa`/`9872eb8e…b808`다. x64 네 EXE는 `0x8664`, ARM64 네 EXE는 `0xaa64`; ARM64 runtime은 `native_unverified`다.
5. x64 isolated finalize·Bridge v2·status, SPDX SBOM 각 7 packages, RustSec 223 dependencies vulnerability/warning 0, pre-sign provenance `sha256:5f819316…a78c`, official Inspector fixed 12/12·core search/describe 17/17과 `validation.run` 종단 성공을 확인했다.
6. origin branch와 GitHub commit API가 exact `b20d234`/tree `ea4407e`를 readback했다. disposable draft release `359047620`의 515-byte asset은 local/provider/download가 모두 `sha256:67b05a54…6637`로 일치했고 release ID/tag/tag ref 최종 상태가 모두 absent다. 증거는 `dist/release-evidence/p0055-b20d234b/github-draft-roundtrip.json`, `sha256:9d764cdb…6cdf`다.
7. 첫 current-host restart transaction `upd_OAA1VfYQ8qhQIlz64fKrYPqgt-yddr_PRFHyq38IHq4`은 setup/EXE 교체와 Codex relaunch에는 성공했지만 active selector가 `rt_9fe838922e279501`에 남아 release action이 6/17이었다. implementation commit `7eedc7b`는 manifest-owned replacement reconcile과 partial recovery를 추가했고 clean FULL 10/10, RELEASE 14/15에서 signing/publication만 unverified였다.
8. 두 번째 restart 시도 `upd_0MMCLNfG-BIK2dSXJJnn9Xs9KGeLMC8E1pxeRaleWXc`은 Codex 종료가 완료되지 않아 apply 전 중단됐지만 구 코드가 receipt를 `draining`에 남기는 결함을 드러냈다. 현재 수정은 close failure를 `aborted`로 종결하고 update lease를 소유한 reconcile만 동일 install root의 중단된 pre-apply receipt를 복구한다. apply 이후·partial 상태는 재분류하지 않는다.
9. current host의 verified payload에는 bundled `rt_c569d8e23ed61e8e`가 이미 있었으므로 Desktop을 다시 재시작하지 않고 새 source updater로 selector만 reconcile했다. prior Controller exact fallback PID 1개를 기록했고 activation revision 5, integration `verified`, declared=ready 17/17을 확인했다. current Codex MCP는 17개 모두 search·describe·invoke했으며 15개 성공, ChangeBundle 없는 일회성 goal의 merge/handoff 2개는 설계된 `COORDINATION_NOT_FOUND`였다. `validation.run` Operation `opn_01KY9TWQERDG6FF2WHVR389VE5`는 TARGET 8/8 PASS, `target/validation/20260724T103147237Z-4620/report.json`, `sha256:4d443a68…f186`으로 종결됐다.
10. 남은 비서명 순서는 현재 source를 exact commit으로 봉인한 뒤 FULL/RELEASE, x64/ARM64 stage·installer·SBOM·audit·provenance, x64 격리 lifecycle, exact GitHub draft byte 왕복과 remote branch readback을 다시 생성하는 것이다. current installed fixed EXE를 새 byte로 교체하는 maintenance restart는 runtime 17/17 복구와 구분하며, 불필요한 추가 restart를 완료 조건으로 만들지 않는다.

## 현재 Context Pack

- repo: `D:\개발\관제\Star-Control`
- branch: `codex/p0055-nonsigning-external-seal`
- base / 직전 recovery commit: `a93de7e68aff3ac02315d3a324aeaa497e1ede38` / `7eedc7b24b6cb912afe588a6aebdab49de720c03`
- 현재 Slice: 무재시작 installed-runtime reconcile과 interrupted pre-apply receipt recovery까지 source·current host에서 검증했다. 현재 변경을 exact commit→FULL/RELEASE→x64/ARM64 package→격리 lifecycle→GitHub draft/remote 증거 순으로 다시 봉인한다.
- 먼저 읽기: `README.md`, `docs/README.md`, `docs/contracts/development-management.md`, `docs/contracts/events-and-state.md`, `docs/contracts/validation-and-evidence.md`, `docs/contracts/versioning-and-migrations.md`, `docs/architecture/state-and-artifacts.md`, `docs/architecture/repository-layout.md`, `docs/roadmap/final-implementation.md`, ADR-0006~0008
- 승인됨: package/dependency 설치, network·외부 도구, disposable install/update/repair/uninstall, GitHub draft·push·tag·remote readback. 각 effect는 exact target을 재검증하고 증거를 남긴다.
- 금지: Authenticode signing, unsigned Stable 공개 publish, `legacy/`·`target/` 정리, 실제 사용자 project/data 손상, installer·AI/browser/scheduler 재개방
- ARM64 결정: native 장비 Gate를 재요구하지 않고 cross-build·PE 검증·simulation corpus로 `native_unverified` Preview를 봉인한다.

## Archive References

- updater/lifecycle: `4f01948`, [계약](docs/contracts/codex-lifecycle-and-updater.md), [E2E](docs/testing/star-updater-restart-e2e-2026-07-18.md)
- MCP 감사: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- 단계 정본: [M1](docs/contracts/project-catalog-and-code-index.md), [M10](docs/contracts/ci-release-evaluation-and-product-completion.md), [M11](docs/features/rust-code-style-auto-fix.md)
