# 최종 구현 로드맵

## 원칙

이 로드맵은 작은 시험판만 만들고 멈추는 계획이 아니다. [기능 범위](../product/scope.md)의 상위 경계, [1인 개발자용 구현 대상 기능](../features/README.md)의 A01~D03과 [최종 Repository·Package·문서 구조](../architecture/repository-layout.md)의 책임 경계를 최종 제품 완료 조건으로 삼는다.

다만 한 번에 전체를 구현하지 않는다. 각 단계는 다음 단계가 믿고 사용할 수 있을 만큼 완성하고 검사한다.

최종 16개 개발 작업 유형은 별도 전문 도구를 각각 만드는 단계가 아니다. 공통 관제·검증 기반 위에 Profile과 adapter로 구현하고, 구체적인 도구와 규칙은 해당 단계 직전에 최신 자료로 다시 조사한다.

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

설계는 [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md), [ADR-0006](../decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md)과 [ADR-0007](../decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md)에 확정했다. 사용자가 P0 구현과 embedded relational backend dependency 추가를 승인했고 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)에 private 선택을 기록했다. 0A~0E의 첫 수직 Slice는 workspace test·clippy·Schema·x64/ARM64 release cross-build까지 로컬 검증을 통과했다. P-0054는 최신 `main`에서 실사용 전 복구 Slice의 public 계약, private persistence, recovery-only Controller·CLI와 disposable 손상 복구 Corpus를 구현했다. 아래 전체 계약 중 M1~M11 제품 경로와 외부 gate는 계속 `PLANS.md`에서 구분한다.

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
- ActiveSetManifest, BackupPlan·BackupSetManifest·BackupApplyResult, RecoveryStatus
- RestorePlan·RestoreApplyResult, RebuildPlan·RebuildApplyResult
- LocalStateBundle과 export/import plan·result
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

P-0054 구현은 startup에서 검증된 active-set만 선택하고, online backup의 manifest를 마지막에 쓰며, restore/rebuild candidate 전체를 검사한 뒤 top-level manifest를 atomic replace한다. recovery-only에서는 status·restore·rebuild·local-state export만 허용한다. 모든 apply는 exact plan fingerprint와 private typed receipt를 사용하고 손상·이전 generation을 삭제하지 않는다. DB v2 migration, installer와 MCP required core 확장은 이 Slice에 포함하지 않는다.

### 0D. project scan과 Finding vertical slice

1. local-first 또는 shared ProjectId와 Windows current-user protected root binding 등록
2. Git ProjectRevision과 dirty WorkspaceSnapshot 수집
3. 하나의 deterministic Rule로 CanonicalSource·Symbol·Occurrence 생성
4. scan generation finalize와 Finding projection
5. reviewed Baseline, 90일 Suppression과 local Disposition stale 판정
6. CLI project→scan→finding query E2E
7. DB 삭제 뒤 source rescan rebuild와 redaction 검사

P-0054 disposable E2E는 backup으로 local decision·Finding·ArtifactRef를 복원한 뒤 다시 손상시켜 source rebuild를 수행하고, source-derived projection·verified ArtifactRef reindex와 local-only loss report를 함께 검증한다. 실제 사용자 management root와 실제 프로젝트는 사용하지 않는다.

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

## M1. 읽기 전용 Project Catalog와 Code Index — P-0042 + P-0054 제품 경로 구현

M1은 사용자가 지정한 **1단계 개발 관리 확장**이다. 기존 제품 로드맵의 `P1. 기초 계약과 설정`과 번호 체계가 다르므로 구현·완료 보고에서는 항상 `M1 Project Catalog·Code Index`라고 쓴다. 의미 정본은 [Project Catalog·Code Index 계약](../contracts/project-catalog-and-code-index.md)이다.

P-0042의 persisted Catalog·Rust index 첫 Slice에 더해 P-0054는 explicit multi-root discovery, source inventory·revision·freshness, current index query와 Project-scoped Controller·stable JSON CLI 제품 경로를 구현했다. scanner/index는 source-derived projection이며 active generation 전환 전 partial 결과를 current로 표시하지 않는다. 자체 scheduler·project watcher·AI semantic engine은 범위가 아니고 구현하지 않았다.

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

## M2. 변경 계획·영향 분석·affected 검사 선택 — P-0043 + P-0054 제품 확장 구현

M2는 사용자가 지정한 **2단계 개발 관리 확장**이다. 의미 정본은 [변경 계획·영향 분석 계약](../contracts/change-planning-and-impact.md)이다. P-0043은 M1 persisted graph를 소비하는 `star-planning` pure engine, full planning contract·generated Schema·fixture, dirty workspace collector, idempotent global projection과 `planning create/get` Controller·CLI를 구현했다. P-0054는 scope revise·status/history·impact/affected inspect·override/waiver·invalidate/replan, append-only revision repository, project-scoped resolved toolchain Check와 previous-success compatibility 판정을 실제 제품 경로와 E2E로 확장했다. P-0031/P-0035 `ValidationPlan` v1과 validator 실행 cache는 역사적 precursor이며 full plan의 실행은 M3가 소유한다.

### 선행 gate와 migration gap

M2 구현은 다음을 먼저 요구한다.

1. M1 Project v1→v2 checkout migration과 ProjectCatalogSnapshot·CodeIndexSnapshot public type이 구현돼야 한다.
2. 대상 Project·Checkout의 inventory·graph required partition이 current이고 tier·coverage·limitation query가 동작해야 한다.
3. P0 ChangePlan v1을 일반 사용자 TaskSpec·ScopeRevision·ImpactAnalysis와 연결하는 v2 migration dry-run·backup·rollback이 있어야 한다.
4. Task·Check·RiskPath descriptor Schema와 conflict·fingerprint·trust conformance가 있어야 한다.
5. M2 application graph에 source-write port, test runner, Codex·AI·network와 cross-repo VCS mutation adapter가 없어야 한다.

M1 input이 stale·partial이면 M2는 이를 confirmed impact로 사용하지 않는다. fresh scan 요구, possible impact·limitation 또는 safe validation fallback 중 계약상 가능한 결과만 반환한다.

### 첫 CLI-only read-only 수직 Slice

1. `TaskSpec`, `ScopeRevision`, `ImpactAnalysis`, 확장 `ChangeSet`, `ValidationPlan`과 nested `ResolvedProfileRef`·`PhaseSubjectExpectation`, `ChangePlan v2` type·Schema·golden fixture를 추가한다.
2. 사용자의 objective, target Project·Checkout, include/exclude, intended change와 완료 조건을 구조화 입력으로 받고 누락·모호함을 fail-closed로 처리한다.
3. requested, analysis, planned change, validation scope를 분리하고 planned change scope 자동 확대를 금지한다.
4. ProjectRevision과 staged·unstaged·untracked actual byte를 비교한 project별 planning-baseline ChangeSet을 만든다.
5. TaskSpec target과 actual ChangeSet에서 path·symbol·package·contract·config·schema seed를 만든다.
6. file·symbol·package·contract·config·schema·test·docs·generated source의 direct/transitive·confirmed/possible ImpactEdge를 계산한다.
7. same literal도 Project·owning Symbol·Contract identity가 다르면 별도 node로 유지한다.
8. current exported entity edge로 여러 Project의 downstream 영향을 read-only 계산하고 stale consumer는 possible로 보존한다.
9. auth·secret, public API·Schema, dependency·lockfile, validator·policy, migration, workflow·release, generated source RiskPathDescriptor를 평가한다.
10. user, risk, impacted entity, Task/Profile, Project Catalog metadata 순으로 Check candidate를 모으고 resolved Profile closure·activation evidence·required family floor를 ValidationPlan에 materialize한다.
11. `not_applicable`, `not_found`, unavailable, user waiver를 서로 다른 outcome으로 만든다.
12. package closure와 Check scope binding을 증명하면 package affected를 선택하고, 불명확하면 workspace, 다시 affected Project full로 승격한다.
13. previous successful revision·ValidationResult의 source lineage, descriptor·tool·config·scope와 current dirty delta compatibility를 판정한다.
14. 같은 TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet fingerprint를 가진 project별 ChangePlan과 global ValidationPlan을 CoordinatedOperation participant receipt로 publish한다.
15. source·Catalog·Index가 계산 중 바뀌면 ready publish를 막고 새 revision 재계획을 요구한다.

### 실행·변경 경계

- CLI-only mode에서 Codex·AI가 계획을 생성하지 않는다.
- 2단계는 test, build, lint, docs generator와 validator를 실행하지 않고 실행 가능한 ValidationPlan만 만든다.
- source, Git metadata, shared config·Catalog와 remote state를 수정하지 않는다.
- 여러 Project 계산은 Project별 snapshot·ChangeSet·affected scope를 유지하며 cross-repo patch·worktree·merge를 만들지 않는다.
- 사용자가 scope·impact·Check 결정을 수정하면 자동 계산보다 우선하고 새 TaskSpec/ScopeRevision과 remaining risk를 남긴다.
- graph/resource limit은 silent truncation이 아니라 possible frontier·limitation·fallback 근거다.

### M2 완료 조건

- 사용자 입력만으로 CLI-only TaskSpec·ScopeRevision·ChangePlan·ValidationPlan을 만들 수 있다.
- AI 없이 current Project Catalog·Code Index와 descriptor로 영향 graph·검사 후보를 결정적으로 계산한다.
- direct/transitive, confirmed/possible, confidence·limitation·no-result가 독립 field로 보존된다.
- related Check `not_found`와 complete applicability의 `not_applicable`이 다르다.
- unsafe한 affected 축소 대신 package→workspace→affected Project full promotion이 기계적으로 재현된다.
- possible impact가 boundary 밖 closure를 만들지 않으면 불필요한 full 검사를 강제하지 않는다.
- previous-success reuse는 exact source lineage·descriptor·tool·config·scope·dirty delta 조건을 모두 만족할 때만 적용된다.
- 여러 Project의 before/after source·Git metadata가 동일하고 local management document·evidence 외 side effect가 없다.
- 3단계 fake validation engine이 ValidationPlan을 다시 선택하지 않고 readiness·Profile closure·phase subject expectation·fingerprint·TaskInvocation binding을 검증해 소비한다.
- stale input, unresolved required Check, unaccepted scope revision은 `replan_required|blocked`이며 성공으로 표시되지 않는다.

## P1. 기초 계약과 설정

계약 의미와 설정 병합 설계는 [데이터 계약 지도](../contracts/README.md), [ADR-0002](../decisions/ADR-0002-데이터-계약과-설정-정본.md), [ADR-0004](../decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)와 [ADR-0005](../decisions/ADR-0005-MCP-구현-계약-동결.md)로 확정했다. MCP exact field·hash·Win32 순서·검증 행렬의 Rust type, generated Schema, fixture와 runtime 수직 Slice는 구현됐다. 2026-07-12 독립 감사 당시 required core 13개 owner·Schema 부재와 stale evidence는 역사적 사실로 보존한다. P-0030·P-0031·P-0035는 먼저 여섯 action을 연결했고 P-0044는 durable Goal/Plan/Run 9개, P-0050은 `merge.status`·`handoff.get` 2개를 추가해 current source readiness를 17/17로 닫았다. 현재 설치본은 과거 6-action build이므로 source candidate의 새 11개 action을 installed success로 추측하지 않는다. source candidate install·current Inspector와 signed x64 Stable lifecycle은 아직 외부 Gate이므로 P1 release 판정은 **BLOCK**이며 완료로 표시하지 않는다. 세부 판정은 [P-0053 최종 출시 감사](../testing/p53-final-release-audit-2026-07-20.md), [MCP 독립 감사](../testing/mcp-independent-audit-2026-07-12.md)와 [MCP 완료 감사](../testing/mcp-completion-audit.md)를 따른다.

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
- Windows 11 24H2 x64 native smoke와 ARM64 Preview cross-build·simulation Gate 통과. native ARM64는 `native_unverified`로 남기고 Stable 성공으로 표시하지 않음

## P2. Plugin 진입과 MCP

### 구현

- Star-Control Plugin
- 개발 작업 Skill
- required core 17개 중 남은 11개 action을 실제 application command handler와 owning Schema에 연결
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

- M2 TaskSpec·ScopeRevision을 재사용한 목표 질문과 사용자 계획 수정
- 단계 분해
- 순서와 병렬 가능성 판단
- 모델·생각 깊이·Max·병렬 실행 배정
- 사용자 계획 수정
- 비용 한도와 승급 규칙

P3는 M2의 change impact·affected selector를 다시 구현하지 않는다. M2 ChangePlan·ValidationPlan을 StageGraph에 연결하고 모델·권한·실행 순서를 배정한다.

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

### 현재 상태

사용자가 지정한 3단계 `M3 공통 검증·품질 Gate`는 [상세 설계](../features/common-validation-gate.md), [검사·증거 계약](../contracts/validation-and-evidence.md), [오류·Diagnostic](../contracts/errors-and-diagnostics.md)과 [설정·Validator Registry](../contracts/config-and-catalog.md)에 목표 계약을 확정했다. P-0044는 ready M2 plan을 재선택 없이 소비하는 deterministic CheckGraph runner, v2 TaskInvocation·ValidationRun·Diagnostic·GateDecision·EvidenceBundle, Project별 원자 persistence와 Goal/Plan/Run core action을 구현했다. P-0054는 trusted typed process executor, Rule/Baseline/Suppression/Disposition·ReviewPack, exact subject/profile binding, single-use permit와 patch pre/post Gate를 Controller·CLI까지 연결했다. cycle·dependency failure·timeout·partial·flaky·human review와 source snapshot TOCTOU fixture가 수직 Slice를 검증한다.

P0의 Finding·ValidationResult와 P-0035 native validator precursor는 history·compatibility 경로로 유지한다. M7~M11 전용 descriptor와 Profile-required Check는 P-0054의 동일 M2→M3 경로를 사용하며, 등록되지 않은 외부 provider의 실행 결과는 합성하지 않는다.

### 선행 gate

P5/M3 제품 구현은 다음을 먼저 요구한다.

1. M1 Project v1→v2 checkout migration, current ProjectCatalogSnapshot·CodeIndexSnapshot과 partition freshness query가 구현돼야 한다.
2. M2 TaskSpec·accepted ScopeRevision·ChangeSet·ImpactAnalysis·ChangePlan v2·`readiness=ready` ValidationPlan과 typed CheckGraph/TaskInvocation이 구현돼야 한다.
3. M2 fake consumer contract에서 stale subject·descriptor·permission이 `replan_required`이고 runner가 Check를 재선택하지 않는 것이 검증돼야 한다.
4. M3 Rule·Baseline·Suppression·Disposition v1→v2와 Diagnostic historical projection migration의 dry-run·backup·rollback, invalid/future-version fixture를 먼저 확정해야 한다.
5. project Check가 trusted ToolDescriptor, typed scope binding, result parser와 Diagnostic mapping을 가져야 한다. raw shell command text는 실행 계약이 아니다.

M1/M2 input이 stale·partial·unverified이거나 ValidationPlan이 ready가 아니면 M3는 검사를 실행해 성공을 합성하지 않는다.

### 첫 공통 수직 Slice

1. M2 ValidationPlan의 `ResolvedProfileRef`·`PhaseSubjectExpectation` conformance를 고정하고, `EvidenceSubjectBinding`, `SubjectBindingRecord`, binding set, `CompletionClaim`, `ClaimEvaluation`, 확장 ValidationRun·ValidationResult·Diagnostic v2·DiagnosticEvaluation·RunSatisfaction·EvidenceRefSet·GateDecision v2·EvidenceBundle v2·ReviewPack v1 contract type과 Schema·golden fixture를 추가한다.
2. Rule v2 domain/producer, Baseline·Suppression·Disposition v2, v1 migration, Diagnostic v1 historical projection, stale/incompatible·expiry와 explicit activation conformance를 구현한다.
3. ValidatorRegistrySnapshot·GatePolicyDescriptor·세 Profile validation metadata의 load·trust·conflict·fingerprint·last-known-good conformance를 구현한다.
4. pure preflight에서 TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet·ChangePlan·ValidationPlan coherence, current WorkspaceSnapshot, Catalog·Rule·Check·Tool identity, CheckGraph와 typed invocation을 검증한다.
5. fake ToolExecutorPort로 CheckGraph order, parallel group, dependency failure, timeout·cancel, output limit, attempt history와 undeclared side effect를 검증한다.
6. external tool result를 stable RuleRef·severity·confidence·LocationRef·evidence·fingerprint·remediation Diagnostic으로 정규화하고 unmapped/truncated/redaction 실패를 fail-closed로 처리한다.
7. B01 `change_scope`에서 actual add·modify·delete·rename, missing/unexpected/out-of-scope, preexisting 변경 보존과 CompletionClaim을 비교한다.
8. B03 `validator_guard`에서 pre-change trusted/current snapshot, Rule 삭제·severity 하향·allowlist 확대·required Check 제거와 positive·negative·edge·regression fixture 누락을 검사한다.
9. baseline `new|existing_unchanged|worsened|improved|not_observed|incompatible|unbaselined`, suppression `active|expired|stale|revoked|invalid`, flaky와 false-positive 상태를 raw Diagnostic·result와 분리해 평가한다.
10. pure Gate engine이 `clean_pass|ratchet_satisfied|unsatisfied|waived_for_review`를 거쳐 `AUTO_PASS|HUMAN_REVIEW|BLOCK`과 deterministic decision fingerprint를 만든다.
11. ValidationRun·raw Diagnostic·Result·Evaluation·GateDecision을 commit한 뒤 `GateDecision -> EvidenceBundle -> ReviewPack` 순서로 hash를 확정하고 `.ai-runs`에 EvidenceBundle·ReviewPack·ReworkDirective를 redaction과 함께 export한다. packaging 실패는 `auto_pass` decision을 다시 쓰지 않지만 자동 완료를 막는다.
12. fake 4단계 Patch engine으로 `patch_pre_apply` exact PatchSet/current binding과 `patch_post_apply` 새 WorkspaceSnapshot·actual ChangeSet Gate, 중간 stale invalidation을 검증한다.
13. CLI-only E2E에서 Codex·App Server·다른 AI·OpenAI API dependency 0개, 의미 검토의 `HUMAN_REVIEW`, before/after source effect manifest를 확인한다.

### B02·B04~B07 확장 Slice

공통 수직 Slice 뒤 같은 contract로 다음을 추가한다.

- B02: related test 선택 근거, test/case 삭제, assertion·expected 약화, skip·ignore·only, timeout·retry, snapshot mass update와 before-fail/after-pass 회귀 pair
- B04: package dependency, cycle, 공개 경계, 금지 import, hardcoding·정본 drift, generated direct edit와 source/generated drift
- B05: built-in secret·위험 command 경량 검사와 등록된 외부 SAST·dependency·license·vulnerability tool 정규화. 자체 취약점 DB는 만들지 않음
- B06: failure fingerprint, ReproductionPack, 같은 test/input/environment의 수정 전·후 evidence
- B07: Markdown link·anchor, 등록 command, config example, CLI·Schema·generated reference drift와 Windows environment 정적 검사

B08 성능과 B09 CI·release는 위 공통 Gate·Diagnostic·EvidenceBundle을 재사용하는 후속 P5 Slice다. 각 project가 선언한 workload/release 경로가 있을 때만 활성화한다.

### 실행 불변식

- P5 runner는 M2가 선택한 Check family·scope·fallback을 조용히 다시 고르지 않는다.
- source·plan·descriptor·config·Catalog·Tool·permission이 달라졌으면 실행을 중단하고 M2 `replan_required` 또는 Gate block으로 돌려보낸다.
- required `not_run`, `partial`, `unverified`, stale와 flaky는 pass가 아니다.
- 다른 revision evidence는 history로만 연결하고 current `AUTO_PASS` 입력으로 사용하지 않는다.
- existing debt ratchet은 raw Check fail을 pass로 다시 쓰지 않고 별도 `ratchet_satisfied` 근거로 설명한다.
- validator 변경은 current self-test만으로 자기 자신을 승인하지 않는다.
- CLI-only mode는 AI 독립 review를 요구하지 않고 의미 판단을 `HUMAN_REVIEW`로 남긴다.

### 완료 조건

- 실제 diff·작업 계약·보고된 add/modify/delete/rename과 완료 주장을 current evidence로 대조한다.
- 모든 required Check가 실제 실행·complete·current·stable이거나 계약상 허용된 existing-debt ratchet임을 기계적으로 설명한다.
- 기존 부채, 신규 문제, 악화, active suppression과 expired/stale exception이 구분된다.
- test·architecture·hardcoding·docs·security 결과가 같은 Diagnostic·Gate contract를 사용한다.
- validator·policy·test harness 약화와 Rule fixture 누락을 별도 guard가 차단한다.
- 외부 scanner·test tool 결과가 registered ToolDescriptor로 실행되고 common Diagnostic으로 정규화된다.
- EvidenceBundle이 authoritative GateDecision을, ReviewPack이 그 EvidenceBundle을 단방향 hash로 가리키며 실패·미실행·오탐·flaky를 숨기지 않는다.
- 자동 수정은 pre-apply `AUTO_PASS` 뒤에만 시작하고, 자동 완료는 post-apply `AUTO_PASS`와 complete EvidenceBundle·ReviewPack packaging 뒤에만 가능하다.
- CLI-only E2E에서 AI dependency가 0이고 필요한 의미 검토는 `HUMAN_REVIEW`다.
- Windows x64·ARM64에서 contract·runner·Corpus·redaction·crash/attempt conformance가 통과한다.

## M4. 안전한 Patch·Refactor·codemod 엔진 — P-0045 + P-0054 제품 경로 구현

### 현재 상태

4단계 정본은 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md)에 확정했다. P-0045는 built-in trailing-whitespace Recipe를 첫 실제 수직 Slice로 사용했고, P-0054는 `ChangeRecipeV2`·`RecipeExecution`·`PatchSetV2`·`PatchApplication`·`WorktreeDecision`, typed selector/operation, isolated prepare, exact approval/permit, source TOCTOU, partial/outcome-unknown recovery와 Controller·CLI를 연결했다. `patch.prepare/apply/recover`는 Project partition과 current source snapshot을 다시 확인하며 unrelated dirty file을 보존한다.

M11 Rust adapter와 managed Registry rewrite가 같은 v2 경계를 사용한다. 등록된 external codemod adapter가 없는 Recipe는 임의 shell로 대체하지 않으며, 실제 사용자 checkout에는 이번 감사 중 apply하지 않았다.

### 선행 gate

M4 제품 구현은 다음을 먼저 요구한다.

1. M1 ProjectCheckout migration, current ProjectCatalogSnapshot·CodeIndexSnapshot과 symbol/reference·contract·generated ownership query가 구현돼야 한다.
2. M2 accepted TaskSpec·ScopeRevision, ChangePlan v2, preview ChangeSet을 소비하는 ImpactAnalysis·Profile closure·affected selector와 `readiness=ready` ValidationPlan이 구현돼야 한다.
3. M3 `patch_pre_apply|patch_post_apply`, current subject binding, B01·B02·B04, external result normalization, EvidenceBundle·ReviewPack이 실제 제품 gate를 통과해야 한다.
4. P0 Recipe/PatchSet v1을 historical reader로 보존하고 ChangeRecipe v2·PatchSet v2·RecipeExecution v1·PatchApplication v1 migration dry-run·backup·rollback fixture를 확정해야 한다.
5. single-project source mutation·worktree port가 exact before/after receipt와 crash recovery를 제공해야 한다. concrete language/codemod tool은 이 공통 contract 뒤에 선택한다.

M1·M2·M3가 문서 설계뿐이거나 stale·partial·unverified이면 M4가 fake input으로 write readiness를 합성하지 않는다.

### 첫 CLI-only 수직 Slice

1. ChangeRecipe v2, TargetSelector, assurance contract, expected postcondition, RecipeExecution v1, PatchSet v2, PatchApplication v1, WorktreeDecision과 operation/recovery nested type·Schema·golden fixture를 추가한다.
2. Recipe stable ID·SemVer·definition fingerprint, local input Schema, language/capability와 `text_replace|syntax_rewrite|symbol_aware_rewrite|codegen` invariant를 구현한다.
3. `managed_declaration|contract|symbol|path_range|finding_occurrence|generator_input` selector를 M1 current index에 bind하고 raw literal-only selector·ambiguous/partial target을 거부한다.
4. `patch.prepare`를 target source effect가 없는 use case로 구현한다. built-in preview는 materialized root, external mutator·formatter·generator는 exact base의 isolated worktree만 사용한다.
5. preview workspace actual before/after에서 `ChangeSet(change_set_kind=recipe_preview)`을 만들고 scope 밖 add·modify·delete·rename·generated output을 거부한다.
6. 같은 M2 impact·risk·Profile closure·affected selector를 preview ChangeSet에 다시 적용한다. 새 change class·risk·Check·fallback이 생기면 candidate를 invalidated하고 replan 뒤 prepare를 다시 실행한다.
7. expected-after에서 같은 Recipe·input·Tool identity를 replay해 operation 0건과 postcondition을 확인한다. idempotence가 failed/unverified이면 automatic apply를 막는다.
8. immutable PatchSet·diff·영향·selected Check·worktree·permission·forward/reverse artifact를 apply 전에 표시한다.
9. fake SourceMutationPort에서 pre Gate·single-use permit, exact operation journal, per-path receipt, fail-before-effect·partial·outcome unknown과 reverse PatchSet을 구현한다.
10. post apply에서 새 WorkspaceSnapshot·`observed_after_change` ChangeSet을 수집하고 M2 selected format·build·test·contract Check를 M3 Gate로 실행한다.
11. Tool Registry external codemod의 typed args·structured output, Tool/executable version/hash와 start failure·timeout·cancel·malformed·output limit·crash evidence를 구현한다. external EXE 자동 retry는 없다.
12. CLI `recipes list/describe/validate`, `change prepare`, `patch show/apply/status/recover`를 같은 ManagementApplicationService에 연결하고 Codex·AI dependency 0을 확인한다.

### write와 worktree 불변식

- prepare command에는 live target apply 경로와 `--apply` shortcut이 없다.
- 한 PatchSet·PatchApplication은 한 Project·한 Checkout만 소유한다.
- external mutating codemod는 live target root를 input·cwd·environment로 받지 않는다.
- base revision·dirty manifest·config·Catalog·Index·Tool·approval drift는 effect 전에 차단한다.
- dirty-disjoint를 complete하게 증명할 수 있을 때만 current checkout apply를 검토한다. overlap·unknown은 block 또는 isolated worktree다.
- multi-file atomicity를 주장하지 않고 partial/outcome unknown을 recovery state로 보존한다.
- rollback은 exact reverse PatchSet 또는 Star-Control-owned isolated worktree 폐기다. primary checkout hard reset·삭제가 아니다.
- post Gate와 complete evidence packaging 없이는 automatic completion을 만들지 않는다.
- cross-project source write·merge·commit·push는 사용자 로드맵 9단계 전에는 port와 CLI에서 거부한다.

### 5단계 Registry 인계 gate

M4는 다음 5단계가 별도 rewrite engine 없이 같은 path를 사용할 수 있어야 완료다.

```text
ManagedDeclarationId + typed desired value
  -> M1 owner Symbol·Contract resolution
  -> M2 impact·ValidationPlan·ChangePlan
  -> M4 ChangeRecipe + PatchSet preview
  -> M3 pre Gate -> PatchApplication -> M3 post Gate
```

이를 위해 `managed_declaration` selector, expected declaration fingerprint, typed postcondition, contract/docs/generated consumer Check ref와 single-project PatchSet fixture를 고정한다.

### 완료 조건

- scan/index와 rewrite/apply가 다른 phase·port다.
- dry-run 없이 즉시 변경되는 기본·숨은 경로가 없다.
- raw literal만으로 global replacement를 만들지 않는다.
- text·syntax·symbol-aware·codegen의 보장과 limitation이 구분된다.
- PatchSet·diff·impact·검사·worktree·rollback을 apply 전에 확인할 수 있다.
- 사용자 dirty change를 overwrite하지 않고 overlap은 block/isolated decision으로 남는다.
- Recipe replay idempotence와 apply idempotency key가 별도 검증된다.
- partial apply·timeout·cancel·malformed output·outcome unknown을 성공으로 표시하지 않는다.
- M2가 선택한 format·build·test·contract Check를 M3 pre/post Gate가 실행한다.
- Recipe·Tool version/hash·input/output·operation receipt·recovery가 EvidenceBundle에 남는다.
- CLI-only E2E가 Codex 없이 Recipe와 target을 받고 같은 application service를 사용한다.
- cross-project write가 거부되고 5단계 managed Registry 변경이 PatchSet으로 표현된다.
- Windows x64·ARM64에서 selector·preview·worktree·apply·partial/recovery·external tool failure Corpus가 통과한다.

## M5. 관리형 Symbol·상수·에러 코드 Registry — P-0046 bounded Slice 구현

### 현재 상태

5단계 정본은 [관리형 Symbol·상수·에러 코드 Registry 계약](../contracts/managed-symbol-registry.md)과 [ADR-0009](../decisions/ADR-0009-Git-정본-Managed-Registry와-Patch-Gate-경계.md)에 확정했다. P-0046의 Git source manifest loader와 `ManagedRegistrySnapshot` 위에 P-0054가 fragment merge·candidate/lifecycle·consumer binding·drift, append-only `DevelopmentRecord`, inspect/plan/rewrite Controller·CLI와 M4 PatchSet apply 경로를 구현했다. DB는 source manifest를 역으로 정본화하지 않고, 실제 source write는 exact Patch approval/Gate를 우회하지 않는다. TOML rewrite는 의미 보존을 검증하지만 comment 보존 formatter는 아니다.

Registry는 세 분류를 유지한다.

- `managed_declaration`: 사용자가 승인한 공유 계약이며 Git manifest가 정본
- `candidate`: scanner가 발견했지만 중앙 소유를 승인하지 않은 값
- `local_implementation_constant`: 검색할 수 있으나 Registry가 소유·변경하지 않는 지역 상수

DB ManagedRegistrySnapshot은 derived Index다. source manifest와 다르면 Git을 우선하고 DB를 stale로 처리하며 DB row에서 source를 직접 동기화하지 않는다.

### 선행 gate

M5 제품 구현은 다음을 먼저 요구한다.

1. M1 current ProjectCatalogSnapshot·CodeIndexSnapshot에서 definition/reference·contract·Schema·docs·generated ownership과 consumer relation을 조회할 수 있어야 한다.
2. M2가 ManagedDeclarationChangeIntent를 typed seed로 받고 namespace·lifecycle·downstream consumer·minimum version을 ImpactAnalysis·ChangePlan·ValidationPlan으로 계산해야 한다.
3. M3가 duplicate ID, namespace collision, alias·ID reuse, binding·generated drift와 consumer 미전환을 pre/post Gate·EvidenceBundle로 판정해야 한다.
4. M4가 `managed_declaration` selector로 one-Project manifest·binding change를 dry-run하고 immutable PatchSet, actual receipt와 recovery를 제공해야 한다.
5. M1→M4 input이 current·complete하지 않으면 Registry DB/UI가 source write readiness를 합성하지 않아야 한다.

### 지원 순서

1. error code·Diagnostic ID
2. Schema ID·version
3. config key·default
4. CLI command·exit code
5. event·capability·permission ID
6. feature flag ID
7. 여러 Project가 공유하는 format·resource ID
8. 사용자가 승인한 전역 상수

각 kind를 시작할 때 uniqueness scope, type/value role, binding strategy, consumer version과 removal Check를 별도 fixture로 고정한다. 같은 raw 값이라는 이유로 의미·owner가 다른 상수를 합치지 않는다.

### 첫 error-code 수직 Slice

1. ManagedRegistryManifest·Fragment·Snapshot, ManagedDeclaration·AliasRecord·BindingSpec·ConsumerContract·RegistryConsistencyRecord type·Schema·minimal/full/invalid/future fixture를 추가한다.
2. root가 명시한 local fragment만 읽고 namespace claim, stable declaration ID, owner, type, `stable_identifier`, source 위치와 영구 tombstone을 검증한다.
3. ErrorEnvelope code와 Diagnostic Rule ID를 candidate로 scan하되 승인 없이 manifest에 넣지 않는다. display message는 stable code와 분리한다.
4. Git manifest에서 current derived snapshot을 만들고 M1 definition/reference, Schema·documentation binding과 consumer observation을 연결한다. invalid source에서는 이전 snapshot을 current로 publish하지 않는다.
5. duplicate ID·public value, namespace collision, owner/type mismatch, alias cycle/window, removed/reserved reuse와 stale DB Index를 fail-closed로 진단한다.
6. user-approved candidate promotion, additive code, message-only update, deprecation+replacement+bounded alias, consumer transition, removal+tombstone ChangeIntent fixture를 만든다.
7. 각 변경을 M2 impact/compatibility → M4 dry-run PatchSet → M3 pre Gate·승인 → single-project apply → rescan → M3 post Gate로 E2E 검증한다.
8. `registry_current` completion claim은 actual manifest hash, current/valid snapshot, complete binding·consumer coverage, blocking drift 0건과 post `AUTO_PASS`가 모두 있을 때만 verified다.

### lifecycle·consumer 완료 gate

- 정상 전이는 `reserved→active`, `active→deprecated`, `deprecated→removed`이며 removed는 terminal이다. 즉시 `active→removed`는 허용하지 않는다.
- alias는 replacement, consumer scope와 유한한 registry-version window를 가지며 cycle·무기한 alias를 거부한다.
- 모든 current required consumer가 새 declaration version을 지원하고 old reference가 0이며 alias 기간이 끝나기 전에는 removal을 막는다.
- generated binding은 authoritative manifest와 pinned generator에서 만들고 direct edit를 차단한다. existing handwritten consumer 변경에는 typed codemod 또는 manual-review PatchSet을 사용한다.
- cross-project consumer impact와 migration order는 read-only로 표현한다. 실제 cross-repo 적용·merge·commit·push는 사용자 로드맵 9단계 전에는 지원하지 않는다.

### 6단계 인계

6단계 계약·문서 drift 검사는 current Git manifest와 ManagedRegistrySnapshot의 관계를 입력으로 받고 `RegistryConsistencyRecord`를 만든다. record는 declaration, binding/consumer, expected/observed value·type·symbol, evidence quality, compatibility status와 remediation을 담는다.

stable drift code는 최소 `in_sync`, `stale_registry_index`, `missing_binding`, `unexpected_binding`, `value_mismatch`, `type_mismatch`, `symbol_name_mismatch`, `deprecated_reference`, `removed_reference`, `alias_window_expired`, `consumer_below_minimum`, `consumer_transition_incomplete`, `generated_output_stale`, `generated_output_unowned`, `docs_schema_drift`, `namespace_collision`, `duplicate_id`, `id_reuse_attempt`을 포함한다. stale·partial·unverified 관찰은 compatibility pass가 아니다.

### 완료 조건

- 세 분류와 전이가 source·Index·CLI result에서 모호하지 않다.
- Git manifest가 정본이고 DB는 rebuildable derived Index다.
- error-code first Slice에서 message-only, new code, deprecation, alias, consumer transition, removal과 tombstone을 E2E 검증한다.
- 모든 변경이 M2→M4→M3를 거치고 DB direct write·raw literal global replacement·generated direct edit가 없다.
- consumer 최소 지원 version과 compatibility window가 removal Gate를 결정한다.
- 6단계가 stable relation·drift code와 complete evidence를 소비할 수 있다.
- Windows x64·ARM64 contract·snapshot·Patch/Gate·crash/rebuild Corpus가 통과한다.

## M6. API·계약·문서·설정·개발 환경 관리 — P-0047 bounded Slice 구현

### 현재 상태

6단계 정본은 [계약 호환성·문서·설정·개발 환경 관리](../contracts/contract-compatibility-and-environment.md)에 확정했다. P-0047의 comparator·doctor engine 위에 P-0054가 contract baseline/diff·compatibility window, docs/config trace·drift, environment snapshot·clean-room report, append-only persistence와 Controller·CLI를 연결했다. doctor는 missing probe를 pass로 만들지 않으며 download·install·system/source write를 수행하지 않는다. provider별 clean VM 생성과 외부 package 설치는 여전히 등록 adapter·별도 승인 범위다.

M6는 M3 Gate와 M5 Registry 위에 다음 흐름을 추가한다.

```text
explicit baseline + exact current source + Managed Registry
  -> kind별 ContractSurfaceSnapshot
  -> CompatibilityReport + consumer migration
  -> DocumentationSnapshot + ConfigKeyTrace
  -> read-only EnvironmentSnapshot + ProjectDoctorReport
  -> M3 Gate/EvidenceBundle
  -> DependencySecurityInputManifest
```

DB/index/report는 derived state다. public surface·baseline·docs/environment constraint는 Project Git `.star-control/contracts.toml`, managed ID·lifecycle은 M5 manifest, command/check 실행 metadata는 Catalog가 정본이다.

### 선행 gate

M6 제품 구현은 다음을 먼저 요구한다.

1. M1이 public source·CLI descriptor·Schema·config reader·generated provenance·consumer를 current coverage/limitation과 함께 관찰할 수 있어야 한다.
2. M3가 selected B04/B07 Check를 exact source·plan·config·Catalog·Tool·environment binding에서 실행하고 `HUMAN_REVIEW`를 pass와 분리해야 한다.
3. M5가 managed declaration, lifecycle, binding, consumer와 `RegistryConsistencyRecord`를 current·complete snapshot으로 제공해야 한다.
4. contract change 적용이 필요하면 M2 ChangePlan·consumer impact, M4 immutable PatchSet과 M3 pre/post Gate가 구현돼야 한다.
5. registered ToolDescriptor가 read-only probe와 mutation/network/package/system effect를 기계적으로 구분해야 한다.

선행 input이 stale·partial·unverified이면 current checkout이나 DB latest를 baseline으로 사용하거나 사람이 쓴 문서를 actual contract로 가정하지 않는다.

### contract·compatibility 수직 Slice

1. `ProjectContractManifest`, `ContractSurfaceSnapshot`, `CompatibilityReport`, `DocumentationSnapshot`, `EnvironmentSnapshot`, `ProjectDoctorReport`, `CleanRoomSpecification`, `DependencySecurityInputManifest` v1 type·Schema와 invalid/future fixture를 추가한다.
2. `.star-control/contracts.toml` loader가 API·CLI·Schema·file format·config·error code surface와 explicit baseline approval을 읽고 M5 declaration/Catalog descriptor ref를 resolve한다.
3. error code와 CLI machine output을 첫 비교 kind로 구현한다. baseline/current canonical shape와 source evidence가 없는 free-form diff는 허용하지 않는다.
4. `unchanged|compatible|additive|breaking|unknown` pure comparator를 kind별 rule/corpus로 구현한다. enum·overload·optional field 추가를 자동 additive로 처리하지 않는다.
5. declared·observed·unresolved consumer를 분리하고 `none|recommended|required|blocked_unknown` migration requirement를 만든다.
6. finite deprecation window, minimum consumer version, replacement/alias, migration guide와 removal evidence를 M5 lifecycle에 연결한다.
7. `ChangePlan.expected_public_surface_delta`에 없는 확대와 generated source/provenance drift를 별도 blocking Diagnostic으로 만든다.
8. public source·Schema/file descriptor·generated reference·docs·compatibility metadata·required migration guide를 `contract_change_group_id`로 연결하고, 같은 Project companion은 한 PatchSet에서 M3 pre/post Gate로 검증한다. cross-project 실제 적용이 필요한 removal은 9단계 전 차단한다.

### docs·config·assumption Slice

1. docs entry를 link·anchor·command·command output·snippet·config example·Schema/generated reference·assumption으로 typed snapshot한다.
2. local link/anchor는 logical path와 canonical heading으로, config example은 exact Schema/version으로 검사한다.
3. command text는 typed candidate와 exact registered descriptor를 비교한다. raw shell을 실행하거나 unregistered command를 추측하지 않는다.
4. snippet은 language·wrapper/context·expected result·disposable execution policy가 모두 선언된 경우에만 실행한다.
5. CLI 실제 parser/output/exit contract, generated Schema/reference hash와 문서 기대를 비교한다. `--help` 일치만으로 behavior를 증명하지 않는다.
6. config의 `declared` 존재, M5 lifecycle `active→deprecated→removed`와 current `documented|read|overridden`을 분리한 `ConfigKeyTrace`를 구현한다.
7. unused key는 complete semantic reader coverage에서만 확정한다. 문서 없는 environment variable은 name/owner/presence만 진단하고 값을 수집하지 않는다.
8. file·command·version·platform/environment support claim은 explicit `AssumptionSpec`과 actual observation으로 비교한다. 자연어를 임의 facts로 추출하지 않는다.

### read-only doctor·clean-room Slice

1. fake `EnvironmentProbe`와 Windows drive·UNC·junction·case·encoding·line-ending·path-length fixture부터 구현한다.
2. 실제 adapter는 exact registered read-only ToolDescriptor만 실행하고 OS/arch, filesystem capability, toolchain/package manager, manifest/lockfile/task fingerprint와 environment variable presence를 수집한다.
3. environment fingerprint에서 username, home/temp/raw absolute path, secret·environment value와 wall-clock timestamp를 제외한다.
4. doctor는 constraint별 observed/expected, evidence, completeness, limitation과 `safe_auto_fix=false` remediation을 만든다.
5. network download, package install/update/restore, lockfile/source/config/generated write와 registry/PATH/code-page/long-path/system setting 변경을 Schema·domain·adapter test 모두에서 거부한다.
6. clean-room readiness는 명세·prerequisite만 진단한다. 실제 검사는 사용자가 이미 준비한 disposable environment에서 M3 selected Check로 별도 실행하고 `dependency_download=deny`, `package_install=deny`, `system_mutation=deny`를 고정한다.
7. missing prerequisite는 자동 설치하지 않고 `not_ready|unknown`, Diagnostic과 `BLOCK|HUMAN_REVIEW`로 남긴다.
8. package manifest·lockfile·toolchain/package manager·environment provenance/coverage/freshness를 `DependencySecurityInputManifest`로 만든다. advisory·취약점·license 결과는 6단계에서 만들지 않는다.

### 실행·Gate 불변식

- M2가 `api_contract_change`와 `docs_config_environment` Profile closure를 ValidationPlan에 materialize하고 M3는 재선택하지 않는다.
- required evidence 누락·stale·partial은 `BLOCK`, complete evidence 뒤 남은 의미 판단만 CLI-only `HUMAN_REVIEW`다.
- doctor/readiness command는 target/system read-only이며 local derived evidence write 외 effect가 없다.
- `--fix`, `--install`, `--download`, `--configure-system` command surface를 만들지 않는다.
- 다른 Project consumer 변경은 사용자 9단계 전 read-only migration table만 만들고 PatchSet을 적용하지 않는다.
- 과거 Compatibility/Doctor report를 current pass로 migrate하지 않고 current source/environment에서 다시 생성한다.

### 7단계 인계

여기서 말하는 후속 **사용자 7단계 dependency·security 검사**는 제품 로드맵의 `P7` 원격 저장소 단계와 다른 번호다. 입력은 `DependencySecurityInputManifest`이며 최소 다음을 포함한다.

- exact Project/Workspace/source revision
- ecosystem별 package manifest와 lockfile logical ref·hash·relation
- toolchain/runtime/package-manager ID·version·fingerprint와 provenance
- dependency build/test/audit에 사용할 registered Task/Check/Tool ref
- OS/arch/filesystem/network/cache environment fingerprint
- source/generated/vendored/cache classification
- completeness·freshness·limitation과 missing reason

7단계는 이 입력을 preflight하고 자체 advisory/source freshness를 추가한다. 6단계는 network resolve·advisory refresh를 수행하거나 vulnerability/security pass를 미리 판정하지 않는다.

### 완료 조건

- explicit immutable baseline과 exact current에서 여섯 public surface kind를 비교한다.
- breaking/additive/compatible/unknown이 소비자·migration·window·public intent와 연결된다.
- 계약 변경과 문서·Schema·generated reference·migration guide의 동시 변경 Gate가 동작한다.
- 문서 command·link·anchor·snippet·config example의 실행 가능 기준과 safe execution 경계가 corpus로 검증된다.
- config key declaration·docs·reader·override·deprecation·removal을 값·secret 없이 추적한다.
- doctor와 clean-room readiness가 install/download/system/source mutation을 하지 않음이 domain·adapter·E2E에서 검증된다.
- Windows x64·ARM64의 path·case·encoding·line-ending·path-length fixture와 redacted fingerprint golden이 통과한다.
- AI 없이 결정 가능한 항목은 deterministic result, 의미 판단은 `HUMAN_REVIEW`로 남는다.
- 7단계 input manifest의 provenance·coverage·freshness가 current M3 EvidenceBundle에 결합된다.

## M7. 실패 재현·보안·의존성 유지보수 — P-0048 bounded Slice 구현

### 현재 상태

7단계 정본은 [실패 재현·보안·의존성 유지보수 계약](../contracts/failure-security-and-dependency-maintenance.md)에 확정했다. P-0048/P-0054의 normalized failure family·local observation·append-only persistence·Controller·CLI 위에 P-0055가 registered external process의 durable Operation과 `DevelopmentEffectReceiptV1`을 연결했다. security refresh/license input은 exact source digest의 성공 영수증을 요구하고, dependency prepare/apply 영수증은 status에 노출하되 canonical manifest·lockfile mutation은 M4 PatchSet 경로를 유지한다. missing/stale/partial/outcome-unknown 외부 결과는 clean이나 applied로 승격하지 않는다.

M7은 새 진단 subsystem이 아니라 기존 관리 흐름의 후속이다.

```text
M1 current Project/Dependency Index
  + M6 DependencySecurityInputManifest
  + common Finding/Evidence/Suppression
  + M2 ChangePlan/ValidationPlan
  -> failure reproduction / supply-chain observation / dependency candidate
  -> M4 isolated PatchSet when change is requested
  -> M3 core Gate and EvidenceBundle
  -> deterministic Maintenance Radar
```

scanner·debugger·package manager는 registered adapter이고, completion은 M3 core Gate만 소유한다. 외부 vulnerability/license/version 자료는 source와 freshness를 가진 input evidence이며 Star-Control DB가 원본 DB가 되지 않는다.

### 선행 gate

M7 제품 구현은 다음을 먼저 요구한다.

1. M1이 Project·workspace·package와 direct/transitive/internal dependency relation, manifest·lockfile·package manager를 current coverage로 관찰해야 한다.
2. M3가 common Diagnostic·Finding·EvidenceSubjectBinding, redaction, freshness/time boundary, GateDecision과 ReviewPack을 구현해야 한다.
3. M6가 exact revision·manifest/lockfile·toolchain/package manager·environment의 `DependencySecurityInputManifest`를 제공해야 한다.
4. update PatchSet 준비에는 M2 actual impact/replan과 M4 isolated preview·immutable PatchSet·rollback이 구현돼야 한다.
5. Tool Registry가 scanner·debugger·package manager의 structured args, executable identity, output Schema, network/cache/process/write effect를 표현해야 한다.
6. network read/download, dependency change, process attach·민감 dump와 PatchSet apply를 `personal_auto`에서도 prompt로 강화할 수 있어야 한다.

선행 자료가 stale·partial·unverified이면 tool별 DB나 새 진단 type으로 우회하지 않는다. read-only inspection은 unknown/limitation을 반환하고, change·security clean claim은 `BLOCK|HUMAN_REVIEW`다.

### 계약·Schema Slice

1. `FailureRecord`, `ReproductionPack`, `RegressionRecord`, `RecoveryPlan` v1 type·Schema를 추가한다.
2. `DependencySnapshot`, `SupplyChainSnapshot`, `ExternalDataSnapshot`, `DependencyUpdatePlan`, `MaintenanceRadarSnapshot` v1 type·Schema를 추가한다.
3. `ExternalDataSourceDescriptor`와 `PackageManagerAdapterDescriptor` Schema를 Catalog에 추가한다.
4. 각 계약에 minimal/full/invalid/future fixture, ID/fingerprint golden, unknown enum/version fixture를 둔다.
5. M3 `EvidenceSubjectBinding`·`EvidenceBundle`의 M7 refs와 error code·RuleRef·remediation enum migration을 함께 구현한다.
6. 기존 Finding·Diagnostic·ArtifactRef·PatchSet을 nested copy로 다시 정의하는 Schema는 거부한다.

첫 Slice는 fake adapter와 in-memory fixture만 사용하며 package 설치·network·source write를 하지 않는다.

### 실패 identity·재현 Slice

1. compile, test, runtime, tool, environment raw result를 common Diagnostic과 `FailureRecord`로 정규화한다.
2. revision을 넘어 재발을 묶는 `family_fingerprint`와 exact revision·structured args·input·seed·environment·tool을 묶는 `occurrence_fingerprint` pure function을 구현한다.
3. timestamp·PID·temp path·stack address·username·secret을 fingerprint에서 제거하고 normalization rule version을 포함한다.
4. 첫 원인을 `root_candidate`와 confidence/evidence로 표현하고 cascade edge DAG cycle을 거부한다.
5. 일반 run log와 `ReproductionPack`을 분리한다. pack은 registered invocation, exact subject, input/seed, environment, expected/actual, attempt와 curated ArtifactRef만 가진다.
6. `quarantined|unknown` artifact를 default report에서 제외하고 safe redaction이 불가능한 bytes는 `dropped_sensitive`로 기록한다.
7. bounded rerun, reducer, VCS bisect, debugger·trace를 ToolDescriptor adapter로 연결한다. 각 attempt의 raw 결과·permission·limitation을 보존한다.
8. 외부 service·device·clock·network 조건이 재현되지 않으면 `blocked_external|unverified`이며 fixed/pass로 만들지 않는다.
9. compatible before failure와 complete·stable after pass를 `RegressionRecord`로 연결하고 이후 같은 family의 호환 occurrence만 `regressed`로 판정한다.
10. rollback·roll-forward·restore를 각각 `RecoveryPlan`으로 분리하고 prerequisite·step·stop·validation·fallback과 rehearsal evidence를 요구한다.

### 보안·공급망 Slice

1. source·config·docs·log·artifact의 secret·token·PII 후보를 값·hash 없이 redacted location·kind·detector provenance로 만든다.
2. auth, session, token, permission, crypto, workflow 변경을 marker로 표시하고 exact semantic evidence가 없으면 suspected/unverified로 유지한다.
3. manifest·lockfile diff를 dependency 목적·source·requested/resolved version·direct/transitive/internal relation에 연결한다.
4. license는 SPDX expression 또는 producer-native value와 source/confidence를, vulnerability는 advisory aliases·affected range·fixed version·severity source·match method를 보존한다.
5. workflow effective permission 확대와 external action의 provider별 immutable pin 여부를 검사한다.
6. release file list·media type·size·digest·manifest를 만들고 이미 존재하는 SBOM·provenance·signature verification refs를 연결한다.
7. scanner output은 raw ArtifactRef와 common Diagnostic mapping을 모두 보존한다. scanner별 DB·Finding model을 만들지 않는다.
8. Star-Control은 vulnerability/license DB, scanner engine, package registry, SBOM signer, key store와 PKI를 만들지 않는다.

### 외부 자료 freshness Slice

1. source/provider URL, exact query/dataset, schema/API version, published/modified/fetched/observed time, content digest와 adapter identity를 `ExternalDataSnapshot`에 기록한다.
2. pagination·ecosystem·package·alias·withdrawn record coverage와 missing reason을 기록한다.
3. source descriptor의 maximum age로 `valid_until`과 `current|stale|unknown|unavailable`을 계산한다.
4. source가 published/modified time을 제공하지 않으면 최근 fetch만으로 current를 확정하지 않는다.
5. required 자료가 stale/unknown이면 warning을 만들고 clean security pass를 막는다.
6. offline snapshot은 마지막 observation과 “현재 외부 상태 미확인”을 명시한다.
7. refresh는 exact provider/query의 `network_read` approval 뒤에만 실행한다. 실패 시 이전 snapshot을 덮지 않고 stale 상태와 failed attempt를 모두 보존한다.

### dependency inventory·후보 Slice

1. M1 relation과 M6 input에서 Project→package, direct/transitive/internal dependency, manifest→lockfile→manager, dependency→affected Project/Task/Check relation을 `DependencySnapshot`으로 만든다.
2. currency `current|outdated|unknown`, vulnerability `not_affected|vulnerable|unknown`, compatibility `compatible|incompatible|unknown`, resolution `resolved|declared_only|ambiguous|unverified`을 독립 축으로 둔다.
3. current version source가 stale/unknown이면 outdated/current recommendation을 current claim으로 승격하지 않는다.
4. `UpdateCandidate`에 patch/minor/major/security/internal, 별도 SemVer delta, source change, affected Project, API/auth/workflow/migration/runtime risk를 기록한다.
5. internal dependency는 registry latest가 아니라 exact ProjectRevision과 public contract compatibility를 사용한다.
6. update dashboard는 snapshot·plan·PatchSet·approval·Gate를 결합한 projection이고 별도 mutable truth가 아니다.

### dependency PatchSet Slice

default workflow는 다음 순서를 고정한다.

1. 사용자 update 요청을 TaskSpec과 exact candidate scope로 만든다.
2. M1/M6 snapshot과 external freshness를 preflight한다.
3. M2가 affected Project·risk·ChangePlan·ValidationPlan을 만든다.
4. network refresh, download, dependency add/change가 필요한 effect마다 사용자 승인을 기다린다.
5. 승인 뒤 등록 package manager를 M4 isolated worktree에서 실행한다.
6. package manager가 만든 actual manifest·lockfile diff와 undeclared write를 수집한다.
7. actual diff로 M2 replan하고 영향·Check가 달라지면 이전 candidate/PatchSet을 supersede한다.
8. previous manifest·lockfile ArtifactRef, reverse PatchSet, rollback validation과 immutable PatchSet을 만든다.
9. `awaiting_apply_approval`에서 멈추고 dashboard에 대기 이유를 표시한다.
10. 사용자가 exact PatchSet을 승인한 뒤에만 M4 apply와 M3 post Gate를 실행한다.
11. 실패하면 이전 lockfile을 보존하고 rollback을 새 PatchApplication/RecoveryAttempt로 실행·검증한다.

lockfile은 ecosystem package manager가 소유한다. core·text codemod가 resolved entry를 직접 편집하거나 dependency closure를 역산하지 않는다. “upgrade” 승인은 새 direct dependency 추가를 포함하지 않는다.

### Maintenance Radar Slice

1. recurring/regressed failure, expired suppression, outdated/stale dependency, unresolved security Finding, flaky required test, docs/config/environment drift와 rollback 근거 부족을 input으로 읽는다.
2. RadarItem은 원본 refs를 가리키고 새 Finding·Diagnostic을 복제하지 않는다.
3. `blocking/protected → risk → freshness → regression/recurrence → evidence completeness → due/age → stable ID` tuple로 정렬한다.
4. 같은 input refs와 `evaluation_time`이면 byte-equivalent ordering을 만든다.
5. `valid_until`은 suppression expiry, external data, Project/Index와 Gate time boundary 중 가장 이른 값이다.
6. optional AI summary는 priority·Gate·approval state를 바꾸지 못한다.

### CLI·Gate 불변식

- read-only `failures inspect`, `security inspect`, `deps scan|status`, `maintenance radar`부터 구현한다.
- `refresh-plan`, `recovery-plan`, `rollback-plan`은 실행 승인이나 action token을 포함하지 않는다.
- `deps prepare`는 승인된 effect 안의 isolated preview와 PatchSet 생성까지만 수행하고 live apply path를 숨기지 않는다.
- debugger·scanner·package manager exit 0과 “no findings/update complete”를 GateDecision으로 매핑하지 않는다.
- required reproduction unverified, unsafe redaction, stale security data, manager-unowned lockfile, unapproved effect와 rollback evidence 누락은 `AUTO_PASS`할 수 없다.
- 현재 단계에는 자체 예약 refresh/update, background watcher와 browser UI를 추가하지 않는다.

### 8단계 인계

다음 사용자 8단계 [migration·performance·language/platform 정본](../contracts/migration-performance-and-platform.md)은 최소 다음을 받는다.

- family/occurrence identity와 minimized ReproductionPack
- compatible before failure·after success·later recurrence의 RegressionRecord
- exact command·input·seed·environment·toolchain·manifest·lockfile fingerprint
- rollback·roll-forward·restore가 분리된 RecoveryPlan과 rehearsal result
- dependency PatchSet, previous manifest·lockfile와 post-apply/rollback GateDecision
- SupplyChain/ExternalData freshness·coverage와 unresolved Radar limitation

migration은 rollback·restore checkpoint와 dependency/public-contract 영향을, performance는 고정 workload·seed·environment와 failure identity를 재사용한다. stale·partial M7 evidence는 8단계 baseline이 아니다.

### 완료 조건

- 실패 재현 pack과 일반 log가 contract·artifact role·retention에서 구분된다.
- secret·token·PII·민감 dump가 default report에 노출되지 않는다.
- root candidate·cascade·before/after·재발·flaky 상태를 같은 failure identity로 설명할 수 있다.
- 재현할 수 없는 외부 조건이 `unverified|blocked_external`이다.
- 외부 advisory/license/version 자료가 source·coverage·freshness·valid_until을 가진다.
- scanner·debugger·package manager가 adapter이고 M3 core Gate가 완료를 단독 판정한다.
- dependency update 기본 결과가 적용 전 immutable PatchSet과 `awaiting_apply_approval`이다.
- package manager가 lockfile을 소유하고 previous lockfile·rollback 근거가 보존된다.
- network/download/dependency 추가·변경·PatchSet apply가 사용자 승인 없이 실행되지 않는다.
- Radar가 AI 없이 결정적으로 정렬되고 stale 입력을 current로 표시하지 않는다.
- Windows x64·ARM64, path·encoding·redaction·timeout·cancel·unmapped output·crash recovery Corpus가 통과한다.
- 8단계가 재현·rollback·restore·dependency baseline을 exact evidence로 소비할 수 있다.

## M8. 데이터·설정·DB migration, 성능·build, 언어·플랫폼 migration — P-0049 + P-0054 제품 경로 구현

### 현재 상태

8단계 정본은 [Migration·성능·언어·플랫폼 계약](../contracts/migration-performance-and-platform.md)에 확정했다. P-0049/P-0054의 plan/result 계약·Schema·append-only persistence·Controller·CLI 위에 P-0055가 registered Tool Operation → exact `DevelopmentEffectReceiptV1` → migration/performance/language apply 경로를 구현했다. destructive effect는 approval·PermissionDecision·GateDecision과 exact subject fingerprint를 모두 요구하고 `failed|partial|outcome_unknown`을 성공으로 승격하지 않는다. ARM64 교차 빌드·simulation은 계속 `native_unverified`다.

M8은 세 Profile을 같은 M2→M3→M4→M6→M7 경계에 조립한다.

```text
data_config_db_migration
  = version/chain + dry-run + backup/restore + rehearsal + checkpoint/resume + invariant + rollback

performance_build
  = explicit workload + comparable cohorts + raw measurements + noise/outlier + correctness/trade-off Gate

language_platform_migration
  = behavior baseline + boundary/coexistence + consumer order + codegen/codemod + equivalence + cutover/rollback
```

0단계 Star-Control 자체 management DB migration은 `star-state` private lifecycle에 남는다. M8의 범용 Project migration은 이를 재사용 가능한 DB product로 노출하지 않고 target project의 registered migration framework를 adapter로 조정한다.

### 선행 gate

M8 제품 구현은 다음을 먼저 요구한다.

1. M1이 exact Project·Checkout·ProjectRevision·WorkspaceSnapshot, source/config/schema/migration class, toolchain·runtime·dependency와 current coverage를 제공해야 한다.
2. M2가 TaskSpec·ScopeRevision·ImpactAnalysis, project별 ChangePlan과 migration/performance/equivalence required Check를 materialize해야 한다.
3. M3가 v2 공통 Gate를 구현하고 M8 v3 phase/ref migration을 valid/old/future fixture로 제공해야 한다.
4. M4가 source·config·migration script·Schema·codegen/codemod를 single-Project immutable PatchSet으로 준비·적용·복구할 수 있어야 한다.
5. M6가 public contract baseline/current, consumer transition, config trace와 exact environment constraint를 제공해야 한다.
6. M7이 compatible ReproductionPack·RegressionRecord, RecoveryPlan, backup/restore·rollback 근거와 dependency/toolchain freshness를 제공해야 한다.
7. Tool Registry가 migration/backup/restore/benchmark/profiler/build/compiler/codegen의 structured args, effect, output Schema, executable identity와 platform capability를 표현해야 한다.
8. protected migration target copy, atomic activation/restore와 numeric metric collector를 fake port에서 먼저 검증해야 한다.

선행 evidence가 stale·partial·unverified이면 version, 성능 수치 또는 equivalence를 추측하지 않는다. read-only plan은 unknown과 blocker를 반환할 수 있지만 live execute, performance pass와 cutover permit은 만들지 않는다.

### 계약·Schema Slice

1. `ProjectMigrationManifest`, `MigrationPlan`, `MigrationCheckpoint`, `MigrationAttempt`, `MigrationValidationReport`, `RestoreVerificationRecord` v1을 추가한다.
2. `PerformanceWorkloadSpec`, `PerformanceRun`, `PerformanceComparison` v1을 추가한다.
3. `LanguageMigrationPlan`, `EquivalenceReport`, `CrossProjectMigrationHandoff` v1을 추가한다.
4. nested MigrationStep/Invariant, Measurement/Cohort, BehaviorContract/EquivalenceDimension/CoexistencePhase를 owning Schema `$defs`에 둔다.
5. 12개 계약마다 minimal/full/invalid/future fixture, canonical fingerprint golden과 redaction sample을 만든다.
6. `ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle`, `ReviewPack` v3에 M8 phase·subject role·ref를 추가하고 v2 historical evidence 자동 승격을 거부한다.
7. `star-contracts` 밖에 같은 상태·metric·equivalence wire type을 만들지 않는다.

첫 Slice는 fake target, fake metric collector와 in-memory repository만 사용하며 live DB, compiler, profiler, benchmark와 source write를 실행하지 않는다.

### 범용 Project migration Slice

1. `.star-control/migrations.toml` 목표 manifest loader와 target/version source·supported range validator를 구현한다.
2. `MigrationVersionVector` axis를 분리하고 unknown/future/corrupt/chain-gap/ambiguous 상태를 fail-closed로 만든다.
3. continuous chain resolver가 gap·cycle·duplicate·ambiguous branch를 거부하고 direct skip을 실제 ordered edge로 펼친다.
4. pure MigrationPlan builder가 한 Project·한 target만 허용하고 strategy·resource·invariant·permission·rollback을 고정한다.
5. dry-run adapter conformance에서 live target write 0, expected change/loss/unknown field/consumer/resource scope를 검사한다.
6. consistent backup created/integrity verified/restore rehearsed/restore validated를 별도 record로 구현한다.
7. protected copy에서 restore rehearsal과 migration rehearsal을 수행하고 execute와 같은 chain·tool·compatible environment를 확인한다.
8. per-step immutable attempt·receipt·checkpoint와 state projection을 구현한다.
9. crash recovery가 actual target을 checkpoint before/expected-after와 reconcile해 safe retry/already-applied/diverged/outcome unknown을 결정한다.
10. side-by-side candidate를 전체 invariant 뒤 atomic activate하고 startup/consumer post Gate를 수행한다.
11. transactional in-place는 adapter가 full transaction·rollback을 conformance로 증명한 경우에만 활성화한다.
12. destructive step, unknown field loss와 irreversible writer change는 exact user approval 없이는 실행하지 않는다.

### 성공·partial·rollback Slice

1. immutable attempt에서 `not_started|awaiting_approval|running|paused_resumable|outcome_unknown|succeeded|partially_succeeded|failed|rollback_required|rolling_back|rolled_back|rollback_failed|abandoned` projection을 만든다.
2. `partially_succeeded`가 target version·Gate를 만족하지 못하며 live partial은 `BLOCK`인지 검사한다.
3. non-replay-safe in-flight step은 commit 여부를 확정하기 전 자동 resume하지 않는다.
4. rollback·roll-forward·restore를 별도 attempt/RecoveryPlan으로 유지한다.
5. reverse tool exit 0이 아니라 before-compatible state·invariant·consumer post-rollback Gate로 `rolled_back`을 결정한다.
6. backup/candidate/손상 원본은 retention·permission 전 삭제하지 않는다.

### 성능·build Slice

1. explicit `PerformanceWorkloadSpec` loader를 구현하고 선언 없는 작업은 `not_declared|not_applicable`로 종료한다.
2. workload/input/tool/environment/config/build/cache mode와 cohort 내부 exact revision을 comparison key로 고정한다.
3. code revision 차이는 exact intended ChangeSet/PatchSet으로만 허용하고 cohort 안 여러 revision을 거부한다.
4. warmup 기본 1과 measured 기본 5·최소 3 attempt를 분리해 raw 결과를 보존한다.
5. numeric value·unit·collector가 없는 metric을 `measurement_unavailable`로 만들고 0·이전 값·추정치로 채우지 않는다.
6. first measured run 전에 noise/outlier/aggregation을 고정하고 양 cohort에 같은 rule을 적용한다.
7. excluded outlier도 보존하고 포함/제외 통계, minimum sample과 bounded additional run을 검사한다.
8. clean·incremental·cache hit·cache miss, time·memory·artifact size·throughput을 별도 comparison item으로 만든다.
9. profiler/build analyzer를 registered adapter로 연결하되 hotspot을 causal proof·GateDecision으로 만들지 않는다.
10. correctness·contract·test Gate와 memory·size·maintainability trade-off를 통과한 비교만 완료 후보로 만든다.

### 언어·플랫폼 migration Slice

1. current behavior를 public surface, I/O, state, error, serialization, concurrency, filesystem/process, security, operational과 declared performance dimension으로 고정한다.
2. existing bug/quirk의 보존 여부를 사용자 decision 없이 contract로 자동 승격하지 않는다.
3. stable boundary adapter 뒤에 old/new 구현을 두고 target source를 분리해 준비한다.
4. authoritative Schema/IDL·generator provenance와 M4 `text|syntax|symbol-aware|codegen` assurance를 보존한다.
5. shadow/differential comparison 뒤 reader-first, low-risk consumer switch, writer/source cutover 순서를 구현한다.
6. `dual_write`는 transaction·idempotency·ordering·divergence reconciliation이 증명된 별도 high-risk plan에서만 허용한다.
7. `EquivalenceReport`가 build/compile과 runtime behavior·error·state·serialization·consumer·platform·performance dimension을 분리한다.
8. compile pass만 있으면 전체는 `partial|unverified`이고 automatic equivalence를 만들지 않는다.
9. reflection·FFI·unsafe·concurrency·numeric/encoding·platform API의 미확인 의미를 `HUMAN_REVIEW`로 보낸다.
10. local Windows, authenticated remote CI, cross-compile, simulator/emulator와 native evidence를 구분한다.
11. finite compatibility window, old reference 0, complete consumer coverage와 rollback readiness 뒤에만 old path를 제거한다.
12. cutover는 exact plan·consumer set·platform evidence·M3 Gate와 사용자 approval을 요구한다.

### CLI·Gate 불변식

- `migration inspect|plan|status`, `performance plan|compare`, `language-migration plan|status` read-only command부터 구현한다.
- `migration dry-run`은 live target write가 0이어야 하며 tool 이름만으로 dry-run을 신뢰하지 않는다.
- `backup|rehearse|execute|resume|rollback`, performance run과 language cutover는 effect별 PermissionDecision을 사용한다.
- migration tool exit 0, backup file 존재, benchmark 단일 수치, profiler hotspot, compiler pass를 GateDecision으로 매핑하지 않는다.
- required invariant·measurement·equivalence가 partial/not_run/unverified/stale/flaky이면 `AUTO_PASS`할 수 없다.
- no measurement/no comparable cohort에서는 percentage·regression·pass를 출력하지 않는다.
- 실제 실행하지 않은 OS·architecture를 verified로 표시하지 않는다.
- 현재 단계에는 자체 scheduler, background benchmark/migration, browser UI와 cross-project executor를 추가하지 않는다.

### 9단계 ChangeBundle 인계

M8은 여러 Project의 provider/consumer/data-owner/tooling 관계를 read-only로 분석해 `CrossProjectMigrationHandoff`를 만든다.

- project별 MigrationPlan/LanguageMigrationPlan과 exact revision
- project별 source PatchSet·GateDecision·backup/restore·rollback
- provider-before-consumer, schema-before-codegen, reader-before-writer dependency edge
- M6 compatibility/deprecation window와 minimum accepted version
- cross-project invariant 후보, stale/partial blocker와 human question

handoff는 approval token, raw root, credential, merge/commit/push instruction과 executable script를 포함하지 않는다. 9단계가 이를 current participant에 다시 bind해 새 `ChangeBundle` revision, apply/compensation order와 combined Gate를 만들어야 한다.

### 완료 조건

- 범용 Project migration과 Star-Control 자체 management DB migration의 manifest·adapter·writer가 분리된다.
- explicit version source, unique continuous chain, unknown field policy와 strategy를 plan에서 재현할 수 있다.
- dry-run, backup integrity, restore rehearsal, migration rehearsal, execute/resume, invariant, activation과 rollback evidence가 연결된다.
- success·partial·failed·outcome unknown·rolled back·rollback failed가 서로 다르다.
- backup 존재와 실제 검증된 restore가 다른 claim이다.
- destructive migration과 irreversible cutover가 exact 사용자 승인 없이는 실행되지 않는다.
- performance는 explicit workload만 실행하고 comparable cohort·raw numeric sample·noise/outlier·correctness를 가진다.
- 측정값이 없거나 비교 불가능하면 결과를 만들지 않는다.
- clean/incremental/cache hit/miss와 time/memory/artifact-size가 별도 결과다.
- language migration의 compile/build와 기능 equivalence가 구분되고 reader/source/consumer 전환 순서·window·rollback이 있다.
- 자동 번역이 확정하지 못한 의미는 `HUMAN_REVIEW`, 실제 지원 환경 밖 결과는 `unverified`다.
- migration/profiler/build/compiler/codegen은 adapter이고 M3 core Gate가 완료를 판정한다.
- 9단계가 project별 plan·PatchSet·Gate·rollback을 `ChangeBundle`로 조정할 수 있는 handoff를 받는다.

## P6. 병렬 작업과 로컬 병합

### 현재 상태

9단계 local Git/worktree/merge 의미는 [CrossRepo ChangeBundle 정본](../contracts/cross-repo-change-bundle.md)과 [병렬 작업과 병합](../architecture/worktrees-and-merge.md)에 확정했다. P-0054는 WorktreeRecord·MergePlan v2·queue/conflict/result 계약·Schema, actual local Git worktree/create/observe/merge adapter, protected projection, exact approval과 Controller·CLI를 disposable repository Corpus에 연결했다. 사용자 checkout에는 worktree·branch·commit·merge를 실행하지 않았다.

P6는 한 repository 안의 local integration을 완성한다. 여러 Project coordination·remote write·10단계 release handoff는 P7이 소유한다.

### 선행 gate

1. M1이 ProjectId와 CheckoutId를 분리하고 Git top-level·git-dir·common-dir·object format, exact revision과 complete dirty manifest를 제공해야 한다.
2. M2가 actual ChangeSet, file/symbol/contract/generated/dependency impact와 project-local ChangePlan을 제공해야 한다.
3. M3가 `patch_pre_apply|patch_post_apply|merge` Gate와 project EvidenceBundle을 구현해야 한다.
4. M4가 한 Project·Checkout의 immutable PatchSet·PatchApplication·reverse/discard recovery와 isolated worktree port conformance를 통과해야 한다.
5. Controller·state repository가 Worktree/Merge event·idempotency·crash reconciliation을 project partition에 기록해야 한다.

선행 evidence가 stale·partial·unverified이면 worktree를 만들거나 queue에 넣지 않는다. 사용자 checkout을 clean으로 만들기 위한 stash/reset/clean을 선행 조치로 제안하지 않는다.

### 계약·Schema Slice

1. `WorktreeRecord`, `MergeQueueRecord`, `MergeConflictRecord`, `ProjectMergeResult` v1을 추가한다.
2. `MergePlan`을 project-local v2로 올리고 ProjectId·repository fingerprint·integration worktree·target base·permission·plan fingerprint를 필수화한다.
3. worktree role `participant_apply|participant_validation|project_integration|conflict_resolution`과 ownership lifecycle을 정의한다.
4. queue entry·overlap item·conflict intent는 owning Schema `$defs`에 두고 top-level wire type을 중복 생성하지 않는다.
5. minimal/full/invalid/future fixture, canonical fingerprint golden과 raw path/secret redaction sample을 만든다.
6. historical MergePlan v1을 current P6 merge-ready로 자동 승격하지 않는다.

### worktree·overlap Slice

1. fake Git port에서 common repository identity, exact base create, registration probe, retain/discard ownership을 먼저 검증한다.
2. Star-owned bounded branch/ref naming 충돌과 linked worktree를 검사한다.
3. complete staged·unstaged·untracked manifest와 preexisting ChangeSet을 worktree decision에 bind한다.
4. file·rename source/destination·range·mode·binary·submodule overlap을 구현한다.
5. semantic Index가 current일 때 symbol·contract·generated owner·manifest/lockfile overlap을 결합한다.
6. 판정은 `disjoint|ordered_overlap|conflict_possible|conflict_confirmed|unknown`이며 unknown을 병렬 허용으로 바꾸지 않는다.
7. user dirty byte를 자동 copy/replay하지 않고 dependency가 있으면 explicit materialization 또는 block으로 보낸다.
8. worktree/process/check/disk/memory/artifact/time reservation 실패에서 새 allocation을 중단하고 checkpoint한다.

### merge queue·conflict Slice

1. Git common repository마다 하나의 serial MergeQueueRecord를 만든다.
2. validated PatchApplication/local commit만 queue input으로 허용한다.
3. entry 실행 직전 target tip·predecessor·input·worktree ownership·overlap·budget·permission을 다시 확인한다.
4. target base가 움직이면 PatchSet을 자동 rebase하지 않고 `integration_stale`과 새 MergePlan을 만든다.
5. `fast_forward_only|merge_commit|squash|apply_patch` 전략을 project policy와 exact permission에 bind한다.
6. conflict에 left/right TaskSpec·ChangePlan·PatchSet intent와 ManagedDeclaration/API/Schema/config/format·compatibility ref를 기록한다.
7. 기계적으로 유일한 resolution만 자동 허용하고 나머지는 `HUMAN_REVIEW` 또는 optional Codex proposal로 보낸다.
8. 실제 resolution은 새 M4 PatchSet·approval·post/merge Gate를 요구한다.
9. integration 뒤 actual ChangeSet, parent set·commit OID와 `ProjectMergeResult`를 만든다.
10. project `merge` Gate와 complete EvidenceBundle 뒤에만 queue entry를 완료한다.

### 사용자 변경·권한 불변식

- primary checkout은 bundle apply/integration target이 아니다.
- `git reset --hard`, `git clean`, broad checkout, silent stash와 user branch force move를 사용하지 않는다.
- commit 생성은 `git_commit`, merge·local branch update는 `git_merge`, cleanup은 exact owned root `local_delete` permission을 사용한다.
- Patch apply 승인은 commit·merge 승인이 아니며 commit 승인은 remote push 승인이 아니다.
- ownership·Git registration·current manifest·evidence hold가 하나라도 불일치하면 worktree를 삭제하지 않는다.
- `partially_applied|rollback_required|held|outcome_unknown` worktree는 자동 정리하지 않는다.

### CLI-only 수직 Slice

1. read-only `worktree plan|status`, `merge plan|queue|conflicts`를 먼저 구현한다.
2. approval-gated `worktree create`, participant Patch apply·validate를 연결한다.
3. queue enqueue와 한 entry local integration을 fake Git adapter에서 E2E 검증한다.
4. conflict·base stale·crash/outcome unknown을 reconcile하고 자동 retry하지 않는다.
5. local result를 `validated_worktree|integrated_uncommitted|local_commit|local_branch_updated`로 구분한다.
6. CLI dependency graph에 Codex·App Server·remote provider client가 없어야 한다.

### 구현

- M4 single-project isolated worktree capability를 병렬 Stage와 project integration worktree로 확장
- WorktreeRecord ownership, complete dirty baseline과 사용자 변경 보존
- file·rename·range·symbol·contract·generated owner·lockfile overlap
- project/worktree/process/validation/disk/memory/time resource reservation
- repository별 serial merge queue, target base stale와 conflict intent/contract
- project-local commit·merge permission과 actual Git receipt
- project `merge` Gate·EvidenceBundle과 recovery/cleanup
- Codex 없이 동작하는 local CLI; Codex parallel은 선택 소비자

### 완료 조건

- 겹치거나 unknown인 수정의 잘못된 병렬 실행을 차단한다.
- 사용자 checkout·dirty·untracked·branch를 자동 reset·stash·삭제·강제 이동하지 않는다.
- 단계별 worktree ownership, PatchSet·actual diff·Gate·merge 결과를 추적할 수 있다.
- 같은 repository merge queue가 직렬이고 base 변화가 old PatchSet/approval을 재사용하지 않는다.
- conflict가 양쪽 의도와 관련 contract를 표시하고 해결 뒤 새 PatchSet·검사를 요구한다.
- local validated worktree·commit·branch update가 서로 다른 결과다.
- 병합 뒤 project Gate가 통과하며 실패·partial·outcome unknown은 완료가 아니다.

## P7. 여러 프로젝트와 원격 저장소

### 현재 상태

P7은 사용자 9단계의 global/project coordination과 remote 경계를 완성한다. P-0054는 [CrossRepo ChangeBundle 정본](../contracts/cross-repo-change-bundle.md)의 coordination 계약·dependency DAG·append-only persistence·Controller·CLI와 actual Git remote observation/push adapter를 구현했다. P-0055는 remote recovery provider의 durable Tool Operation을 exact `remote_recovery` 영수증으로 봉인하고, plan fingerprint·approval·permission·Gate·succeeded effect를 검증한 뒤에만 recovery apply를 기록한다. PR·remote merge는 provider별 별도 action/approval 경계를 유지한다.

### 선행 gate

1. P6 project-local worktree·MergePlan v2·queue/conflict/result와 Git fake conformance가 통과해야 한다.
2. M5/M6가 current provider·consumer·contract baseline·minimum accepted version·compatibility window를 제공해야 한다.
3. M8 `CrossProjectMigrationHandoff`가 있으면 read-only seed로만 사용하고 participant를 current revision에 다시 bind해야 한다.
4. M3가 evidence v4 `change_bundle_prepare|change_bundle_goal_exit` scope/phase와 project binding aggregation을 지원해야 한다.
5. global/project store event·projection이 participant receipt와 partial/outcome-unknown crash recovery를 지원해야 한다.
6. remote provider는 fake adapter에서 pagination·auth limitation·rate/error·idempotency·after-snapshot conformance를 통과해야 한다.

### 계약·state Slice

1. `MultiProjectGoal`, `CrossRepoChangeBundle`, `ChangeBundleParticipant`, `RemoteOperationRecord`, `ChangeBundleReleaseHandoff` v1을 추가한다.
2. `RemoteStateSnapshot`을 adapter descriptor, exact local/remote commit, PR/check/release subject, completeness·valid_until을 가진 v2로 올린다.
3. `ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle` v4에 ChangeBundle/worktree/merge/remote/release handoff ref와 subject role을 추가한다.
4. provider/consumer relation과 BundleStep graph를 분리해 provider open → consumer transition → provider close를 DAG로 만든다.
5. bundle/participant state reducer가 `prepared|awaiting_apply|partially_applied|awaiting_validation|rollback_required|held|outcome_unknown|completed`를 구분한다.
6. global bundle은 project detail을 inline하지 않고 participant DocumentRef·fingerprint·summary만 가진다.
7. `CoordinatedOperation` 완료를 Git/remote transaction 성공으로 해석하는 fixture를 invalid로 만든다.

### local CrossRepo ChangeBundle Slice

1. M8 handoff 또는 TaskSpec에서 current participant·base·dirty·PatchSet·Gate·recovery를 다시 수집한다.
2. confirmed/current provider·consumer relation만 자동 order에 사용하고 possible/unknown/cycle은 block/review한다.
3. `change_bundle_prepare` Gate 뒤 dependency-ready participant만 P6 local flow로 보낸다.
4. 병렬 project/worktree/process/check와 serial repository merge/remote write budget을 예약한다.
5. project별 apply·validation·local integration state와 EvidenceBundle을 분리한다.
6. 일부 성공이면 `partially_applied`를 유지하고 downstream dependency를 막는다.
7. `resume_remaining|roll_forward|compensate|hold|abandon_partial`을 새 current plan/effect로 구현한다.
8. 모든 required local result와 compatibility invariant 뒤 `change_bundle_goal_exit` Gate를 실행한다.

### remote snapshot·operation Slice

1. read-only `remote refresh`로 `RemoteStateSnapshot` v2를 만들고 local state와 별도 축으로 표시한다.
2. push·PR 생성/수정·remote merge·publish를 서로 다른 RemoteOperationRecord로 만든다.
3. 각 effect는 current before snapshot, exact local commit/target과 action별 사용자 ApprovalRequest를 요구한다.
4. `personal_auto`·RemoteWriteScope는 승인 후보 범위를 좁힐 뿐 current approval을 대체하지 않는다.
5. adapter response 뒤 after snapshot으로 ref·PR head/base·check subject·merge commit을 재확인한다.
6. timeout·connection loss·after snapshot missing은 `outcome_unknown`; 자동 retry하지 않는다.
7. force push·history rewrite·protected bypass·account change는 기본 deny다.
8. local commit/branch update, pushed, PR open, checks pending/failed, remote merged를 별도 상태로 유지한다.

### 10단계 인계

1. completion target이 `local_integrated|remote_merged|release_handoff_ready`이고 전체 Gate가 current인지 확인한다.
2. ProjectId별 immutable commit OID·ProjectRevision·ProjectMergeResult·Gate/EvidenceBundle을 연결한다.
3. build/package ArtifactRef가 exact project source revision에 binding됐는지 검사한다.
4. provider/consumer dependency order, compatibility window와 rollback/migration risk를 포함한다.
5. uncommitted worktree, stale Gate, 다른 commit artifact와 partial participant를 release-ready input에서 제외한다.
6. 10단계가 source·artifact·remote 상태를 다시 bind하고 별도 release Gate·publish approval을 만들게 한다.

### CLI·Codex 경계

- `change-bundle plan|import-handoff|show|preflight|status|conflicts|release-handoff plan`은 read-only다.
- local `worktree create|apply|validate|merge enqueue/run|hold|resume|recovery plan/apply`는 typed application command다.
- remote `prepare`는 ApprovalRequest만 만들고 `apply`는 exact approved action에서만 실행한다.
- CLI-only graph에는 Codex·App Server·AI client가 없다.
- Codex parallel/Conflict proposal은 같은 command를 호출하는 선택 소비자이고 state·Git·remote·Gate writer가 아니다.

### 구현

- `MultiProjectGoal`과 project relation·BundleStep DAG·compatibility window
- M8 handoff participant의 current rebind와 project별 PatchSet/MigrationPlan·Gate·recovery
- 비원자적 CrossRepoChangeBundle·participant state·partial/hold/resume/compensation
- project-local P6 flow와 전체 prepare/Goal Gate
- RemoteStateSnapshot v2와 action별 승인된 push·PR·merge operation
- local/remote state 분리와 after-snapshot reconciliation
- ProjectId·opaque binding 기반 global/project storage·evidence
- project별 source revision·artifact·Gate의 10단계 release handoff
- 인터넷 조사와 SourceRecord·freshness

### 완료 조건

- 프로젝트별 Git history·base·dirty·PatchSet·MergeResult·Gate·evidence가 분리된다.
- 여러 repository를 하나의 transaction이라고 주장하지 않는다.
- partial·rollback required·held·outcome unknown participant를 전체 성공으로 숨기지 않고 resume/compensation 상태를 보존한다.
- provider compatibility open → consumer migration → provider close와 minimum version/window를 표현한다.
- 사용자 기존 변경을 자동 reset·stash·삭제하지 않는다.
- local validated/commit/branch update와 remote push·PR/check/merge 상태가 분리된다.
- remote truth는 current adapter snapshot이며 추측하지 않는다.
- remote upload·PR·merge·publish는 `safe_default`와 `personal_auto` 모두 action별 명시적 승인 없이는 실행되지 않는다.
- stable ProjectId를 사용하고 다른 project·global evidence에 absolute path를 복제하지 않는다.
- CLI-only local ChangeBundle이 Codex 없이 동작하고 Codex parallel은 선택 소비자다.
- 10단계가 project별 immutable source revision·artifact hash·Gate를 exact 연결할 수 있다.
- 출처 없는 최신 정보와 stale remote snapshot을 current evidence로 사용하지 않는다.

## M10. CI·Release·평가·최종 제품 완성 — P-0051/P-0053 + P-0054 제품 경로 구현

### 현재 상태와 선행 gap

- 0~9단계 전용 정본·읽는 순서·roadmap·PLANS 연결은 모두 존재한다. stage별 제품 상태와 10단계가 소비하는 input은 [10단계 gap matrix](../contracts/ci-release-evaluation-and-product-completion.md#09단계-선행-정본-gap-matrix)가 소유한다.
- P-0041~P-0050은 M1~M9의 첫 bounded 제품 Slice와 required core source surface를 구현했다. P-0051은 M10 `ReleaseManifest` v2·`EvaluationRun` v2·Catalog lifecycle·build-once engine을, P-0053은 P-0026 technical manifest/installer와 local release lifecycle 경계를 구현·감사했다. P-0054는 Controller·CLI에서 build-once candidate, artifact byte 재검증, M3 evidence binding, promote/lifecycle, EvaluationRun·Catalog와 exact release approval을 연결했다. signer·clean signed installer lifecycle·GitHub publisher와 authenticated remote effect는 외부 Gate이며 publisher adapter가 없으면 apply는 fail-closed다.
- 10단계는 0~9단계 handoff를 가짜 fixture success로 대체하지 않는다. M1~M9 제품 Gate와 required core owner가 실제로 완료되기 전에는 release `ready`를 만들 수 없다.
- 10단계는 기존 제품 roadmap의 P8 evaluation과 P9 release를 한 최종 설계 단계로 묶는다. M11 Rust style Profile은 P8 뒤·P9 공개 배포 Gate 앞에 추가하며 같은 Task·source·Profile·Gate·evidence를 재사용하고 별도 Package를 만들지 않는다.

### 공통 선행 gate

| 선행 | 요구 결과 | 미충족 시 |
|---|---|---|
| P0/M1 | Controller single writer, current Project·Checkout·Index | source/artifact subject를 만들 수 없어 block |
| M2/M3 | ready ValidationPlan, current Gate·EvidenceBundle·validator guard | 검사 계층·완료 판정 block |
| M4/M5/M6 | actual ChangeSet, Registry·public contract·docs/config compatibility | package·metadata·consumer readiness block |
| M7/M8 | supply-chain freshness, recovery, migration·restore·platform evidence | license/security/install/rollback block |
| M9 | project별 current source·artifact·Gate와 remote observation handoff | multi-project release block |
| M11 | final 16번째 Rust style Profile의 toolchain/policy/coverage/Patch/Gate/CLI-only conformance | P9 final audit·public release `ready` block |
| MCP/core | required core 17/17 owning handler와 input/output Schema source readiness 구현; installed runtime current evidence는 P-0053에서 재생성 | current 설치본 audit 누락 시 release block |

### 공통 구현 불변식

- local_quick·target·full·release는 같은 Task ID, source revision, config, Catalog, logical Tool ID/version/descriptor와 resolved Profile fingerprint를 사용한다. architecture별 executable hash 차이는 declared platform artifact로만 허용한다.
- final artifact는 한 번 build·package하고 digest로 봉인한다. 검증·승격·publish를 위해 다시 build하지 않는다.
- `ready`, `approved`, `published`, `publish_outcome_unknown`, `rollback_required`를 별도 상태로 유지한다.
- publish·deploy·withdrawal·remote rollback은 action별 명시적 승인과 exact before/after remote snapshot을 요구한다.
- Rule·Check·Profile·Recipe 평가는 validator guard·Corpus·new/worsened ratchet을 약화할 수 없다.
- CLI-only가 core이고 Codex-integrated run은 같은 application command의 선택 소비자다.
- compiler, scanner, debugger, profiler, package manager, CI, installer, signer, registry와 deploy service를 재구현하지 않는다.

## P8. 비용·비교 시험·규칙 개선

### 현재 상태

- `CostRecord`, `BudgetSnapshot`, `EvaluationRun` v1 개념과 `evals/` 목표 위치는 있으나 Rule·Check·Profile·Recipe별 adjudication·comparability·lifecycle 구현은 없다.
- 10단계 target은 `EvaluationRun` v2다. 상세 field·metric·recommendation은 [10단계 평가 정본](../contracts/ci-release-evaluation-and-product-completion.md#evaluationrun-v2-평가-단위)을 따른다.
- M8 `performance_build`는 대상 Project의 declared workload 성능이고, P8은 Star-Control 자신의 routing·planning·validation·release 자동화 효용을 평가한다. 두 결과를 같은 metric으로 합치지 않는다.

### 계약·Schema Slice

1. EvaluationRun v2와 case result nested type, minimal/full/invalid/future fixture를 구현한다.
2. subject를 route/policy, Rule, Check, Profile, ChangeRecipe 중 하나의 stable ID·version·definition fingerprint로 고정한다.
3. `evaluation_context=cli_only|codex_integrated`, `mode=offline|replay|shadow`를 분리한다.
4. corpus/case version, selection, sample floor, retry·timeout·measurement protocol과 threshold를 결과 전에 고정한다.
5. actual defect·false positive·unresolved·not-applicable adjudication과 denominator를 구현한다.
6. CostRecord는 provider가 검증한 usage·금액·price source만 허용하고 missing 값은 `measurement_unavailable`이다.

### pure metric·comparability Slice

1. Rule·Check·Profile·Recipe별 duration, finding, actual defect, false positive, false negative, flaky와 suppression을 집계한다.
2. baseline relation `new|worsened`를 `existing_unchanged`보다 우선 보호하되 기존 부채를 숨기지 않는다.
3. 재계획·재실행·수동 수정·review duration, tool failure, Gate block, rollback·revert와 사용자 accept/reject를 집계한다.
4. case·source·config·Catalog·Tool·environment·protocol dimension별 comparability를 계산한다.
5. denominator 0, sample 부족, partial adjudication과 provider data 부재를 0·100%·무료로 만들지 않는다.
6. faster candidate의 false negative·critical/new finding·rollback 증가를 비용/시간 개선으로 상쇄하지 않는다.

### shadow·trial Slice

1. baseline과 candidate를 같은 recorded case에서 offline/replay로 실행한다.
2. shadow는 actual route·Check·permission·source·release를 바꾸지 않는다.
3. `keep|trial|accept|reject|needs_review` recommendation pure engine을 구현한다.
4. bounded trial은 exact Project/user scope, 기간, fallback baseline, stop trigger와 retention을 별도 승인한다.
5. candidate가 required Rule·Check, severity, ratchet, suppression expiry, evidence freshness 또는 Corpus를 약화하면 validator guard가 block한다.
6. CLI-only와 Codex-integrated cohort를 별도 실행하고 model usage·review·rework의 추가 효용을 분리한다.

### Radar·deprecation·migration Slice

1. Maintenance Radar item에 last EvaluationRun·trend·replacement·owner·deadline·next review를 연결한다.
2. Rule·Check·Profile·Recipe lifecycle `active -> deprecated -> retired`, trial `rejected` tombstone을 구현한다.
3. deprecated item에는 replacement·migration guide·compatibility window·deadline을 요구한다.
4. retired item은 새 plan에서 선택하지 않지만 historical CatalogSnapshot·baseline·suppression·Finding·partial Recipe recovery를 해석할 exact byte를 보존한다.
5. recommendation은 Catalog/config source를 자동 수정하지 않고 review된 source change·migration·M3 Gate로만 반영한다.

### P8 완료 조건

- 같은 case·subject·environment의 baseline/candidate를 비교할 수 있다.
- Rule·Check·Profile·Recipe별 actual defect·FP·flaky·suppression·rework·failure·duration이 남는다.
- 비용은 provider 검증 자료가 있을 때만 기록되고 unknown을 0으로 만들지 않는다.
- 새 code의 new/worsened 문제 방지가 기존 부채 일괄 차단보다 우선한다.
- pass율을 높이기 위한 validator·Corpus·Gate 약화가 차단된다.
- 효용 부족·오탐·불안정·비교 불가는 trial/reject/needs_review로 남는다.
- CLI-only와 Codex-integrated 효용이 별도 cohort다.
- deprecation·replacement·migration·tombstone과 Radar가 연결된다.

## M11. Rust 코드 스타일 자동 교정 Profile — P-0052 + P-0054 공통 Gate 연결 구현

### 현재 상태와 선행 blocker

- [Rust 코드 스타일 자동 교정 정본](../features/rust-code-style-auto-fix.md)과 [ADR-0011](../decisions/ADR-0011-Stable-rustfmt-Allowlisted-Clippy-Personal-Auto-경계.md)에 따라 P-0052가 Rust nested type·Schema·fixture와 fixed adapter/PatchSet core를 구현했다. P-0054는 owned isolated preview, candidate `cargo check`·test no-run, exact durable approval·`personal_auto`, M2 `rust_style_auto_fix` Profile resolution, M4 PatchSetV2, M3 pre/post Gate·Evidence와 Controller/CLI `inspect|check|prepare|auto-apply`를 하나의 persisted 흐름으로 연결했다. disposable Corpus에서 exact apply/recovery를 검증했지만 사용자 checkout apply와 background automation은 수행하지 않는다.
- root `Cargo.toml`과 `rust-toolchain.toml`은 workspace edition 2024와 exact Rust `1.96.0`, rustfmt·Clippy·rust-analyzer·rust-src component를 current source로 고정한다. installed executable identity와 component availability는 실행마다 별도 관찰하며 source pin만으로 available을 추측하지 않는다.
- 현재 repository 검증에 쓰인 `cargo fmt --all -- --check`와 `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`는 Star-Control 자체 command 기록이다. 범용 Project Profile은 mutually exclusive feature를 고려해 `--all-features`를 기본 복사하지 않는다.
- M1 current graph, M2 planning, M3 runner/evidence와 M4 isolated immutable PatchSet의 P-0042~P-0045 bounded Slice를 선행 입력으로 사용했다. 미구현 extension이나 historical evidence를 current candidate/pre/post Gate로 자동 승격하지 않는다.
- P-0052 테스트의 fake adapter는 fault injection과 state-machine 판정용이며 actual Rust 1.96 adapter smoke와 구분한다. 실제 사용자 checkout apply readiness는 exact current binding·approval·pre/post Gate 없이는 만들지 않는다.

### 제품 구조 불변식

- core 기능 수는 23개, runtime executable은 `star.exe`, `star-controller.exe`, `star-mcp.exe`, `star-updater.exe` 4개를 유지한다. updater는 update one-shot이며 상주 worker가 아니다.
- `rust_style_auto_fix`는 C01의 최종 16번째 Profile이고 pipeline은 `rust_style_v1@1`이다.
- `star-project`가 Cargo/toolchain/config discovery, `star-application`이 fixed workflow, `star-execution`이 registered typed process, `star-validation`이 Diagnostic/coverage/Gate, 기존 M4가 Patch/apply/recovery를 소유한다.
- formatting/lint/toolchain source는 Project Git config이고 exact Clippy fix allowlist·coverage·auto ceiling은 versioned Catalog/user policy다. DB는 resolved snapshot·run·Patch·Evidence derived projection이다.
- separate formatter/parser/AST/LSP/Clippy engine, `star-rust-style.exe`, raw shell pipeline, AI/OpenAI/browser/scheduler와 `cargo fix`·edition migration·nightly rustfmt를 추가하지 않는다.

### 구현 순서

아래 순서를 바꾸지 않는다. 각 항목은 type/Schema migration, positive·negative fixture, fingerprint golden과 CLI-only conformance를 함께 추가한다.

1. **read-only Cargo workspace/toolchain/config discovery** — `cargo metadata`와 filesystem observation으로 package/target/feature/required-feature, source ownership, toolchain/rustfmt/Clippy config 후보와 precedence를 수집한다. install·network·source write는 없다.
2. **`RustToolchainBinding`과 coverage contract** — cargo/rustc/rustfmt/clippy-driver identity/hash, parsing/style edition, MSRV, host/target/component completeness와 package/target/feature/triple/cfg cell Schema를 고정한다.
3. **rustfmt check와 Diagnostic normalization** — `star.rust.style.rustfmt.check`의 typed cargo fmt check, drift·exit/output·complete manifest와 공통 Diagnostic mapping을 구현한다.
4. **Clippy check와 exact lint/suggestion 수집** — `star.rust.style.clippy.check`, rustc JSON/applicability parser, project lint source와 coverage limitation을 구현한다. build script/proc macro는 trusted process다.
5. **isolated rustfmt PatchSet 수직 Slice** — exact base/current byte의 owned preview에서 cargo fmt만 실행하고 step/final diff, handwritten `.rs` modify allow-scope와 immutable PatchSet/reverse 자료를 만든다.
6. **Clippy exact allowlist fix와 hunk-to-suggestion 검증** — formatted state에서 Diagnostic을 다시 수집하고 exact lint ID + `MachineApplicable` suggestion만 선택한다. cell별 독립 fix 결과의 actual hunk가 suggestion과 byte-exact 대응하지 않으면 전체 candidate를 거부한다.
7. **`rustfmt -> clippy fix -> rustfmt` convergence/idempotence** — fixed step 순서와 adapter fingerprint를 고정하고 expected-after fresh preview에서 전체 mutation replay operation 0을 요구한다.
8. **candidate pre-validation과 affected build/test** — final fmt check, required coverage 전체 Clippy check, M2가 고른 build/test/contract Check와 preview impact reconciliation을 `rust_style_candidate` Gate로 묶는다.
9. **exact PatchSet apply와 post Gate** — exact policy approval·M3 pre Gate·single-use permit 뒤 기존 SourceMutationPort로 PatchSet byte만 적용하고 actual-after rescan·fmt/Clippy/affected Check를 실행한다.
10. **`personal_auto` policy approval** — user standing grant ceiling을 candidate에 재평가해 exact PatchSet ApprovalRequest를 policy evaluator가 해소한다. `safe_default` 사용자 승인과 state/event/evidence를 분리한다.
11. **CLI-only E2E** — `star style rust inspect|check|prepare|auto-apply`와 기존 `patch show|status|recover`를 같은 application service로 검증하고 AI/OpenAI/browser dependency 0을 증명한다.
12. **Windows x64·ARM64, multi-crate, feature/target Corpus** — path/case/process-tree, toolchain/config drift, mutually exclusive feature, inactive cfg/target와 build script/proc macro side effect를 native 환경에서 검증한다.
13. **release ToolDescriptor/Catalog/Schema와 독립 검토** — four Tool role, Profile/policy, v6 evidence, stable reason code와 Corpus manifest를 동결하고 P9 final audit input으로 승격한다.

### mutation·coverage·자동 적용 Gate

- canonical formatter는 stable cargo fmt이고 direct rustfmt는 probe/config identity에만 쓴다. style edition은 parsing edition과 별도로 resolve해 source와 fingerprint를 남긴다.
- Clippy fix built-in v1 allowlist는 exact version+Corpus 근거가 없으면 빈 list다. group/wildcard, `#[allow]` 추가·삭제와 non-`MachineApplicable` suggestion은 수정하지 않는다.
- feature set은 project Catalog가 compatible하다고 선언한 sorted matrix만 실행한다. `--all-features`·`--all-targets`를 inactive cfg·다른 triple까지 complete한 근거로 쓰지 않는다.
- cargo/rustfmt/Clippy mutator는 live checkout에서 실행하지 않는다. source 밖 owned `CARGO_TARGET_DIR`과 network deny를 사용하고 process 전·후 complete filesystem diff를 검사한다.
- `.rs` modify 이외 operation, generated/vendor/out-of-scope/public/config/lockfile write, unmatched hunk, conflicting cell과 replay diff는 PatchSet publish를 차단한다.
- `safe_default`는 prepare 뒤 exact 사용자 승인을 요구한다. `personal_auto`는 exact Project/Profile/pipeline/policy/scope/diff grant, exact PatchSet policy ApprovalDecision과 permit 전 candidate/pre `AUTO_PASS`를 요구하며, apply 뒤 post `AUTO_PASS` 전에는 성공으로 완료하지 않는다. 둘 다 apply/recovery M4 경로를 우회하지 않는다.

### 최소 Corpus와 M11 완료 조건

최소 case 전체 목록과 기대 결과는 [M11 정본의 구현 순서와 최소 Corpus](../features/rust-code-style-auto-fix.md#18-구현-순서와-최소-corpus)가 소유한다. release handoff에는 최소 다음 matrix가 current evidence로 있어야 한다.

- compliant no-op, rustfmt-only drift, allowed MachineApplicable Clippy fix와 allowlist/non-applicable skip
- feature/target/cfg partial·conflict, mutually exclusive feature, explicit/inferred/mixed style edition
- toolchain/component/target/config drift·missing, dirty overlap/unknown과 Windows x64·ARM64
- generated/vendor/build-script write, unmatched hunk, second-run diff, preview/post failure와 partial apply recovery
- `safe_default`/`personal_auto` exact approval 차이와 CLI-only AI/OpenAI dependency 0

M11 구현 완료는 모든 case가 current tool/config/source/Catalog fingerprint에서 통과하고 EvidenceBundle v6·ReviewPack이 complete일 때만 표시한다. 문서 완료, process exit 0, partial coverage, x64 native 또는 ARM64 target/cfg simulation 한쪽만의 결과는 구현 완료가 아니다. M11 implementation/conformance가 완료되기 전 P9 release `ready`를 만들지 않는다.

## P9. 공개 배포와 최종 완성 — P-0055 current host 17/17 완료, exact reseal 진행, public blocked_external

### 현재 상태

- Windows runtime·Plugin package의 P-0026 transport, architecture별 stage·Inno Setup installer·installation record·실제 경로 렌더링과 P-0051 M10 build-once/status/publish-after-state engine을 구현했다. P-0053은 x64 native build·격리 lifecycle, ARM64 Preview cross-build·PE/file manifest·installer model·fake lifecycle과 signed-stage fail-closed 재봉인을 검증했다. P-0055는 exact `ReleaseAssetBindingV1`과 GitHub CLI publisher를 Controller lifecycle에 연결해 draft-first create·no-clobber upload·publish·read-only reconcile·모든 asset download/hash를 구현했다. local basename과 다른 remote name은 bounded exact-name snapshot으로 업로드한다. unsigned Stable은 signing policy에서 계속 fail-closed다.
- final 16번째 `rust_style_auto_fix`는 P-0052/P-0054에서 fixed adapter·immutable PatchSetV2·exact approval·idempotence·multi-crate Corpus와 M2 Profile→M3 pre/post Gate→M4 apply/recovery Controller·CLI까지 구현·검증했다. 실제 사용자 checkout apply는 수행하지 않았으며 public release blocker와 혼동하지 않는다.
- Windows 11 24H2 build 26100 이상에서 x64는 Stable, ARM64는 `native_unverified` Preview다. x64 clean installer lifecycle과 ARM64 cross-build·architecture·manifest·signature·installer model·fake lifecycle이 각각 required Gate다.
- installer는 current-user Inno Setup 6 `.exe`로 확정했다. portable 공개 지원은 없다. public GitHub Releases의 Runtime·installer Authenticode는 required이며 certificate·timestamp provider가 없으면 `blocked_external`이다.
- P-0055 직전 exact candidate `7eedc7b24b6cb912afe588a6aebdab49de720c03`는 replacement installer가 root manifest-owned Runtime Generation을 승격하고 live declared/ready exact set을 검사하도록 보강했으며 clean FULL 10/10, RELEASE 14/15에서 signing/publication만 unverified였다. current host에서 이어진 종료 실패는 apply 전 receipt를 `aborted`로 종결해야 함을 드러냈고, 설치 payload가 이미 verified인 stale selector는 Codex restart 없이 복구하는 `reconcile-installed-runtime` 경계를 추가했다.
- current installed payload의 bundled `rt_c569d8e23ed61e8e`는 activation revision 5로 reconcile됐고 integration verified, Registry revision 7 declared=ready 17/17을 확인했다. current Codex MCP 17개 action은 모두 search·describe·invoke됐으며 15개 성공, ChangeBundle 없는 disposable goal의 merge/handoff 2개는 설계된 `COORDINATION_NOT_FOUND`였다. `validation.run` Operation `opn_01KY9TWQERDG6FF2WHVR389VE5`는 TARGET 8/8 PASS로 종결됐다. 현재 보강 source의 exact FULL/RELEASE·x64/ARM64 package·SBOM/audit/provenance·격리 lifecycle·GitHub draft/remote readback을 재봉인해야 하며, certificate·timestamp와 signed external evidence가 없으면 공개 Stable의 `blocked_external`을 유지한다.

### ReleaseManifest·evidence v6 Slice

1. ReleaseManifest v2와 ValidationPlan/Run/Gate/EvidenceBundle/ReviewPack v6 Schema·fixture를 구현한다. v6는 M11 Rust toolchain/policy/coverage/step binding을 포함하고 v5 release evidence compatibility를 보존한다.
2. Task·Scope·project source revision, config·Catalog·Tool·Profile·environment와 ChangeBundleReleaseHandoff를 exact bind한다.
3. `release_preflight`, `release_build`, `release_verify`, `release_package`, `release_install_lifecycle`, `release_ready`, publish preflight·verify phase를 추가한다.
4. artifact entry·included-files manifest·artifact set digest canonical hash golden을 구현한다.
5. v1 ready/published·approval·historical evidence를 v2 current 상태로 자동 승격하지 않는 migration을 구현한다.

### 검사 계층 Slice

1. `local_quick`: format·link·Schema와 bounded affected fast Check
2. `target`: M2 selected affected Check·change class Profile·regression/contract Gate
3. `full`: clean Windows의 workspace build·test·lint·docs·contract·security·Corpus
4. `release`: 봉인 artifact의 package·metadata·platform·install lifecycle·publish preflight
5. previous success reuse·invalidation과 낮은 계층이 높은 계층을 대체하지 않는 contract test
6. 모든 계층에서 Task/source/config/Catalog/Tool/Profile identity 일치 검증

### build-once·package Slice

1. fake clean builder/package port에서 architecture별 한 번 build·package한다.
2. final artifact byte·size·media type·architecture·SHA-256과 set digest를 봉인한다.
3. target/full/release verifier가 같은 byte를 다시 hash하고 release 재build를 거부한다.
4. signing이 byte를 바꾸면 signed output을 새 candidate로 만들고 모든 release Check를 다시 실행한다.
5. package dry-run에서 missing/extra, wrong architecture, case/path collision, source·legacy·user state 혼입을 검사한다.
6. root version source, changelog, package metadata, license·third-party notice와 generated provenance를 검사한다.
7. SBOM·provenance·signing을 각각 required/not-required/unavailable/incomplete/complete로 판정한다.

### clean Windows install lifecycle Slice

1. clean x64 installer에서 program·Plugin·MCP·Hook·Controller startup identity를 확인한다.
2. 관리자 executor/service 없이 current-user install과 autostart opt-out/status를 확인한다.
3. `safe_default`에서 network·remote·paid·source effect 없는 first-run smoke를 실행한다.
4. supported previous version에서 side-by-side update, config unknown preservation, store backup/migration·restore를 검증한다.
5. update failure·crash 지점별 previous artifact/state pointer rollback을 검증한다.
6. uninstall이 program payload·startup entry만 제거하고 user config·management state·project evidence를 보존하는지 검사한다.
7. ARM64 Preview는 같은 lifecycle model을 fake adapter로 통과하고 cross-build·PE architecture·file manifest·signature·installer model을 검증한다. native install·runtime 성공으로 표시하지 않는다.

### ready·approval·publish Slice

1. pure status reducer에서 `draft -> candidate -> ready -> approved -> publishing -> published`를 구현한다.
2. `blocked`, `blocked_external`, `publish_outcome_unknown`, `rollback_required`, `withdrawn`을 독립 상태로 유지한다.
3. approval을 exact manifest revision·artifact digest·channel·provider·destination·expiry·before snapshot에 bind한다.
4. GitHub Release publish·withdrawal·rollback을 서로 다른 RemoteOperationRecord와 승인으로 실행한다. 별도 server deploy는 만들지 않는다.
5. provider receipt 뒤 after RemoteStateSnapshot이 exact version·source/tag·artifact digest를 확인할 때만 published/deployed verified를 만든다.
6. timeout·partial response는 자동 retry하지 않고 read-only reconcile 뒤 새 precondition·승인을 요구한다.
7. publication observation window·withdrawal/rollback trigger를 publish 전에 versioned policy로 고정한다.

### 최종 제품 감사 Slice

1. A01~A10, B01~B09, C01, D01~D03의 의미 정본·Package owner·Writer를 전수 대조한다.
2. 최종 16개 Profile이 공통 Project/Planning/Validation/Patch/ChangeBundle/Release engine을 재사용하고 별도 제품·engine이 아닌지 확인한다. `rust_style_auto_fix`는 fixed Tool adapter와 M4 경로만 사용해야 한다.
3. management DB, Project Catalog, Code Index, common Finding projection, Managed Registry, ChangeRecipe와 CrossRepo ChangeBundle 소유권을 대조한다.
4. 계약·설정·Profile·정책·증거의 source와 derived state가 한 곳씩인지 확인한다.
5. Controller single writer, CLI-only core와 Codex 선택 소비자 경계를 의존 graph·E2E로 검사한다.
6. local AI·다른 provider·OpenAI API 직접 호출·browser UI·자체 scheduler가 없는지 검사한다.
7. compiler·scanner·debugger·profiler·package manager·CI·installer·signer·deploy service 재구현이 없는지 검사한다.
8. 현재 구현, 설계 전용과 외부 gate가 문서·CLI·release report에서 분리되는지 확인한다.
9. 문서 읽는 순서·내부 link·anchor·용어·상태·선행조건과 package ownership을 검사한다.

### P9 완료 조건

- clean Windows x64에서 build·test·package·install·safe_default first-run·update·failure rollback·repair·uninstall이 실제 통과하고, ARM64 Preview는 cross-build·PE architecture·manifest·signature·installer model·fake lifecycle을 통과한 `native_unverified`로 남는다.
- 한 번 build한 final artifact와 source revision·digest·file list가 publish byte까지 같다.
- version·changelog·metadata·license와 applicable supply-chain 자료가 complete하다.
- `ready`, `approved`, `published`가 분리되고 after-state 없이 published가 없다.
- GitHub Release publish·withdrawal·rollback이 action별 명시적 승인과 before/after snapshot을 가진다.
- 배포 실패에서 rollback과 user config·state·source·evidence 보존이 실제 검증된다.
- 포함된 모든 최종 기능과 16 Profile이 구현되고 공통 engine·Package owner·single writer 검사를 통과한다.
- M11 `rust_style_auto_fix`의 stable toolchain·exact allowlist·coverage·isolated PatchSet·idempotence·`personal_auto`·Windows x64 CLI-only·ARM64 target/cfg simulation conformance가 EvidenceBundle v6으로 complete하다.
- 제외 기능과 전문 도구 재구현이 없다.
- 전체 검사·보안·개인정보·독립 최종 검토가 current artifact subject에서 통과한다.

## 구현 순서 변경

의존관계가 유지된다면 한 단계 안의 세부 순서는 바꿀 수 있다. 다음은 바꾸지 않는다.

- 문서 확정 전에 제품 코드 작성 금지
- 상태와 계약보다 실행 자동화를 먼저 만들지 않음
- P0 application·repository 계약과 승인 없이 DB dependency·backend를 먼저 고정하지 않음
- M1 current graph·freshness와 M2 TaskSpec·ScopeRevision 없이 affected 검사나 source 변경 자동화를 먼저 만들지 않음
- M2 ValidationPlan을 P5 runner가 임의 재선택하거나 stale input에서 실행하지 않음
- M1→M2→M3 제품 gate와 immutable dry-run PatchSet보다 source rewrite·codemod apply를 먼저 만들지 않음
- M1→M4 제품 gate와 Git source manifest보다 Managed Registry DB/UI·generator·consumer rewrite를 먼저 만들지 않음
- M3·M5 current evidence와 explicit immutable baseline보다 M6 comparator·docs/config pass·doctor 완료 주장을 먼저 만들지 않음
- M1 dependency relation·M3 Gate·M6 input manifest보다 M7 scanner/debugger/package-manager adapter와 maintenance projection을 먼저 만들지 않음
- tool별 failure/security/dependency DB나 중복 Finding·Diagnostic·completion model을 만들지 않음
- external data source·coverage·freshness 없이 vulnerability/license/version clean claim을 만들지 않음
- package manager보다 core가 lockfile을 직접 편집·역산하지 않음
- network/download/dependency change 승인, actual preview replan, previous lockfile·rollback과 immutable PatchSet보다 dependency apply를 먼저 만들지 않음
- `personal_auto`를 M7 network/download/dependency change·debug attach·민감 dump·PatchSet apply의 암묵 승인으로 사용하지 않음
- deterministic Radar보다 AI risk score·priority를 먼저 만들지 않음
- M3·M4·M6·M7 current evidence와 M8 contract/state fixture보다 live migration·benchmark·language cutover adapter를 먼저 만들지 않음
- backup byte 존재를 restore 검증으로, migration partial/outcome unknown을 success로 만들지 않음
- explicit workload·comparable cohort·numeric unit/collector보다 성능 percentage·regression/pass를 먼저 만들지 않음
- behavior baseline·consumer order·platform runtime evidence보다 compile-only equivalence·language cutover를 먼저 만들지 않음
- 9단계 ChangeBundle 전에 CrossProjectMigrationHandoff를 cross-project apply 권한으로 사용하지 않음
- project doctor·clean-room readiness에 download/install/system/source mutation 자동 경로를 추가하지 않음
- external codemod를 live target checkout에서 직접 실행하거나 raw literal 전역 치환을 기본 경로로 만들지 않음
- M1→M2→M3→M4 제품 Gate, pinned stable toolchain과 complete coverage보다 M11 rustfmt/Clippy source mutation을 먼저 만들지 않음
- 범용 Rust Project에 `--all-features`를 기본 적용하거나 Clippy group/wildcard·non-MachineApplicable suggestion·`cargo fix`를 style 자동 수정으로 사용하지 않음
- `personal_auto` standing grant를 exact PatchSet ApprovalDecision·permit 전 candidate/pre `AUTO_PASS`·single-use permit·성공 전 post `AUTO_PASS`의 대체로 사용하지 않음
- M11 toolchain/config/coverage/step·Windows x64/ARM64 CLI-only conformance보다 P9 final 16 Profile audit와 release `ready`를 먼저 만들지 않음
- single-project M4·M5 apply가 안정되고 9단계 coordination 계약이 확정되기 전에 cross-project write를 만들지 않음
- 단일 실행이 안정되기 전에 병렬 병합을 기본값으로 만들지 않음
- 검사와 증거 없이 원격 자동화를 완료로 보지 않음
- 공개 안전 기본값 없이 personal_auto만 배포하지 않음
