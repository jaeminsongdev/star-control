# 변경 계획·영향 분석·affected 검사 선택 계약

## 상태와 문서 소유권

이 문서는 Star-Control 2단계인 **변경 계획, 영향 분석과 affected 검사 선택**의 설계 정본이다. 현재 상태는 **설계 확정, 제품 구현 전**이다. 이 문서 변경만으로 planner, graph engine, check selector, test runner, CLI command, DB Schema 또는 migration이 구현됐다고 표시하지 않는다.

2단계는 사용자가 직접 입력한 목표와 범위를 1단계의 read-only Project Catalog·Code Index에 결합해 다음 산출물을 만든다.

```text
사용자 TaskSpec
  + ProjectCatalogSnapshot·CodeIndexSnapshot
  + Registry task이면 ManagedRegistrySnapshot·ManagedDeclarationChangeIntent
  + ProjectRevision·dirty WorkspaceSnapshot·ChangeSet
  + Task·Check·RiskPath descriptor CatalogSnapshot
  -> ScopeRevision
  -> ImpactAnalysis
  -> ChangePlan[] + ValidationPlan
```

계약 정의는 다음 문서가 나누어 소유한다.

| 책임 | 정본 |
|---|---|
| 사용자 입력 `TaskSpec`, `ScopeRevision`, StageSpec 연결과 사용자 우선권 | [단계 분해와 실행 계약](goal-and-stage.md) |
| Project·Revision·WorkspaceSnapshot과 ChangePlan의 공통 lifecycle | [공통 개발 관리 계약](development-management.md) |
| Project Catalog·Code Index entity·graph·tier·freshness·no-result | [Project Catalog·Code Index 계약](project-catalog-and-code-index.md) |
| ChangeSet·ImpactEdge·ValidationPlan·check 선택 결과 shape | [검사·완료·증거](validation-and-evidence.md) |
| 4단계 Recipe preview를 actual source와 분리해 재계산하는 protocol | [안전한 Patch·Refactor·codemod 엔진](safe-patch-and-codemod.md) |
| 5단계 관리 분류·Git manifest·binding·lifecycle·consumer compatibility | [관리형 Symbol Registry](managed-symbol-registry.md) |
| Task·Check·RiskPath descriptor와 planning resource 설정 | [설정과 Catalog 계약](config-and-catalog.md) |
| command ErrorEnvelope와 stable error code | [오류와 진단 계약](errors-and-diagnostics.md) |
| 2단계 계산 순서, 전파 규칙, fallback, 재계획과 `ImpactAnalysis` | 이 문서 |
| Package·module 소유권과 금지 의존 | [Repository·Package 구조](../architecture/repository-layout.md) |
| event·projection·evidence 저장 | [이벤트·상태](events-and-state.md), [상태 기록과 이어하기](../architecture/state-and-artifacts.md) |

## 선행조건과 현재 정본 대조

2단계 설계는 다음 0·1단계 계약만 입력으로 사용한다.

| 선행조건 | 현재 정본 상태 | 2단계 사용 방식 |
|---|---|---|
| ProjectId, ProjectRevisionId, WorkspaceSnapshotId, source/DB/evidence 분리, Controller 단일 Writer | P0 첫 수직 Slice 구현·검증 완료 | ID·fingerprint·repository transaction을 재사용 |
| ProjectCheckout, ProjectCatalogSnapshot, CodeIndexSnapshot | M1 설계 확정·제품 구현 전 | 구현된 것처럼 합성하지 않고 M2 구현 선행 gate로 둠 |
| package·symbol·contract·dependency graph와 tier·coverage·limitation | M1 설계 확정·제품 구현 전 | current partition만 confirmed 근거로 사용 |
| ChangeSet·ValidationPlan | 기존 설계 계약 | 2단계 target field와 선택 근거를 확장 |
| ChangePlan v1 | P0 Finding·Recipe 수직 Slice 구현 | 일반 사용자 변경 계획을 수용하는 v2 target migration 필요 |

0단계와 1단계의 의미 충돌은 없다. P0의 clean ProjectRevision과 dirty WorkspaceSnapshot 분리, M1의 actual workspace byte 우선, partition freshness와 `confirmed_empty` 구분은 그대로 유지한다. M1 migration과 graph/query surface가 구현되기 전에는 M2 제품 구현을 시작하지 않는다.

## 목표와 제외 범위

### 목표

1. AI 없이도 사용자 입력만으로 완전한 TaskSpec과 초기 ScopeRevision을 만든다.
2. current source revision과 staged·unstaged·untracked dirty byte를 포함한 ChangeSet을 영향 seed로 사용한다.
3. file, symbol, package, contract, config, schema, test, docs와 downstream project의 직접·전이 영향을 계산한다.
4. confirmed impact와 possible impact를 evidence tier·resolution·freshness로 구분한다.
5. auth·secret, public API·Schema, dependency·lockfile, validator·policy, migration, workflow·release와 generated source 위험 경로를 식별한다.
6. Project가 선언한 Task·Check metadata에서 관련 test, build, lint, docs와 contract 검사를 선택한다.
7. affected 경계를 증명할 수 없을 때 package→workspace→project full 순서로 안전하게 승격한다.
8. 검사 후보를 찾지 못한 상태와 해당 검사가 적용되지 않는다는 판정을 구분한다.
9. 사용자 수정, 예상 밖 영향과 새 위험을 immutable scope revision·replan 이력으로 남긴다.
10. 3단계 validation engine이 재해석 없이 소비할 ChangePlan과 ValidationPlan을 만든다.

### 제외 범위

- project source, Git index·branch·worktree, shared declaration과 외부 상태를 수정하지 않는다.
- cross-repo patch, codemod, source apply, worktree 생성, commit, merge, push와 release를 수행하지 않는다.
- test, build, lint, docs generator와 validator를 실행하지 않는다. 2단계는 실행 가능한 계획만 만든다.
- Codex, 다른 AI, embedding, LLM 의미 추론과 OpenAI API를 호출해 계획을 만들지 않는다.
- Project Catalog·Code Index의 stale·partial 결과를 current semantic truth로 승격하지 않는다.
- 같은 literal이라는 이유만으로 서로 다른 symbol·contract·Project ownership을 합치지 않는다.
- 모든 프로젝트의 모든 검사를 기본으로 선택하지 않는다.

이 단계의 `read_only`는 대상 project source와 Git·remote state에 effect가 없다는 뜻이다. Controller는 TaskSpec, ScopeRevision, ImpactAnalysis, ChangePlan, ValidationPlan projection과 evidence를 local management store·`.ai-runs`에 기록할 수 있다.

## 핵심 용어

| 용어 | 의미 |
|---|---|
| requested scope | 사용자가 직접 포함·제외·대상 Project로 지정한 범위 |
| analysis scope | 영향 graph를 읽기 위해 계산기가 탐색하는 read-only 범위 |
| planned change scope | 다음 write 단계가 수정 대상으로 제안할 수 있는 범위. 사용자 승인 없이 requested scope 밖으로 넓히지 않음 |
| validation scope | affected Check가 관찰해야 하는 package·workspace·project 범위 |
| seed | TaskSpec target 또는 ChangeSet entry에서 graph 탐색을 시작하는 entity |
| direct impact | seed 자체 또는 하나의 허용 relation으로 도달한 영향 |
| transitive impact | 둘 이상의 허용 relation으로 도달한 영향 |
| confirmed impact | current·required coverage에서 resolved evidence만으로 성립한 영향 |
| possible impact | 낮은 tier, inferred·ambiguous·unresolved edge, partial coverage 또는 limit 때문에 가능성만 확인된 영향 |
| affected Check | 영향·위험과 연결되어 ValidationPlan 후보가 된 CheckDescriptor |
| fallback promotion | 좁은 검사 범위를 증명할 수 없어 package→workspace→project full로 넓힌 결정 |

`analysis scope`와 `validation scope`의 자동 확대는 source write 권한 확대가 아니다. `planned change scope` 확대에는 사용자의 새 scope decision이 필요하다.

## CLI-only application 흐름

### 필수 사용자 입력

CLI-only mode는 Codex에게 질문하거나 계획 생성을 위임하지 않는다. 사용자는 JSON/TOML input 또는 구조화 CLI option으로 최소 다음을 제공한다.

Goal·Stage에 연결할 때도 이 단계의 `executor_kind`는 `deterministic_local`이다. RouteDecision, CapabilitySnapshot, model·reasoning field와 Codex thread를 만들지 않는다.

- 목표 설명 `objective`
- 하나 이상의 대상 Project와 사용할 Checkout
- 포함 범위와 제외 범위
- 예상 변경 대상. 파일·symbol·package·contract selector를 함께 쓸 수 있음
- 완료 조건과 확인 방법
- 하지 않을 행동과 허용하지 않을 경로
- 필요한 경우 기준 revision·이전 성공 결과 사용 정책
- 사용자가 강제할 검사, 생략 요청과 수동 결정 이유

필수 입력이 빠진 non-interactive command는 값을 추측하지 않고 `PLANNING_TASK_INPUT_INCOMPLETE`로 실패한다. interactive CLI는 누락 필드를 터미널에서 사용자에게 물을 수 있지만 AI 문장 생성이나 자동 목표 해석을 하지 않는다. redaction을 통과한 비민감 원문과 normalized value는 함께 보존하되 secret·credential·개인 절대 path 원문은 저장하지 않고 SecretRef·root binding·ProjectPathRef 또는 redacted category로 바꾼다.

### application use case

아래 이름은 application service의 typed use case다. 실제 CLI subcommand는 이 계약과 1:1로 연결하고 source effect를 추가하지 않는다.

| use case | 주요 입력 | 결과 | project source effect |
|---|---|---|---:|
| `task.create` | 사용자 입력, ProjectRef selector | TaskSpec revision 1 | 없음 |
| `task.revise` | 이전 TaskSpec ref, field patch, user reason | 새 TaskSpec revision | 없음 |
| `scope.resolve` | TaskSpec, current ProjectCatalogSnapshot | ScopeRevision | 없음 |
| `changes.collect` | TaskSpec, ScopeRevision, ProjectRevision, current WorkspaceSnapshot, baseline policy | project별 ChangeSet | 없음 |
| `impact.analyze` | ScopeRevision, ChangeSet, current CodeIndexSnapshot set | ImpactAnalysis | 없음 |
| `affected.select` | ImpactAnalysis, CatalogSnapshot, previous success evidence | ValidationPlan draft/ready | 없음 |
| `change.plan` | TaskSpec, ScopeRevision, ImpactAnalysis, ValidationPlan | planned-change Project별 ChangePlan draft/ready | 없음 |
| `plan.inspect` | document refs, output format | 근거·limitation·fallback view | 없음 |
| `plan.revise` | user decision 또는 invalidation reason | 새 ScopeRevision과 재계산 결과 | 없음 |

각 command는 expected document revision, current store revision과 idempotency key를 받는다. 입력 fingerprint가 달라졌으면 기존 결과를 덮어쓰지 않고 `PLANNING_INPUT_CHANGED`를 반환한다.

### 결정적 처리 순서

```text
1. TaskSpec validate
2. Project·Checkout exact resolution
3. ScopeRevision requested/analysis/change/validation scope normalize
4. current ProjectCatalog·CodeIndex freshness probe
5. ProjectRevision + dirty WorkspaceSnapshot -> ChangeSet
6. task target + actual change -> typed seed set
7. graph traversal -> ImpactEdge set
8. risk path evaluation -> RiskPathFinding set
9. Task·Check candidate resolution
10. affected scope and fallback promotion
11. previous-success comparison
12. ValidationPlan + project별 ChangePlan materialization
13. input fingerprint 재검사와 coordinated publish
```

단계 4 또는 13에서 source·config·adapter가 달라지면 중간 결과를 ready로 publish하지 않는다. 새 WorkspaceSnapshot·index가 필요한 이유를 반환하고 재계획한다.

여러 Project output은 하나의 DB transaction으로 가장하지 않는다. application은 global `CoordinatedOperation(prepared)`에 expected StoreVersionVector와 participant fingerprint를 고정하고, project store별 ChangeSet·ImpactEdge·ChangePlan과 receipt를 store-local transaction으로 commit한 뒤 모든 postcondition이 맞을 때만 global ImpactAnalysis·ValidationPlan current ref와 operation을 completed로 publish한다. crash·participant 실패 시 incomplete participant는 current plan으로 보이지 않으며 기존 current document를 유지하고 coordination recovery를 따른다.

## scope 해석과 우선순위

### selector 정규화

TaskSpec selector는 다음 tagged shape만 사용한다.

- ProjectId·CheckoutId
- ProjectPathRef 또는 project-relative glob
- package·workspace stable key
- SymbolId 또는 qualified symbol selector
- Contract·ConfigKey·SchemaId·PublicSurface entity key
- test·docs·generated source class selector

절대 path, branch 표시 이름, 자유 형식 SQL·graph query와 raw shell은 selector가 아니다. 이름 selector가 둘 이상의 entity에 대응하면 사용자가 stable key를 고르기 전까지 `ambiguous`다.

### 범위 우선순위

1. 사용자의 explicit excluded scope는 그 항목의 `applies_to` 축에서 자동 expansion보다 우선한다. 생략 기본값은 `planned_change`다.
2. 같은 revision·같은 scope 축에서 explicit include와 exclude가 겹치면 임의 우선순위를 주지 않고 `PLANNING_SCOPE_CONFLICT`로 중단한다.
3. 자동 계산은 해당 축 exclusion을 넘지 않는 범위에서 analysis scope와 validation scope를 넓힐 수 있지만 planned change scope는 넓히지 않는다.
4. 사용자가 automatic candidate를 거부하면 거부한 범위를 삭제하지 않고 user decision과 remaining risk를 기록한다.
5. 안전·permission·외부 제한은 사용자 선택으로 우회하지 않는다. 다만 이 단계는 실행하지 않으므로 실행 불가 이유를 plan readiness에 남긴다.

`ScopeRevision`의 각 항목은 `source=user|task_descriptor|impact|risk_path|fallback|user_override`, 근거 document/edge와 reason code를 가진다. 단순 path set만 저장하지 않는다.

analysis·validation exclusion이 required graph closure나 risk floor를 자르면 exclusion을 자동 해제하지 않는다. exclusion 안에서 sound한 대체 Check도 만들 수 없으면 `PLANNING_USER_DECISION_REQUIRED` 또는 blocked ValidationPlan을 반환하며, 잘린 범위를 영향 없음·검사 불필요로 표시하지 않는다.

## ChangeSet과 baseline 계산

2단계는 project·checkout별 ChangeSet 하나를 만든다. exact field는 [ChangeSet 계약](validation-and-evidence.md#changeset-계약)이 소유한다.

### current source 기준

1. Git Project의 `base_revision`은 TaskSpec이 고정한 revision 또는 planning 시작 시 local HEAD다. remote default branch는 current source가 아니다.
2. observed workspace는 staged blob이 아니라 최종 filesystem byte를 사용하고 staged·unstaged metadata를 별도 보존한다.
3. delete는 tombstone, rename은 old/new identity와 similarity 근거, untracked는 actual byte hash로 기록한다.
4. non-Git Project는 manifest fingerprint와 content hash를 사용하며 mtime만으로 equality를 증명하지 않는다.
5. source 관찰이 partial·unverified이면 ChangeSet을 empty나 complete로 만들지 않는다.

### dirty change 분리

각 entry는 `origin=preexisting|task_declared|tool_applied|unknown`과 `scope_relation=planned|necessary_expansion|unrelated|unknown`을 별도 축으로 가진다.

- planning 시작 전에 있던 dirty entry는 기본 `preexisting`이다.
- 사용자가 현재 dirty entry를 이번 작업에 포함한다고 명시하면 origin을 바꾸지 않고 `scope_relation=planned`로 연결한다.
- TaskSpec 밖 dirty entry는 되돌리거나 숨기지 않는다. `unrelated`가 증명되지 않으면 `unknown`이다.
- preexisting entry도 같은 workspace에서 Check 결과를 오염시킬 수 있으므로 validation scope 계산 입력에는 남는다.
- 여러 Project의 entry를 하나의 ChangeSet으로 합치지 않는다. ImpactAnalysis가 project별 ChangeSet reference를 묶는다.

ChangeSet이 0건일 수는 있지만 `collection_state=complete`와 비교한 scope가 있어야 한다. source를 아직 바꾸지 않은 신규 계획에서 0건은 “계획할 변경이 없음”이 아니라 “현재 관찰된 실제 변경이 없음”이다.

ChangeSet은 입력 TaskSpec·ScopeRevision을 고정한 immutable actual-comparison document다. impact·risk output을 ChangeSet에 나중에 추가하지 않는다. 예상 밖 영향이나 사용자 결정으로 ScopeRevision이 바뀌면 같은 workspace라도 새 ScopeRevision ref로 ChangeSet을 다시 수집·분류하고 이전 ChangeSet은 superseded input으로 보존한다.

### 4단계 Recipe preview 재계산

위 `planning_baseline` ChangeSet은 **현재 target checkout에서 이미 관찰된 byte**만 표현한다. 4단계가 격리 workspace 또는 메모리에서 Recipe 결과를 materialize한 뒤 만드는 `recipe_preview` ChangeSet은 미래 변경 제안이며 같은 document나 같은 origin으로 합치지 않는다.

4단계는 새 impact engine을 만들지 않고 이 문서의 계산기를 다음 입력으로 다시 호출한다.

1. 원래 accepted TaskSpec·ScopeRevision과 `planning_baseline` ChangeSet
2. exact RecipeExecution·PatchSet 후보에 연결된 `recipe_preview` ChangeSet
3. preview byte로 다시 만든 current·complete Index partition과 원래 base Index의 비교
4. 같은 CatalogSnapshot·EffectiveConfig 또는 명시적으로 replan한 새 snapshot

재계산 결과는 preview가 원래 ChangePlan의 planned scope·expected impact·risk floor 안에 있는지, format·build·test·contract Check가 추가 또는 승격되는지를 판정한다. 다음 중 하나면 기존 ChangePlan·ValidationPlan을 그대로 apply에 사용하지 않는다.

- preview에 planned change scope 밖 add·modify·delete·rename이 있음
- selector resolution, generated ownership 또는 direct/transitive impact가 원래 예상과 다름
- required Check가 추가되거나 fallback floor가 넓어짐
- preview Index가 partial·stale·unverified이거나 external mutator 결과를 완전히 열거하지 못함
- Recipe, Tool, config, Catalog, base revision 또는 dirty manifest fingerprint가 바뀜

accepted 범위 안에서 impact·validation scope만 sound하게 넓어지는 경우에도 새 `ImpactAnalysis`와 `ValidationPlan` revision을 만든다. planned change scope 확대가 필요하면 proposed `ScopeRevision`을 만들고 사용자 수락 전에는 PatchSet을 `ready`로 만들지 않는다. preview가 원래 계획과 exact하게 일치해도 `planning_baseline`을 덮어쓰지 않으며, pre-apply Gate는 두 ChangeSet과 reconciliation fingerprint를 함께 고정한다.

## impact seed 생성

seed는 다음 집합의 합집합이다.

1. `intent_seeds`: 사용자가 TaskSpec에서 지정한 path·package·symbol·contract·config·schema target
2. `observed_seeds`: ChangeSet의 planned·necessary_expansion·unknown entry와 그 changed range에 대응하는 entity
3. `registry_seeds`: Registry task의 ManagedDeclaration ID·namespace·binding·consumer와 typed desired lifecycle/value. current ManagedRegistrySnapshot과 source manifest hash가 필수다.

Registry seed는 DB row·raw literal·symbol display name에서 합성하지 않는다. `ManagedDeclarationChangeIntent`가 대상 ID, expected item/source fingerprint, change kind, typed desired state와 requester decision을 제공해야 한다. candidate promotion은 사용자의 ownership 승인까지 `proposed`이며 local implementation constant는 명시적 reclassification 결정 없이는 Registry change seed가 아니다.

`unrelated`가 current complete graph와 사용자 결정으로 확인된 preexisting entry는 task impact 전파 seed에서 제외할 수 있지만 validation contamination 근거에는 남긴다. `unknown`을 unrelated로 간주하지 않는다.

path seed는 SourceEntry를 거쳐 package·module·symbol·contract ownership으로 해석한다. delete 전 symbol은 base revision index, add·modify 후 symbol은 current WorkspaceSnapshot index를 사용한다. 두 snapshot 중 하나만 있으면 가능한 방향만 계산하고 limitation을 남긴다.

TaskSpec이 아직 존재하지 않는 add path를 명시하면 `prospective_source` seed를 만든다. ProjectPathRef와 exact owning package/workspace를 manifest로 확인할 수 있으면 그 containment만 confirmed user intent로 기록하고 reference·consumer 영향은 possible이다. owner도 확인할 수 없으면 `unresolved`이며 최소 project-level validation fallback을 검토한다. 존재하지 않는 symbol·contract 이름을 current definition처럼 합성하지 않는다.

seed mapping 결과는 `resolved`, `ambiguous`, `unresolved`, `excluded`, `stale` 중 하나다. mapping 0건은 empty impact가 아니라 `IMPACT_NO_SEED_MAPPING` limitation이다.

## 영향 graph 계산

### node와 relation

사용 가능한 node는 M1의 Project, Checkout, Workspace, Source, Package, Module, Symbol, Definition, Contract, ConfigKey, SchemaId, ErrorCode, Constant, PublicSurface, ExternalDependency와 unresolved target이다. test와 docs는 별도 의미 type을 새로 만들지 않고 Source class/facet과 `tests`·`documents` edge로 표현한다.

기본 전파 relation은 다음과 같다.

| relation | 전파 방향 | 대표 영향 | confirmed 최소 근거 |
|---|---|---|---|
| `contains`, `member_of` | child→owner와 owner→member | file→package, package→workspace | current inventory·manifest의 exact membership |
| `declares`, `defines` | source↔entity | file change→symbol·Schema | current syntax/declared exact range |
| `references`, `calls`, `imports` | target→consumer | API/symbol change→caller/importer | resolved semantic 또는 adapter가 exact라고 선언한 edge |
| `depends_on` | provider→consumer | package/dependency change→dependent package | declared manifest 또는 resolved build graph |
| `implements`, `exposes` | contract→implementation·consumer | public contract change | declared/semantic resolved edge |
| `managed_by`, `binds`, `consumes` | declaration↔definition·consumer | stable ID·binding·consumer change | current Registry manifest와 current M1 binding observation |
| `aliases`, `replaces` | old→new declaration·consumer | deprecation·호환 기간·migration | valid bounded AliasRecord와 lifecycle |
| `tests` | subject→test | affected source→관련 test | exact declared mapping 또는 resolved reference |
| `documents` | subject→docs | public surface→관련 docs | declared mapping 또는 resolved reference |
| `generates`, `generated_from` | input↔output | Schema/generator↔generated source | generator manifest·provenance |
| `migrates` | versioned subject→migration | Schema/config change→migration | declared migration target·version |
| `reads`, `writes` | config/schema→consumer | config key change→reader/writer | semantic resolved edge |
| `nested_project`, `submodule`, `workspace_member` | owner→member | root·workspace change | current ProjectCatalog relation |

text-only 동일 literal, filename 유사성, 같은 표시 이름과 remote URL 유사성은 전파 relation이 아니다. 이런 evidence는 possible candidate를 설명할 수 있지만 confirmed path를 만들지 않는다.

### direct와 transitive

- seed 자체는 `distance=0`, `impact_kind=direct`다.
- 하나의 허용 relation으로 도달한 node는 `distance=1`, `direct`다.
- 둘 이상의 relation으로 도달한 node는 `distance>=2`, `transitive`다.
- containment를 숨은 0-hop으로 압축하지 않는다. 사람이 같은 결과를 재현할 수 있도록 모든 edge를 path에 남긴다.
- 같은 target에 여러 path가 있으면 가장 짧은 path를 대표로 쓰되 certainty가 더 높은 path와 risk path를 모두 evidence set에 보존한다.

### confirmed와 possible

path 전체가 다음 조건을 만족할 때만 confirmed다.

1. 모든 input ProjectCatalog·CodeIndex partition이 current다.
2. relation에 필요한 scope·tier coverage가 complete다.
3. 모든 edge resolution이 `resolved`이고 relation별 최소 근거를 충족한다.
4. source·config·adapter fingerprint가 analysis 종료 probe에서도 같다.
5. traversal limit이나 excluded subtree가 해당 path의 closure를 자르지 않았다.

하나라도 만족하지 않으면 path는 possible이다. possible을 confirmed로 올리는 numeric threshold는 두지 않는다.

`confidence`는 `high|medium|low`이며 path의 가장 약한 evidence로 결정한다.

| confidence | 기준 |
|---|---|
| `high` | declared/semantic exact, resolved, current·complete, limitation 없음 |
| `medium` | current·complete syntax/inferred relation이며 adapter가 target resolution 범위를 명시 |
| `low` | text candidate, ambiguous·unresolved, fallback tier, partial·unverified scope 또는 resource limit |

여러 low edge의 수를 더해 medium/high로 올리지 않는다. `confirmed`는 보통 high이고 relation policy가 exact syntax를 허용한 경우에만 medium일 수 있다.

### 같은 literal과 소유 계약 분리

literal occurrence identity는 최소 ProjectId, CanonicalSourceId, owning SymbolId 또는 lexical scope, contract/config entity key, source range와 content hash를 포함한다.

- 같은 문자열이어도 owning SymbolId·Contract key·ProjectId가 다르면 별도 seed·node다.
- owner를 찾지 못한 text occurrence는 `unowned_literal` possible candidate다.
- 두 literal을 같은 contract로 합치려면 declared contract entity 또는 resolved implements/exposes/config relation이 있어야 한다.
- Managed Registry에서는 추가로 같은 current `managed_declaration_id` 또는 명시적인 alias/replacement 관계가 있어야 한다. raw value equality나 DB candidate group은 ownership 근거가 아니다.
- literal equality 자체로 downstream project edge를 만들지 않는다.
- secret 후보 literal은 원문이나 그 hash를 저장하지 않고 redacted category·ownership evidence만 기록한다.

Registry classification·identity·same-value 분리의 exact 규칙은 [Managed Registry 정본](managed-symbol-registry.md#세-관리-분류)이 소유한다.

### traversal과 resource limit

1. node key와 relation key를 byte-order로 정렬한 deterministic queue를 사용한다.
2. seed별 visited set은 `(node_key, relation_policy_version, scope_revision)`으로 구분한다.
3. cycle은 같은 path에서 다시 방문하지 않고 cycle edge evidence만 남긴다.
4. EffectiveConfig의 max depth·node·edge·downstream project limit을 적용한다.
5. limit 도달 시 결과를 잘라 complete로 만들지 않고 frontier, skipped count와 `IMPACT_GRAPH_LIMIT`을 기록한다.
6. 잘린 frontier가 package closure 안이면 workspace, workspace closure도 불명확하면 project full validation fallback 후보를 만든다.

`ImpactAnalysis.calculation_fingerprint`는 TaskSpec ref, ScopeRevision hash, project별 ChangeSet fingerprint, Catalog·Index snapshot ref, relation policy·RiskPath descriptor·EffectiveConfig fingerprint와 정렬된 seed/edge/result fingerprint를 JCS로 hash한다. timestamp, 표시 이름, cursor와 render option은 제외한다.

## 여러 프로젝트의 read-only 영향 계산

여러 Project는 하나의 source tree처럼 합치지 않는다.

1. ProjectCatalogSnapshot의 Project relation과 global cross-project exported entity edge로 후보 provider/consumer를 찾는다.
2. 각 Project마다 명시적 CheckoutId, ProjectRevisionId, WorkspaceSnapshotId와 CodeIndexSnapshotId를 고정한다.
3. cross-project confirmed edge는 provider·consumer 양쪽 snapshot이 current이고 exported entity key가 exact match할 때만 만든다.
4. consumer index가 missing·stale·partial이면 consumer Project는 possible downstream이며 확인하지 못한 boundary를 limitation으로 남긴다.
5. fallback은 영향받은 Project별 package→workspace→project full이다. 한 Project의 불확실성 때문에 등록된 모든 Project를 자동 full 대상으로 만들지 않는다.
6. cross-project closure 자체를 계산할 수 없으면 사용자가 지정한 Project set 안에서만 conservative validation scope를 제안하고, 범위 밖 가능성은 `IMPACT_DOWNSTREAM_UNVERIFIED`로 보고한다.

2단계는 다른 Project의 source를 수정하거나 merge 순서를 만들지 않는다. provider 우선 수정·cross-repo PatchSet·merge는 이후 단계의 별도 ScopeRevision과 PermissionPlan 대상이다.

## 위험 경로 계산

RiskPathDescriptor의 exact field와 built-in ID는 [설정과 Catalog 계약](config-and-catalog.md#riskpathdescriptor)이 소유한다. 2단계는 descriptor version을 CatalogSnapshot에 고정하고 다음 의미로 평가한다.

| 위험 경로 | seed·edge 예 | 최소 affected 검증 방향 | 기본 범위 floor |
|---|---|---|---|
| auth·secret | auth policy, credential access, permission config, secret-sensitive source | auth negative/positive test, secret exposure, policy·redaction | owning workspace; global policy면 project full |
| public API·Schema | PublicSurface, Contract, SchemaId, exported symbol | compatibility diff, contract test, consumer compile/test, docs | provider workspace + confirmed consumer scope |
| dependency·lockfile | manifest, lockfile, ExternalDependency, build graph | lock consistency, build, dependency policy·security/license | owning workspace; root lockfile면 project full |
| validator·policy | Check/Rule implementation, gate policy, test fixture | validator self-test, negative fixture, guard/corpus | owning workspace; shared gate면 project full |
| migration | schema/config/store version, migration, backup/rollback contract | forward/rollback rehearsal, invariant, compatibility | project full unless isolated migration domain proven |
| workflow·release | CI workflow, packaging, release config·script | syntax/lint, package dry-run, release readiness | project full for release path |
| generated source | generator input, generated output, provenance edge | regeneration consistency, generated diff, consumer build/test | generator ownership workspace |

RiskPathFinding은 risk ID/version, matched seed·ImpactEdge path, certainty, severity floor, affected Project/package/workspace, required check family, fallback floor, limitation과 evidence ref를 가진다. path match가 possible이면 위험을 확정 위반으로 표시하지 않지만 required check 후보와 fallback 근거로 사용할 수 있다.

위험 경로가 없다는 판정은 descriptor set이 current CatalogSnapshot에서 완전하고 모든 required selector input이 current·complete일 때만 `confirmed_empty`다. metadata 부재나 excluded source는 “위험 없음”이 아니다.

## affected 검사 선택

### 후보 발견 순서

검사 후보는 다음 순서로 합집합을 만들며 앞 순서가 뒤 후보를 조용히 삭제하지 않는다.

1. TaskSpec에서 사용자가 강제한 Check ID·family
2. RiskPathFinding이 요구한 Check family
3. impacted entity·source class와 exact match하는 CheckDescriptor
4. TaskDescriptor, `change_planning` planning Profile과 이번 Task에 resolved된 downstream validation Profile의 required/default Check
5. package·workspace manifest 또는 canonical docs에서 발견된 뒤 별도 등록·trust 절차로 CatalogSnapshot에 이미 승격된 Project Check
6. previous successful ValidationPlan의 compatible Check

M2는 candidate를 모으기 전에 Profile closure를 확정한다. 명시·기본 `change_planning` Profile 외에 source 변경을 적용하거나 자동 완료할 계획이면 `ai_development_validation`을 mandatory validation Profile로 포함한다. test·fixture·snapshot·test harness 변화 또는 correctness risk는 `test_correctness`, package·public contract·Schema·generated·architecture policy 변화 또는 architecture risk는 `architecture_quality`를 활성화한다. trigger는 TaskSpec intended change, planning-baseline ChangeSet, ImpactAnalysis risk/path와 Catalog metadata로만 계산하며 자연어 추측이나 M3 실행 시 재선택에 맡기지 않는다.

resolved Profile ID/version/content hash, parent closure, activation reason·evidence와 병합한 required Rule·Check·evidence/policy floor는 ValidationPlan `profile_refs`·`profile_resolution_fingerprint`에 저장한다. trigger input이 unknown이면 해당 family를 조용히 빼지 않고 candidate unknown과 conservative fallback 또는 human review를 남긴다. 적용 뒤 actual ChangeSet이 다른 change class를 드러내면 기존 plan을 실행 중 확장하지 않고 `VALIDATION_PROFILE_CLOSURE_STALE`로 invalidated한 뒤 M2를 다시 계산한다.

manifest script나 문서에 command text가 있다는 사실만으로 실행 가능한 CheckDescriptor를 합성하지 않는다. trusted ToolDescriptor, typed argument binding과 result parser가 없으면 `unresolved_not_found` 또는 `blocked_untrusted`다.

candidate는 Catalog ID byte-order로 정렬하고 indexed applicability prefilter 뒤 `change_planning.max_check_candidates`를 적용한다. 상한을 넘으면 나머지를 optional로 버리지 않고 frontier count·selector를 `AFFECTED_CHECK_CANDIDATE_LIMIT`로 기록한다. required family closure를 끝내지 못했으면 ValidationPlan은 blocked/human review다.

### 적용 판정

각 Check family는 반드시 다음 결과 중 하나를 가진다.

| outcome | 의미 | ValidationPlan 처리 |
|---|---|---|
| `selected_required` | 영향·위험·사용자 조건상 필수 | `required_checks` |
| `selected_optional` | 추가 confidence·진단용 | `optional_checks` |
| `omitted_not_applicable` | descriptor 조건을 complete metadata로 평가해 적용 대상이 아님 | `omitted_checks` + evidence |
| `unresolved_not_found` | 필요한 family지만 실행 가능한 descriptor를 찾지 못함 | `unresolved_checks`, readiness block/review |
| `blocked_unavailable` | descriptor는 있으나 tool·platform·permission precondition 불충족 | `unresolved_checks` |
| `user_waived` | 사용자가 계산 결과보다 생략을 선택함 | candidate의 required origin을 보존한 OmittedCheck·waiver·remaining risk·human review |

`not_required`는 명시적 applicability expression이 false이고 그 입력 coverage가 complete일 때만 사용한다. 관련 test를 검색했지만 mapping이나 descriptor가 없으면 `not_found`다. test file 0건, query `[]`, 이전 성공 결과와 작은 diff는 그 자체로 `not_required` 근거가 아니다.

### 가장 좁은 sound scope

selector는 비용이 아니라 soundness를 먼저 확인한 뒤 가장 좁은 범위를 고른다.

1. CheckDescriptor가 package-scoped invocation과 coverage contract를 제공한다.
2. affected package set과 dependency closure가 current·complete graph로 확정된다.
3. shared workspace config, root manifest·lockfile, global policy·Schema, generated owner와 cross-package ambiguous edge가 없다.
4. previous success comparison과 invalidation rule이 현재 ChangeSet을 모두 포함한다.

네 조건을 만족하면 package scope를 선택한다. 하나라도 불명확하면 아래 promotion을 적용한다.

### package→workspace→project full fallback

| promotion | trigger | 결과 |
|---|---|---|
| package→workspace | package ownership ambiguous, cross-package edge, workspace manifest/toolchain/config 변경, package-scoped command 없음 | 해당 workspace Check |
| workspace→project full | workspace boundary ambiguous, root lockfile·policy·workflow·release·migration, graph/frontier limit, required partition stale/partial | 해당 Project의 full Check |
| affected projects 확대 | current cross-project provider→consumer edge 또는 사용자가 Project 추가 | 새 Project별 package/workspace/full 계산 |
| block/review | full Check descriptor도 없음, Project root unavailable, source changed during plan | unresolved reason과 수동 결정 요구 |

`project full`은 등록된 모든 Project의 전체 검사를 뜻하지 않는다. 현재 ImpactAnalysis가 affected 또는 user-targeted로 식별한 Project 하나의 full Check다.

possible impact 하나만으로 무조건 full로 승격하지 않는다. 다음을 함께 평가한다.

- possible frontier가 현재 package/workspace 밖으로 나가는가
- 빠진 relation이 해당 Check family의 soundness에 필요한가
- risk path가 명시한 범위 floor가 무엇인가
- 더 좁은 Check가 same failure class를 관찰한다는 descriptor evidence가 있는가

범위 안 possible edge이고 Check coverage가 보수적으로 포함하면 현재 scope를 유지한다. 반대로 누락 가능성이 범위 밖 closure를 가리키면 promotion한다. 선택·유지 어느 쪽이든 reason code와 limitation을 남긴다.

### 검사 family 결합

| 영향 종류 | 기본 candidate family |
|---|---|
| source·symbol | format, lint/static, compile/build, related test |
| test | test trust, changed test, owning source regression |
| docs | local link, docs build/example, public contract docs |
| contract·public API·Schema | schema/contract diff, compatibility, provider/consumer test |
| config | parse, invalid/default/compatibility, docs example |
| dependency·lockfile | lock consistency, build, security/license policy |
| migration | forward/rollback rehearsal, invariant, backup/restore |
| workflow·release | workflow syntax, package/release dry-run, provenance |
| generated source | regeneration consistency, generated diff, consumer build/test |

이 표는 candidate family를 만들 뿐 Project에 없는 tool을 자동 설치하거나 가짜 Check를 만들지 않는다.

## 이전 성공 revision 비교

이전 성공 결과는 “예전에 통과했다”는 이유로 현재 검사를 생략하는 cache가 아니다. `PreviousSuccessComparison`은 CheckPlan 안의 선택 근거 record이며 다음 조건을 모두 비교한다.

| 항목 | 요구 조건 |
|---|---|
| subject | 같은 ProjectId와 호환되는 checkout/source ownership |
| source relation | Git이면 previous revision이 current base의 ancestor임을 local object로 확인; non-Git이면 exact manifest lineage |
| check identity | 같은 CheckDescriptor ID/version/hash, ToolDescriptor/executable identity와 parser contract |
| config | Check input에 영향을 주는 EffectiveConfig·policy fingerprint 동일 |
| scope | previous covered scope가 current selected scope를 포함 |
| result | required run이 `pass`, completeness `complete`, stale 아님; GateDecision이 `auto_pass` 또는 explicit accepted human review |
| delta | previous WorkspaceSnapshot부터 current dirty WorkspaceSnapshot까지 ChangeSet complete |
| invalidation | descriptor의 `invalidates_on` selector와 current delta를 전부 평가 |

previous success candidate가 primary `planning_baseline`의 base보다 오래된 revision이면 selector는 project별 보조 `ChangeSet(change_set_kind=previous_success_delta)`을 요구한다. 이 document는 같은 TaskSpec·ScopeRevision을 참조하고 이전 successful source revision/WorkspaceSnapshot을 base, 현재 dirty WorkspaceSnapshot을 observed로 둔다. primary ChangeSet을 바꾸거나 두 delta를 합치지 않으며 `PreviousSuccessComparison`이 보조 ChangeSet ref와 fingerprint를 가진다. ancestor object·이전 manifest·중간 source를 읽을 수 없어 complete delta를 만들지 못하면 comparison은 `unknown`이다.

모두 만족하고 CheckDescriptor가 deterministic reuse를 명시했을 때만 `reusable`이다. reusable이어도 risk path가 `always_run`을 요구하거나 current source effect를 확인해야 하는 Check는 실행한다. ancestor를 확인할 수 없거나 dirty delta가 partial이면 `comparison=unknown`이며 생략 근거가 아니다.

이전 성공 결과는 다음에만 사용한다.

- unchanged scope의 optional Check 중복 실행을 생략
- package-level affected Check로 좁힐 때 unchanged package evidence 제공
- regression baseline과 failure history 연결

필수 Check 생략에는 CheckDescriptor의 cache contract, exact result fingerprint와 `omitted_checks.alternative_evidence_refs`가 모두 필요하다.

## 사용자 결정 우선권

사용자는 자동 계산보다 우선해 다음을 바꿀 수 있다.

- target Project·Checkout과 include/exclude scope
- planned change scope
- Check 추가와 package→workspace→full 승격
- possible impact의 수용·제외 판단
- 자동 후보 Check의 생략 요청

모든 사용자 변경은 새 TaskSpec 또는 ScopeRevision을 만든다. 기존 ImpactAnalysis·ValidationPlan·ChangePlan을 수정하지 않는다.

사용자 결정이 위험 metadata와 다를 때 계산기는 결정을 거부하지 않고 다음을 함께 기록한다.

1. `decision_source=user_override`
2. 자동 계산과 달라진 내용
3. 생략된 coverage와 remaining risk
4. 필요한 Waiver 또는 `human_review` gate
5. permission·외부 제한 때문에 실행할 수 없는 항목

사용자 override를 자동 계산으로 다시 덮지 않는다. 반대로 사용자가 제외하지 않은 범위를 자동 planned change scope로 추가하지 않는다.

## scope revision과 재계획

다음 조건은 새 ScopeRevision과 전체 또는 부분 재계획을 요구한다.

- TaskSpec objective, Project, include/exclude, 완료 조건 또는 강제 Check 변경
- ProjectRevision·WorkspaceSnapshot·dirty ChangeSet fingerprint 변경
- ProjectCatalog·CodeIndex freshness·coverage·tier 변경
- 예상 밖 direct/transitive impact가 planned boundary 밖에서 발견됨
- 새 RiskPathFinding 또는 더 높은 severity floor 발견
- CheckDescriptor·ToolDescriptor·CatalogSnapshot·EffectiveConfig 변경
- required Check `not_found`, unavailable 또는 fallback floor 변경
- 사용자가 impact·scope·Check 결정을 수정함

재계획 범위는 input dependency로 계산한다.

| 바뀐 입력 | 최소 재계산 |
|---|---|
| 사용자 표시 문구만 변경 | render만; scope hash가 같아야 함 |
| Check metadata만 변경 | affected selection·ValidationPlan |
| risk descriptor 변경 | risk path 이후 전체 |
| ChangeSet·graph·snapshot 변경 | seed부터 ImpactAnalysis·두 plan 전체 |
| 4단계 `recipe_preview` 또는 Recipe·Tool fingerprint 변경 | preview seed부터 ImpactAnalysis·ValidationPlan·PatchSet 준비 전체 |
| Project target·exclude 변경 | ScopeRevision부터 전체 |

자동 계산이 해당 축 exclusion 안에서 analysis/validation scope를 넓히면 근거가 있는 accepted derived addition으로 기록할 수 있다. planned change scope 확대나 사용자 결정을 바꾸는 제안만 `approval_state=proposed` ScopeRevision으로 만들고, 사용자가 수락·거부하면 새 accepted user-decision revision을 만든다. source 변경 단계는 accepted revision만 사용할 수 있다.

필수 event는 `task.created|revised`, `scope.resolved|revised`, `impact.calculated|invalidated`, `validation.planned`, `change_plan.created|revised`, `plan.replanned`다. event payload에는 old/new ref, reason code, changed field set, actor, causation, source snapshot과 calculation fingerprint를 둔다.

## ImpactAnalysis 계약

`ImpactAnalysis`는 `star.impact-analysis` top-level document다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `impact_analysis_id`, `revision` | 예 | immutable analysis instance와 revision |
| `task_spec_ref` | 예 | 사용자 입력 revision |
| `scope_revision_ref` | 예 | requested/analysis/change/validation scope |
| `project_inputs` | 예 | project별 Checkout·Revision·Workspace·Catalog·Index ref와 freshness |
| `change_set_refs` | 예 | project별 actual dirty comparison |
| `catalog_snapshot_ref` | 예 | relation policy·RiskPath·Task·Check metadata 근거 |
| `effective_config_fingerprint` | 예 | limit·fallback·policy 입력 |
| `seeds` | 예 | project별 ImpactSeed detail을 가리키는 ref와 redacted summary |
| `impacted_nodes` | 예 | project별 node detail ref와 direct/transitive·confirmed/possible·confidence summary |
| `impact_edges` | 예 | project store의 [ImpactEdge](validation-and-evidence.md#impactedge-계약)를 가리키는 `ImpactEdgeRef` array |
| `risk_paths` | 예 | project별 RiskPathFinding detail ref와 cross-project summary |
| `affected_projects` | 예 | project별 confirmed/possible과 closure 상태 |
| `no_results` | 예 | query/check family별 reason·searched scope·tier·coverage |
| `limitations` | 예 | stale·partial·unsupported·ambiguous·limit·excluded scope |
| `confidence_summary` | 예 | certainty·confidence별 count와 가장 약한 required boundary |
| `calculation_fingerprint` | 예 | 모든 의미 input/output의 canonical hash |
| `status` | 예 | `complete`, `partial`, `blocked`, `invalidated` |
| `generated_at` | 예 | identity에는 제외 |

전역 `ImpactAnalysis`는 여러 Project 계산을 묶는 coordinator document이며 다른 Project의 private source path·symbol detail을 복제하지 않는다. `ImpactSeedRef`, impacted-node ref, `ImpactEdgeRef`, RiskPathFinding ref는 최소 `project_id`, owning document ID·revision, content fingerprint와 해당 Project 안에서만 유효한 stable local ID를 가진다. `ImpactEdgeRef`에는 전역 판단에 필요한 `certainty`, `confidence`, `distance`, `cross_project` 여부와 redacted endpoint kind만 함께 둘 수 있다. exact source range·literal·private symbol name·전체 evidence chain은 owning project store의 detail record와 ArtifactRef에서 읽는다. 따라서 selector와 3단계 consumer는 ref fingerprint를 확인한 뒤 해당 Project repository를 통해 detail을 조회하며, global projection만으로 source-level 판단을 다시 만들지 않는다.

`complete`는 영향이 전부 확실하다는 뜻이 아니라 설정된 scope와 limit 안에서 모든 required input·traversal·no-result를 설명했다는 뜻이다. possible impact가 있어도 complete일 수 있다. stale required input, unmapped required seed, 잘린 closure를 숨기면 complete가 아니다.

### no-result reason

| reason | 의미 |
|---|---|
| `confirmed_empty` | current·complete requested scope에서 결과 없음 |
| `not_indexed` | 필요한 partition 자체 없음 |
| `unsupported_language` | adapter 미지원 |
| `parse_failed` | 대상이지만 parse 실패 |
| `semantic_unavailable` | required resolution tier 없음 |
| `excluded_by_policy` | 명시적 policy로 미관찰 |
| `stale` | current source와 input 불일치 |
| `partial` | 일부 scope만 관찰 |
| `ambiguous` | 여러 target 중 해소 못함 |
| `limit_exceeded` | traversal/resource frontier 잘림 |
| `no_seed_mapping` | 사용자/변경 target을 entity로 매핑 못함 |
| `descriptor_not_found` | 필요한 Task·Check·Risk metadata 없음 |
| `not_applicable` | complete applicability 평가 결과 대상 아님 |

`descriptor_not_found`와 `not_applicable`을 같은 empty state로 직렬화하지 않는다.

## ChangePlan·ValidationPlan 출력 연결

2단계의 성공 출력은 다음 document graph다.

```text
TaskSpec(revision)
  -> ScopeRevision(accepted|proposed)
       -> ChangeSet[]
       -> ImpactAnalysis
            -> ChangePlan[]
            -> ValidationPlan
```

ChangePlan v2 target의 full field는 [공통 개발 관리 계약](development-management.md#changeplan--starchange-plan)이 소유한다. 하나의 ChangePlan은 하나의 Project만 소유하고, 2단계는 planned-change Project마다 다음 값을 반드시 채운다.

- TaskSpec·ScopeRevision·ImpactAnalysis ref
- 해당 Project의 target Checkout·Revision·WorkspaceSnapshot과 ChangeSet ref
- stable user-intended change unit, accepted planned scope 안의 target과 intended postcondition
- unit dependency graph, expected ImpactEdge ref와 TaskSpec completion criterion mapping
- Finding/Recipe가 있으면 typed ref, 없으면 `change_origin=user_planned`
- risk·permission requirement와 ValidationPlan ref
- unresolved impact, precondition과 readiness
- Registry task이면 before/desired declaration fingerprint, source manifest·ManagedRegistrySnapshot ref, binding·consumer transition과 alias/lifecycle precondition

ValidationPlan의 full field는 [검사·완료·증거](validation-and-evidence.md#validationplan-계약)가 소유한다. 2단계는 candidate 전부의 selected/omitted/unresolved 상태, affected scope, fallback, previous success comparison과 plan readiness를 채운다.

모든 project ChangePlan과 ValidationPlan은 같은 `task_spec_ref`, `scope_revision_ref`, `impact_analysis_ref`, 해당 project ChangeSet fingerprint와 EffectiveConfig/Catalog fingerprint를 가져야 한다. 하나라도 다르면 `PLANNING_OUTPUT_COHERENCE` 오류다.

Registry task이면 `managed_registry_expectations`도 같은 before snapshot·source manifest hash, declaration set, namespace/tombstone, binding·consumer fingerprint를 사용해야 한다. downstream Project는 read-only impact와 proposed migration unit만 가지며 9단계 전 apply-ready cross-project ChangePlan을 만들지 않는다.

4단계 prepare는 이 graph를 수정하지 않고 `RecipeExecution(mode=preview)`와 `recipe_preview` ChangeSet을 추가한 뒤 같은 M2 계산으로 impact·ValidationPlan을 재조정한다. 그 결과가 accepted ScopeRevision과 호환될 때만 immutable PatchSet을 만든다. preview와 initial plan의 차이를 application layer가 임의로 무시하거나 post-apply runner가 새 Check를 즉석 선택하지 않는다.

## 3단계 입력 계약

3단계 Validation engine은 ValidationPlan을 재해석해 검사 종류를 새로 고르지 않는다. 실행 전 다음 precondition만 기계적으로 확인한다.

1. ValidationPlan `readiness=ready`
2. TaskSpec·ScopeRevision이 current이고 proposed user decision이 없음
3. ChangePlan과 ValidationPlan의 ImpactAnalysis·ChangeSet fingerprint가 동일
4. project별 current WorkspaceSnapshot probe가 plan subject와 동일
5. required CheckDescriptor·ToolDescriptor·Catalog hash가 동일하고 trusted/available
6. required check의 TaskInvocation template가 typed argument로 완전히 bind됨
7. CheckGraph가 acyclic이고 required dependency closure·failure policy가 완전함
8. unresolved required Check, unaccepted waiver와 blocked permission 없음
9. selected scope가 fallback floor보다 좁지 않음
10. Registry task이면 authoritative source manifest와 ManagedRegistrySnapshot이 current·valid하고 expected declaration·binding·consumer fingerprint가 plan과 동일

precondition 실패 시 3단계는 검사를 임의 확대·축소하거나 stale plan을 실행하지 않고 `replan_required`를 반환한다. 새로운 source 변경이 발견되면 새 ChangeSet부터 2단계를 다시 실행한다.

3단계 실행 결과는 각 CheckPlan ID와 선택 근거를 그대로 ValidationRun·ValidationResult에 연결한다. 따라서 “왜 이 검사를 했는가”, “왜 전체 검사를 하지 않았는가”, “어떤 fallback으로 넓어졌는가”를 plan과 result 사이에서 추적할 수 있어야 한다.

## persistence와 evidence

- TaskSpec·ScopeRevision·ImpactAnalysis summary·ValidationPlan은 run state 또는 global planning coordinator의 local operational document다. Project별 ChangePlan·ChangeSet·ImpactEdge detail은 해당 project store에 둔다. source에서 완전히 재구축할 수 없으므로 모두 backup/export 대상이다.
- global TaskSpec·ScopeRevision에는 사용자가 직접 선언한 ProjectPathRef·stable selector를 보존할 수 있다. 계산 중 관찰한 source range·literal·private symbol detail과 전체 edge path는 global planning record에 복제하지 않고 project participant DocumentRef와 redacted cross-project summary만 둔다.
- 전체 graph dump, diff, traversal frontier와 selector trace는 `.ai-runs` ArtifactRef로 분리한다.
- 다른 Project의 absolute path·private symbol detail을 global store나 상대 project store에 복제하지 않는다.
- invalidated analysis를 삭제하지 않고 새 revision이 `supersedes`로 연결한다.
- 4단계 `recipe_preview` ChangeSet과 reconciliation 결과는 RecipeExecution·PatchSet fingerprint에 묶인 local operational evidence이며 actual dirty ChangeSet이나 적용 완료 기록으로 승격하지 않는다.
- DB와 evidence에는 source literal·secret·개인 path를 저장하지 않는다.

## application·Package 경계

```text
star-application
  -> star-project: current snapshot·typed graph query·freshness probe
  -> star-validation/change_set: TaskSpec·ScopeRevision 기준 actual comparison
  -> star-planning: TaskSpec normalize·scope·seed·impact·risk·ChangePlan draft
  -> star-validation: Check candidate·affected scope·fallback·ValidationPlan
  -> star-state/star-evidence: atomic projection·artifact commit
```

- `star-project`는 observed graph와 query quality만 제공하고 task-specific impact, risk severity와 Check 선택을 결정하지 않는다.
- `star-validation/change_set`은 star-project가 관찰한 Revision·WorkspaceSnapshot delta를 TaskSpec·ScopeRevision에 bind해 immutable ChangeSet으로 만들며 ImpactEdge·RiskPathFinding을 backfill하지 않는다.
- `star-planning`은 전달받은 immutable contract value로만 계산하는 pure engine이며 filesystem·Git·DB·process handle을 받지 않는다.
- `star-validation/selector`는 descriptor와 ImpactAnalysis를 ValidationPlan으로 만들지만 check를 실행하지 않는다.
- `star-checks/change_scope`는 이후 실제 ChangeSet이 accepted ScopeRevision·ChangePlan과 일치하는지 검사한다. 초기 계획 graph를 소유하지 않는다.
- `star-application`만 current probe·repository transaction·engine 호출 순서를 조정한다.

## stable error와 limitation code

command error의 정본 설명은 [오류와 진단 계약](errors-and-diagnostics.md#변경-계획영향-분석-대표-오류)이 소유한다. 이 절은 2단계 결과가 어떤 code set을 사용해야 하는지와 limitation 경계만 고정한다.

### command error

- `PLANNING_TASK_INPUT_INCOMPLETE`
- `PLANNING_PROJECT_AMBIGUOUS`
- `PLANNING_SCOPE_CONFLICT`
- `PLANNING_INPUT_CHANGED`
- `PLANNING_SNAPSHOT_STALE`
- `PLANNING_OUTPUT_COHERENCE`
- `PLANNING_USER_DECISION_REQUIRED`
- `IMPACT_REQUIRED_INPUT_UNAVAILABLE`
- `IMPACT_OUTPUT_INVALID`
- `AFFECTED_REQUIRED_CHECK_UNRESOLVED`
- `AFFECTED_SCOPE_UNBINDABLE`

### result limitation·reason

- `IMPACT_NO_SEED_MAPPING`
- `IMPACT_GRAPH_PARTIAL`
- `IMPACT_GRAPH_LIMIT`
- `IMPACT_TIER_FALLBACK`
- `IMPACT_EDGE_AMBIGUOUS`
- `IMPACT_DOWNSTREAM_UNVERIFIED`
- `IMPACT_EXCLUDED_SCOPE`
- `RISK_PATH_METADATA_MISSING`
- `AFFECTED_CHECK_NOT_FOUND`
- `AFFECTED_CHECK_UNAVAILABLE`
- `AFFECTED_CHECK_CANDIDATE_LIMIT`
- `AFFECTED_SCOPE_PROMOTED_WORKSPACE`
- `AFFECTED_SCOPE_PROMOTED_PROJECT_FULL`
- `PREVIOUS_SUCCESS_INCOMPATIBLE`
- `PREVIOUS_SUCCESS_RELATION_UNKNOWN`

error는 command가 유효한 document를 만들지 못했다는 뜻이고 limitation은 결과 안의 정확도·coverage 제한이다. limitation이 있어도 safe fallback과 readiness가 완전하면 plan은 ready일 수 있다. required closure나 Check가 해결되지 않으면 blocked다.

## 구현·fixture 순서

제품 구현은 다음 순서를 바꾸지 않는다.

1. TaskSpec, ScopeRevision, ImpactAnalysis, 확장 ChangeSet·ValidationPlan·ChangePlan type과 valid/invalid/future-version Schema fixture
2. Task·Check·RiskPath descriptor Schema, reference·conflict·fingerprint conformance
3. pure scope normalizer와 seed mapper golden
4. direct/transitive·confirmed/possible·literal ownership graph corpus
5. risk path와 affected selector table-driven corpus
6. package→workspace→project full promotion, no-result와 previous-success fixture
7. multi-project current/stale/partial cross-project fixture
8. repository/event/idempotency·invalidation conformance
9. CLI-only E2E와 before/after source·Git metadata equality
10. 3단계 fake runner가 ValidationPlan을 재선택 없이 소비하는 contract test

최소 corpus에는 다음 경계를 포함한다.

- 같은 literal, 다른 symbol·contract·Project
- file rename/delete와 base/current symbol mapping
- dirty staged+unstaged+untracked 혼합
- public Schema provider와 current consumer, stale consumer
- root lockfile·workspace manifest·generated source·migration·workflow 변경
- test mapping 0건의 `not_found`와 true `not_applicable`
- semantic unavailable에서 syntax/text possible impact
- graph cycle, ambiguous edge, max depth/node/edge 초과
- previous successful ancestor/non-ancestor·tool hash 변경
- 사용자 Check 생략 override와 human review

## 설계 수용 기준

- 사용자 입력만으로 CLI-only TaskSpec·ScopeRevision·ChangePlan·ValidationPlan을 만들 수 있다.
- Codex·AI 호출 없이 current index·graph와 deterministic descriptor로 영향과 검사 후보를 계산한다.
- direct/transitive, confirmed/possible, confidence·limitation·no-result가 각각 독립 field다.
- actual dirty ChangeSet과 user-intended change scope가 섞이지 않는다.
- 같은 literal이 ownership이 다르면 다른 영향 node다.
- `not_found`와 `not_applicable`이 다른 plan 상태다.
- package scope soundness를 증명하지 못하면 workspace, 다시 project full로 명시적으로 승격한다.
- possible impact만 있다는 이유로 무조건 full을 선택하지 않고 closure·risk floor 근거를 기록한다.
- 여러 Project 영향은 read-only·project-partitioned이며 source 수정·merge를 하지 않는다.
- user override가 자동 계산보다 우선하고 remaining risk·waiver·replan lineage가 남는다.
- 3단계가 ValidationPlan을 기계적으로 검증·실행할 수 있고 stale input을 안전하게 거부한다.
- 4단계 preview도 같은 impact·affected selector를 사용하며 actual dirty ChangeSet과 `recipe_preview`를 섞지 않고 새 plan revision으로 reconciliation한다.
- 현재 구현, 설계 확정과 미구현 표현이 문서 전반에서 일치한다.
