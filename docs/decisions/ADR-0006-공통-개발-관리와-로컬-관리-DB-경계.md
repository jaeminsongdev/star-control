# ADR-0006: 공통 개발 관리와 로컬 관리 DB 경계

## 상태

채택 — 2026-07-12

저장소 topology와 운영 기본값은 후속 [ADR-0007](ADR-0007-P0-하이브리드-저장소와-운영-정책.md)이 구체화한다.

## 맥락

scanner, validator, patch 도구와 이후 CLI·Codex 진입점이 각각 project, finding, source revision과 validation 의미를 만들면 같은 문제를 다른 ID로 기록하고 상태·증거·완료 판단이 갈라진다.

반대로 source code와 공유 규칙을 로컬 DB만의 자료로 만들면 review·복구·여러 PC 사용이 불가능해지고 DB 손상이 source 정본 손상으로 이어진다. 큰 diff·log·trace를 DB에 넣으면 backup·migration·query와 redaction 범위도 불필요하게 커진다.

따라서 후속 구현 전에 공유 계약, 저장 계층과 writer 경계를 먼저 고정해야 한다.

## 결정

### 세 저장 계층

- Git의 선언 파일·Schema·Catalog·source code가 팀이 공유하는 정본이다.
- 로컬 관리 DB는 source-derived 검색·관계·projection과 명시적인 local-only 운영 상태를 가진다.
- `.ai-runs`는 큰 diff, patch, log, trace, report와 검증 가능한 evidence byte를 가지며 DB는 ArtifactRef만 저장한다.

DB는 source code의 유일한 정본이 아니다. 연결된 project와 같은 config·Rule input을 다시 제공하면 current source-derived projection을 재구축할 수 있어야 한다. local-only decision·진행 이력은 backup·export 없이는 복구되지 않는다고 명시한다.

### 공통 domain

[공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md)의 다음 개념을 모든 후속 도구가 공유한다.

- Project, ProjectRevision, WorkspaceSnapshot
- ScanRun, Rule, Finding, Occurrence
- Symbol, SymbolReference, CanonicalSource
- Suppression, Baseline, Disposition
- ChangePlan, PatchSet, ChangeRecipe
- ValidationResult, GateDecision, ArtifactRef

공유 선언 ID, source-derived deterministic ID와 실행 instance ID를 구분한다. fingerprint는 versioned JCS payload의 full SHA-256이며 secret, 사용자 이름, raw 절대 경로와 민감 literal을 input에 넣지 않는다.

### Writer와 진입점

- Controller 한 process만 management repository를 read-write로 연다.
- CLI·MCP handler와 향후 Codex entry adapter는 DB·artifact 파일을 직접 읽거나 쓰지 않고 같은 `ManagementApplicationService`를 호출한다.
- CLI-only command graph는 Codex, App Server, 다른 AI provider와 OpenAI API client를 구성하거나 호출하지 않는다.
- 이후 Codex 연동은 별도 진입점이지만 같은 typed command·query, policy, repository와 gate를 사용한다.

### persistence abstraction

- public 경계는 backend-neutral `ManagementRepository`와 stable contract·error만 노출한다.
- SQL, table, connection string, database filename, journal mode와 backend 이름은 `star-state` private adapter 밖으로 노출하지 않는다.
- event, projection, idempotency와 store revision은 한 transaction이다.
- scan batch는 invisible generation에 쓰고 complete finalization에서만 current generation을 바꾼다.
- project-scoped record는 ProjectId partition과 project-relative path를 사용한다.
- raw project root는 DB에 저장하지 않고 current-user protected opaque root binding으로 분리한다.

### lifecycle

- `management_store_version`을 데이터 계약·설정·IPC version과 분리한다.
- migration·repair 전 consistent backup, 검증된 side-by-side generation과 rollback을 사용한다.
- 미래 version 또는 suspect store는 Controller-owned read-only recovery로만 연다.
- 손상 store는 덮어쓰지 않고 verified backup restore 또는 Git·Catalog·source·`.ai-runs` 기반 rebuild를 수행한다.
- retention은 startup·수동 plan으로만 실행하며 source·공유 선언과 `.ai-runs` byte를 DB row와 함께 자동 삭제하지 않는다.

## backend·dependency gate

이 ADR 자체는 SQLite나 다른 embedded DB를 선택하지 않는다. 이후 사용자가 embedded relational 방향과 P0 구현 진행을 승인했고, concrete private adapter 선택과 근거는 [ADR-0008](ADR-0008-P0-embedded-relational-backend.md)에 기록했다.

dependency 선택 시 Windows x64·ARM64, crash 내구성, single-writer transaction, consistent backup, integrity·read-only open, migration, license, 보안 update, binary 크기와 Rust adapter 경계를 비교한다. 선택 뒤에도 이 항목은 release 검증 gate로 남는다.

선정된 backend는 `star-state` private adapter의 구현 세부다. backend 변경이 public contract·StarConfig·CLI·MCP wire 변경을 요구해서는 안 된다.

## 결과

- 후속 scanner·validator·patch 도구가 같은 ID·fingerprint·project revision·config 경계를 사용한다.
- DB가 사라져도 current source-derived 상태는 재scan할 수 있고 복구 불가 local-only state가 명확하다.
- 큰 evidence와 query store의 lifecycle·retention이 분리된다.
- CLI와 Codex 진입점 사이에 engine·policy·DB writer가 중복되지 않는다.
- persistence dependency 결정이 contract 설계와 분리돼 승인 전 제품 code 변경을 막는다.

비용은 contract와 conformance fixture가 늘고 root binding, artifact와 DB를 함께 다루는 transaction·recovery 설계가 필요하다는 점이다.

## 선택하지 않은 대안

### DB를 source code와 Rule의 유일한 정본으로 사용

review·공유·rebuild가 어렵고 손상 범위가 source 의미까지 확대되므로 선택하지 않았다.

### 각 CLI·scanner·MCP가 DB를 직접 사용

writer 경쟁, redaction 우회와 진입점별 의미 차이를 만들므로 선택하지 않았다.

### 큰 diff·log·trace를 DB blob으로 저장

backup·migration·retention과 손상 범위를 키우므로 ArtifactRef로 분리한다.

### 계약 확정 전에 SQLite dependency를 고정

공개 repository 계약과 Windows lifecycle 요구를 동결하기 전에 구현 선택이 계약으로 새어 나가므로 승인 gate 뒤로 미룬다.

## 구현 상태

이 ADR 채택 시점에는 설계만 존재했다. 후속 구현 상태는 [최종 구현 로드맵](../roadmap/final-implementation.md)과 `PLANS.md`에서 관리한다.

## 연결 문서

- [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md)
- [데이터 계약 지도](../contracts/README.md)
- [설정과 Catalog 계약](../contracts/config-and-catalog.md)
- [이벤트와 상태 계약](../contracts/events-and-state.md)
- [검사·완료·증거](../contracts/validation-and-evidence.md)
- [Version과 Migration 계약](../contracts/versioning-and-migrations.md)
- [상태 기록과 이어하기](../architecture/state-and-artifacts.md)
- [Repository·Package 구조](../architecture/repository-layout.md)
- [최종 구현 로드맵](../roadmap/final-implementation.md)
- [ADR-0007](ADR-0007-P0-하이브리드-저장소와-운영-정책.md)
