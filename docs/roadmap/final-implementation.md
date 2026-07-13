# 최종 구현 로드맵

## 원칙

이 로드맵은 작은 시험판만 만들고 멈추는 계획이 아니다. [기능 범위](../product/scope.md)의 상위 경계, [1인 개발자용 구현 대상 기능](../features/README.md)의 A01~D03과 [최종 Repository·Package·문서 구조](../architecture/repository-layout.md)의 책임 경계를 최종 제품 완료 조건으로 삼는다.

다만 한 번에 전체를 구현하지 않는다. 각 단계는 다음 단계가 믿고 사용할 수 있을 만큼 완성하고 검사한다.

15개 개발 작업 유형은 별도 전문 도구를 각각 만드는 단계가 아니다. 공통 관제·검증 기반 위에 Profile과 adapter로 구현하고, 구체적인 도구와 규칙은 해당 단계 직전에 최신 자료로 다시 조사한다.

## D0. 설계 확정 — 완료

### 결과

- 새 프로젝트 헌장
- 전체 구조
- 단계 분해 기준
- 모델 배정 규칙
- Codex 통합 방식
- 승인과 검사 기준
- 상태와 증거 저장 방식
- 병렬 작업과 병합 기준
- 기능 범위와 제외 사항
- 1인 개발자용 구현 대상 기능과 작업 Profile
- 최종 Repository·Package·문서 구조와 의존 규칙
- RouteDecision의 모델 역할·원시 생각 깊이·단계 성격·실행 방식 분리
- 책임별 문서 폴더 migration과 내부 링크 갱신
- D0 최종 설계 결정 기록
- 공개 배포 기준

### 완료 조건

- 새 문서만으로 설계 전체 이해 가능
- 문서 사이 기준 충돌 없음
- 사용자가 최종 방향을 승인했고 [ADR-0001](../decisions/ADR-0001-최종-설계-기준.md)에 고정함

## P0. 공통 개발 관리 계약과 로컬 관리 DB 기반

### 현재 상태

설계는 [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md), [ADR-0006](../decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md)과 [ADR-0007](../decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md)에 확정했다. 사용자가 P0 구현과 embedded relational backend dependency 추가를 승인했고 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)에 private 선택을 기록했다. 0A~0E의 첫 수직 Slice는 workspace test·clippy·Schema·x64/ARM64 release cross-build까지 로컬 검증을 통과했다. 아래 전체 계약 중 후속 lifecycle 범위와 외부 gate는 `PLANS.md`에서 구분한다.

기존 MCP Gateway·IPC·Registry·외부 EXE Runtime 수직 Slice와 P0 관리 수직 Slice는 서로 별도 범위다. 한쪽 검증 결과를 다른 쪽 완료 근거로 사용하지 않는다.

### 목적

모든 후속 scanner, validator, patch 도구, CLI와 Codex 진입점이 같은 project·source·finding·change·validation 의미와 로컬 상태를 사용하게 한다.

```text
Git 선언·Schema·Catalog·source
  -> ProjectRevision + WorkspaceSnapshot
  -> ScanRun + Symbol graph + Finding
  -> ChangePlan + PatchSet
  -> ValidationResult + GateDecision

local management repository
  = global directory·coordination store
  + ProjectId별 derived projection·local operational store

.ai-runs
  = diff·patch·log·trace·report 같은 큰 evidence
```

### 0A. 계약 type과 deterministic fixture

- Project, ProjectRevision, WorkspaceSnapshot
- ScanRun, Rule, Finding, Occurrence
- Symbol, SymbolReference, CanonicalSource
- Suppression, Baseline, Disposition
- ChangePlan, PatchSet, ChangeRecipe
- ValidationResult, GateDecision, ArtifactRef, ManagementStoreStatus
- typed ID, full SHA-256 fingerprint payload, ProjectPathRef와 redaction contract
- minimal/full valid, invalid, future-version과 fingerprint golden fixture

0A type·fixture 자체는 DB dependency에 의존하지 않는다. in-memory fake는 contract conformance용일 뿐 persistence 완료 근거가 아니며, concrete dependency는 0C의 private adapter에만 존재한다.

### 0B. application service와 repository port

- `ManagementApplicationService` command·query와 optimistic revision·idempotency
- backend-neutral `ManagementRepositorySet`, global/project repository, `ArtifactStore`, `ProjectRootBinding` port
- ProjectId partition과 cross-store `StoreVersionVector`
- scan invisible generation·batch·atomic publish
- store-local event·projection·idempotency·store revision transaction
- global `CoordinatedOperation`과 project participant receipt 기반 crash recovery
- CLI·MCP handler의 DB·artifact 직접 접근 금지
- CLI-only composition의 Codex·App Server·다른 AI·OpenAI API dependency 0개

첫 수직 Slice의 query는 current Project·latest Scan·Finding 목록에 한정한다. 대량 목록용 stable cursor와 arbitrary at-store-revision query는 P1 query 계약을 추가할 때 구현하며 P0 완료 근거에 포함하지 않는다.

### 승인 gate — backend와 dependency

사용자가 embedded relational 방향과 dependency 조사·추가를 승인했고 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)은 private `rusqlite 0.40.1` bundled adapter를 선택했다. 0A·0B public contract는 계속 backend-neutral이어야 한다.

- Windows x64·ARM64와 crash 내구성
- single-writer transaction, consistent backup와 integrity 검사
- side-by-side migration·read-only recovery
- license, 보안 update, binary 크기와 유지보수
- Rust error·cancellation·threading 경계

선택 결과는 `star-state` private adapter에만 두고 public contract·StarConfig·CLI·MCP에 backend 이름을 노출하지 않는다. dependency를 추가한 뒤에는 lockfile, license·advisory, Windows x64·ARM64 build와 persistence conformance를 FULL gate로 검사한다.

### 0C. persistence lifecycle

- Controller exclusive writer lease와 startup StoreStatus
- global store와 ProjectId별 project store generation·active pointer
- cross-store prepared/participant/completed crash recovery와 호환 backup generation set
- store version compatibility, migration plan과 pre-migration backup
- backend structural·relation·fingerprint·artifact integrity 검사
- future version과 suspect store의 read-only recovery
- verified backup restore와 side-by-side rebuild·atomic active pointer
- source-derived projection 재scan, `.ai-runs` reindex와 local-only state loss 보고
- startup/manual retention plan과 hold·permission

### 0D. project scan과 Finding vertical slice

1. local-first 또는 shared ProjectId와 Windows current-user protected root binding 등록
2. Git ProjectRevision과 dirty WorkspaceSnapshot 수집
3. 하나의 deterministic Rule로 CanonicalSource·Symbol·Occurrence 생성
4. scan generation finalize와 Finding projection
5. reviewed Baseline, 90일 Suppression과 local Disposition stale 판정
6. CLI project→scan→finding query E2E
7. DB 삭제 뒤 source rescan rebuild와 redaction 검사

### 0E. change·validation vertical slice

1. Finding과 versioned ChangeRecipe에서 ChangePlan 생성
2. base hash가 있는 immutable PatchSet과 `.ai-runs` patch artifact 생성
3. explicit preview·apply와 dirty workspace exact before hash·overlap·permission precondition 확인
4. 적용 뒤 실제 WorkspaceSnapshot을 재수집하고 같은 scan service로 재검사
5. 실행한 검증을 ValidationResult로 정규화
6. Baseline·Suppression·Disposition·policy를 포함한 GateDecision
7. crash·partial apply·stale config·incomplete scan 회귀

P0는 `ChangeSet`·`ValidationRun`이라는 별도 persisted type을 새로 만들지 않는다. P1에서 여러 effect와 validator 실행을 묶을 때 추가하되, 여기서 확정한 `PatchSet`·`WorkspaceSnapshot`·`ValidationResult` reference를 재사용한다.

### Package 소유권

- `star-contracts`: persisted type·ID·fingerprint payload
- `star-domain`: invariant와 stable state
- `star-ports`: repository·artifact·root binding interface
- `star-project`: revision·snapshot·scan·source graph
- `star-validation`: Rule·Finding·decision·ValidationResult·gate
- `star-application`: 공통 command·query와 workflow
- `star-execution`: patch effect·recovery
- `star-state`: DB adapter·transaction·migration·backup·integrity·retention
- `star-evidence`: redaction·diff·report와 `.ai-runs`
- `star-controller`: concrete adapter 조립과 유일한 Writer

### 완료 조건

- Git 정본, DB projection·local state와 `.ai-runs` evidence가 byte·writer 수준으로 분리됨
- 같은 source·Rule·scan config에서 derived ID·fingerprint golden 일치
- global/project store 책임이 분리되고 모든 project-scoped relation이 ProjectId로 격리되며 raw 절대 경로를 복제하지 않음
- secret, 사용자 이름, 개인 절대 경로와 민감 literal이 DB·event·fingerprint에 없음
- scan crash 중 이전 visible generation을 유지하고 batch retry가 idempotent함
- DB 손실 뒤 source-derived current projection을 재구축하고 복구 불가 local-only state를 명확히 보고함
- migration backup·rollback, corruption quarantine와 read-only recovery 통과
- 큰 diff·log·trace·report byte가 DB가 아니라 ArtifactRef로 연결됨
- CLI와 MCP가 same application service를 사용하고 DB 파일을 직접 열지 않음
- CLI-only E2E에서 Codex, App Server, 다른 AI와 OpenAI API 호출이 없음
- backend conformance와 Windows x64·ARM64 persistence smoke 통과
- dependency 선택·license·보안 근거와 사용자 승인 기록

## M1. 읽기 전용 Project Catalog와 Code Index — 설계 확정, 구현 전

M1은 사용자가 지정한 **1단계 개발 관리 확장**이다. 기존 제품 로드맵의 `P1. 기초 계약과 설정`과 번호 체계가 다르므로 구현·완료 보고에서는 항상 `M1 Project Catalog·Code Index`라고 쓴다. 의미 정본은 [Project Catalog·Code Index 계약](../contracts/project-catalog-and-code-index.md)이다.

현재 반영은 문서 설계뿐이다. scanner, parser, DB schema·migration, cache와 watcher 제품 code를 만들지 않았고 CLI command도 구현된 것으로 표시하지 않는다.

### 선행 gate와 migration gap

P0의 공통 ID·fingerprint, source/DB/evidence 경계, global/project store, invisible scan generation, Controller 단일 Writer와 backend-private 원칙은 선행조건으로 충족한다. 다만 P0 `star.project` schema v1은 Project 하나에 `root_binding_id` 하나를 소유하므로 여러 checkout·linked worktree를 정확히 표현할 수 없다.

M1 구현은 다음 gate를 먼저 통과해야 한다.

1. `Project` stable identity에서 local `ProjectCheckout` attachment를 분리한 v2 contract와 Schema를 만든다.
2. 기존 attached Project row 하나를 명시적인 primary checkout 하나로 옮기는 deterministic migration plan·fixture·backup·rollback을 만든다.
3. binding 누락·중복·manifest identity conflict는 추측 복구하지 않고 `detached` 또는 migration block으로 표시한다.
4. ProjectCatalogSnapshot·CodeIndexSnapshot의 global/project store partition과 rebuild·retention 관계를 persistence conformance로 검증한다.
5. 이 gate 전에는 복수 checkout discovery 결과를 current DB 사실로 publish하지 않는다.

### 첫 read-only 수직 Slice

아래 순서를 하나의 구현 Slice로 유지하되 각 번호는 독립 검증 가능한 commit 경계가 될 수 있다.

1. `ProjectCheckout`, `ProjectCatalogSnapshot`, `CodeIndexSnapshot`, partition·tier·freshness·quality envelope type, JSON Schema와 golden fixture를 추가한다.
2. P0 Project v1→v2 migration과 global/project repository port를 먼저 구현하고 in-memory fake와 concrete private adapter의 conformance를 맞춘다.
3. user/CLI가 제공한 여러 protected root binding에서 Git top-level·git/common dir·object format, linked worktree, nested repository, workspace member와 non-Git marker를 read-only로 발견한다.
4. Project stable identity, checkout/worktree identity와 containment·workspace·repository edge를 계산하고 incomplete discovery를 limitation과 함께 ProjectCatalogSnapshot으로 publish한다.
5. source inventory를 만들고 source, test, docs, config, schema, migration, generated, vendor, cache, output primary class와 fixture·example·docs-example facet을 근거·conflict와 함께 확정한다.
6. 언어, build system, package manager, lockfile, toolchain, 실행하지 않은 주요 command 후보와 프로젝트별 AGENTS·정본 문서 우선순위를 발견한다.
7. 사용자가 요청한 첫 manual full scan에서 모든 eligible text index와 첫 지원 언어의 syntax definition/reference adapter를 실행한다. semantic adapter가 없으면 `semantic_unavailable`이며 syntax·text fallback 품질을 그대로 반환한다.
8. project·package·module·symbol·contract·dependency graph, config key·Schema ID·error code·constant·public surface 후보를 만들고 unresolved·ambiguous edge를 보존한다.
9. 절대 경로, URL·IP·port, timeout·retry·limit, raw command, 중복 error string, config 중복 literal을 근거 있는 hardcoding candidate로 만들되 `candidate|warning|review|allowed` assessment와 분리한다.
10. ScanRun staging batch를 ProjectId별 store에 쓰고 complete partition만 atomic publish한다. 재생성 cache와 `.ai-runs` evidence는 DB current pointer와 분리한다.
11. 두 번째 실행부터 Git revision·porcelain status·actual dirty/untracked byte와 file hash로 incremental partition을 선택하고 넓은 manifest·config·adapter 변화는 full scan으로 승격한다.
12. `project discover`, `project list/get`, `scan plan/run/status`, `index status/search/definitions/references`, `graph neighbors`, `finding list/get`의 typed application command/query와 stable cursor를 CLI에 연결한다.

모든 M1 command는 `source_effect=none`이다. source, test, docs, config, schema, migration, generated file, Git index·branch·worktree metadata를 쓰는 port와 `star-execution`을 dependency graph에 넣지 않는다. package install·dependency update·build output 생성·network fetch도 하지 않는다. Controller는 derived DB projection과 cache·evidence만 쓸 수 있다.

### 실행·갱신 경계

- 최초 scan은 사용자가 CLI로 시작하는 full scan이다.
- 후속 scan은 revision·status·file hash와 의미 fingerprint 기반 incremental이며 silent partial update를 current로 승격하지 않는다.
- 주기 scan이 필요하면 OS scheduler 또는 CI가 CLI를 호출한다. Star-Control 자체 scheduler·cron·interval 기능은 만들지 않는다.
- file watcher는 실제 latency·scan 비용·missed-change 자료로 필요성이 검증된 뒤의 선택 기능이다. 첫 Slice와 완료 조건에 포함하지 않는다.
- AI 호출, 의미 추론 모델, Codex App Server, 다른 AI provider와 OpenAI API는 M1 dependency나 성공 조건이 아니다.

### M1 완료 조건

- 여러 root·nested repository·workspace·linked worktree·non-Git project가 stable Project와 local checkout 관계로 재현된다.
- generated/vendor/cache/output과 test/fixture/docs-example가 근거·facet과 함께 구분되고 project별 ignore provenance를 조회할 수 있다.
- 최초 full scan부터 incremental reuse·invalidate·full 승격까지 상태 흐름과 실패 지점이 ScanRun evidence로 남는다.
- DB index보다 current source·config·adapter가 달라지면 partition이 `stale_*`이며 최근 timestamp나 cache hit로 current가 되지 않는다.
- parse 실패, unsupported language, semantic unavailable, partial index와 no-result가 서로 다른 상태이고 실제 text/syntax/semantic tier·coverage·limitation이 query에 남는다.
- hardcoding 결과는 근거 있는 후보이며 자동 확정·자동 수정 경로가 없다.
- required partition과 generation integrity가 complete한 generation만 current이고 crash·cancel·required tier 실패·limit 초과 시 이전 complete generation을 유지한다.
- read-only CLI E2E에서 대상 project source·Git metadata before/after manifest가 같고 M1 discover·scan·index graph의 AI·network·자체 scheduler·project watcher dependency가 0이다. 별도 Tool Registry watcher는 이 경로에서 호출하지 않는다.
- 2단계 영향 분석에 versioned ProjectCatalogSnapshot, CodeIndexSnapshot, graph, source/classification/toolchain/guidance fingerprint와 freshness proof를 제공한다.

## P1. 기초 계약과 설정

계약 의미와 설정 병합 설계는 [데이터 계약 지도](../contracts/README.md), [ADR-0002](../decisions/ADR-0002-데이터-계약과-설정-정본.md), [ADR-0004](../decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)와 [ADR-0005](../decisions/ADR-0005-MCP-구현-계약-동결.md)로 확정했다. MCP exact field·hash·Win32 순서·검증 행렬의 Rust type, generated Schema, fixture와 runtime 수직 Slice는 구현됐다. 공식 MCP Inspector와 native ARM64 Windows 11 25H2 build 26200 실기는 통과했고 exact 24H2 baseline과 독립 최종 검토가 남아 있으므로 P1 전체를 완료로 표시하지 않는다. 세부 판정은 [MCP 완료 감사](../testing/mcp-completion-audit.md)를 따른다. P1의 아직 미구현 공통 계약과 P2 이후 도구는 P0 관리 계약을 소비하며 별도 DB type을 만들지 않는다.

### 첫 수직 Slice

1. [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)의 Rust type·고정 12 tool Schema·JCS hash fixture
2. [Manifest Reference](../contracts/tool-package-manifest-reference.md)의 ToolPackageManifest·ToolDescriptor·TrustRecord·RegistryCache Schema와 generated required `star-control-core.toml`
3. authenticated named pipe, deterministic loader·trust·search index·LKG·watcher+demand scan
4. `rmcp 2.2.0` 기반 fixed `star-mcp.exe`와 fake Controller vertical slice
5. [Windows Tool Runtime](../architecture/windows-tool-runtime.md)의 argv·JSON-STDIO·identity lease·Job Object
6. [MCP 검증 행렬](../testing/mcp-verification-matrix.md)의 실제 Codex same-session C001~C008

실제 `rg`, validator와 debugger 연결은 이 slice가 통과한 뒤 TOML 예시로만 추가한다.

### 구현

- GoalSpec
- StageSpec
- RouteDecision
- `model_role`, `reasoning_effort`, `stage_mode`, `execution_mode`, CapabilitySnapshot의 분리 계약
- PermissionPlan
- ValidationPlan
- EvidenceBundle
- Checkpoint
- MergePlan
- CapabilitySnapshot
- 외부 Tool Registry와 executable trust
- 설정 계층과 프로필
- 상태 전이와 안전한 파일 저장
- foundation Package와 기계 계약 생성 흐름
- Package 의존 방향 검사

### 완료 조건

- 잘못된 입력을 명확히 거부
- 중단 중 파일 손상 없음
- 이전 상태 재개 가능
- safe_default와 personal_auto 동작 구분
- fake EXE·TOML 추가가 Gateway source 변경 없이 같은 MCP session의 search 결과에 나타남
- TOML path 변경과 같은 path의 호환 EXE 교체가 MCP·Controller·Codex 재시작 없이 다음 호출에 반영됨
- 잘못된 candidate는 해당 package last-known-good를 유지하고 다른 package를 막지 않음
- descriptor hash·risk lane·Schema·executable update policy 불일치가 side effect 전에 거부됨
- MCP 검증 행렬 전체 통과, 미실행·flaky·quarantined test 0개
- Windows 11 24H2 x64·ARM64 smoke 통과

## P2. Plugin 진입과 MCP

### 구현

- Star-Control Plugin
- 개발 작업 Skill
- exact 13개 core action을 실제 application command handler에 연결
- installer MCP 설정, Controller startup과 Plugin entry readiness 연결
- 사용자 입력 검사
- 실행 전후 검사
- 설치 상태 확인 명령

### 완료 조건

- Codex 앱 입력에서 Star-Control 목표 시작
- 계획 없는 수정 도구 호출 차단
- 단순 대화는 불필요하게 차단하지 않음
- Plugin, Hook, MCP가 꺼졌을 때 안전하게 실패

## P3. 단계 계획과 자동 배정

### 구현

- 목표 질문
- 단계 분해
- 순서와 병렬 가능성 판단
- 모델·생각 깊이·Max·병렬 실행 배정
- 사용자 계획 수정
- 비용 한도와 승급 규칙

### 완료 조건

- 지나치게 작은 작업 분해를 피함
- 배정 이유를 사람이 이해할 수 있음
- 사용자 선택이 자동 배정보다 우선함
- 지원되지 않는 모델 선택을 안전하게 대체

## P4. Codex 실행과 필요한 자료 묶음

### 구현

- App Server 초기화
- 모델과 기능 조회
- 새 작업 생성, 재개, 분기, 중단
- 단계별 모델·생각 깊이·권한 지정
- 프로젝트 규칙과 관련 파일 탐색
- 앞 단계 결과 전달
- Windows 배경 Controller

### 완료 조건

- OpenAI API 직접 호출 없음
- 앱 종료 뒤에도 상태 재개 가능
- 불필요한 전체 자료 전달을 피함
- App Server 실패 원인과 복구 방법 기록

## P5. 검사·증거·이어하기

### 구현

- 변경 종류별 검사 선택
- 프로젝트 검사 등록
- 범위, 비밀정보, 테스트 약화, 의존 항목 검사
- 실제 diff·완료 주장·증거 검증과 Review Pack
- 테스트 신뢰성, 검증기 보호와 회귀 Corpus
- 계약·구조·설정·보안·실패 재현·문서·성능·release 검증 Profile
- 자동 수정과 제한된 재시도
- 독립 검토
- 증거 묶음과 최종 요약
- 이어하기 기록

### 완료 조건

- 필요한 검사를 빠뜨리지 않음
- 불필요한 전체 검사 남용 없음
- 실패와 미실행 검사를 숨기지 않음
- 자동 완료 조건을 기계적으로 판단

## P6. 병렬 작업과 로컬 병합

### 구현

- 단계별 Git worktree
- 동시 수정 충돌 사전 검사
- 병렬 Codex 실행 한도
- 로컬 검토 정보
- 병합 대기열
- 충돌 처리
- 통합 검사

### 완료 조건

- 겹치는 수정의 잘못된 병렬 실행 방지
- 사용자 기존 변경 보존
- 단계별 결과 추적 가능
- 병합 뒤 전체 목표 검사 통과

## P7. 여러 프로젝트와 원격 저장소

### 구현

- 여러 프로젝트 목표
- 프로젝트 간 순서와 연결 계약
- 로컬 변경 기록
- 원격 업로드
- 검토 요청 생성과 갱신
- 상태 검사와 병합
- 인터넷 조사와 출처 기록

### 완료 조건

- 프로젝트별 변경과 증거 분리
- 제공하는 프로젝트를 먼저 처리
- 원격 대상과 결과 추적
- `safe_default` remote write는 항상 prompt, `personal_auto`도 user-only exact scope opt-in 전에는 prompt
- 출처 없는 최신 정보 사용 방지

## P8. 비용·비교 시험·규칙 개선

### 구현

- 시간, 사용량, 실패, 재작업 수집
- 실제 개발 작업 모음
- 모델과 생각 깊이 비교
- 배정 규칙 변경 기록
- 한도 초과 중단

### 완료 조건

- 거짓 가격이나 사용량을 만들지 않음
- 품질과 안전을 낮춰 비용을 맞추지 않음
- 규칙 변경 전후를 비교할 수 있음

## P9. 공개 배포와 최종 완성

### 구현

- Windows 설치, 업데이트, 제거
- installer-first 배포, 설명된 current-user Controller autostart 기본 활성화와 opt-out
- Plugin 패키징
- Hook 신뢰 안내
- 상태와 기록 정리 명령
- 배포 준비 검사
- 보안과 개인정보 검토
- 전체 사용자 문서

### 완료 조건

- 깨끗한 Windows 환경에서 설치 가능
- 관리자 권한 executor 없이 current-user install·autostart enable/disable 가능
- safe_default 첫 작업 성공
- personal_auto 설정 가능
- 업데이트와 복구 성공
- 포함된 모든 최종 기능 구현
- A01~D03과 15개 작업 Profile의 연결 검증
- Package 소유권·단일 Writer·adapter 경계 검증
- 제외 기능이 다시 들어오지 않음
- 전체 검사와 독립 최종 검토 통과

## 구현 순서 변경

의존관계가 유지된다면 한 단계 안의 세부 순서는 바꿀 수 있다. 다음은 바꾸지 않는다.

- 문서 확정 전에 제품 코드 작성 금지
- 상태와 계약보다 실행 자동화를 먼저 만들지 않음
- P0 application·repository 계약과 승인 없이 DB dependency·backend를 먼저 고정하지 않음
- 단일 실행이 안정되기 전에 병렬 병합을 기본값으로 만들지 않음
- 검사와 증거 없이 원격 자동화를 완료로 보지 않음
- 공개 안전 기본값 없이 personal_auto만 배포하지 않음
