# PLANS.md

## 목적

이 문서는 Star-Control의 현재 작업 판단을 위한 bounded 원장이다. MCP 상세 감사·결함·증거는 [MCP 구현 완료 감사](docs/testing/mcp-completion-audit.md)에, 독립 reviewer 입력은 [Sol Max MCP 독립 검토 인계문](docs/testing/sol-max-mcp-review-handoff.md)에 둔다.

## Context Pack

### 현재 목표

- P-0014 P0 하이브리드 관리 저장소와 공통 개발 관리 첫 수직 Slice를 최신 `origin/main`의 공개 validation/evidence 계약과 통합하고 FULL 검증한 뒤 승인된 `main`에 push한다.
- 정본 계약 v1의 MCP Gateway·Local IPC·Live Tool Registry·외부 EXE Runtime·보안·복구·관리 CLI와 170개 검증 범위를 유지한다.
- 후속 lifecycle 확장은 새 P-ID와 별도 승인 범위로 시작한다.

### 반드시 지켜야 할 제약

- Codex 전용, Windows 전용이며 local STDIO MCP와 current-user Controller IPC만 사용한다.
- 로컬 AI, 다른 AI 제공자, OpenAI API 직접 호출, HTTP MCP, 브라우저 UI와 자체 예약 실행을 추가하지 않는다.
- raw shell·script host·PATH lookup을 외부 EXE 우회 수단으로 허용하지 않는다.
- `legacy/`, 사용자 변경, 기존 미추적 `--check/`와 루트 `manifest.json`을 보존한다.
- package 설치, 시스템 설정 변경, 파일 삭제, 외부 계정 변경은 별도 승인 없이는 하지 않는다. 현재 대화는 `origin/main` commit·push만 승인했다.
- DB backend 이름·SQL·filename은 public 계약·StarConfig·CLI·MCP에 노출하지 않고 `star-state` private adapter에만 둔다.

### 이미 끝난 것

- 고정 12-tool Gateway, authenticated IPC, live Registry/LKG, 외부 EXE Runtime, 보안·복구·관리 CLI를 구현했다. required core 13 action은 ID·command·lane 선언만 exact하며 owning handler·Schema는 아직 없다.
- Schema·fixture·JCS·manifest parser, 170개 matrix ID와 의미 감사 회귀를 구현·검증했다.
- P-0013에서 68개 inventory, P0 상세 계약, ADR-0006·ADR-0007을 문서 확정했다.
- P-0014에서 21개 persisted type·generated Schema·fixture와 fingerprint golden을 구현했다.
- `star-domain`, `star-ports`, `star-project`, `star-validation`, `star-execution`, `star-application`, `star-state`, `star-evidence`와 backend-neutral repository port를 추가했다. `rusqlite 0.40.1` bundled dependency는 `star-state` private adapter에만 둔다.
- Controller 단일 Writer, CLI→IPC→application service, local-first 등록, deterministic scan·Finding, shared decision projection, preview·explicit patch apply·재검증, backup·integrity·retention·source rebuild plan/apply의 첫 수직 Slice를 구현했다.
- 최신 `main`의 `evidence::GateDecision`과 `evidence::ArtifactRef`를 단일 공개 정본으로 유지하고, 관리 Slice의 ProjectRevision·WorkspaceSnapshot·decision 입력은 namespaced `star.management` extension으로 통합했다.

### 아직 남은 것

- P-0014 통합 결과는 workspace FULL, x64·ARM64 release cross-build, Schema/matrix, 문서·diff gate를 통과했으며 승인된 `origin/main` 게시와 원격 정합성 확인만 남았다.
- required core 13개 owning command handler·Schema, current Codex·Inspector·native ARM64 evidence, exact Windows 11 24H2 baseline은 MCP release blocker로 남는다.
- corrupt/future store의 실제 restore candidate 준비·atomic generation activation과 과거 DB version migration fixture는 후속 lifecycle 확장이다.

### 건드리면 안 되는 것

- `legacy/`
- 기존 사용자 미추적 `--check/`, `manifest.json`
- 공개 원격의 사용자 소유 외부 상태. 현재 승인 범위 밖 release·배포·외부 계정·유료 기능

### 먼저 확인할 파일

- `docs/contracts/development-management.md`
- `docs/contracts/validation-and-evidence.md`
- `docs/decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md`
- `docs/decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md`
- `docs/decisions/ADR-0008-P0-embedded-relational-backend.md`
- `crates/foundation/star-contracts/src/evidence.rs`
- `crates/foundation/star-contracts/src/management.rs`
- `crates/infrastructure/star-state/src/lib.rs`

### 먼저 실행할 명령

- `git status --short --branch`
- `cargo fmt --all -- --check`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo run --locked -p star-schema-gen -- --check`
- `cargo run --locked -p star-matrix-check -- --details`
- `git diff --check`

### 현재 차단 요소

- 제품 blocker는 required core 13개 owning command handler·Schema 부재다. current search/describe/invoke는 fail-closed `unavailable`이며 release 완료 조건을 충족하지 않는다.
- current Codex·Inspector·native ARM64 evidence와 exact Windows 11 24H2 baseline이 없다.
- P-0014 push 자체는 승인됐고 최종 FULL 검증을 통과했다. 게시 직전 `origin/main`을 다시 fetch해 fast-forward 가능성과 원격 정합성을 확인한다.

## 현재 활성 작업

| ID | 상태 | 목표 | 근거 | 다음 조치 |
|---|---|---|---|---|
| P-0012 | IN PROGRESS — verdict BLOCK | MCP 정본·제품·테스트·생성물·실행 증거 독립 감사와 결함 해소 | `docs/testing/mcp-independent-audit-2026-07-12.md` | core owner 계약 구현 뒤 current evidence 재생성 |
| P-0014 | READY TO PUBLISH — FULL PASS | P0 관리 계약·repository·embedded relational persistence·scan/Finding·patch/validation 첫 수직 Slice | local source commit `07a40fe`; 최신 `main` 공개 GateDecision·ArtifactRef 통합; FULL gate 통과 | commit → fetch/recheck → `origin/main` push |

## 최근 완료 작업

| ID | 결과 | 근거 |
|---|---|---|
| P-0013 | DONE — P0 공통 개발 관리·local management DB 상세 설계와 정본 반영 | `docs/contracts/development-management.md`, ADR-0006·ADR-0007 |

## 열린 리스크

| ID | 내용 | 현재 통제 | 다음 조치 |
|---|---|---|---|
| R-0009 | exact Windows 11 24H2 baseline smoke 미실행 | x64·ARM64 25H2와 최소 build 26100 통과 기록 | 별도 24H2 hardware/VM |
| R-0014 | evidence가 과거 binary hash를 가리킬 수 있음 | current/과거 evidence를 명시적으로 분리 | 제품 수정 뒤 current evidence 재생성 |
| R-0015 | ARM64 cross-build와 native ARM64 persistence 실행은 다름 | cross-build를 실행 증거로 과장하지 않음 | P9 native 장비·installer smoke |
| R-0016 | DB 손실 시 local-only decision·진행 이력은 source로 재구축 불가 | backup 계약과 source rebuild loss report | verified restore·export 후속 Slice |
| R-0017 | required core 13개 owning handler·Schema 부재 | fail-closed `unavailable` | application owner 계약·handler·release gate |
| R-0018 | Codex·Inspector·ARM64 evidence provenance 미완료 | 독립 감사에서 stale evidence를 blocker로 유지 | current binary 실기 재실행 |
| R-0019 | future/corrupt active store의 candidate restore·atomic activation 미구현 | 원본 overwrite 금지, read-only inspection | lifecycle 후속 Slice |

## 최신 검증 상태

- 최신 `main` 통합본: `cargo fmt --all -- --check`, `cargo test --workspace --locked`, 경고 0 `cargo clippy`, Schema 재현성, 170/170 matrix 통과.
- x64·`aarch64-pc-windows-msvc` workspace release build, `cargo audit --deny warnings`, `cargo deny check advisories` 통과.
- Markdown 54개 local link·code fence, 공개 GateDecision·ArtifactRef 단일 정의, staged diff·legacy·비밀 패턴·backend 경계 검사를 통과했다.
- 기존 Windows incremental finalize `os error 5` note는 명령 exit 0일 때 비차단 noise로 분류한다.

## 다음 작업 시작점

1. P-0014 통합본의 STRICT review와 local `main` commit을 완료한다.
2. `origin/main` 재확인 뒤 fast-forward push하고 HEAD·remote hash 정합성을 확인한다.
3. 후속 P0 lifecycle Slice는 backup-set 검증·candidate 준비·사용자 선택·active-set atomic switch와 이전 DB version migration fixture다.
4. MCP release 재개는 required core 13개 owner command·Schema·handler와 current Codex·Inspector·native ARM64 evidence를 먼저 해소한다.

## Archive References

- 상세 완료 감사: `docs/testing/mcp-completion-audit.md`
- 독립 감사 판정: `docs/testing/mcp-independent-audit-2026-07-12.md`
- Sol Max 상세 인계문: `docs/testing/sol-max-mcp-review-handoff.md`
- 170개 정본 행: `docs/testing/mcp-verification-matrix.md`
- P-0014 원본 보존 commit: `07a40fe`
