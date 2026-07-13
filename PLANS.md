# PLANS.md

## 목적

이 문서는 Star-Control의 현재 작업 판단을 위한 bounded 원장이다. MCP 상세 감사·결함·증거는 [MCP 구현 완료 감사](docs/testing/mcp-completion-audit.md)에, 독립 reviewer 입력은 [Sol Max MCP 독립 검토 인계문](docs/testing/sol-max-mcp-review-handoff.md)에 둔다.

## Context Pack

### 현재 목표

- P-0015의 1단계 **읽기 전용 Project Catalog와 Code Index** 상세 설계와 정본 동기화는 완료했다.
- scanner, parser, DB migration·cache·watcher와 CLI 제품 구현은 별도 승인·새 P-ID가 필요한 다음 범위다.
- P-0012 MCP release blocker 해소는 이 설계와 독립된 활성 작업으로 유지한다.

### 반드시 지켜야 할 제약

- Codex 전용, Windows 전용이며 local STDIO MCP와 current-user Controller IPC만 사용한다.
- 로컬 AI, 다른 AI 제공자, OpenAI API 직접 호출, HTTP MCP, 브라우저 UI와 자체 예약 실행을 추가하지 않는다.
- raw shell·script host·PATH lookup을 외부 EXE 우회 수단으로 허용하지 않는다.
- `legacy/`, 사용자 변경, 기존 미추적 `--check/`와 루트 `manifest.json`을 보존한다.
- package 설치, 시스템 설정 변경, 파일 삭제, 원격 push와 외부 계정 변경은 별도 승인 없이는 하지 않는다.
- DB backend 이름·SQL·filename은 public 계약·StarConfig·CLI·MCP에 노출하지 않고 `star-state` private adapter에만 둔다.
- source, test, docs, config, schema, migration, generated, vendor, cache와 output을 구분하며 이 단계의 모든 application command는 project source에 `source_effect=none`이어야 한다.

### 이미 끝난 것

- P-0013·P-0014 — 0단계 공통 관리 계약·하이브리드 DB·첫 Slice를 완료했다: [정본](docs/contracts/development-management.md), [로드맵](docs/roadmap/final-implementation.md), `0e94b23`.
- 고정 MCP Gateway·IPC·Registry·외부 EXE Runtime 수직 Slice와 170개 검증 행은 유지한다. 정확한 release blocker는 [MCP 독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md)가 소유한다.
- Git source, local management projection·local-only state와 `.ai-runs` evidence의 정본·Writer 경계는 1단계 선행조건을 충족한다.

### 아직 남은 것

- M1 제품 구현은 Project v1→v2 checkout migration·fixture·backup·rollback gate부터 시작해야 한다.
- 첫 language syntax adapter와 corpus가 정해지기 전에는 언어별 정확도·성능·지원 범위를 완료로 표시할 수 없다.
- 2단계 영향 분석은 fresh ProjectCatalogSnapshot·CodeIndexSnapshot과 tier·coverage·limitation을 소비해야 한다.
- required core 13개 owning command handler·Schema, current Codex·Inspector·native ARM64 evidence, exact Windows 11 24H2 baseline은 MCP release blocker로 남는다.

### 건드리면 안 되는 것

- `legacy/`
- 기존 사용자 미추적 `--check/`, `manifest.json`
- 공개 원격의 사용자 소유 외부 상태. 현재 승인 범위 밖 release·배포·외부 계정·유료 기능

### 먼저 확인할 파일

- `docs/contracts/development-management.md`
- `docs/contracts/project-catalog-and-code-index.md`
- `docs/features/core-control.md`
- `docs/features/profiles.md`
- `docs/contracts/goal-and-stage.md`
- `docs/contracts/config-and-catalog.md`
- `docs/contracts/validation-and-evidence.md`
- `docs/architecture/state-and-artifacts.md`
- `docs/architecture/repository-layout.md`
- `docs/roadmap/final-implementation.md`

### 먼저 실행할 명령

- `git status --short --branch`
- Markdown local link·code fence 검사
- 문서별 구현/설계 상태와 read-only·no scheduler·CLI-only 용어 충돌 검사
- `git diff --name-only`로 제품 코드 무변경 확인
- `git diff --check`

### 현재 차단 요소

- P-0015 문서 작업의 blocker는 없다. M1 제품 구현은 0단계 단일 `root_binding_id`를 `ProjectCheckout`으로 옮기는 승인된 migration 없이는 시작할 수 없다.
- 제품 blocker는 required core 13개 owning command handler·Schema 부재다. current search/describe/invoke는 fail-closed `unavailable`이며 release 완료 조건을 충족하지 않는다.
- current Codex·Inspector·native ARM64 evidence와 exact Windows 11 24H2 baseline이 없다.

## 현재 활성 작업

| ID | 상태 | 목표 | 근거 | 다음 조치 |
|---|---|---|---|---|
| P-0012 | IN PROGRESS — verdict BLOCK | MCP 정본·제품·테스트·생성물·실행 증거 독립 감사와 결함 해소 | `docs/testing/mcp-independent-audit-2026-07-12.md` | core owner 계약 구현 뒤 current evidence 재생성 |

## 최근 완료 작업

| ID | 결과 | 근거 |
|---|---|---|
| P-0013 | DONE — P0 공통 개발 관리·local management DB 상세 설계와 정본 반영 | `docs/contracts/development-management.md`, ADR-0006·ADR-0007 |
| P-0014 | DONE — P0 첫 수직 Slice를 최신 공개 evidence 계약과 통합·FULL 검증·`main` 게시 | `0e94b23`, `docs/roadmap/final-implementation.md` |
| P-0015 | DONE — M1 read-only Project Catalog·Code Index 상세 설계와 정본 동기화 | `docs/contracts/project-catalog-and-code-index.md`, `docs/roadmap/final-implementation.md` |

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
| R-0020 | 언어별 syntax·semantic adapter 정확도 차이 | tier·coverage·limitation과 text fallback을 별도 기록 | 언어별 fixture·conformance는 구현 단계에서 추가 |
| R-0021 | 복수 checkout 도입이 P0 Project v1 단일 root binding과 호환되지 않음 | Project identity와 local ProjectCheckout 분리, v1 자동 재해석 금지 | 구현 전 schema v2·migration fixture 확정 |
| R-0022 | 큰 dirty/non-Git tree의 hash 비용과 partial 관찰 | bounded scope·content hash·incomplete 상태, 이전 complete generation 유지 | 실제 corpus로 limit·cache 기본값 검증 |

## 최신 검증 상태

- TARGETED PASS — 변경 대상은 `PLANS.md`와 `docs/**/*.md`뿐이며 staged·제품 code 변경은 없다. 기존 미추적 `--check/`·`manifest.json`은 보존했다.
- PASS — 변경 문서 local link, target anchor, code fence와 Markdown table 열 수; `docs/README.md` 읽는 순서 1~40; 계약 Inventory 71개.
- PASS — identity·discovery·classification·tier/fallback·hardcoding·full/incremental·freshness·read-only·2단계 입력 필수 계약 표식.
- PASS — 문서에 기록한 Git `rev-parse`, `worktree --porcelain -z`, `status --porcelain=v2 -z` surface를 현재 저장소에서 read-only 실행.
- PASS — `git diff --check`; STRICT 자체 검토에서 BLOCKER·MAJOR 없음.

## 다음 작업 시작점

1. M1 제품 구현을 승인받으면 새 P-ID를 만들고 [Version·Migration 정본](docs/contracts/versioning-and-migrations.md)의 Project v1→v2 checkout migration부터 수행한다.
2. migration conformance 뒤 [M1 첫 read-only Slice](docs/roadmap/final-implementation.md#m1-읽기-전용-project-catalog와-code-index-설계-확정-구현-전)를 순서대로 구현한다.
3. language adapter 선택은 corpus·dependency·license·offline·Windows 근거를 검토한 뒤 정하고 public 계약에 제품명을 넣지 않는다.
4. 2단계 영향 분석은 fresh ProjectCatalogSnapshot·CodeIndexSnapshot, graph, tier·coverage·limitation만 입력으로 사용한다.

## Archive References

- 상세 완료 감사: `docs/testing/mcp-completion-audit.md`
- 독립 감사 판정: `docs/testing/mcp-independent-audit-2026-07-12.md`
- Sol Max 상세 인계문: `docs/testing/sol-max-mcp-review-handoff.md`
- 170개 정본 행: `docs/testing/mcp-verification-matrix.md`
- P0 구현·통합 commit: `0e94b23`
