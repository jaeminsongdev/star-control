# 핵심 관제 기능

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## A01. 목표·작업 계약

Star-Control은 자연어 요청을 바로 실행하지 않고 다음 내용을 질문으로 확정한다.

- 최종 목표와 성공한 결과의 모습
- 포함 범위와 하지 않을 일
- 대상 프로젝트와 허용·금지 경로
- 필요한 결과물과 완료 조건
- 돈이 드는 행동, 외부 변경, 파괴 행동의 승인 조건
- 사용자가 이미 정한 기술·설계·우선순위

확정된 계약은 이후 단계 계획, 권한, 검사와 최종 판정의 기준이 된다. 작업 도중 목표가 바뀌면 원래 계약을 조용히 덮지 않고 변경 이유와 새 범위를 기록한다.

CLI-only `change_planning`에서는 이 입력을 `TaskSpec`으로 구조화하며 Codex·AI가 목표·포함·제외·완료 조건을 대신 생성하지 않는다. 필수 항목이 빠졌거나 이름 selector가 모호하면 CLI가 사용자 입력을 요구하거나 fail-closed로 중단한다. 사용자가 확정한 Project·scope·Check 결정은 자동 영향 계산보다 우선한다. 상세 type은 [TaskSpec·ScopeRevision 계약](../contracts/goal-and-stage.md#taskspec-상세-계약)이 소유한다.

## A02. 단계 계획과 재계획

작업은 파일 수나 코드 줄 수가 아니라 모델·생각 깊이·실행 방식·검증 방식이 달라지는 경계로 나눈다.

- 조사, 설계, 구현, 검증, 검토, 병합처럼 성격이 다른 단계 분리
- Project scan·impact 계산 같은 deterministic local 단계와 Codex 실행 단계를 `executor_kind`로 분리
- 단계 사이 선행 조건과 결과 전달 관계 기록
- 서로 독립적인 단계의 병렬 가능성 판단
- 같은 성격의 큰 작업은 지나치게 잘게 나누지 않음
- 단계별 목표, 입력 자료, 결과, 완료 조건, 실패 처리 정의
- 예상 밖 변경, 새 위험, 검사 실패, 범위 확대 시 재계획
- 원래 계획과 실제 실행의 차이 기록

change planning 단계는 requested scope, read-only analysis scope, planned change scope와 validation scope를 같은 path set으로 합치지 않는다. 예상 밖 영향, 새 risk path, source·dirty ChangeSet 변경, stale index와 required Check 부재가 생기면 immutable ScopeRevision과 Stage revision을 만들고 이전 ImpactAnalysis·ChangePlan·ValidationPlan을 invalidated로 연결한다. planned change scope를 자동 확대하지 않으며 사용자 수정은 새 accepted revision으로 보존한다.

CLI-only change planning은 `executor_kind=deterministic_local`이므로 RouteDecision·CapabilitySnapshot·Codex plan을 만들지 않는다. 이후 source 변경·검토 단계가 Codex를 사용할 때만 별도 RouteDecision을 만든다.

## A03. 프로젝트 이해와 Context Pack

매 작업마다 전체 저장소를 다시 읽지 않도록 current checkout의 최소 사실과 현재 작업에 필요한 자료를 snapshot으로 묶는다. 상세 identity·discovery·index·freshness 계약은 [읽기 전용 Project Catalog와 Code Index](../contracts/project-catalog-and-code-index.md)가 소유한다.

- 여러 explicit root, Git·non-Git Project, nested repository, submodule, build workspace와 linked worktree 발견
- stable ProjectId와 local CheckoutId를 분리하고 같은 Project의 dirty worktree를 서로 다른 WorkspaceSnapshot으로 유지
- source, test, docs, config, schema, migration, generated, vendor, cache, output과 fixture·docs example facet 분류
- 언어, build system, package manager, toolchain, lockfile, 주요 명령 발견
- 적용 scope가 있는 AGENTS, README, 설계 문서, 정책과 프로젝트별 정본 우선순위·충돌 확인
- text search, syntax index와 available semantic index를 실제 tier·coverage·limitation과 함께 사용
- package·module·symbol·definition·reference와 project·contract·dependency graph 탐색
- config key, Schema ID, error code, 전역 상수, public surface와 hardcoding Finding 후보 탐색
- 작업 유형별로 필요한 파일, 계약, 최근 변경과 검증 명령 선택
- 각 Context 항목에 ProjectId·CheckoutId, source hash, 포함 이유, source authority, index tier, freshness와 누락 가능성 기록
- token·자료량 한도와 단계별 Context Profile
- working tree의 staged·unstaged·untracked actual byte를 HEAD·default branch보다 최신 사실로 반영

Project Catalog와 Code Index는 Git source를 대체하지 않는 derived projection이다. 최초 scan은 CLI에서 수동 실행하고 이후 Git revision·file hash 기반 incremental scan을 사용한다. semantic adapter가 없으면 syntax·text로 fallback하고 그 한계를 숨기지 않는다. 이 기능은 project source를 수정하거나 자체 scheduler·AI 호출을 요구하지 않는다.

현재 A03의 이 확장은 **1단계 목표 설계**이며 scanner·parser·DB·watcher와 CLI 제품 구현 완료를 뜻하지 않는다.

## A04. 변경 영향·위험 분석

사용자 TaskSpec과 실제 ChangeSet을 current Project Catalog·Code Index에 결합해 무엇이 영향을 받고 무엇을 어느 수준으로 검사해야 하는지 계산한다. 전체 계산·fallback·출력 계약은 [변경 계획·영향 분석 정본](../contracts/change-planning-and-impact.md)이 소유한다.

- source revision과 dirty WorkspaceSnapshot에서 add, modify, delete, rename, mode, binary, submodule, staged·unstaged·untracked 변경을 project별 ChangeSet으로 구조화
- 사용자가 지정한 path·symbol·package·contract·config·schema target과 실제 변경 entry를 별도 seed로 보존
- Registry task이면 current ManagedRegistrySnapshot의 managed declaration, candidate, local implementation constant 분류와 binding·consumer를 typed seed로 보존
- 변경 file·symbol·package·contract·config·schema·test·docs·generated source와 downstream Project 관계 수집
- source, test, dependency, workflow, schema, migration, security, release 등 변경 종류 분류
- auth·secret, public API·Schema, dependency·lockfile, validator·policy, migration, workflow·release, generated source 위험 경로 표시
- direct/transitive와 confirmed/possible을 다른 축으로 기록하고 path의 가장 약한 evidence로 confidence 계산
- 같은 literal이어도 Project·owning symbol·contract/config identity가 다르면 별도 영향으로 유지
- raw literal equality만으로 4단계 rewrite target이나 cross-file replacement set을 만들지 않고 managed declaration·contract·symbol·explicit path/range selector를 우선
- 분석 결과에 source snapshot, edge path, tier·resolution·freshness, confidence, limitation과 no-result 이유 기록
- related Check의 `not_found`와 complete applicability의 `not_applicable`을 구분
- affected package closure를 증명하지 못하면 workspace, 다시 affected Project full로 명시적으로 승격
- possible impact 하나만으로 무조건 full을 선택하지 않고 boundary 밖 closure·risk floor·Check coverage를 함께 판단
- 요청과 관계없는 변경, 과도한 diff와 숨은 변경 탐지
- previous successful revision·Check result와 current dirty delta의 exact compatibility 비교
- 관련 test, build, lint, docs, contract 검사를 Task·Check·RiskPath metadata로 선택
- 여러 Project 영향은 read-only로 계산하고 Project별 범위·근거를 분리
- 결과를 같은 TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet fingerprint의 ChangePlan·ValidationPlan으로 출력
- 4단계 Recipe preview가 실제 add·modify·delete·rename을 만들면 `recipe_preview` ChangeSet으로 영향·risk·Profile closure·affected Check를 다시 reconcile하고 새 범위·change class가 나오면 apply 대신 재계획
- Registry 변경이면 namespace·alias·lifecycle·minimum supported version과 미전환 consumer를 영향 graph에 포함하고, 다른 Project에는 read-only ChangePlan만 제안

CLI-only mode에서 A04는 Codex·AI, test runner와 source-write port를 사용하지 않는다. stale·partial graph는 confirmed impact가 아니며 safe fallback이나 human review 근거다. P-0043은 TaskSpec→ImpactAnalysis→ready ValidationPlan의 첫 bounded 제품 Slice를 구현했다. 다만 전체 language/provider graph traversal과 모든 affected selector까지 완료했다는 뜻은 아니다.

4단계는 A04를 복제하지 않는다. [안전한 Patch·Refactor·codemod 엔진](../contracts/safe-patch-and-codemod.md)이 immutable PatchSet preview를 만든 뒤 같은 A04·M2 application use case로 impact와 ValidationPlan을 reconcile한다. runner가 즉석에서 검사 범위를 넓히거나 literal 수만으로 영향이 없다고 판정하지 않는다.

## A05. Codex 능력 확인과 단계별 배정

Star-Control은 다른 AI 제공자를 선택하지 않는다. 실행자는 Codex 하나이며 다음 Codex 내부 선택만 관리한다.

- 실행 시점에 사용할 수 있는 모델, 생각 깊이, Max, 병렬 기능과 도구 능력 확인
- 단계별 필수 능력과 권한을 먼저 적용하는 hard constraint
- 작업 복잡도, 위험, 검증 가능성, 비용·한도에 따른 배정
- 설계·구현·검증·독립 검토 단계의 서로 다른 배정
- 지원되지 않거나 한도에 걸린 선택의 안전한 대체와 중단
- 적합한 실행 방식이 없을 때 억지로 배정하지 않고 질문 또는 중단
- 배정 이유와 대체 이유를 사람이 읽을 수 있게 표시
- 사용자의 수동 배정이 자동 선택보다 우선

## A06. Codex 실행 제어와 터미널 조작

Codex의 공식 통합 지점을 사용해 계획된 단계를 실제 작업으로 연결한다.

- Plugin·MCP·Hook을 통한 Star-Control 시작과 진입 검사
- Codex 제어 기능 초기화와 지원 기능 확인
- 단계별 새 작업 생성, 기존 작업 재개, 분기, 중단과 상태 조회
- 모델·생각 깊이·권한·Context Pack과 단계 지시 전달
- 단계 결과와 다음 단계의 인계 자료 수집
- 장시간 작업을 감시하고 상태를 복구하는 Windows 배경 Controller
- 목표 목록, 현재 단계, 진행 상태, 질문, 중단, 재개, 취소를 다루는 터미널 명령
- Plugin·Hook·MCP가 꺼졌거나 신뢰되지 않을 때 닫힌 상태로 중단

Controller는 계획된 작업을 이어주는 역할만 한다. 반복 시간표와 예약 실행은 Codex가 제공하는 기능을 사용한다.

## A07. 상태·Checkpoint·이어하기·자체 복구

Star-Control 자신의 장시간 작업 상태는 로컬 파일에 안전하게 보존한다.

- 목표, 단계 계획, 배정, 권한, 상태, 질문, 검사, 비용, 병합과 최종 결과 저장
- 요청, 실행 중, 검사 중, 승인 대기, 차단, 실패, 취소, 완료 상태 구분
- 원자적 저장, 경로 이탈 방지, 추가 전용 사건 기록과 artifact 참조
- 중복 실행과 같은 단계의 동시 변경을 막는 lock
- 단계·병합·외부 행동 전 Checkpoint
- 앱 종료, 대화 변경과 작업 중단 뒤 재개
- 새 대화가 바로 이어갈 수 있는 목표·진행·변경·검사·남은 일 요약
- 손상 JSON, 잘린 기록, 남은 임시 파일과 누락 artifact의 읽기 전용 검사
- 원본을 보존하는 복구본, dry-run 계획, 승인된 정리·교체와 복구 결과 기록
- 기록 보존 기간과 정리 명령

## A08. 권한·승인·격리·비밀정보 보호

권한은 사람 수가 아니라 행동의 영향으로 판단한다.

- 행동별 자동 실행, 본인 확인, 금지 설정
- 공개 배포용 `safe_default`와 개인용 `personal_auto` 분리
- 개인 기본값은 유료 사용, 외부 상태 변경, 삭제·덮어쓰기처럼 되돌리기 어려운 행동을 확인 대상으로 설정
- 프로젝트 경로, 명령 종류, network, environment, secret 접근과 실행 시간 제한
- dependency·workflow·validator·policy·release·계정·권한 변경의 별도 취급
- 승인 요청에 행동, 영향 대상, 비용, 위험, 증거와 되돌리기 방법 표시
- 계획이나 대상이 바뀐 오래된 승인을 재사용하지 않음
- raw shell 문자열보다 등록된 명령과 구조화 인자 우선
- secret·token·개인정보 후보를 Context, log, report와 외부 전달에서 가림
- 어떤 자료를 어디에 전달했는지 기록

4단계 source mutation에는 다음을 추가로 적용한다.

- `change prepare`는 target source를 바꾸지 않고 PatchSet·diff·영향·검사·rollback을 먼저 표시
- `patch apply`는 별도 command이며 exact PatchSet fingerprint, target Checkout, action set과 expiry에 승인 범위를 bind
- source·plan·Recipe·config·Catalog·Index·Tool·approval fingerprint가 바뀌면 오래된 승인을 재사용하지 않음
- external mutating codemod는 trusted ToolDescriptor와 구조화 인자로 격리 preview worktree에서만 실행하고 live target write path는 전달하지 않음
- PatchSet apply는 M3 `patch_pre_apply`와 single-use in-memory permit 없이는 source-write port를 열지 않음
- delete·mass move·dependency·validator·contract change는 operation별 ActionId와 더 강한 policy를 유지
- reverse PatchSet과 isolated worktree 폐기도 별도 current precondition·permission을 요구
- raw source secret을 Recipe input·DB·evidence에 저장하지 않고 redaction 불가 output은 자동 apply를 차단

`safe_default`와 `personal_auto`는 prompt 수가 다를 수 있지만 dry-run, pre/post Gate, 사용자 기존 변경 보존과 evidence 의무를 약화할 수 없다.

## A09. Worktree·병렬 작업·병합

혼자 여러 단계와 Project를 동시에 진행할 때 기존 변경과 project별 Git history·evidence를 잃지 않게 한다. 상세 wire/state/CLI 정본은 [9단계 CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md), project-local Git 알고리즘은 [병렬 작업과 병합](../architecture/worktrees-and-merge.md)이 소유한다.

- `MultiProjectGoal`이 stable ProjectId, provider·consumer·data owner·tooling relation과 step DAG를 고정
- `CrossRepoChangeBundle`이 project별 base revision·dirty manifest·PatchSet·Gate·rollback ref만 조정하고 source detail은 participant에 유지
- provider compatibility open → consumer transition → provider old path close 순서와 finite window 관리
- 독립 단계별 `participant_apply` worktree와 repository별 `project_integration` worktree
- file·rename·range·symbol·contract·generated owner·lockfile overlap을 prepare·dispatch·merge 직전에 재검사
- dependency가 없고 overlap이 `disjoint`이며 worktree/process/check/disk/memory/time budget 안일 때만 병렬 실행
- repository별 직렬 merge queue, base tip 변화 시 stale MergePlan과 새 preflight
- conflict에 left/right TaskSpec·ChangePlan·PatchSet intent와 관련 contract·compatibility window를 함께 표시
- 사용자 checkout을 자동 stash·reset·clean·checkout·강제 이동하지 않고 Star-Control owned worktree만 정리
- project별 post/merge Gate 뒤 전체 `change_bundle_goal_exit` Gate를 별도로 수행
- local validated worktree·commit·branch update와 remote pushed·PR/check·merged를 다른 상태축으로 유지
- 일부 participant만 성공하면 `partially_applied|rollback_required|held|outcome_unknown`을 보존하고 전체 성공으로 승격하지 않음
- remote upload·PR·merge·publish는 action별 현재 사용자 승인과 adapter after-snapshot 없이는 실행·성공 처리하지 않음

4단계 M4는 A09 전체 병렬·merge 기능보다 좁은 **single-project isolated worktree**만 사용한다.

- `WorktreeDecision=current_checkout|isolated_worktree|blocked`를 base revision·dirty manifest·overlap·transformer kind로 결정
- external codemod·formatter·generator가 source를 쓰면 preview는 항상 격리 worktree
- 사용자 dirty change가 target·range·rename·generated owner와 겹치거나 disjoint 판정이 unknown이면 current checkout apply 금지
- 격리 worktree는 exact committed base를 재현할 수 있을 때만 사용하며 필요한 dirty byte를 clean base에 조용히 복제하지 않음
- M4는 worktree 결과를 자동 merge·commit하지 않고 PatchSet과 evidence만 보존
- rollback은 reverse PatchSet 또는 Star-Control이 소유한 격리 worktree의 승인된 폐기이며 primary checkout 삭제·hard reset이 아님
- 한 PatchSet은 한 Project·한 Checkout만 수정한다. M4 자체는 cross-project writer가 아니며 9단계 ChangeBundle만 여러 project-local application을 조정한다.

9단계에서도 PatchSet 자체를 cross-project로 넓히지 않는다. ChangeBundle participant가 M4 PatchSet·PatchApplication과 M3 Gate를 project별로 참조하고, 여러 repository를 하나의 원자적 transaction처럼 표시하지 않는다. local CLI는 Codex 없이 plan·preflight·apply·validate·merge·recovery·status를 수행할 수 있고 Codex 병렬 실행은 같은 command를 소비하는 선택 경로다.

상세 M4 decision matrix와 partial apply 복구는 [4단계 엔진 계약](../contracts/safe-patch-and-codemod.md#base-revisiondirty-stateworktree-결정), 9단계 전체 state/evidence/remote/release handoff는 [CrossRepo ChangeBundle 정본](../contracts/cross-repo-change-bundle.md)이 소유한다.

## A10. 작업·도구·검증·프로필 Registry

여러 프로젝트의 반복 절차를 코드에 박아 넣지 않고 선언한다.

이 절의 Task·Tool·Check·Profile Catalog와 5단계 **Managed Registry**는 이름만 비슷하고 소유 대상이 다르다. Catalog는 실행 metadata를 소유하고, [관리형 Symbol·상수·에러 코드 Registry](../contracts/managed-symbol-registry.md)는 여러 Project·언어·문서가 공유하는 계약 값과 binding·lifecycle을 소유한다. live Tool Registry는 실행 가능한 외부 EXE 상태를 소유한다.

- 프로젝트 Task ID와 format, lint, build, test, docs, security, release 명령
- 도구의 목적, 입력·결과, side effect, 권한, timeout과 결과 parser
- 검증 Profile과 선행 관계, 실패 정책, cache 가능 여부
- 위험 경로, 계약, 허용·금지 행동, 승인 정책
- 개발 작업 Profile의 단계·Context·도구·검사·증거 기본값
- 설정 계층과 project·user·run override
- effective config 조회와 출처 설명
- 설정·템플릿·정책 version과 변경 기록

4단계 ChangeRecipe descriptor는 다음을 선언한다.

- stable Recipe ID, SemVer, definition fingerprint와 local input Schema
- target language·rewrite kind `text_replace|syntax_rewrite|symbol_aware_rewrite|codegen`
- allowed typed selector, required Index tier·coverage와 assurance limitation
- source·revision·dirty·path·tool precondition과 기계적인 expected postcondition
- built-in/private adapter 또는 trusted ToolDescriptor, typed input binding
- replay idempotence, path·resource limit, permission·risk, required validation과 reverse/discard rollback

Recipe에 raw shell·동적 script·AI prompt·SQL을 넣지 않는다. 특정 언어 codemod를 core dependency로 고정하지 않고 Tool Registry 또는 bounded adapter로 연결한다. external tool version/hash는 Recipe version과 별도로 execution evidence에 고정한다.

이 descriptor Catalog는 A03의 Project Catalog와 다르다. A03은 실제 Project·Checkout·source를 관찰한 snapshot이고, A10은 Task·Tool·Rule·Profile 선언의 정본이다. 발견한 manifest script·문서 명령은 provenance와 confidence를 가진 command 후보일 뿐 이 단계에서 실행하지 않는다. hardcoding detector threshold와 class별 제외 규칙은 versioned Rule·Policy로 선언하고 scanner code에 고정하지 않는다.

Managed Registry는 다음 불변식을 따른다.

- `managed_declaration`: 사용자가 승인한 공유 계약이며 Git manifest가 정본이다.
- `candidate`: scanner가 발견했지만 승인되지 않은 값이며 검색·분류 evidence일 뿐이다.
- `local_implementation_constant`: 지역 구현이 소유하고 Registry가 변경하지 않는 상수다.
- 지원 순서는 error code·Diagnostic ID, Schema ID·version, config key·default, CLI command·exit code, event·capability·permission ID, feature flag, 공유 format·resource ID, 사용자 승인 전역 상수다.
- stable ID, namespace, owner, type, source, language symbol binding, `active|deprecated|reserved|removed`, bounded alias, consumer minimum version과 tombstone을 선언한다.
- DB snapshot은 derived Index이며 Git manifest와 다르면 stale다. 같은 raw 값이라는 이유로 의미가 다른 declaration을 합치거나 local constant를 config로 승격하지 않는다.
- 변경은 `ManagedDeclarationChangeIntent`에서 M2 ChangePlan, M4 dry-run/PatchSet, 승인과 M3 pre/post Gate를 거친다. DB row 직접 치환과 generated output 직접 편집을 금지하며, 여러 Project 변경은 9단계 ChangeBundle 밖에서 직접 쓰지 않는다.

ChangeRecipe의 full M4 계약과 CLI-only flow는 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md), Managed Registry의 exact wire·lifecycle·consumer 계약은 [전용 정본](../contracts/managed-symbol-registry.md), 외부 process manifest 경계는 [외부 Tool Registry](../contracts/external-tool-registry.md)가 소유한다. 이후 Codex와 Managed Registry는 새 engine을 만들지 않고 같은 Recipe·ChangePlan·PatchSet application service를 호출한다.
