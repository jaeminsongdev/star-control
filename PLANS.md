# PLANS.md

## 목적

이 문서는 Star-Control `v0.1.0` 완성·출시 작업의 현재 판단에 필요한 bounded snapshot만 소유한다. 상세 계약은 [문서 읽는 순서](docs/README.md), 단계 정의는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 실측·감사는 `docs/testing/`, `benchmarks/`, Git history가 소유한다.

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

| 범위 | 상태 | local seal / 근거 |
|---|---|---|
| P-0039 | DONE | `4f01948`, updater/lifecycle STRICT + FULL |
| P-0040 | DONE | `416ed3e`, x64 Stable·ARM64 Preview 정책과 current inventory |
| P-0041 | DONE | Project v1→v2 migration, register, backup/resume/rollback/conflict |
| P-0042 | DONE | M1 Catalog·Rust index·scan·cache·10,000-file corpus |
| P-0043~P-0045 | DONE | planning, CheckGraph/evidence, immutable PatchSet·TOCTOU·rollback |
| P-0046~P-0050 | DONE | M5~M9 registry/doctor/radar/migration/change-bundle/merge/handoff |
| P-0051 | DONE | M10 build-once release/evaluation engine과 fake provider fault corpus |
| P-0052 | DONE | M11 Rust fixed pipeline, persisted pre/post Gate, exact apply/recovery |
| P-0053 local | DONE | clean source seal, source 17/17, candidate Inspector 17/17, x64 isolated lifecycle, ARM64 simulation, pre-sign supply chain과 signing-negative audit |
| P-0053 public | `blocked_external` | trusted signing, signed clean install, current Codex 17/17 invoke, final provenance와 remote reconcile 필요 |

P-0041~P-0053 implementation·Schema·fixture·문서 snapshot은 branch `codex/p0041-p0053-completion`의 `b29c178..ac3ca70` commit chain으로 봉인했다. `ac3ca70`은 Windows CI에서 드러난 Rust semantic URI, project allowlist fixture와 long-poll scheduler 경계를 포함한 최종 candidate source revision이다. 큰 변경을 빌드 불가능한 P-ID snapshot으로 만들지 않도록 crate·Schema·fixture·문서 단위의 소규모 commit으로 나눴다.

## 구현 요약

- `Project` v2/`ProjectCheckout`, allowlisted idempotent register와 lossless offline migration을 구현했다.
- persisted Catalog/CodeIndex, text/Rust syntax/optional semantic partition, incremental cache·recovery·large corpus를 구현했다.
- Task planning, deterministic CheckGraph, diagnostics/Gate/evidence, durable Goal/Plan/Run과 required core 17/17 source readiness를 구현했다.
- immutable PatchSet isolated preview, exact approval, TOCTOU apply와 reverse rollback을 구현했다.
- M5~M9 registry·compatibility doctor·maintenance radar·migration/performance·ChangeBundle coordination을 구현했다.
- M10 build-once candidate/evaluation ratchet와 timeout/digest/rollback/unknown-outcome fake providers를 구현했다.
- M11 `rustfmt → allowlisted MachineApplicable Clippy → rustfmt`, sealed scope/config/toolchain/policy/coverage, persisted pre/post Gate, single-use `personal_auto`, atomic apply/recovery를 구현했다.

## 검증과 로컬 출시 증거

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

## 열린 Gate

1. 승인된 Authenticode certificate·private key·timestamp provider를 공급한다. SDK `signtool.exe`는 확인됐지만 usable signing certificate는 CurrentUser·LocalMachine 모두 0개다.
2. Runtime EXE 서명 → `seal-signed` → installer build·서명 → final digest/SBOM/provenance 순서로 새 candidate를 만든다.
3. disposable clean x64 install·first run·update failure rollback·repair·uninstall/user-data 보존을 검증한다.
4. signed candidate를 실제 Codex integration에 설치한 뒤 current Codex에서 required core 17/17 search·describe·invoke를 재감사한다.
5. exact GitHub destination·manifest·digest가 일치할 때만 tag/draft/upload/publish하고 remote digest를 read-back한다.

## 현재 Context Pack

- repo: `D:\개발\관제\Star-Control`
- branch: `codex/p0041-p0053-completion`
- base: P-0040 `416ed3e`; implementation/candidate source chain `b29c178..ac3ca70`
- 먼저 읽기: `README.md`, `docs/roadmap/final-implementation.md`, `docs/testing/p53-final-release-audit-2026-07-20.md`, `benchmarks/p53-release-audit-x64-arm64.json`
- 다음 명령: trusted signing material이 공급되면 `seal-signed` 순서로 새 candidate 생성
- public blocker: trusted Authenticode signing material과 signed clean-install/current-Codex-17/17/final-provenance/remote evidence

## Archive References

- updater/lifecycle: `4f01948`, [계약](docs/contracts/codex-lifecycle-and-updater.md), [E2E](docs/testing/star-updater-restart-e2e-2026-07-18.md)
- MCP 감사: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- 단계 정본: [M1](docs/contracts/project-catalog-and-code-index.md), [M10](docs/contracts/ci-release-evaluation-and-product-completion.md), [M11](docs/features/rust-code-style-auto-fix.md)
