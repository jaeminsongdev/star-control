# PLANS.md

## 목적

이 문서는 Star-Control 전체 완성·출시 작업의 **현재 구현 상태와 다음 Gate만** 남기는 bounded snapshot이다. 상세 설계는 [문서 읽는 순서](docs/README.md), 단계 정의는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 과거 감사·실측은 `docs/testing/`과 Git history가 소유한다.

## 확정 결정

- 첫 언어는 Rust다. syntax adapter는 private `tree-sitter-rust`, semantic adapter는 pinned external `rust-analyzer`다.
- 공개 버전은 `v0.1.0`, 공개 채널은 GitHub Releases다. 별도 서버 deploy는 없다.
- x64는 signed Stable, ARM64는 cross-build·simulation 기반 `native_unverified` Preview다. ARM64 native 성공을 추측하지 않는다.
- Authenticode 인증서가 없으면 unsigned Stable로 낮추지 않고 release를 `blocked_external`로 유지한다.
- Rust 기준은 1.96, runtime executable은 `star.exe`, `star-controller.exe`, `star-mcp.exe`, `star-updater.exe` 4개, required core action은 17개다.
- 과거 13-action 감사는 당시 snapshot을 설명하는 역사 자료로 보존하며 current inventory로 승격하지 않는다.

## 핵심 불변식

- v1은 migration input으로만 읽고 v2만 쓴다. `ProjectId`와 `CheckoutId`를 분리하고 path·remote URL·HEAD 유사성으로 CheckoutId를 추측하지 않는다.
- source/manifest가 canonical이고 DB/index/cache는 derived다. Controller만 current projection을 쓴다.
- `partial`, `stale`, `unsupported`, `unverified`, `not_run`, `flaky`, unknown을 pass나 `confirmed_empty`로 승격하지 않는다.
- required core action은 owning handler와 generated Schema가 모두 있을 때만 `ready`다.
- local AI, 다른 AI provider, OpenAI API 직접 호출, browser/HTTP control UI, 자체 scheduler, compiler·CI·signer 재구현을 추가하지 않는다.
- 다른 `D:\개발` 저장소는 수정하지 않는다. cross-repo 검증은 read-only 또는 disposable fixture repository만 사용한다.
- 기존 dirty/untracked 파일, linked worktree, `legacy/`, `target/`, Codex cache·runtime state를 임의 정리하지 않는다.

## 승인 경계

- `tree-sitter`, Rust grammar, `rust-analyzer` 또는 toolchain component 추가·설치는 실행 직전 dependency 승인을 받는다.
- 인증서·timestamp provider·비용과 secret 사용은 별도 승인을 받는다. secret은 repo·DB·로그에 저장하지 않는다.
- push, PR, tag, GitHub draft, asset upload와 publish는 exact action별 승인을 받는다.
- 파일 삭제·대량 이동, system setting·PATH 변경과 실제 사용자 설치 제거는 별도 승인을 받는다.
- 검증된 P-ID별 local commit은 허용되지만 push하지 않는다.

## 현재 상태

| ID | 상태 | 완료 조건 |
|---|---|---|
| P-0039 | DONE — `4f01948` | updater/lifecycle STRICT review, workspace FULL 10/10 |
| P-0040 | DONE — local seal pending | x64 Stable·ARM64 Preview 정책/ADR, Rust 1.96·4 EXE·required core 17 정본 정렬 |
| P-0041 | PENDING | Project v1→v2 lossless migration, backup/dry-run/resume/rollback/conflict Gate |
| P-0042 | PENDING | M1 Catalog·Rust text/syntax/semantic index·scan·대형 corpus, 첨부 미해결 17개 종료 |
| P-0043~P-0050 | PENDING | M2~M9 단계별 제품 구현과 TARGET/FULL Gate |
| P-0051 | PENDING | M10 release/evaluation build-once engine과 fake provider fault corpus |
| P-0052 | PENDING | M11 Rust 자동 교정, exact PatchSet approval·idempotence·coverage corpus |
| P-0053 | PENDING | required core 17/17 current evidence, x64 release lifecycle, ARM64 simulation, 승인 기반 publish |

## P-0040 봉인 결과

- ADR-0015와 `packaging/release.toml`이 `v0.1.0` GitHub Releases, signed x64 Stable, signed ARM64 `native_unverified` Preview와 no-deploy 정책을 소유한다.
- current inventory는 Rust 1.96, 4 Runtime EXE, required core 17개로 정렬했고 2026-07-12의 13-action 감사에는 역사 snapshot 경계를 추가했다.
- product code·dependency·lockfile·설치본·runtime state·다른 저장소와 원격은 변경하지 않았다.
- release policy source 변경 때문에 requested QUICK가 FULL로 승격됐고 10/10 complete·stable PASS했다.

## 검증 기준

- 공개 계약은 minimal/full/invalid/future fixture, fingerprint golden과 migration compatibility를 가진다.
- Rust adjudicated fixture의 잘못된 `confirmed` definition/reference는 0건이어야 하며 나머지는 ambiguous/unresolved limitation으로 남긴다.
- unchanged scan은 eligible partition을 재사용하고 단일 파일 변경은 해당 partition만 무효화한다.
- 성능 budget은 reference x64 반복 p95의 120%이며 full/incremental/cache hit/miss를 분리한다.
- 단계별 TARGET, 공개 API·Schema·lockfile 뒤 workspace FULL, 출시 전 release Gate를 실행한다.
- signed byte는 새 candidate다. publish timeout은 쓰기 재시도 없이 read-only reconcile하고 `publish_outcome_unknown`을 유지한다.

## 최신 증거와 열린 위험

- P-0039 source FULL: `target/validation/20260720T090022553Z-29032/report.json`, 10/10 stable PASS, 38.1초.
- P-0040 workspace FULL: `target/validation/20260720T104746874Z-23600/report.json`, 10/10 complete·stable PASS, 46.8초.
- 현재 native 검증 환경은 Windows x64다. ARM64는 cross-build·PE architecture·file manifest·installer model·fake lifecycle까지만 증명한다.
- Authenticode certificate/timestamp provider가 없으면 P-0053 publication은 `blocked_external`이다.
- required core 17/17, M1~M11 runtime evidence와 clean x64 install lifecycle은 아직 완료되지 않았다.
- 공식 Codex 표면에 전체 UI task safe-point census가 없어 P-0039 task count는 unknown으로 보존한다.

## 현재 Context Pack

- repo: `D:\개발\관제\Star-Control`
- branch: `codex/p0040-release-policy-alignment`
- base commit: `4f01948` (`P-0039`)
- remote: `origin`, push 금지
- 먼저 읽을 파일: `README.md`, `docs/README.md`, `docs/roadmap/final-implementation.md`, release/installation 계약, MCP 독립 감사.
- 다음 명령: current 숫자·용어 검색 → P-0040 문서 수정 → QUICK → STRICT diff review → local commit.

## Archive References

- updater/lifecycle: `4f01948`, [계약](docs/contracts/codex-lifecycle-and-updater.md), [E2E](docs/testing/star-updater-restart-e2e-2026-07-18.md)
- MCP 역사 감사: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- 단계 정본: [M1](docs/contracts/project-catalog-and-code-index.md), [M10](docs/contracts/ci-release-evaluation-and-product-completion.md), [M11](docs/features/rust-code-style-auto-fix.md)
