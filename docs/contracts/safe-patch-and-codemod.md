# 안전한 Patch·Refactor·codemod 엔진 계약

## 상태와 문서 소유권

이 문서는 Star-Control 4단계인 **안전한 Patch·Refactor·codemod 엔진**의 설계 정본이다. 현재 상태는 **공통 첫 수직 Slice 구현, 확장 Recipe 진행 전**이다. P-0045는 built-in trailing-whitespace Recipe에 대해 isolated in-memory preview, sealed immutable artifact, exact approval, before/after hash, pre-write TOCTOU 재관찰, atomic apply와 exact rollback을 구현했다. external mutator worktree와 모든 v2 selector·migration이 구현됐다는 뜻은 아니다.

4단계는 새 scanner, planner, validator 또는 AI 실행기를 만들지 않는다. 다음 선행 계약과 같은 application service를 조합해 한 Project의 immutable 변경 제안을 만들고, 검증된 제안만 적용한다.

| 책임 | 정본 |
|---|---|
| `ChangeRecipe`, `ChangePlan`, `PatchSet` 공통 identity와 lifecycle | [공통 개발 관리 계약](development-management.md) |
| Project·Checkout·source 분류, symbol/reference와 freshness | [Project Catalog·Code Index 계약](project-catalog-and-code-index.md) |
| 영향 graph, risk path, affected Check와 `ChangePlan` v2 | [변경 계획·영향 분석 계약](change-planning-and-impact.md) |
| `patch_pre_apply`, `patch_post_apply`, Gate 알고리즘 | [3단계 공통 검증·품질 Gate](../features/common-validation-gate.md) |
| `ChangeSet`, evidence binding, `EvidenceBundle` wire | [검사·완료·증거 계약](validation-and-evidence.md) |
| Recipe·Tool·Check·Profile Catalog resolution | [설정과 Catalog 계약](config-and-catalog.md) |
| 외부 codemod 실행·ToolDescriptor·process protocol | [외부 Tool Registry](external-tool-registry.md), [Manifest Reference](tool-package-manifest-reference.md), [Windows Tool Runtime](../architecture/windows-tool-runtime.md) |
| permission, dirty source와 secret 경계 | [승인·권한·안전](../architecture/security-and-permissions.md) |
| single-project worktree 선택과 이후 병합 경계 | [병렬 작업과 병합](../architecture/worktrees-and-merge.md) |
| Package·port·adapter 소유권 | [Repository·Package 구조](../architecture/repository-layout.md) |
| stable error·Diagnostic namespace | [오류와 진단 계약](errors-and-diagnostics.md) |
| 5단계 ManagedDeclaration·Git manifest·lifecycle·binding·consumer | [관리형 Symbol Registry 계약](managed-symbol-registry.md) |

이 문서는 4단계의 실행 순서, selector 정확도, rewrite 보장 수준, dry-run, idempotence, partial apply와 복구 알고리즘을 소유한다. 위 문서가 소유하는 공통 field와 process 세부를 별도 의미로 다시 정의하지 않는다.

## 선행조건과 완료 판정

4단계 제품 구현은 다음 네 계약이 **문서에만 존재하는 상태가 아니라 제품 gate를 통과한 상태**여야 시작할 수 있다.

| 선행조건 | 4단계가 소비하는 값 | 현재 상태 | 미충족 시 처리 |
|---|---|---|---|
| 0단계 공통 변경 계약 | `ChangeRecipe`, `ChangePlan`, immutable `PatchSet`, ID·fingerprint, Controller 단일 Writer | P0 첫 수직 Slice만 구현 | M4 target Schema·migration 전에는 historical v1을 자동 apply에 사용하지 않음 |
| 1단계 Index | current `ProjectCatalogSnapshot`, `CodeIndexSnapshot`, symbol/reference·contract·generated ownership, tier·coverage·limitation | M1 첫 Rust 수직 Slice 구현 | selector resolution·semantic rewrite는 current tier 증거가 없으면 차단 |
| 2단계 영향 분석 | accepted `ScopeRevision`, `ChangePlan` v2, `ImpactAnalysis`, `readiness=ready` `ValidationPlan` | M2 첫 수직 Slice 구현 | Patch 준비·검사 선택 제품 구현 차단 |
| 3단계 공통 Gate | current evidence binding, `patch_pre_apply`, `patch_post_apply`, B01·B02·B04와 EvidenceBundle | M3 공통 runner·evidence 첫 Slice 구현 | 미구현 Rule family가 필요한 write permit은 차단 |

선행 계약 하나라도 current·complete하지 않으면 4단계는 최대 `recipe.validate`·`recipe.describe` 같은 정적 조회만 제공할 수 있다. 가짜 index, mock-only Gate 또는 사용자의 완료 설명으로 source write readiness를 합성하지 않는다.

4단계의 상태 표현은 다음처럼 구분한다.

- **설계 확정**: 이 문서와 관련 정본의 계약이 충돌 없이 구현 가능한 수준으로 고정됨
- **제품 구현 전**: contract type·Schema·migration·engine·adapter·CLI·Corpus가 없음
- **제품 준비됨**: M1→M2→M3 gate와 M4 conformance를 실제 제품 code로 통과함
- **적용 완료**: 한 `PatchApplication`의 post-apply Gate와 evidence packaging이 완료됨

설계 확정과 적용 완료를 같은 `done`으로 표시하지 않는다.

## 목표

1. 사용자가 Codex 없이 CLI에서 Recipe와 target을 지정해 deterministic 변경 preview를 만들 수 있게 한다.
2. scan·impact 계산과 rewrite·apply side effect를 서로 다른 application phase와 port로 분리한다.
3. 모든 source write 전에 immutable `PatchSet`, diff, 예상 영향, 검사 계획과 rollback 자료를 먼저 만든다.
4. raw literal의 우연한 일치가 아니라 symbol, contract ID, managed declaration과 명시적 target selector를 우선한다.
5. text, syntax, symbol-aware, codegen 변환의 보장 수준과 허용 범위를 구분한다.
6. base revision, dirty state, config·Catalog·Index·Tool identity가 바뀐 stale apply를 막는다.
7. 사용자 기존 미커밋 변경을 덮어쓰지 않고 overlap이면 중단하거나 격리 worktree를 선택한다.
8. Recipe idempotence와 application command idempotency를 별도 계약으로 검증한다.
9. multi-file apply의 부분 성공을 정상 성공으로 숨기지 않고 deterministic 복구 자료와 상태를 남긴다.
10. M2가 고른 format·build·test·contract Check를 M3 pre/post Gate에서 그대로 실행한다.
11. 외부 codemod CLI를 Tool Registry와 구조화 인자로만 호출하고 실제 tool·version·hash·입출력 evidence를 남긴다.
12. 이후 Codex와 5단계 managed Registry가 별도 rewrite engine 없이 같은 application command를 사용하게 한다.

## 제외 범위

4단계는 다음을 지원하지 않는다.

- raw 문자열 하나를 전체 Project 또는 여러 Project에 전역 치환하는 기능
- source scan과 rewrite를 한 process의 암묵적 side effect로 합치는 기능
- `prepare`와 `apply`를 하나의 기본 CLI command로 합치는 기능
- live target checkout에서 외부 codemod EXE를 직접 실행해 source를 바꾸는 기능
- Recipe에 PowerShell·`cmd`·shell command string, 동적 script, AI prompt, SQL 또는 실행 code를 저장하는 기능
- Codex·다른 AI·OpenAI API가 Recipe, target, patch 또는 완료 판정을 필수로 생성하는 기능
- parser, compiler, LSP, generator 또는 언어별 codemod framework 자체의 재구현
- 지원하지 않는 language·macro·reflection·dynamic dispatch를 semantic exact로 가장하는 기능
- generated source를 source-of-truth처럼 직접 수정하는 기능
- binary·opaque source와 reparse point를 일반 text operation으로 수정하는 기능
- 한 `PatchSet`에서 둘 이상의 Project를 쓰거나 cross-repo apply·merge·commit·push하는 기능
- timeout·cancel·crash 뒤 external tool 또는 Patch apply를 자동 재시도하는 기능
- Gate 실패를 이유로 사용자 기존 변경을 `git reset`, `checkout`, 삭제 또는 overwrite하는 기능

여러 Project의 영향은 M2에서 계속 read-only로 계산할 수 있다. 여러 Project source write는 이 단계가 아니라 사용자 로드맵 **9단계**의 별도 coordination·merge 계약으로 미룬다.

## 핵심 불변식

1. `scan -> plan -> prepare -> inspect -> pre-gate -> apply -> post-gate` 순서를 건너뛰지 않는다.
2. `prepare`는 target source와 Git metadata에 effect가 없는 dry-run이다.
3. 외부 mutating codemod는 target checkout이 아니라 폐기 가능한 격리 preview workspace에서만 실행한다.
4. source write port는 persisted Recipe나 ToolDescriptor가 아니라 single-use in-memory `PatchApplyPermit`으로만 열린다.
5. `PatchSet`은 immutable preview다. 실제 적용 상태는 별도 `PatchApplication`이 소유한다.
6. target selector 해석 결과와 모든 file operation은 한 Project·한 Checkout에 속한다.
7. before hash·mode·존재 상태가 하나라도 다르면 아무 operation도 시작하지 않는다.
8. 사용자 기존 변경과 byte range·rename·delete·generated ownership이 겹치면 overwrite하지 않는다.
9. 외부 tool이 만든 diff도 Star-Control 내부 operation validator와 M2·M3 Gate를 우회하지 않는다.
10. M2가 선택한 Check를 M4 runner가 추가·삭제·축소하지 않는다. 실제 preview가 다른 change class를 만들면 replan한다.
11. post-apply 실제 상태가 expected-after와 다르면 성공이 아니라 `partially_applied`, `outcome_unknown` 또는 `recovery_required`다.
12. post-apply `AUTO_PASS`와 complete EvidenceBundle·ReviewPack 없이는 자동 완료하지 않는다.

## document graph

```text
M1 current ProjectCatalogSnapshot·CodeIndexSnapshot
  + M2 accepted TaskSpec·ScopeRevision
  + M2 ChangePlan v2·ready ValidationPlan
  + resolved ChangeRecipe·typed input·target selector
  -> RecipeExecution(mode=preview)
       -> isolated/in-memory preview result
       -> preview ChangeSet + diff
       -> M2 impact·Profile·affected Check reconciliation
       -> RecipeExecution(mode=idempotence_replay)
       -> immutable PatchSet v2
            -> M3 patch_pre_apply GateDecision
            -> in-memory PatchApplyPermit
            -> PatchApplication
                 -> actual WorkspaceSnapshot·observed_after_change ChangeSet
                 -> M3 patch_post_apply GateDecision
                 -> EvidenceBundle -> ReviewPack
```

`RecipeExecution`은 preview를 어떻게 만들었는지, `PatchSet`은 무엇을 제안하는지, `PatchApplication`은 실제 target에 무슨 일이 있었는지를 각각 소유한다. 세 문서를 하나의 mutable status record로 합치지 않는다.

## ChangeRecipe M4 target

`ChangeRecipe`는 사람이 검토하는 Git·Catalog 선언이다. M4 writer target은 `star.change-recipe` descriptor `format_version=2`이며 P0 v1 reader compatibility와 자동 실행 eligibility를 분리한다. exact serialization field는 `star-contracts` 한 곳에서만 정의한다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `recipe_id` | 예 | 공통 namespace Catalog ID. 표시 이름·file path와 분리하고 재사용 금지 |
| `recipe_version` | 예 | SemVer exact version. range를 실행 evidence에 저장하지 않음 |
| `definition_fingerprint` | 예 | 실행 의미 field의 JCS SHA-256 |
| `input_schema_ref` | 예 | local Draft 2020-12 root object, `additionalProperties=false` |
| `target_languages` | 예 | stable language ID와 version/capability 조건. 빈 집합·암묵적 `any` 금지 |
| `rewrite_kind` | 예 | `text_replace`, `syntax_rewrite`, `symbol_aware_rewrite`, `codegen` |
| `assurance_contract` | 예 | 보장 축·required index tier·coverage·limitation 처리 |
| `target_selector_contract` | 예 | 허용 selector kind, multiplicity, 최대 target 수, required resolution |
| `preconditions` | 예 | source·revision·dirty·path·index·tool·config 조건 |
| `expected_postconditions` | 예 | 기계적으로 관찰 가능한 after predicate |
| `transformer_ref` | 예 | built-in transformer, language adapter 또는 trusted ToolDescriptor exact ref |
| `transformer_input_binding` | 예 | Recipe input·selector를 transformer typed input으로 매핑 |
| `allowed_path_scope` | 예 | ProjectPathRef selector와 source class/facet allow/deny |
| `dirty_policy` | 예 | `require_clean`, `allow_disjoint`, `isolated_only` |
| `idempotency_contract` | 예 | replay 방식·no-op 판정·unknown 처리 |
| `validation_requirements` | 예 | required Profile·Rule·Check family floor. M2가 최종 materialize |
| `risk_class`, `permission_actions` | 예 | 최소 policy input. 권한 자체를 부여하지 않음 |
| `rollback_contract` | 예 | reverse PatchSet 또는 isolated workspace disposal 조건 |
| `resource_limits` | 예 | target·operation·byte·duration·output 상한 |
| `supported_execution_contexts` | 예 | 최소 `cli_only`; 선택적으로 `codex_managed` |

### stable ID·version·fingerprint

- `recipe_id`는 공통 `catalog_id` grammar를 사용한다. built-in 예시는 `star.change-recipe.rename-symbol`처럼 namespace를 포함한다.
- 같은 ID는 같은 변경 의도를 유지한다. 전혀 다른 변환에 폐기된 ID를 재사용하지 않는다.
- compatible input 확장, 진단 개선처럼 기존 실행 의미를 깨지 않는 변경은 minor, pre/postcondition·selector·output 의미의 incompatible 변경은 major를 올린다.
- typo나 표시 문구만 바뀌어 실행 의미가 같으면 Recipe version을 올리지 않을 수 있지만 source byte hash는 Catalog provenance에 남긴다.
- 같은 `recipe_id + recipe_version`에 다른 `definition_fingerprint`가 발견되면 우선순위로 덮지 않고 Catalog conflict다.
- fingerprint에는 input Schema, target language/capability, selector, pre/postcondition, transformer binding, idempotence, path·permission·validation·resource limit을 포함한다.
- display text, 예시 순서, timestamp와 source file absolute path는 definition fingerprint에서 제외한다.
- external tool version은 Recipe version과 별개다. 실행 시 resolved ToolDescriptor·executable version/hash를 추가로 고정한다.

### input Schema

1. root는 object이고 unknown field를 거부한다.
2. remote `$ref`, executable format assertion, script와 dynamic expression을 허용하지 않는다.
3. default는 validation 뒤 실제 normalized input에 materialize하고 JCS hash에 포함한다.
4. target identity를 parameter string으로 숨기지 않는다. target은 별도 typed selector로 전달한다.
5. source에 쓸 raw secret, token과 credential value를 입력으로 받지 않는다. 필요한 경우 secret placeholder나 `SecretRef` identity만 허용하고 source byte로 materialize하지 않는다.
6. 큰 template·mapping input은 hash가 있는 local ArtifactRef로 전달하고 Recipe TOML에 inline script처럼 저장하지 않는다.
7. input validation 실패는 transformer·external process·preview workspace 생성 전에 끝낸다.

### 대상 언어와 capability

`target_languages`의 각 항목은 language ID, optional language version constraint, required source facet, minimum index tier, transformer capability와 unsupported construct를 가진다.

- text rewrite도 encoding·line ending·source class를 확인하므로 암묵적 모든 file 적용이 아니다.
- syntax rewrite는 해당 language의 parse·render capability가 current여야 한다.
- symbol-aware rewrite는 semantic definition/reference resolution과 coverage contract가 current·complete여야 한다.
- codegen은 language parser보다 authoritative generator input·output ownership과 generator capability가 우선이다.
- adapter가 없거나 version·capability가 맞지 않으면 lower tier로 몰래 실행하지 않는다. Recipe가 명시적으로 허용한 더 낮은 rewrite kind의 별도 Recipe를 선택해야 한다.
- 특정 parser·codemod library 이름은 core public 계약이 아니다. 실제 adapter 선택은 구현 직전 corpus, license, offline·Windows 지원과 최신 공식 자료를 검토한 뒤 private dependency 또는 external ToolDescriptor로 결정한다.

## target selector 계약

`TargetSelector`는 다음 tagged union만 허용한다.

| kind | identity | 최소 current 근거 | 대표 사용 |
|---|---|---|---|
| `managed_declaration` | `ManagedDeclarationId`, owner contract와 expected declaration fingerprint | declared registry mapping + owning symbol/source | 5단계 상수·오류 코드·config key 변경 |
| `contract` | ContractId·SchemaId·ErrorCode·ConfigKey와 optional member | declared/semantic exact owner·consumer relation | 공개 계약·Schema·설정 변경 |
| `symbol` | SymbolId와 `definition\|references\|definition_and_references` | required semantic tier·coverage | rename·signature·qualified reference 변경 |
| `path_range` | ProjectPathRef, before content hash, byte/range anchor | exact current source byte | 좁은 text·syntax 변경 |
| `finding_occurrence` | FindingId·fingerprint·OccurrenceId·source hash | current complete ScanRun | Rule이 제안한 반복 수정 |
| `generator_input` | generator owner ID, input ProjectPathRef·hash, output set fingerprint | declared `generates\|generated_from` relation | codegen 입력 변경·재생성 |

`raw_literal`은 selector kind가 아니다. literal은 이미 resolve된 `path_range`, symbol body 또는 contract member 안에서 exact before predicate로만 사용할 수 있다.

selector resolution은 다음 순서를 따른다.

1. ProjectId·CheckoutId와 accepted planned change scope를 고정한다.
2. managed declaration 또는 contract stable identity를 먼저 해석한다.
3. symbol selector는 current CodeIndexSnapshot에서 definition·reference set을 해석한다.
4. explicit path/range와 occurrence를 exact source hash로 확인한다.
5. literal predicate는 위 target 내부 byte가 예상과 같은지 확인한다.
6. multiplicity `exactly_one|one_or_more|bounded_set`과 max target을 검증한다.
7. 결과를 `resolved_unique|resolved_set|ambiguous|unresolved|stale|partial|excluded`로 기록한다.

`ambiguous|unresolved|stale|partial`을 임의 path set으로 바꾸지 않는다. Recipe가 required exact resolution을 요구하면 prepare를 중단한다. text search 결과 수가 우연히 하나라는 사실만으로 symbol이나 contract identity를 만들지 않는다.

각 resolved target은 selector ref, owning Project·Checkout, Index snapshot·tier·coverage, source path/range, before content identity, resolution evidence와 `selector_binding_fingerprint`를 가진다. raw source literal은 DB record에 복제하지 않고 필요하면 redacted patch ArtifactRef에만 둔다.

## precondition과 expected postcondition

### 공통 precondition

모든 Recipe는 최소 다음을 검증한다.

- exact ProjectId·CheckoutId·ProjectRevisionId·WorkspaceSnapshotId
- base commit 또는 non-Git source fingerprint
- current dirty manifest·collection completeness
- accepted ScopeRevision과 ChangePlan revision·fingerprint
- current ProjectCatalogSnapshot·CodeIndexSnapshot과 required partition freshness
- EffectiveConfig·CatalogSnapshot·Recipe definition fingerprint
- transformer adapter 또는 ToolDescriptor·executable identity
- target path class·facet·generated/vendor/excluded status
- target before hash·mode·existence와 selector binding
- resource limit, permission requirement와 preview workspace capability

precondition은 자연어 설명이 아니라 typed predicate와 stable reason code를 가진다. unknown은 true가 아니다.

### expected postcondition

자동 apply 가능한 Recipe의 postcondition은 다음 predicate 조합으로 표현한다.

- `path_state`: path의 expected existence·mode·content hash
- `syntax_shape`: parser success, node kind·count·normalized shape fingerprint
- `symbol_binding`: expected Symbol definition/name/signature와 reference set coverage
- `contract_value`: Contract/member·managed declaration의 typed expected value·version
- `generator_manifest`: authoritative input hash, generator identity와 exact output set hash
- `absence_within_scope`: bounded exact scope에서 old target identity가 0건
- `no_unexpected_paths`: PatchSet operation 밖 source 변화 0건
- `validation_gate`: named post-apply ValidationPlan과 required Check satisfaction
- `idempotence_replay_noop`: expected-after에서 같은 Recipe 재실행 operation 0건

자연어 설명만 있는 postcondition은 ReviewPack에는 넣을 수 있지만 자동 완료 predicate가 아니다. dynamic/reflection 때문에 absence나 reference completeness를 증명할 수 없으면 `HUMAN_REVIEW` 또는 `BLOCK`이다.

## rewrite 종류와 보장 수준

네 rewrite kind는 단순한 품질 등급 하나가 아니다. 각각 다른 사실을 보장한다.

| kind | 자동화가 보장하는 것 | 보장하지 않는 것 | 기본 자동 apply 경계 |
|---|---|---|---|
| `text_replace` | exact selected byte range의 before→after와 대상 밖 byte 불변 | symbol identity, parser 의미, 같은 literal의 동일 소유권 | explicit path/range·hash와 bounded occurrence set만 |
| `syntax_rewrite` | supported parser에서 selected syntax node 변환, before/after parse와 shape postcondition | reference resolution, runtime semantic equivalence | 한 Project의 parse-complete file/module, unresolved node 0건 |
| `symbol_aware_rewrite` | current semantic index가 resolve한 definition/reference set과 declared contract에 대한 변환 | reflection·dynamic lookup·unsupported macro 밖 전체 프로그램 등가성 | required semantic coverage complete, possible frontier 처리 완료 |
| `codegen` | authoritative input + pinned generator/config에서 declared output set의 재현성 | generator 자체 correctness와 미선언 side effect | generated file 직접 편집 금지, isolated generation과 exact output manifest |

### `text_replace`

- raw `old`/`new` string pair만으로 Project-wide 실행하지 않는다.
- target은 exact file/range·before hash와 최대 occurrence count를 가진다.
- encoding, BOM, line ending과 range boundary를 보존하거나 expected after에 명시한다.
- overlap하는 match, zero-width match, regex backtracking script와 replacement expression을 허용하지 않는다.
- regex가 필요하면 bounded declarative pattern과 capture substitution contract를 별도 versioned transformer로 두며 arbitrary code replacement는 금지한다.
- identifier, contract ID, error code 또는 config key를 text-only로 바꾸려면 semantic 보장이 없다는 limitation과 required B04 review를 남긴다.

### `syntax_rewrite`

- node selector는 language adapter의 stable normalized node kind·anchor를 사용한다. parser library의 private AST type을 public wire에 노출하지 않는다.
- parse error가 있는 file은 Recipe가 error-tolerant mode와 exact limitation을 선언하지 않는 한 거부한다.
- before/after 모두 parse하고 target node count·shape postcondition을 확인한다.
- formatter 실행은 별도 M2 CheckPlan이다. formatter가 source를 바꾸면 그 결과도 같은 PatchSet preview operation에 포함한다.
- syntax node가 같아도 binding이 다른 identifier reference를 semantic rename으로 광고하지 않는다.

### `symbol_aware_rewrite`

- stable SymbolId, definition·reference edge와 owning contract를 target으로 한다.
- required scope의 semantic partition이 current·complete하고 모든 target이 `resolved`여야 한다.
- possible dynamic/reflection/macro edge는 limitation이 아니라 correctness에 필요한 frontier이면 자동 apply를 막는다.
- rename은 definition만 바꾸거나 text occurrence 전체를 바꾸는 방식이 아니라 resolved reference set과 post-apply re-index를 비교한다.
- public symbol·contract면 provider뿐 아니라 M2가 찾은 consumer contract/build/test Check를 post Gate에서 요구한다.
- 한 언어 adapter의 guarantee를 다른 언어·generated binding·template에 확장하지 않는다.

### `codegen`

- generated output은 target selector가 아니라 `generator_input`과 declared output set으로 소유한다.
- generator executable·version·hash, input·config·template hash와 environment fingerprint를 고정한다.
- generator는 격리 preview workspace에서 실행하고 output set 밖 file 변화는 차단한다.
- 같은 input으로 두 번 실행한 output manifest가 같아야 automatic idempotence를 충족한다.
- nondeterministic timestamp·random ID가 output에 들어가면 normalization contract가 명시적으로 의미를 제거하지 않는 한 자동 apply를 허용하지 않는다.
- generated output을 직접 고친 patch는 B04 `generated direct edit`로 차단한다.

## dry-run과 preview workspace

`prepare`의 기본값이 dry-run이라는 말은 단순 `--dry-run` flag가 있다는 뜻이 아니다. **prepare command에는 live target apply code path 자체가 없다.**

preview 방식은 다음 둘 중 하나다.

| 방식 | 사용 조건 | target effect |
|---|---|---|
| `materialized_preview` | built-in text/syntax/symbol transformer가 after byte를 만들 수 있음 | target source·Git metadata effect 없음. Controller preview root에만 materialize |
| `isolated_git_worktree` | external mutating codemod·formatter·generator 또는 repository context가 필요함 | target checkout effect 없음. exact base의 격리 worktree만 수정 |

external tool에는 live target checkout의 write path를 전달하지 않는다. `working_directory=stage_worktree`는 격리 preview worktree를 가리킨다. tool이 target root absolute path를 요구하거나 cwd 밖 write를 해야 하면 M4 automatic Recipe로 등록할 수 없다.

preview workspace도 local write이므로 PermissionPlan, path scope, lock, temp·retention과 evidence를 가진다. 다만 preview write가 target source apply나 완료를 뜻하지 않는다.

## prepare 알고리즘

`patch.prepare` application use case는 다음 순서를 바꾸지 않는다.

1. Recipe ID·exact version·definition fingerprint를 current CatalogSnapshot에서 resolve한다.
2. input Schema를 검증하고 defaults를 materialize해 redacted input fingerprint를 만든다.
3. target Project·Checkout이 하나인지, accepted ScopeRevision·ChangePlan v2·ValidationPlan이 current·ready인지 확인한다.
4. base revision, WorkspaceSnapshot, dirty manifest, config·Catalog·Index fingerprint를 다시 probe한다.
5. typed selector를 current M1 index에서 resolve하고 multiplicity·path scope·source class·generated ownership을 확인한다.
6. dirty overlap과 transformer kind를 바탕으로 `WorktreeDecision`을 만든다.
7. target effect가 없는 preview workspace를 준비하고 exact before snapshot manifest를 기록한다.
8. built-in adapter 또는 Tool Registry action을 typed input으로 한 번 실행한다. 외부 EXE 자동 retry는 없다.
9. preview workspace를 완전 재관찰해 add·modify·delete·rename·mode·binary·unexpected path를 수집한다.
10. Recipe output이나 tool 보고가 아니라 실제 before/after byte를 기준으로 preview `ChangeSet(change_set_kind=recipe_preview)`을 만든다.
11. operation이 accepted planned scope·Recipe path scope·target selector에 모두 속하는지 확인한다.
12. preview ChangeSet을 M2 impact·risk·Profile closure·affected selector에 다시 넣어 기존 ChangePlan·ValidationPlan과 reconcile한다.
13. 새 path·change class·risk·Check·fallback floor가 생기면 candidate를 `replan_required`로 invalidated하고 새 plan 승인 뒤 prepare를 다시 시작한다. 실행 중 plan을 확장하지 않는다.
14. 같은 Recipe·input·tool identity를 expected-after preview에 다시 실행해 idempotence를 판정한다.
15. forward·reverse operation artifact, exact expected-after manifest와 diff를 finalize한다.
16. `RecipeExecution`과 immutable `PatchSet`을 commit하고 사용자에게 PatchSet·diff·영향·검사·worktree·remaining risk를 먼저 표시한다.

1~6 실패에서는 preview workspace나 external process를 만들지 않는다. 7~14 실패에서 target source는 여전히 바뀌지 않는다. 실패한 preview workspace는 evidence·retention 정책에 따라 격리하고 apply 대상으로 승격하지 않는다.

## preview impact reconciliation

M2 초기 계획은 사용자 의도와 current dirty ChangeSet을 기준으로 한다. Recipe가 만든 실제 preview가 더 정확한 change class를 드러낼 수 있으므로 final PatchSet 전에 다음을 다시 확인한다.

- preview operation의 path·symbol·contract·generated owner가 planned unit과 매핑되는가
- 실제 add·modify·delete·rename이 intended operation과 일치하는가
- initial ImpactAnalysis에 없던 direct/transitive edge·risk path가 생겼는가
- `refactor_codemod`, `test_correctness`, `architecture_quality` Profile closure가 그대로 충분한가
- selected format·build·test·contract Check와 fallback scope가 preview change를 모두 관찰하는가
- completion criterion이 expected postcondition과 CheckPlan에 연결되는가

reconciliation은 M4가 새 영향 engine을 만드는 과정이 아니다. 같은 `star-planning`·`star-validation/selector`를 preview ChangeSet input으로 다시 호출한다. 결과가 달라지면 기존 RecipeExecution·candidate diff는 evidence일 뿐 apply 가능한 PatchSet이 아니며, 새 M2 plan revision 뒤 새 prepare가 필요하다.

## PatchSet v2 target

M4 writer는 `star.patch-set` `schema_version=2`를 사용한다. PatchSet은 적용 전 확정되는 immutable proposal이며 runtime status를 갱신하지 않는다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `patch_set_id`, 공통 envelope | 예 | immutable instance와 producer |
| `change_plan_ref`, `planned_change_unit_refs` | 예 | accepted M2 plan exact revision |
| `project_id`, `target_checkout_id` | 예 | 단일 Project·Checkout |
| `base_project_revision_id`, `base_workspace_snapshot_id` | 예 | target before identity |
| `base_workspace_content_fingerprint` | 예 | 비교 scope actual byte |
| `preexisting_change_set_ref`, `preexisting_manifest_fingerprint` | 예 | 사용자 변경 보존 기준 |
| `recipe_refs`, `recipe_execution_refs` | 예 | exact Recipe·preview·idempotence lineage |
| `resolved_target_bindings` | 예 | selector resolution과 before identity |
| `preview_change_set_ref`, `impact_analysis_ref` | 예 | 실제 preview diff와 reconciled 영향 |
| `validation_plan_refs` | 예 | pre/post phase plan과 Profile closure |
| `worktree_decision` | 예 | current apply 가능성 또는 isolated target |
| `operations` | 예 | deterministic ordered file operation manifest |
| `expected_after_manifest` | 예 | path·hash·mode·existence 전체 |
| `forward_artifact_refs`, `reverse_artifact_refs` | 예 | apply와 복구 byte |
| `expected_postconditions` | 예 | Recipe predicate와 평가 방법 |
| `idempotence_evaluation` | 예 | first run·replay ref와 `noop\|proved\|failed\|unverified` |
| `permission_requirements`, `risk_refs` | 예 | apply·delete·move·dependency 등 action |
| `preview_completeness` | 예 | `complete\|partial\|unverified`; auto apply는 complete만 |
| `patch_fingerprint` | 예 | 위 의미 field와 artifact hash의 JCS SHA-256 |

PatchSet은 `applied`, `failed`, `reverted` 같은 mutable 상태를 갖지 않는다. historical v1 status는 read-only 표시와 migration evidence로만 유지하고 실제 M4 lifecycle은 `PatchApplication`이 소유한다.

### PatchOperation

각 operation은 다음을 가진다.

- stable `operation_id`와 ChangePlan unit·Recipe·selector binding ref
- `kind=add|modify|delete|rename`
- source와 destination ProjectPathRef
- before·after existence, full content SHA-256, size, mode와 source class/facet
- rename이면 source/destination collision과 content identity
- forward/reverse content 또는 delta ArtifactRef
- generated owner·contract·symbol ref와 assurance kind
- preexisting range overlap result
- `atomic_group_id`와 deterministic apply order
- permission ActionId와 required postcondition ref

binary, symlink·junction·reparse point, submodule, alternate data stream과 unsupported mode는 일반 operation으로 축약하지 않는다. 해당 adapter·permission·rollback contract가 별도 설계되기 전에는 M4가 거부한다.

## 사용자에게 먼저 표시할 preview

`patch.prepare` 성공 출력은 최소 다음 순서를 가진다.

1. Recipe ID·version·definition fingerprint와 transformer kind
2. Project·Checkout·base revision·dirty policy와 WorktreeDecision
3. resolved target selector, index tier·coverage·limitation
4. add·modify·delete·rename file 수와 path 목록
5. redaction한 unified diff 또는 binary 미지원 이유
6. initial 대비 reconciled direct/transitive impact와 risk path
7. selected format·build·test·contract Check와 fallback scope
8. idempotence 결과와 expected postcondition
9. apply permission, rollback 방식과 remaining risk
10. PatchSet ID와 exact fingerprint

diff나 impact가 너무 크면 전체 text를 stdout에 강제로 넣지 않고 hash가 있는 ArtifactRef를 제공한다. 요약만 보여 주더라도 operation count·unexpected path·delete/rename·unverified limitation을 숨기지 않는다.

## base revision·dirty state·worktree 결정

`WorktreeDecision`은 `strategy=current_checkout|isolated_worktree|blocked`, reason code, target checkout, base revision, dirty manifest, overlap result, required permission과 cleanup policy를 가진다.

| 상태 | prepare | current checkout apply | isolated worktree |
|---|---|---|---|
| clean, exact base | 허용 | pre Gate 뒤 허용 | optional, external mutator preview에는 필수 |
| dirty지만 target·range·rename·generated owner와 disjoint가 complete하게 증명됨 | 허용 | 대상 밖 byte invariant를 유지할 때만 허용 | 허용 |
| dirty overlap | preview는 clean base 의미가 사용자 의도와 맞는 경우만 | 금지 | 사용자가 clean-base 결과를 별도 worktree에 받는 선택을 했을 때만 |
| dirty change가 target 의미·parse·generator input에 필요 | current exact snapshot용 materialized preview만 | exact non-overlap을 증명하지 못하면 금지 | clean worktree로 재현 불가하므로 기본 block |
| status collection partial·unverified | 금지 | 금지 | 금지 |
| base revision·config·Catalog·Index drift | stale, 새 prepare | 금지 | 새 worktree decision 필요 |
| non-Git Project | materialized preview capability가 있으면 허용 | exact atomic/recovery adapter가 구현된 범위만 | Git worktree 없음; 지원을 합성하지 않음 |

overlap은 file path equality만 보지 않는다.

- 같은 file byte/range와 line-ending rewrite
- rename source·destination과 case-only rename
- delete 대상과 사용자 add/modify
- generated input·output ownership
- formatter가 넓히는 file set
- contract/symbol selector가 가리키는 같은 declaration
- path ancestor·directory move와 reparse boundary

overlap 판정이 `unknown`이면 disjoint가 아니다. 사용자 기존 byte를 Recipe expected-before로 조용히 바꾸거나 isolated clean base에 복제하지 않는다.

## Recipe idempotence와 재실행 판정

Recipe semantic idempotence와 application command idempotency key는 다른 계약이다.

### semantic idempotence

`idempotency_contract`는 다음을 선언한다.

- `mode=required_replay_noop|postcondition_probe_only|manual_only`
- already-satisfied target 판정 방법
- first-run expected-after를 replay input으로 만드는 방법
- no-op의 정의: operation 0건 + 모든 postcondition true
- external tool·generator의 deterministic output manifest 조건
- replay resource limit과 failure 처리

automatic apply는 기본 `required_replay_noop`만 허용한다.

1. first preview가 operation 0건이고 postcondition이 모두 true면 PatchSet은 `noop` 결과로 렌더링하며 apply하지 않는다.
2. first preview가 변경을 만들면 exact expected-after snapshot에서 같은 Recipe·input·target·tool identity를 다시 실행한다.
3. replay가 operation 0건이고 postcondition이 true면 `proved`다.
4. replay가 다시 변경을 만들면 `failed`, process·coverage를 확인하지 못하면 `unverified`다.
5. `failed|unverified|manual_only` Recipe는 preview·human review에는 사용할 수 있지만 automatic apply·automatic completion에는 사용할 수 없다.

### command idempotency

- 같은 `patch.apply` idempotency key와 같은 canonical input은 이미 terminal `PatchApplication`이 있으면 그 ref를 반환한다.
- 같은 key에 다른 PatchSet·fingerprint·target·approval은 `MANAGEMENT_IDEMPOTENCY_CONFLICT`다.
- partially applied, outcome unknown과 recovery-required application은 성공 replay 대상이 아니다. 먼저 actual state를 reconcile한다.
- Recipe가 semantic idempotent여도 timeout·crash 뒤 external tool이나 source apply를 자동 재실행하지 않는다.
- already-applied expected-after를 발견하면 새 write를 하지 않고 기존 receipt와 source lineage를 확인해 `already_applied_verified` 또는 `state_conflict`로 분류한다.

## 적용 전 Gate

`patch.apply`는 [M3 `patch_pre_apply`](../features/common-validation-gate.md#적용-전-patch_pre_apply)를 먼저 실행한다. 다음을 모두 만족해야 in-memory `PatchApplyPermit`을 만들 수 있다.

1. PatchSet v2 Schema·fingerprint·artifact hash가 valid다.
2. ChangePlan·ScopeRevision·ImpactAnalysis·ValidationPlan lineage가 current·accepted다.
3. Project·Checkout·base revision·WorkspaceSnapshot과 full dirty manifest가 current probe와 같다.
4. 모든 operation before hash·mode·existence와 final path identity가 같다.
5. preexisting change와 overlap이 없고 대상 밖 manifest가 보존 가능하다.
6. Recipe·Catalog·config·Index·Tool fingerprint가 preview 때와 compatible하다.
7. idempotence가 `proved`이고 preview completeness가 `complete`다.
8. delete·rename·generated·dependency·validator·contract 등 위험 action이 PermissionPlan에 있다.
9. forward·reverse artifact를 읽고 hash를 검증할 수 있다.
10. pre-apply GateDecision이 `auto_pass`, 또는 exact PatchSet fingerprint에 대한 사용자 승인과 policy가 허용한 `human_review`다.

`PatchApplyPermit`은 GateDecision ref, PatchSet fingerprint, before binding set, permission/approval fingerprint, target checkout lock identity와 `automatic|manual_approved` kind를 가진다. 직렬화하거나 다른 apply에 재사용하지 않는다.

## PatchApplication과 apply 알고리즘

`PatchApplication`은 `star.patch-application` v1 persisted document다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `patch_application_id`, `revision` | 예 | application lifecycle |
| `patch_set_ref`, `patch_fingerprint` | 예 | immutable proposal |
| `project_id`, `target_checkout_id`, `worktree_decision` | 예 | 실제 target |
| `pre_apply_gate_ref`, `permit_binding_fingerprint` | 예 | source effect 권한 근거. permit token 자체는 저장 금지 |
| `application_state` | 예 | 아래 state machine |
| `operation_receipts` | 예 | started/completed operation과 actual before/after identity |
| `actual_operation_manifest` | 해당 시 | 실제 add·modify·delete·rename |
| `applied_workspace_snapshot_ref`, `observed_change_set_ref` | effect 뒤 | actual source state |
| `post_apply_gate_ref` | Gate 뒤 | final validation decision |
| `reverse_patch_set_ref`, `recovery_state` | 예 | rollback 가능성과 현재 상태 |
| `tool_effect_refs` | 예 | apply 자체는 external codemod를 호출하지 않으므로 기본 empty |
| `event_range`, `evidence_refs` | 예 | durable intent·receipt·failure·recovery |

apply 순서는 다음과 같다.

1. target Checkout exclusive mutation lock을 얻는다.
2. permit binding과 모든 precondition을 lock 안에서 다시 확인한다.
3. operation별 after byte와 reverse byte를 Controller-owned same-volume staging area에 materialize하고 hash를 확인한다.
4. operation journal과 `effect.requested`를 source effect 전에 durable commit한다.
5. path·reparse·file identity handle을 다시 확인하고 대상 밖 preexisting manifest baseline을 고정한다.
6. rename collision을 피할 temporary reservation plan, add/modify replacement, delete-last 순서를 deterministic하게 만든다.
7. 각 operation 시작 전 intent, 완료 직후 actual receipt를 기록한다.
8. 한 operation이라도 실패하면 남은 forward operation을 시작하지 않는다.
9. 전체 forward operation 뒤 workspace를 새로 수집해 PatchSet expected-after와 actual manifest를 비교한다.
10. 대상 밖 preexisting byte가 전과 같은지 확인한다.
11. `patch_post_apply` ValidationPlan을 exact after binding에서 실행한다.
12. GateDecision·EvidenceBundle·ReviewPack이 complete하게 commit된 뒤에만 `validated` 또는 `completed` projection을 만든다.

Windows filesystem은 여러 file 전체를 하나의 원자 transaction으로 보장하지 않는다. 구현은 per-path atomic replacement, 사전 staging, journal과 exact reverse precondition으로 위험을 줄이되 multi-file atomicity를 거짓으로 주장하지 않는다.

## partial apply와 복구

state machine은 최소 다음을 가진다.

```text
requested
  -> preflighted
  -> applying
       -> applied
            -> post_gating
                 -> validated_auto_pass
                 |  awaiting_human_review
                 |  recovery_required
       |  failed_before_effect
       |  partially_applied
       |  outcome_unknown

partially_applied | outcome_unknown | recovery_required
  -> reconciling
       -> reverse_ready -> reversing -> reverted
       |  isolated_discard_ready -> discarded
       |  rollback_blocked
```

- `failed_before_effect`만 source 불변을 단정할 수 있다.
- `partially_applied`는 operation receipt 일부가 성공했거나 actual manifest가 expected set의 proper subset/superset일 때다.
- `outcome_unknown`은 crash·timeout·I/O 결과 유실 때문에 effect 여부를 확정할 수 없을 때다.
- 이 상태를 `failed` 하나로 줄이거나 새 apply로 덮지 않는다.
- startup recovery는 journal, actual file identity·hash와 WorkspaceSnapshot을 대조해 먼저 reconcile한다.
- 자동 reverse는 완료 receipt의 exact after hash가 아직 같고, 실패 뒤 사용자·다른 process 변경이 없으며, 대상 밖 manifest가 같을 때만 허용한다.
- reverse는 completed operation의 역순으로 실행하는 별도 reverse PatchSet이다. 원본 directory 전체 삭제나 Git hard reset이 아니다.
- reverse precondition을 만족하지 못하면 byte를 덮지 않고 `rollback_blocked`와 manual recovery ArtifactRef를 남긴다.
- isolated worktree 결과는 primary checkout을 바꾸지 않았으면 merge하지 않고 보존·폐기할 수 있다. 폐기도 exact owned root와 정책 승인을 요구하며 primary source 삭제로 처리하지 않는다.
- delete operation의 original byte는 reverse artifact가 finalize된 뒤에만 forward delete를 시작한다.

## 적용 후 Gate와 검사 연결

Patch engine은 어떤 검사를 실행할지 자체적으로 고르지 않는다.

1. Recipe의 `validation_requirements`는 M2 candidate family의 최소 floor다.
2. M2는 target·preview ChangeSet·ImpactAnalysis·RiskPath·Profile closure에서 exact CheckPlan과 scope를 고정한다.
3. M3 pre Gate는 apply 가능성을 검사한다.
4. apply 뒤 M3는 새 `observed_after_change` ChangeSet과 expected-after를 비교한다.
5. M2가 선택한 format·lint/static·build/compile·test·contract·docs·generated·security Check를 post binding에서 실행한다.
6. B01은 PatchSet operation, actual operation, preexisting 보존과 CompletionClaim을 비교한다.
7. B02는 test/case·assertion 약화와 required regression pair를 확인한다.
8. B04는 symbol/contract/public boundary, hardcoding drift, generated owner와 codegen reproducibility를 확인한다.
9. actual change class가 plan과 다르면 runner가 검사 추가를 하지 않고 `VALIDATION_PROFILE_CLOSURE_STALE`로 replan/recovery한다.
10. required `not_run|partial|unverified|stale|flaky`는 자동 완료 근거가 아니다.

formatter나 generator가 post Check 중 source를 다시 바꾸는 설정은 기본 허용하지 않는다. source-changing formatter·generator는 prepare preview에 포함해 PatchSet operation으로 고정해야 한다. post Check는 검사-only 또는 declared build output effect만 가진다.

## 외부 codemod CLI 경계

특정 language tool을 Star-Control core dependency로 고정하지 않는다. 외부 codemod는 다음 두 integration mode만 허용한다.

| mode | 실행 위치 | 결과 |
|---|---|---|
| `structured_patch_producer` | materialized input·artifact root 또는 isolated worktree read view | output Schema를 통과한 candidate operation/data |
| `isolated_workspace_mutator` | exact base의 isolated preview worktree | tool report + Controller가 다시 수집한 full filesystem diff |

두 mode 모두 current ToolRegistrySnapshot, ToolDescriptor ID/version/hash, executable identity/version/full hash, typed arguments, cwd anchor, environment, timeout, output limit과 permission을 고정한다.

M4 automatic Recipe에 사용할 external action은 다음 조건을 모두 만족해야 한다.

- ToolPackageManifest와 input/output Schema가 valid·trusted다.
- `argv_v1` argument binding 또는 `star_json_stdio_v1` structured request만 사용한다.
- shell, `.cmd`, `.bat`, `.ps1`, command string과 PATH lookup을 사용하지 않는다.
- target은 ProjectPathRef·ArtifactRef 같은 typed input으로 전달한다.
- mutator는 `working_directory=stage_worktree`, `local_write`와 실제 side effect를 선언한다.
- live target Checkout path를 input·cwd·environment에 전달하지 않는다.
- network, external write, account, paid, system change와 secret access를 요구하지 않는다.
- automatic result는 structured output Schema와 complete filesystem diff 둘 중 필요한 계약을 모두 통과한다.
- output 밖 file·artifact와 undeclared child effect를 성공으로 흡수하지 않는다.
- tool·adapter의 own retry를 manifest가 숨기지 않고 실행 attempt evidence에 드러낸다.

### M11 Rust style fixed adapter 소비

[Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)은 새 Patch engine·RecipeExecution·PatchApplication을 만들지 않고 위 `isolated_workspace_mutator`와 immutable PatchSet 경계를 소비한다. `rust_style_v1@1`은 Catalog의 일반 사용자 Recipe가 아니라 ordered step·Tool role·argument/output/side-effect policy가 동결된 bounded application adapter다.

M11 prepare는 다음 ref를 기존 M4 record에 연결한다.

- `RecipeExecution`: `rust_style_auto_fix` Profile/ref, `rust_style_v1` fixed adapter fingerprint, RustToolchainBinding·RustStylePolicySnapshot·RustStyleCoverageMatrix와 ordered RustStyleStepExecution ref
- `output_artifact_refs`: raw/normalized Diagnostic, selected/nonselected suggestion, suggestion-to-hunk mapping, step별 diff와 complete final filesystem manifest/diff
- `observed_preview_change_set_ref`: final handwritten `.rs` modify candidate만 포함한 ChangeSet. 거부된 side effect를 삭제해 만든 sanitized diff가 아님
- `postcondition_evaluations`: allowed operation, coverage complete, exact hunk mapping, M2 impact reconciliation, full-pipeline replay no-op와 candidate Check 결과
- `PatchSet.recipe_execution_refs`: first-run/replay 및 모든 step evidence fingerprint, tool/config/policy/coverage binding

`cargo fmt -> cargo clippy --fix -> cargo fmt`의 중간 filesystem state를 여러 PatchSet으로 나누지 않는다. step diff는 evidence이고 최종 complete diff만 한 immutable PatchSet operation 집합이 된다. 중간 step에서 `Cargo.lock`, config, generated/vendor/out-of-scope file 또는 허용 suggestion과 대응하지 않는 hunk가 생기면 뒤 step이 원상복구했더라도 `RUST_STYLE_SIDE_EFFECT_VIOLATION`이며 candidate 전체를 거부한다.

Clippy coverage cell별 mutator는 first-rustfmt state의 독립 preview에서 실행하고 actual replacement가 byte-exact 같을 때만 deterministic하게 reconcile한다. 서로 반대 suggestion, overlapping replacement 또는 실행 순서에 따라 결과가 달라지면 M4가 한쪽을 고르지 않고 PatchSet을 만들지 않는다. replay는 final source에서 전체 fixed mutation pipeline을 다시 실행해 operation 0을 요구한다.

M11 external mutator는 live target에 절대 실행하지 않는다. apply phase는 cargo/rustfmt/Clippy를 재실행하지 않고 이미 만들어진 PatchSet operation만 SourceMutationPort로 적용한다. post Check도 target actual-after의 exact-byte isolated validation mirror에서 source-changing formatter/fix가 아닌 fixed `cargo fmt <typed-scope> -- --check`와 Clippy Diagnostic/build/test 검사 mode만 사용하고, mirror/target subject binding을 대조한다.

`personal_auto`는 `patch apply --fingerprint`의 exact 승인 불변식을 우회하지 않는다. standing grant는 candidate ceiling일 뿐이고, prepare 뒤 policy evaluator가 exact PatchSet fingerprint·Checkout·action·toolchain/policy/coverage/evidence에 묶인 기존 ApprovalRequest를 `decision=approved`, `resolved_by=policy_evaluator`로 해소한다. M3 `patch_pre_apply=AUTO_PASS` 뒤에만 `PatchApplyPermit(kind=automatic)`을 발급하며 permit과 PatchApplication은 일반 `patch.apply` 경로를 그대로 사용한다. `safe_default`는 사용자 exact approval 없이는 apply하지 않는다.

### process·output 실패 처리

| 상태 | RecipeExecution | PatchSet 생성 | target source |
|---|---|---|---|
| success + valid complete output | `completed` | scope·impact·idempotence 통과 뒤 가능 | 불변 |
| declared warning + valid output | `completed_with_diagnostics` | policy가 허용해도 최소 human review | 불변 |
| process start 실패 | `failed_before_start` | 금지 | 불변 |
| non-success exit/status error | `failed` | 금지 | 불변 |
| timeout | `timed_out`, process tree 종료·outcome 확인 | 금지 | 불변. isolated preview만 suspect |
| user cancellation | `cancelled` | 금지 | 불변. isolated preview evidence 보존 |
| malformed JSON·JSONL·frame·Schema | `protocol_invalid` | 금지 | 불변 |
| stdout/stderr/output limit 초과 | `output_incomplete` | 금지 | 불변 |
| Controller crash·final frame 유실 | `outcome_unknown` | 금지 | 불변. preview workspace reconcile 뒤 폐기/격리 |
| undeclared file 변화 | `side_effect_violation` | 금지 | 불변. preview diff evidence 보존 |

외부 tool이 실패했더라도 isolated preview에서 보인 diff를 “유용해 보인다”는 이유로 PatchSet으로 승격하지 않는다. 자동 retry도 하지 않는다. 사용자가 다시 시도하면 새 RecipeExecution ID·attempt·evidence를 만든다.

## RecipeExecution evidence

`RecipeExecution`은 `star.recipe-execution` v1 immutable attempt document다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `recipe_execution_id` | 예 | preview 또는 replay attempt ID |
| `mode`, `attempt` | 예 | `preview\|idempotence_replay`, attempt 번호 |
| `recipe_ref`, `definition_fingerprint` | 예 | exact Recipe |
| `change_plan_ref`, `target_bindings` | 예 | 계획·selector resolution |
| `base_subject_binding` | 예 | Project·Checkout·Revision·Workspace·config·Catalog·Index |
| `normalized_input_fingerprint` | 예 | redacted typed input hash |
| `transformer_binding` | 예 | built-in adapter ID/version/hash 또는 ToolDescriptor·Registry·EXE identity |
| `task_invocation_ref` | external일 때 | typed args·cwd·env·timeout·output limit |
| `preview_workspace_ref` | 예 | materialized/isolated root의 opaque identity·base fingerprint |
| `process_result` | external일 때 | start state, exit/status, termination, completeness |
| `input_artifact_refs`, `output_artifact_refs` | 예 | hash·size·redaction 상태 |
| `observed_preview_change_set_ref` | effect 관찰 뒤 | 실제 preview diff |
| `postcondition_evaluations` | 예 | predicate별 true/false/unknown과 evidence |
| `outcome`, `limitations` | 예 | 성공·실패·cancel·protocol·coverage |
| `execution_fingerprint` | 예 | 모든 의미 input/output의 canonical hash |

표시용 timestamp와 duration은 evidence에 남기지만 execution identity의 의미 hash와 분리한다. raw 개인 absolute path, secret와 민감 literal은 저장하지 않는다.

## EvidenceBundle·ReviewPack 연결

M4 EvidenceBundle은 기존 M3 field에 다음 ref를 추가한다.

- exact Recipe ID·version·definition fingerprint와 Catalog source
- 모든 RecipeExecution preview·idempotence attempt
- transformer adapter 또는 Tool Registry/ToolDescriptor/executable version·hash
- redacted typed input fingerprint와 input/output ArtifactRef manifest
- resolved selector·Index tier·coverage·limitation
- WorktreeDecision과 preview workspace provenance
- PatchSet v2, forward/reverse artifact와 preview ChangeSet·impact reconciliation
- pre-apply Gate, PatchApplication operation receipt와 actual ChangeSet
- partial/outcome unknown/recovery·reverse PatchSet 또는 isolated discard evidence
- post-apply Gate와 format·build·test·contract Check 결과

ReviewPack은 `planned_vs_actual_changes`에서 Recipe/selector, PatchSet preview, actual apply, preexisting 보존과 recovery 상태를 같은 순서로 보여 준다. external tool version/hash, idempotence 실패와 unverified limitation을 숨기지 않는다.

## permission·보안 경계

| 행동 | 최소 ActionId 예 | 기본 통제 |
|---|---|---|
| Recipe/Catalog 조회·selector resolution | `local_read` | 자동 가능 |
| built-in materialized preview | `local_read`, preview root `local_write` | target effect 없음 |
| external preview process | `process_run`, 필요 시 preview root `local_write` | trusted ToolDescriptor·isolated cwd |
| isolated Git worktree 생성 | `local_write`, `process_run`, `plan_execute` | exact Project·base·owned root |
| current checkout Patch apply | `local_write`, operation별 `local_delete\|local_mass_move\|dependency_change` | pre Gate·permit·lock 필수 |
| reverse PatchSet | 원 forward와 동등한 write/delete/move action | 새 precondition·PermissionPlan |
| isolated worktree 폐기 | exact owned root `local_delete` | evidence 보존·정책 승인 |

- `safe_default`와 `personal_auto` 모두 dry-run·pre/post Gate·evidence를 건너뛸 수 없다.
- 별도 prompt 여부는 policy가 정하지만 `prepare`와 `apply`는 별도 command다.
- approval은 PatchSet fingerprint, target Checkout, action set, worktree strategy와 expiry를 포함한다.
- PatchSet·source·plan·tool identity가 바뀌면 이전 approval을 재사용하지 않는다.
- preview·diff·stdout·stderr·artifact에 secret 후보가 있으면 저장·표시 전 redaction한다. 안전하게 redaction할 수 없으면 PatchSet completeness를 낮추고 자동 apply를 막는다.
- executable trust는 local code trust다. `trusted_desktop`이 filesystem sandbox라는 주장을 하지 않는다.
- Recipe path scope와 Tool permission 선언은 malicious EXE를 스스로 가두지 못하므로 external mutator에는 isolated preview root만 준다.

## CLI-only application 계약

CLI와 이후 Codex adapter는 같은 typed application command를 호출한다. CLI handler가 DB·Git·source·Tool process를 직접 다루지 않는다.

### command surface

```text
star recipes list [--language <id>] [--rewrite-kind <kind>] [--json]
star recipes describe <recipe-id>@<semver> [--json]
star recipes validate <recipe-file> [--json]

star change prepare
  --project <project-id>
  --checkout <checkout-id>
  --recipe <recipe-id>@<semver>
  --target-file <selector.json>
  --parameters <input.json>
  [--task-spec <document-ref>]
  [--change-plan <document-ref>]
  [--workspace auto|current|isolated]
  [--json]

star patch show <patch-set-id> [--diff] [--impact] [--json]
star patch apply <patch-set-id> --fingerprint <sha256> [--workspace current|isolated] [--json]
star patch status <patch-application-id> [--json]
star patch recover <patch-application-id> --strategy reverse-patch|discard-isolated [--json]
```

exact public syntax는 구현 Slice에서 fixture와 함께 동결하되 의미는 다음을 지켜야 한다.

- `change prepare`에는 `--apply`가 없다.
- `patch apply`에는 PatchSet ID와 exact fingerprint가 모두 필요하다.
- selector와 parameters는 shell string이 아니라 Schema를 통과한 JSON document다.
- `--task-spec`·`--change-plan`이 없으면 CLI convenience use case가 명시적 objective·Recipe postcondition·target으로 `task.create -> scope.resolve -> changes.collect -> impact.analyze -> affected.select -> change.plan`을 같은 application service에서 결정적으로 호출한다. 필요한 사용자 입력이 없으면 AI로 채우지 않고 중단한다.
- `patch recover`는 실제 상태 reconciliation 뒤 제시된 strategy만 실행하고 `reset --hard` 같은 숨은 fallback을 갖지 않는다.
- 여러 Project target이 selector에 들어오면 prepare 전에 거부한다.

CLI-only dependency graph에는 Codex, App Server, 다른 AI provider와 OpenAI API client가 없다. 이후 Codex는 `ManagementApplicationService.change_prepare`, `patch_apply`, `patch_status`, `patch_recover`를 호출하는 entry adapter일 뿐이다.

## application·Package·adapter 경계

```text
star-application
  -> star-project: current snapshot·selector resolution·preview 재관찰
  -> star-planning: M2 impact·risk·ChangePlan reconciliation
  -> star-validation: affected selector + pre/post Gate
  -> star-execution: Recipe prepare·idempotence·Patch apply/recovery state machine
  -> star-vcs: single-project WorktreeDecision·isolated worktree lifecycle
  -> star-policy: permission·approval
  -> star-state/star-evidence: document·event·artifact transaction
  -> ports
       - RewriteTransformerPort
       - SourceMutationPort
       - WorktreePort
       - ToolExecutorPort
       - ProjectObserverPort
```

- `star-execution`은 concrete filesystem, Git, parser library와 external CLI를 직접 import하지 않는다.
- `RewriteTransformerPort`는 bounded source/selector/input을 candidate after byte·typed transformation evidence로 바꾼다.
- `SourceMutationPort`는 exact before/after operation, per-path atomic receipt와 reverse precondition만 다룬다. 영향·permission·Gate를 결정하지 않는다.
- `WorktreePort`는 한 Project의 exact base worktree create/inspect/discard만 M4에 제공한다. merge·commit·multi-project coordination은 제공하지 않는다.
- `ToolExecutorPort`는 Tool Registry의 typed invocation과 Windows Runtime 결과만 반환한다. PatchSet·GateDecision을 만들지 않는다.
- language-specific transformer는 private adapter 또는 trusted external tool이다. core contract는 AST/library type을 알지 않는다.
- `star-application`만 current probe, plan revision, Gate, permit, source effect와 repository/evidence transaction 순서를 조정한다.
- CLI·MCP·Codex adapter는 PatchSet을 직접 적용하거나 GateDecision을 재해석하지 않는다.

## 5단계 managed Registry 인계

5단계는 상수·오류 코드·설정 key·contract member 같은 managed declaration의 source-of-truth와 drift를 관리하되 별도 DB-to-source sync engine을 만들지 않는다. exact 분류, manifest wire, lifecycle와 consumer compatibility는 [Managed Registry 정본](managed-symbol-registry.md)이 소유하고 이 절은 M4 mutation seam만 고정한다.

```text
사용자: ManagedDeclarationChangeIntent + expected source/declaration fingerprint
  -> M1: declaration owner Symbol·Contract·binding·consumer resolution
  -> M2: lifecycle·호환 영향·risk·affected Check·ChangePlan
  -> M4: managed_declaration selector를 받는 ChangeRecipe
  -> PatchSet preview·diff·impact
  -> M3 pre Gate -> M4 PatchApplication -> M3 post Gate
```

M4가 5단계를 위해 보장해야 하는 public seam은 다음과 같다.

- `TargetSelector.kind=managed_declaration`
- stable declaration ID와 expected declaration fingerprint
- contract/member와 owning Symbol·source mapping evidence
- typed intended postcondition
- 한 Project PatchSet operation과 reverse artifact
- contract·docs·generated·consumer Check를 M2/M3에 연결하는 risk/validation ref
- authoritative manifest root·fragment와 namespace/tombstone before/expected-after fingerprint
- generated output은 `generator_input`/`codegen` operation만 사용하고 직접 target하지 않는 provenance
- 같은 `ManagementApplicationService` command

5단계 Registry가 raw literal을 source 전체에서 찾아 바꾸거나 관리 DB row로 source를 직접 overwrite하면 구조 위반이다. source와 DB Index가 다르면 Git manifest를 우선하고 Patch candidate를 stale로 만든다. 한 Project PatchSet만 적용하며 cross-project consumer 영향은 read-only이고 실제 cross-repo apply는 9단계 전까지 거부한다.

## stable error와 event

4단계는 다음 error family를 사용한다. exact 사용자 message와 ErrorEnvelope는 [오류와 진단 계약](errors-and-diagnostics.md)이 소유한다.

- `RECIPE_CONTRACT_INVALID`
- `RECIPE_VERSION_CONFLICT`
- `RECIPE_INPUT_INVALID`
- `RECIPE_TARGET_UNRESOLVED`
- `RECIPE_TARGET_AMBIGUOUS`
- `RECIPE_LANGUAGE_UNSUPPORTED`
- `RECIPE_ASSURANCE_UNSATISFIED`
- `RECIPE_IDEMPOTENCE_FAILED`
- `PATCH_PREVIEW_INCOMPLETE`
- `PATCH_SCOPE_VIOLATION`
- `PATCH_DIRTY_OVERLAP`
- `PATCH_REPLAN_REQUIRED`
- `PATCH_PRECONDITION_FAILED`
- `PATCH_PARTIAL_APPLY`
- `PATCH_OUTCOME_UNKNOWN`
- `PATCH_POSTCONDITION_FAILED`
- `PATCH_RECOVERY_BLOCKED`

외부 process는 기존 `TOOL_PROCESS_START_FAILED`, `TOOL_TIMEOUT`, `TOOL_OUTPUT_LIMIT`, `TOOL_PROTOCOL_INVALID`, `TOOL_OUTCOME_UNKNOWN`을 그대로 사용한다. tool error를 Recipe success로 재분류하지 않는다.

최소 event 흐름은 다음이다.

```text
recipe.execution_started
recipe.execution_finished | recipe.execution_failed
patch.previewed
patch.replan_required
patch.apply_requested
patch.preflighted
effect.requested
patch.operation_recorded*
patch.applied | patch.partially_applied | patch.outcome_unknown
patch.post_gate_completed
patch.recovery_requested
patch.reverted | patch.isolated_discarded | patch.rollback_blocked
```

event에는 큰 diff·source byte·tool output을 inline으로 넣지 않고 document/ArtifactRef와 fingerprint만 둔다.

## 구현·migration 순서

제품 구현은 다음 순서를 바꾸지 않는다.

1. ChangeRecipe v2, PatchSet v2, TargetSelector, RecipeExecution v1, PatchApplication v1, WorktreeDecision과 nested operation type·Schema
2. P0 Recipe/PatchSet v1 historical reader, migration dry-run·backup·rollback과 invalid/future-version fixture
3. stable ID/version/fingerprint, input Schema, selector·postcondition invariant golden
4. fake current Project observer와 M1 selector resolution conformance
5. text/syntax/symbol/codegen assurance decision table과 unsupported fallback corpus
6. materialized preview, preview ChangeSet, M2 impact/Profile/affected reconciliation fake vertical slice
7. Recipe idempotence replay와 already-satisfied/no-op corpus
8. fake ToolExecutor의 success·warning·start failure·timeout·cancel·malformed·output limit·outcome unknown conformance
9. single-project WorktreeDecision과 fake isolated worktree create/retain/discard conformance
10. PatchSet forward/reverse artifact, dirty overlap과 target-outside detection
11. fake SourceMutationPort의 per-path receipt, fail-before-effect·partial·outcome unknown·reverse recovery corpus
12. real M3 `patch_pre_apply`·`patch_post_apply` binding과 EvidenceBundle·ReviewPack 연결
13. CLI-only `recipe -> prepare -> show -> apply -> status/recover` E2E, AI dependency 0 확인
14. Windows x64·ARM64, file lock·case-only rename·long path·line ending·encoding·crash recovery conformance

언어별 concrete transformer와 외부 codemod 선택은 1~13 공통 contract가 통과한 뒤 별도 Slice로 추가한다. 특정 언어 tool을 공통 contract보다 먼저 core dependency로 고정하지 않는다.

## 최소 Corpus

- 같은 raw literal, 다른 SymbolId·ContractId·Project ownership
- explicit path/range 1건과 accidental global occurrence 다수
- syntax node는 같지만 binding이 다른 identifier
- semantic current/complete와 stale/partial/dynamic reference frontier
- generated input 1개·output 여러 개와 undeclared output
- clean, dirty-disjoint, dirty-overlap, partial status와 base drift
- add·modify·delete·rename·case-only rename과 rename collision
- Recipe first run no-op, replay no-op, oscillating transform, nondeterministic generator
- external tool success, warning, start failure, nonzero, timeout, cancel, malformed JSON, double final frame, output overflow, crash
- preview scope 밖 file 변화와 external mutator의 live target path 요청
- apply 0번째·중간·마지막 operation 실패와 Controller crash
- partial apply 뒤 user edit가 있어 reverse precondition이 깨진 상태
- exact reverse PatchSet 성공과 isolated worktree discard
- pre Gate 뒤 source/config/Catalog/Tool/approval drift
- post apply actual extra file·missing file·wrong hash·preexisting byte 변화
- format/build/test/contract Check `pass|fail|not_run|partial|stale|flaky`
- CLI-only 실행 중 Codex·AI adapter 0개
- 둘 이상의 Project target 거부와 read-only downstream impact 보존
- 5단계 `managed_declaration` selector가 한 Project PatchSet으로 표현되는 fixture

## 설계 수용 기준

- scan/index와 rewrite/apply가 다른 phase·port다.
- `prepare`에서 live target을 즉시 변경하는 기본·숨은 경로가 없다.
- raw literal만으로 Project-wide 또는 cross-project replacement를 만들 수 없다.
- Recipe stable ID·version·input Schema·language·pre/postcondition·assurance·idempotence가 명시돼 있다.
- text, syntax, symbol-aware, codegen의 보장 수준과 unsupported frontier가 구분된다.
- PatchSet·diff·impact·selected Check·worktree·rollback이 apply 전에 표시된다.
- base revision·dirty state·config·Catalog·Index·Tool fingerprint를 prepare와 apply에서 다시 확인한다.
- 사용자 기존 미커밋 변경과 overlap이면 overwrite하지 않고 block 또는 isolated worktree를 선택한다.
- Recipe semantic idempotence와 command idempotency가 별도다.
- partial apply를 성공으로 만들지 않고 actual receipt·reverse PatchSet·recovery 상태를 남긴다.
- format·build·test·contract Check는 M2가 선택하고 M3 pre/post Gate가 실행·판정한다.
- timeout·cancel·process failure·malformed output이 PatchSet success로 승격되지 않는다.
- 외부 codemod는 Tool Registry·typed arguments·isolated preview를 사용한다.
- RecipeExecution과 EvidenceBundle에 Recipe·tool version/hash·redacted input·output artifact가 남는다.
- rollback은 reverse PatchSet 또는 owned isolated worktree 폐기이며 primary source 삭제·hard reset이 아니다.
- 한 PatchSet은 한 Project·한 Checkout만 수정한다. M4 application은 항상 cross-project write를 거부하고, 9단계 ChangeBundle이 여러 project-local PatchSet application을 별도 participant로 조정한다.
- CLI-only로 Recipe와 target을 지정할 수 있고 Codex dependency가 없다.
- 이후 Codex와 5단계 Registry가 같은 ChangePlan·PatchSet application service를 호출한다.
- P-0042~P-0045의 M1·M2·M3·M4 첫 bounded Slice를 전체 Recipe family·CLI·post-apply 제품 완료로 확대 표시하지 않는다.
