# 개발 작업 Profile

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## C01. 16개 작업 유형별 Profile

아래 항목은 서로 다른 제품 16개가 아니다. A·B 기능을 작업 성격에 맞게 조합하는 설정·템플릿이며, 대상 프로젝트가 가진 기존 도구를 adapter로 호출한다.

| Profile | 조합할 기능 | 기본 적용 경계 |
|---|---|---|
| `project_understanding` | Project Catalog, checkout·workspace, source 분류, text/syntax/semantic index, graph·freshness, Context Pack | 새 프로젝트 또는 큰 범위 작업을 시작할 때 |
| `change_planning` | 사용자 TaskSpec, scope revision, ChangeSet, 영향 graph, 위험 경로, affected 검사·fallback, ChangePlan·ValidationPlan | 여러 파일·계약에 걸친 변경 |
| `refactor_codemod` | typed Recipe·managed/contract/symbol target, text/syntax/symbol/codegen assurance, dry-run PatchSet·영향, pre/post Gate·복구 | 한 Project의 검증 가능한 기계적 변경이 필요한 경우 |
| `dependency_upgrade` | manifest·lockfile, 호환성·보안, 단계적 upgrade, rollback | dependency 또는 framework 변경 |
| `language_platform_migration` | 현재 동작 계약, 단계별 공존, 경계 adapter, equivalence와 전환 증거 | 언어·runtime·플랫폼 이동 |
| `data_config_db_migration` | version 사슬, rehearsal, invariant, backup·restore, 재개와 rollback | 데이터·설정·DB 형식 이동 |
| `api_contract_change` | ManagedDeclaration·lifecycle, 공개 계약 diff, 소비자 최소 version·호환 기간, contract test, migration guide | API·CLI·Schema·파일 형식·error/config ID 변경 |
| `test_correctness` | 관련 테스트, 약화 탐지, 회귀 증거, 조건부 고급 테스트 | 버그 수정, 핵심 로직과 테스트 변경 |
| `architecture_quality` | layer·의존 규칙, cycle, 공개 경계, 예외와 ratchet | 구조 변경 또는 부채 정리 |
| `debug_recovery` | 실패 fingerprint, 재현 Pack, bisect·debug adapter, 수정 전후 증거 | 원인이 불명확한 실패와 복구 작업 |
| `performance_build` | workload, baseline, 반복 측정, profiler·build 분석, trade-off | 선언된 성능 경로나 build 병목 |
| `docs_config_environment` | 문서 명령·링크, 설정 계약, doctor, clean-room 재현 | 문서·설정·개발 환경 변경 |
| `ci_release_deploy` | CI 일치, package dry-run, artifact 신원, 배포·rollback 준비 | workflow, release 또는 배포 변경 |
| `security_supply_chain` | secret, dependency, 취약점·license, workflow, provenance | 보안 경로 또는 공급망 변경 |
| `ai_development_validation` | TaskSpec, scope·claim·evidence, test 약화, 검증기 보호, Review Pack | Codex가 생성하거나 수정한 모든 결과의 공통 마감 관문 |
| `rust_style_auto_fix` | pinned stable cargo/rustfmt·Clippy, exact fix allowlist·coverage, isolated PatchSet, pre/post Gate·`personal_auto` | Rust package/workspace의 검증 가능한 style 교정 |

다음 검사는 해당 프로젝트에 실제 대상이 있을 때만 Profile에 붙인다.

- GUI·웹 화면이 있으면 대상 프로젝트의 UI test 도구 연결
- DB가 있으면 migration 도구와 사본 환경 rehearsal 연결
- AI·RAG 기능이 있으면 prompt, retrieval, tool-use와 평가 자료 검증 연결
- 고위험 계산·parser·protocol에는 property·fuzz·mutation 도구 연결
- 여러 OS를 지원하는 대상 프로젝트만 해당 플랫폼 CI 결과 연결

## 16개 Profile 공통 engine 재사용 불변식

16개 Profile은 실행 engine 16개가 아니다. 모두 다음 공통 경로를 data-driven metadata로 조합한다.

```text
TaskSpec·ScopeRevision
  -> Project Catalog·Code Index
  -> ChangePlan·ValidationPlan
  -> registered Task·Tool·Check
  -> common Diagnostic·Baseline·Suppression·Gate
  -> EvidenceBundle·ReviewPack
  -> 필요한 Profile만 Patch·migration·ChangeBundle·release effect
```

- Profile source는 `catalog/profiles`이고 resolved ID·item version·definition hash·parent closure는 CatalogSnapshot과 `profile_resolution_fingerprint`에 고정한다.
- required Rule·Check·evidence는 union, permission·baseline·suppression·stability·review floor는 가장 엄격한 값을 사용한다.
- M2가 Profile closure와 exact Check를 선택하고 M3 runner는 추가·제거·완화하지 않는다.
- Profile은 compiler, scanner, debugger, profiler, package manager, CI·deploy service와 AI engine을 구현하지 않는다. registered adapter를 선택할 뿐이다.
- CLI-only에서는 Codex·AI가 required Check나 pass 근거가 아니다. Codex-integrated run도 같은 core Gate를 사용한다.
- 새 Profile은 기존 공통 engine으로 표현할 수 없고 독립 dependency·state·배포 경계가 입증되기 전에는 새 Package를 만들지 않는다.
- 각 Profile의 평가·trial·deprecation은 [10단계 EvaluationRun](../contracts/ci-release-evaluation-and-product-completion.md#evaluationrun-v2-평가-단위)을 사용한다.

### `change_planning`

`change_planning`은 사용자가 직접 입력한 [TaskSpec과 ScopeRevision](../contracts/goal-and-stage.md)을 [Project Catalog·Code Index](../contracts/project-catalog-and-code-index.md)에 결합하는 **CLI-only·source read-only** Profile이다. Codex·AI가 목표나 계획을 생성하지 않으며 test, build, lint와 validator도 이 단계에서 실행하지 않는다.

필수 입력은 다음과 같다.

- 사용자 objective, target Project·Checkout, include/exclude, intended change와 완료 조건
- current ProjectCatalogSnapshot·ProjectRevision·dirty WorkspaceSnapshot
- project별 current CodeIndexSnapshot과 tier·coverage·limitation
- Registry task이면 current ManagedRegistrySnapshot, source manifest hash와 declaration·binding·consumer observation
- Task·Check·RiskPath descriptor를 고정한 CatalogSnapshot
- 필요 시 compatible previous ValidationResult·GateDecision

Profile 단계 template은 다음 순서다.

1. TaskSpec exact input 검증과 selector ambiguity 해소
2. requested·analysis·planned change·validation scope를 분리한 ScopeRevision 생성
3. source revision과 staged·unstaged·untracked actual byte의 ChangeSet 수집
4. path·symbol·package·contract·config·schema seed 생성
5. file, test, docs, generated source와 downstream project의 direct/transitive·confirmed/possible 영향 계산
6. auth·secret, public API·Schema, dependency·lockfile, validator·policy, migration, workflow·release, generated source risk path 계산
7. affected Check candidate와 `selected_required|selected_optional|not_applicable|not_found|unavailable|user_waived` 판정
8. sound scope를 증명할 수 없을 때 package→workspace→project full promotion
9. same TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet fingerprint의 ChangePlan·ValidationPlan 생성

사용자가 수정한 Project·scope·Check 결정은 자동 계산보다 우선한다. 자동 계산과 다른 선택은 remaining risk, waiver·human review 필요성과 함께 새 ScopeRevision에 남기고 다시 덮지 않는다. related Check를 찾지 못한 상태는 `not_found`이며 complete applicability가 false인 `not_applicable`과 다르다.

여러 Project는 exported entity·Project relation을 이용해 read-only로 계산하고 Project별 ChangeSet·affected scope를 유지한다. cross-repo source 수정, worktree·merge·remote write는 이 Profile의 output도 side effect도 아니다. 9단계 [CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md)은 이 project relation·ChangePlan을 current participant에 다시 bind해 사용하며 `change_planning` 결과를 실행 승인으로 재사용하지 않는다.

상세 계산, fallback과 3단계 입력 계약은 [변경 계획·영향 분석 정본](../contracts/change-planning-and-impact.md)이 소유한다. 이 확장은 현재 **2단계 설계 확정·제품 구현 전이며 planner·selector·runner·CLI 구현 완료를 뜻하지 않는다**.

### `refactor_codemod`

`refactor_codemod`는 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md)을 사용하는 **CLI-first·single-project mutation Profile**이다. Codex 없이 사용자가 Recipe와 typed target을 지정할 수 있고, 이후 Codex도 같은 `ManagementApplicationService` command를 호출한다.

필수 입력은 다음과 같다.

- exact ID·SemVer·definition fingerprint가 있는 `ChangeRecipe`
- Schema를 통과한 parameters와 `managed_declaration|contract|symbol|path_range|finding_occurrence|generator_input` selector
- current ProjectCatalogSnapshot·CodeIndexSnapshot과 target resolution tier·coverage·limitation
- managed declaration target이면 authoritative manifest ref와 current ManagedRegistrySnapshot·binding·consumer fingerprint
- accepted TaskSpec·ScopeRevision, project-scoped ChangePlan v2와 `readiness=ready` ValidationPlan
- base ProjectRevision·WorkspaceSnapshot·complete dirty manifest와 preexisting ChangeSet
- current EffectiveConfig·CatalogSnapshot·ToolRegistrySnapshot·PermissionPlan

Profile metadata 기본값은 다음과 같다.

| metadata | 값 |
|---|---|
| `project_cardinality` | 정확히 1. downstream Project impact는 read-only ref만 허용 |
| `prepare_mode` | `dry_run_required`; live target source effect 없음 |
| `required_profiles` | `change_planning`, `ai_development_validation`; change class에 따라 `test_correctness`, `architecture_quality` union |
| `gate_phases` | `patch_pre_apply`, `patch_post_apply` 모두 필수 |
| `selector_policy` | stable managed/contract/symbol selector 우선, raw literal-only global selector 금지 |
| `dirty_policy` | 기본 `allow_disjoint`; overlap·unknown은 block 또는 isolated worktree |
| `idempotency_policy` | expected-after replay가 no-op인 Recipe만 automatic apply 가능 |
| `external_tool_policy` | trusted ToolDescriptor·typed args·isolated preview workspace·자동 retry 없음 |
| `completion_policy` | post-apply `AUTO_PASS` + complete EvidenceBundle·ReviewPack |

rewrite 보장은 다음처럼 분리한다.

- `text_replace`: explicit path/range·before hash에 대한 byte exact만 보장한다. symbol·contract 의미를 보장하지 않는다.
- `syntax_rewrite`: supported parser의 target node·before/after parse와 syntax shape를 보장한다. reference resolution을 보장하지 않는다.
- `symbol_aware_rewrite`: current·complete semantic definition/reference coverage 안의 binding 변경을 보장한다. dynamic/reflection·unsupported macro frontier는 자동 성공이 아니다.
- `codegen`: authoritative input과 pinned generator/config에서 declared output manifest 재현성을 보장한다. generated source 직접 편집은 금지한다.

Profile 단계 template은 다음 순서다.

1. Recipe·input Schema·target language/capability·selector를 resolve한다.
2. base revision·dirty state·config·Catalog·Index·Tool fingerprint를 preflight한다.
3. current checkout 또는 격리 preview worktree의 `WorktreeDecision`을 만든다.
4. target source에는 effect를 내지 않고 RecipeExecution preview를 수행한다.
5. preview workspace actual diff에서 `ChangeSet(change_set_kind=recipe_preview)`을 만들고 scope 밖 file을 거부한다.
6. M2 impact·risk·Profile closure·affected Check를 preview diff로 reconcile한다. 달라지면 replan 후 새 prepare다.
7. expected-after에서 같은 Recipe를 다시 실행해 idempotence no-op을 확인한다.
8. immutable PatchSet·diff·영향·Check·permission·rollback을 먼저 표시한다.
9. M3 `patch_pre_apply`와 exact PatchSet fingerprint 승인 뒤 single-use permit으로 apply한다.
10. actual WorkspaceSnapshot·ChangeSet을 다시 수집하고 M2 selected format·build·test·contract Check를 M3 `patch_post_apply`에서 실행한다.
11. partial/outcome unknown은 recovery 상태로 남기고 reverse PatchSet 또는 owned isolated worktree 폐기만 제안한다.

외부 codemod CLI는 live target checkout에서 실행하지 않는다. `structured_patch_producer` 또는 `isolated_workspace_mutator`로 Tool Registry를 통해 호출하고 ToolDescriptor·executable version/hash·redacted input·output artifact를 evidence에 남긴다. timeout·cancel·malformed output·output limit과 undeclared file 변화는 PatchSet success가 아니다.

`change prepare`와 `patch apply`는 별도 command다. prepare에 숨은 `--apply` 경로를 두지 않으며 PatchSet은 한 Project·한 Checkout만 소유한다. 9단계에서도 이를 cross-project PatchSet으로 넓히지 않고 `ChangeBundleParticipant`가 project별 Profile 실행을 조정한다. merge·commit·push는 M4 Profile completion이 아니며 별도 9단계 Git/remote permission·Gate를 따른다.

이 Profile은 현재 **4단계 설계 확정·제품 구현 전**이다. M1·M2·M3 제품 gate, Recipe/PatchSet v2 Schema·migration, transformer·source mutation·worktree adapter, CLI와 Corpus가 구현됐다는 뜻이 아니다.

### `api_contract_change`

`api_contract_change`는 공개 API·CLI·Schema·file format·config·error code의 baseline/current 호환성을 평가하고 소비자 migration까지 계획한다. Managed Registry 대상이면 [관리형 Symbol·상수·에러 코드 Registry 계약](../contracts/managed-symbol-registry.md)이 identity·lifecycle 정본이고, [6단계 계약 호환성·환경 정본](../contracts/contract-compatibility-and-environment.md)이 비교·consumer impact·compatibility window·companion change 판정을 소유한다. Profile은 `change_planning`과 `refactor_codemod`를 조합하지만 source 적용은 M4·M3 Gate를 우회하지 않는다.

필수 입력은 다음과 같다.

- explicit immutable baseline approval과 baseline/current `ContractSurfaceSnapshot`
- current `ProjectContractManifest`, Git Registry manifest와 current·complete `ManagedRegistrySnapshot`
- 대상 surface·ManagedDeclaration ID, source·Schema·documentation·generated output binding과 before/expected-after fingerprint
- declared·observed·unresolved consumer, `minimum_supported_version`, accepted version과 전환 deadline
- proposed `unchanged|compatible|additive|breaking|unknown`, lifecycle, replacement, bounded alias와 compatibility 기간
- required companion set: public source, Schema/file-format descriptor, generated reference, 문서, compatibility metadata, consumer migration guide
- TaskSpec·ScopeRevision, M2 ChangePlan·ImpactAnalysis·ValidationPlan, M4 RecipeExecution·PatchSet 후보와 M3 GatePolicy

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `contract_compatibility`, `consumer_migration`, `public_surface`, `managed_registry`, `generated_drift`, `companion_change` |
| `required_check_families` | kind별 contract diff, consumer coverage, Schema/generated/docs drift, contract test와 M2 selected build/test |
| `baseline_policy` | immutable release/Git artifact와 approval required; current checkout 자동 baseline 금지 |
| `unknown_policy` | required evidence 누락은 `BLOCK`, complete evidence 뒤 남은 의미 판단은 `HUMAN_REVIEW` |
| `migration_policy` | `none\|recommended\|required\|blocked_unknown`, breaking은 guide·window·owner required |
| `public_expansion_policy` | 호환 가능해도 `ChangePlan.expected_public_surface_delta`에 없으면 block |
| `cross_project_policy` | 같은 change group의 project별 plan·migration table만 read-only; 9단계 전 다른 Project source apply와 이를 요구하는 removal 완료 금지 |

Profile 단계 template은 다음 순서다.

1. baseline ref·approval·hash와 current subject binding을 검증하고 암묵 baseline을 거부한다.
2. API·CLI·Schema·file format·config·error code surface를 kind별 canonical shape로 수집한다.
3. M1 Index와 M5 Registry에서 definition/reference/generated output과 declared·observed·unresolved consumer를 분리한다.
4. kind별 rule로 change를 분류한다. stale·partial 관찰, 동적 dispatch와 모호한 enum/overload 의미를 compatible로 승격하지 않는다.
5. M2가 consumer별 migration requirement·최소 지원 version·alias window·순서와 affected Check를 materialize한다.
6. 의도된 public 확대와 required companion set이 `ChangePlan`에 모두 포함됐는지 확인한다.
7. codegen은 authoritative input만 target하고 codemod는 handwritten source·consumer 전환에만 선택한다. generated output을 직접 target하지 않는다.
8. M4가 한 Project의 manifest·binding·docs·Schema 변경을 dry-run해 immutable PatchSet을 만든다.
9. M3 pre Gate에서 baseline, compatibility, duplicate ID, namespace, consumer, migration/window, companion set과 PatchSet exact binding을 검사한다.
10. post Gate에서 actual-after snapshot, contract test, removed/deprecated reference, generated provenance, docs·Schema·consumer migration을 재검증한다.
11. 모든 required consumer가 전환되고 finite window가 끝난 뒤에만 `removed`를 허용하며 tombstone을 영구 보존한다.

첫 6단계 수직 Slice는 explicit baseline의 error code·CLI machine output 비교다. display message만 고치면 stable code를 유지하고, 의미·owner·retry·exit mapping이 달라지면 breaking replacement와 migration을 요구한다. 그 뒤 Schema, config key, public API와 file format을 순차 지원한다.

cross-project 영향과 migration table은 read-only output이다. 9단계 [ChangeBundle](../contracts/cross-repo-change-bundle.md)이 provider compatibility open → consumer transition → provider removal 순서와 finite window를 current participant별 PatchSet·Gate로 조정한다. 이 Profile 자체는 여러 Project 적용·merge·commit·push writer가 아니며 현재 상태는 **6단계 설계 확정·제품 구현 전**이다.

### `docs_config_environment`

`docs_config_environment`는 문서·config·generated reference·개발 환경의 declared assumption과 actual observation을 비교하는 **CLI-only·target read-only** Profile이다. [6단계 정본](../contracts/contract-compatibility-and-environment.md)의 `DocumentationSnapshot`, `ConfigKeyTrace`, `EnvironmentSnapshot`, `ProjectDoctorReport`, `CleanRoomSpecification`을 사용하며 package manager, container/runtime version manager를 대신하지 않는다.

적용 trigger는 다음 중 하나다.

- docs, config Schema/example, CLI descriptor, generated reference 또는 environment assumption 변경
- package manifest, lockfile, toolchain/runtime version, task/command descriptor 변경
- ManagedDeclaration의 documentation·Schema·generated binding 또는 config lifecycle 변경
- Windows path·case·encoding·line-ending·path-length 지원 범위 변경
- clean-room 또는 reproducible build claim 추가·변경

필수 입력은 다음과 같다.

- current project/source/workspace snapshot과 `ProjectContractManifest`
- docs source inventory, generated manifest, current CLI/Task/Tool/Check Catalog와 Schema refs
- current Managed Registry snapshot과 config-key BindingSpec·consumer observation
- `EffectiveConfig`의 secret-redacted field provenance
- package manifest·lockfile·toolchain declaration과 registered read-only probe
- environment constraints와 applicable `CleanRoomSpecification`

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `documentation_drift`, `config_lifecycle`, `assumption_drift`, `environment_doctor`, `clean_room_readiness`, `validator_guard` |
| `required_check_families` | link·anchor, registered command signature, snippet/config example, Schema/generated reference, config trace, doctor/readiness |
| `command_policy` | typed candidate가 exact registered descriptor와 일치하고 read-only/safe일 때만 실행 |
| `config_policy` | `declared` 존재, M5 `active→deprecated→removed`와 `documented\|read\|overridden` 관찰을 분리; 값·secret 수집 금지 |
| `doctor_effect` | target/system read-only, local derived evidence write만 허용 |
| `forbidden_actions` | network download, install/update, restore, source/config write, registry/PATH/code-page/long-path 변경 |
| `unknown_policy` | required 관찰 누락은 `BLOCK`, complete 관찰 뒤 의미 판단은 `HUMAN_REVIEW` |

단계 template은 다음과 같다.

1. docs source와 명시된 link·anchor·command·snippet·config example·Schema/generated reference·assumption을 snapshot한다.
2. local link/anchor와 Schema/config example을 pure validator로 검사한다.
3. command text를 typed candidate로 parse하고 exact registered descriptor와 비교한다. unsafe/unregistered command는 실행하지 않는다.
4. safe disposable fixture가 선언된 snippet과 CLI behavior만 실행하고 exit/output/schema evidence를 보존한다.
5. config key별 declaration·docs·semantic reader·override provenance·consumer·lifecycle coverage를 연결한다.
6. 환경 변수는 name/owner/presence/redaction policy만 비교하고 값은 수집하지 않는다.
7. doctor가 OS·toolchain·package manager·manifest·lockfile·Windows path/case/encoding/line-ending/path-length를 registered read-only probe로 관찰한다.
8. clean-room 명세의 source/toolchain/lockfile/command/network/cache/path constraint와 금지 행동을 확인한다. 환경 생성이나 설치는 하지 않는다.
9. M3가 exact Documentation/Environment/Doctor snapshot을 ValidationPlan subject에 결합하고 Diagnostic을 평가한다.
10. 후속 `security_supply_chain` Profile에 `DependencySecurityInputManifest`를 전달한다. advisory·vulnerability 판정은 여기서 만들지 않는다.

사용되지 않는 config key는 complete semantic reader coverage가 있을 때만 확정한다. text-only 후보, dynamic command, 자연어 support promise, 플랫폼 의미처럼 결정적 판정이 불가능한 항목은 AI 없이 `HUMAN_REVIEW`다. doctor가 설치나 system mutation을 필요로 하면 `mutation-required`와 수동 remediation만 내며 자동 실행하지 않는다.

이 Profile은 현재 **6단계 설계 확정·제품 구현 전**이다. docs validator, config tracer, doctor, clean-room runner나 environment probe가 구현됐다는 뜻이 아니다.

### `debug_recovery`

`debug_recovery`는 compile, test, runtime, tool, environment 실패를 공통 identity로 정규화하고 재현·회귀·복구 근거를 만드는 **CLI-first Profile**이다. [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)의 `FailureRecord`, `ReproductionPack`, `RegressionRecord`와 `RecoveryPlan`을 사용한다. debugger·tracer를 만들거나 adapter 결과를 완료 판정으로 승격하지 않는다.

적용 trigger는 다음 중 하나다.

- compile/test/runtime/tool/environment failure가 작업 완료를 막음
- bug fix가 수정 전 실패·수정 후 성공 evidence를 요구함
- 같은 failure fingerprint가 다시 관찰되거나 flaky test가 있음
- rollback, roll-forward 또는 restore 절차가 필요한 high-risk 변경
- migration·performance 작업의 stable reproduction baseline이 필요함

필수 입력은 다음과 같다.

- exact Project·Checkout·WorkspaceSnapshot·ProjectRevision·ChangeSet
- current Catalog/Index, registered Task·Check·Tool descriptor와 ValidationPlan
- failure Diagnostic·raw artifact refs와 normalization/fingerprint rule version
- structured args, logical cwd, environment fingerprint, input·seed
- 민감 artifact policy와 PermissionDecision

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `failure_identity`, `reproduction`, `regression_evidence`, `recovery_plan`, `redaction` |
| `required_check_families` | original failure Check, compatible rerun, after-fix regression, recovery validation |
| `reproduction_policy` | family/occurrence fingerprint 분리, 외부 조건은 `blocked_external\|unverified` |
| `attempt_policy` | bounded rerun; raw attempt를 모두 보존하고 마지막 pass로 flaky를 숨기지 않음 |
| `tool_policy` | reducer·bisect·debugger·trace는 exact registered adapter와 permission이 있을 때만 실행 |
| `artifact_policy` | ReproductionPack과 general log role 분리, `quarantined\|unknown`은 default report 제외 |
| `gate_floor` | required pack unverified·after incompatible·required flaky면 최소 HUMAN_REVIEW, protected path면 BLOCK |

단계 template은 다음과 같다.

1. failure occurrence를 common Diagnostic으로 정규화하고 root candidate·cascade edge를 만든다.
2. stable family와 exact occurrence fingerprint를 계산하고 이전 occurrence와 호환성을 비교한다.
3. ReproductionPack 최소 manifest를 만들고 secret·token·PII·개인 경로를 수집 전 또는 저장 전 가린다.
4. 같은 input·seed·environment constraint로 bounded rerun한다.
5. 승인·adapter가 있으면 input reduction, VCS bisect, debugger·trace를 독립 ToolInvocation으로 수행한다.
6. 수정 전 verified failure와 수정 후 complete·stable pass를 RegressionRecord로 연결한다.
7. rollback·roll-forward·restore를 서로 다른 RecoveryPlan으로 만들고 rehearsal·검증 상태를 기록한다.
8. M3가 current subject·evidence completeness·flaky·redaction을 판정한다.

`not_reproduced`는 fixed가 아니다. 외부 service·device·clock·network 조건을 확인할 수 없으면 `blocked_external` 또는 `unverified`다. 이 Profile은 현재 **7단계 설계 확정·제품 구현 전**이다.

### `security_supply_chain`

`security_supply_chain`은 secret·개인정보, auth·permission·crypto·workflow, dependency·license·vulnerability와 release material을 같은 M3 Gate에 결합하는 **read-first Profile**이다. M1 dependency graph와 M6 `DependencySecurityInputManifest`를 입력으로 사용하고, [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)의 `SupplyChainSnapshot`·`ExternalDataSnapshot`을 만든다.

적용 trigger는 다음 중 하나다.

- manifest·lockfile·package source·dependency relation 변경
- auth, session, token, permission, crypto 또는 위험 API 변경
- workflow permission·external action·release manifest 변경
- package·artifact publish 또는 SBOM·provenance·signature 검토
- unresolved security Finding, stale scanner DB 또는 만료 suppression

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `secret_redaction`, `sensitive_change`, `dependency_supply_chain`, `workflow_security`, `release_integrity`, `external_data_freshness` |
| `required_check_families` | built-in redaction, manifest/lockfile diff, applicable registered scanner, workflow permission/pin, release file/digest/manifest |
| `source_policy` | advisory·license·version result마다 source/query/schema, tool identity, coverage, freshness와 valid_until 필수 |
| `unknown_policy` | required 외부 자료 stale·unknown·partial이면 clean/pass 금지 |
| `scanner_policy` | adapter raw result와 common Diagnostic을 모두 보존; scanner exit가 Gate가 아님 |
| `permission_policy` | offline current input 우선; network refresh·download는 exact 사용자 승인 필요 |
| `excluded_systems` | 자체 vulnerability/license DB, scanner, registry, SBOM signer와 PKI |

단계 template은 다음과 같다.

1. exact subject와 M1/M6 manifest·lockfile·package-manager·environment input을 preflight한다.
2. source/config/docs/log/artifact의 secret·token·PII 후보를 값·hash 없이 common Diagnostic으로 만든다.
3. auth·permission·crypto·workflow 변경 marker와 external action immutable pin 여부를 평가한다.
4. dependency 목적·source·requested/resolved version·relation·license·advisory evidence를 연결한다.
5. release file list·digest·manifest와 이미 존재하는 SBOM·provenance·signature verification evidence를 묶는다.
6. 외부 자료마다 provenance·coverage·`current|stale|unknown|unavailable`을 판정한다.
7. 여러 producer의 중복 현상은 evidence를 유지한 correlation으로 묶고 별도 DB를 만들지 않는다.
8. M3가 redaction·freshness·coverage·protected risk를 최종 판정한다.

refresh가 필요하면 `network_read` 승인 대기 상태를 출력한다. 승인이 없으면 offline snapshot과 stale/unknown 경고를 보존하고 network에 접근하지 않는다. 이 Profile은 현재 **7단계 설계 확정·제품 구현 전**이다.

### `dependency_upgrade`

`dependency_upgrade`는 dependency와 내부 package relation을 관찰하고 update 후보·영향·검증·rollback을 설계한 뒤 **승인 가능한 immutable PatchSet에서 멈추는 Profile**이다. package manager가 manifest·lockfile 변경을 소유하고 Star-Control은 resolution을 역산하지 않는다.

적용 trigger는 다음 중 하나다.

- outdated, vulnerable, incompatible 또는 unknown dependency 상태
- patch/minor/major/security/internal dependency update 요청
- manifest·lockfile drift 또는 package manager/toolchain 변경
- 여러 Project에 영향을 주는 internal package revision 변경

필수 입력은 다음과 같다.

- current `DependencySnapshot`과 `SupplyChainSnapshot`
- current 외부 version/advisory/license snapshot과 freshness
- exact package identity·source·current version과 affected Project relation
- M2 ChangePlan·ValidationPlan, M4 Recipe/Patch capability와 current M3 Gate
- registered package manager ToolDescriptor, before manifest·lockfile ArtifactRef

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `candidate_kinds` | `patch\|minor\|major\|security\|internal`; security와 SemVer delta를 별도 축으로 보존 |
| `default_stop` | `patch_prepared → awaiting_apply_approval` |
| `lockfile_policy` | 등록 package manager만 생성·갱신; core/text codemod 직접 편집 금지 |
| `permission_policy` | network read/download, dependency add/change와 live apply는 각각 exact 사용자 승인 필요 |
| `workspace_policy` | isolated worktree preview, undeclared file change는 BLOCK |
| `replan_policy` | 실제 preview diff가 scope·impact·Check를 바꾸면 M2 재계획 뒤 새 PatchSet |
| `rollback_policy` | before manifest·lockfile bytes/hash와 reverse PatchSet·post-rollback Gate 필수 |

단계 template은 다음과 같다.

1. M1 relation과 M6 input을 결합해 currency·vulnerability·compatibility·resolution 상태를 독립 판정한다.
2. update kind, proposed constraint/resolution, 목적·source·freshness와 affected Project를 후보에 기록한다.
3. M2가 public contract, auth/permission, workflow/release, migration과 runtime 영향 및 Check를 계획한다.
4. 외부 refresh·package download·dependency change가 필요하면 effect별 approval을 기다린다.
5. 승인된 경우에만 M4 isolated worktree에서 registered package manager를 실행한다.
6. actual manifest·lockfile diff를 다시 수집하고 out-of-scope write를 거부한다.
7. actual diff로 M2 replan하고 previous lockfile·rollback·validation을 포함한 immutable PatchSet을 만든다.
8. dashboard 상태를 `awaiting_apply_approval`로 만들고 live source에 적용하지 않는다.
9. 사용자가 exact PatchSet을 승인한 뒤에만 M4 apply와 M3 post Gate를 수행한다.
10. 실패하면 partial success로 포장하지 않고 이전 lockfile 보존 상태와 rollback readiness를 표시한다.

package 추가는 upgrade 승인에 포함되지 않는다. `personal_auto`도 이 Profile의 network/download/dependency change를 자동 승인하지 않는다. 이 Profile은 현재 **7단계 설계 확정·제품 구현 전**이다.

internal dependency가 여러 Project에 걸리면 각 Project의 package-manager-owned PatchSet·previous lockfile·Gate·rollback을 유지한 read-only participant input을 만든다. 9단계 ChangeBundle이 provider package revision과 consumer constraint/lockfile 순서를 current base에 다시 bind하며 이 Profile 하나가 cross-repo apply·merge·push를 수행하지 않는다.

### `data_config_db_migration`

`data_config_db_migration`은 한 Project의 data·config·DB·state·file format을 explicit version chain으로 이동하고 dry-run·backup·restore·rehearsal·resume·rollback evidence를 만드는 **approval-gated Profile**이다. 범용 workflow와 상태기계는 [8단계 Migration·성능·언어·플랫폼 계약](../contracts/migration-performance-and-platform.md)이 소유한다.

적용 trigger는 다음 중 하나다.

- TaskSpec이 data/config/DB/state/file-format version 이동을 요구함
- current version이 supported writer보다 낮고 manifest에 unique migration chain이 있음
- M6 compatibility report가 consumer migration과 persisted format 전환을 요구함
- dependency/language/platform change가 data·config·state 변환을 동반함
- M7 failure/recovery가 roll-forward migration 또는 verified restore를 요구함

Star-Control 자체 관리 DB의 `management_store_version` migration은 이 Profile의 대상이 아니다. 0단계 `star-state` lifecycle이 최소 구현을 소유하고, M8은 대상 Project에 재사용할 범용 `ProjectMigrationManifest`·plan·attempt·evidence를 완성한다.

필수 입력은 다음과 같다.

- exact Project·Checkout·ProjectRevision·WorkspaceSnapshot과 current Code Index
- 사용자 TaskSpec·ScopeRevision·ImpactAnalysis와 M2 selected Check
- reviewed `ProjectMigrationManifest`, target ID와 version source observation
- current/target `MigrationVersionVector`, unique ordered step chain과 invariant set
- M6 contract/consumer/config/environment evidence
- M7 ReproductionPack·RecoveryPlan과 available backup/restore evidence
- source migration script·Schema·config 변경이 있으면 M4 PatchSet·post Gate
- registered migration·backup·restore ToolDescriptor와 permission effect

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `migration_version`, `migration_chain`, `unknown_preservation`, `backup_restore`, `migration_invariant`, `migration_recovery`, `consumer_compatibility` |
| `required_check_families` | version probe, dry-run no-live-write, backup integrity, restore rehearsal, migration rehearsal, per-step/after invariant, consumer/contract, post-rollback |
| `gate_phases` | `migration_pre_execute\|migration_post_execute\|migration_post_rollback` |
| `strategy_floor` | side-by-side copy 또는 atomic replace 우선; in-place는 full transaction capability 증명 필요 |
| `unknown_policy` | preserve/opaque를 증명하지 못하면 block; drop은 destructive approval |
| `backup_policy` | byte 존재·integrity·restore rehearsal·restore behavior를 별도 상태로 유지 |
| `resume_policy` | checkpoint before/expected-after 재관찰, non-replay-safe outcome unknown은 자동 retry 금지 |
| `destructive_policy` | exact plan·loss set·backup/restore·irreversible boundary 사용자 prompt; personal_auto 완화 금지 |
| `default_stop_state` | `awaiting_approval`, `pending_action=execute` |
| `cross_project_policy` | project별 read-only handoff만; 9단계 ChangeBundle 전 source/data execute 금지 |

단계 template은 다음과 같다.

1. version source·target identity·coverage를 읽고 `current_supported|migratable|read_only_supported|future_version|chain_gap|ambiguous_chain|unknown_version|corrupt`를 판정한다.
2. 연속 `from_version→to_version` step과 invariant, tool binding, expected write/loss scope를 materialize한다.
3. live write 없이 dry-run하고 unknown field, row/item/byte delta, disk·lock·downtime, consumer와 rollback 영향을 만든다.
4. consistent backup을 만들고 byte/hash/header·set consistency를 검증한다.
5. 사본 환경에 실제 restore해 structural·behavior Check를 수행하고 `RestoreVerificationRecord`를 만든다.
6. 같은 chain·tool·compatible environment로 migration rehearsal을 끝까지 실행하고 checkpoint·receipt·before/after invariant를 남긴다.
7. destructive/live effect가 있으면 exact plan fingerprint·loss·irreversible boundary·rollback을 보여주고 사용자 승인을 기다린다.
8. 승인 뒤 side-by-side candidate 또는 proven transaction에서 ordered step을 실행하고 durable boundary마다 checkpoint를 commit한다.
9. crash·cancel 뒤 actual target을 checkpoint before/expected-after와 reconcile해 safe resume, already-applied, diverged 또는 `outcome_unknown`을 판정한다.
10. target version·모든 invariant·consumer Check를 검증하고 M3 post Gate 뒤에만 `succeeded`로 집계한다.
11. 실패 시 rollback·roll-forward·restore를 별도 attempt로 수행하고 post-rollback Gate를 통과해야 `rolled_back`이다.
12. 여러 Project가 필요하면 project별 plan·PatchSet·Gate·restore/rollback ref를 `CrossProjectMigrationHandoff`로 내보내고 실행하지 않는다. 9단계 ChangeBundle은 이를 current participant에 다시 bind한 뒤에만 project-local effect를 조정한다.

`partially_succeeded`는 success가 아니다. backup file과 checksum만 있으면 `restore_rehearsed`가 아니며, tool exit 0만 있으면 migration 완료가 아니다. 이 Profile은 현재 **8단계 설계 확정·제품 구현 전**이며 실제 migration, backup, restore와 DB 변경을 실행하지 않았다.

### `performance_build`

`performance_build`는 사용자가 중요하다고 선언한 runtime 또는 build workload만 exact baseline/candidate cohort에서 비교하는 **opt-in measurement Profile**이다. 모든 작업에 붙이지 않으며 숫자가 없거나 비교 불가능한 조건에서 결과를 합성하지 않는다.

적용 trigger는 다음 중 하나다.

- TaskSpec이 특정 latency·throughput·memory·artifact-size·build budget을 요구함
- reviewed `PerformanceWorkloadSpec`의 critical path를 사용자가 선택함
- M2가 declared performance/build risk path를 match함
- data/language migration이 downtime·throughput·memory 또는 artifact-size equivalence를 required로 선언함

필수 입력은 다음과 같다.

- workload ID/version/spec fingerprint와 중요성 선언
- comparison intent와 baseline/candidate 사이 exact `allowed_delta_axes`
- registered benchmark/build invocation, input manifest·seed와 expected output
- baseline/candidate exact ProjectRevision·WorkspaceSnapshot·ChangeSet
- EffectiveConfig, Catalog, Tool version/hash와 environment fingerprint
- build/cache mode, warmup·measurement run 수, metric unit·collector
- predeclared noise threshold, outlier detector, aggregation과 추가 실행 상한
- workload별 budget/threshold가 있으면 그 source·version
- correctness·contract·test Check와 optional profiler/build analyzer descriptor

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `activation` | explicit user/TaskSpec 또는 reviewed workload만; default off |
| `required_rule_families` | `performance_comparability`, `measurement_integrity`, `noise_outlier`, `build_mode`, `correctness_tradeoff` |
| `required_check_families` | workload correctness, metric collector, environment/cache probe, baseline/candidate run, comparison; declared budget이면 budget Gate |
| `gate_phases` | `performance_compare`와 source 최적화의 일반 `patch_post_apply` |
| `sample_floor` | measured run 최소 3, 기본 5; warmup 기본 1과 분리 |
| `outlier_policy` | 첫 measured run 전에 고정, raw 보존, 포함/제외 통계 모두 보고 |
| `missing_policy` | numeric value·unit·collector 없으면 `no_measurement`, 0·추정·이전 값 금지 |
| `comparability_policy` | workload·input·driver/collector·environment·mode 동일, cohort 내부 exact revision, intent별 exact delta만 예외 |
| `tool_policy` | profiler/build analyzer는 external adapter·cause candidate, Gate writer 아님 |
| `result_state` | `comparable\|inconclusive\|not_comparable\|no_measurement\|noisy`; `completeness`와 분리, live mutation 없음 |

단계 template은 다음과 같다.

1. explicit workload activation과 budget/threshold source를 확인한다. 선언이 없으면 `not_declared|not_applicable`로 종료한다.
2. baseline/candidate의 workload·input·tool·environment·config·build/cache mode와 comparison intent·allowed delta를 고정한다.
3. correctness Check와 output equivalence precondition을 실행해 서로 다른 기능을 비교하지 않게 한다.
4. 각 cohort에서 warmup attempt를 별도 기록한 뒤 같은 exact revision으로 measured attempt를 수행한다.
5. numeric value·unit·collector, cache probe, environment와 raw artifact를 attempt마다 기록한다.
6. predeclared noise·outlier rule을 양쪽에 동일하게 적용하고 모든 excluded sample을 보존한다.
7. clean, incremental, cache hit, cache miss, memory, artifact size와 runtime metric을 별도 item으로 집계한다.
8. required sample·comparability·noise를 만족할 때만 relative delta와 budget result를 계산한다.
9. profiler/build analyzer가 있으면 hotspot·candidate cause를 연결하되 causal proof로 승격하지 않는다.
10. M3 correctness·contract·test Gate와 memory·size·maintainability trade-off를 함께 평가한다.

baseline/candidate revision이 코드 비교 때문에 다르면 각 cohort는 단일 exact revision이고 차이는 declared ChangeSet/PatchSet이어야 한다. toolchain/config/cache 비교라면 revision은 양쪽 동일하며 해당 axis만 exact delta다. 여러 axis가 동시에 달라지고 factorial plan이 없으면 causal improvement를 만들지 않는다. high noise, sample 부족과 의도하지 않은 environment 차이는 `inconclusive|not_comparable`이며 regression/pass가 아니다.

이 Profile은 현재 **8단계 설계 확정·제품 구현 전**이다. benchmark, profiler, build analyzer, compiler 또는 build cache를 실행·구현하지 않았다.

### `ci_release_deploy`

`ci_release_deploy`는 local 검증 subject와 clean CI·final artifact·설치 lifecycle·원격 publish 결과를 같은 identity로 연결하는 10단계 Profile이다. 상세 알고리즘과 상태기계는 [10단계 정본](../contracts/ci-release-evaluation-and-product-completion.md)이 소유한다.

필수 입력은 다음과 같다.

- 같은 Task ID·accepted ScopeRevision과 project별 immutable source revision
- multi-project이면 current `ChangeBundleReleaseHandoff`, project별 Gate·EvidenceBundle·compatibility·rollback ref
- `local_quick`, `target`, `full`, `release` phase가 materialize된 ValidationPlan
- EffectiveConfig, CatalogSnapshot, logical Tool ID/version/descriptor set, environment별 ToolRegistrySnapshot과 resolved Profile closure fingerprint
- product version source, changelog, package policy·file ownership, license·notice source
- clean Windows 11 24H2 build 26100 이상 x64·ARM64 environment spec
- final artifact entry·SHA-256·artifact set digest와 build/package invocation
- install·update source version·rollback target·uninstall preserve policy
- publish target이 있으면 current remote before snapshot; capability는 approval이 아님

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `validation_layer_refs` | `local_quick`, `target`, `full`, `release`; 낮은 계층이 높은 계층을 대체하지 않음 |
| `required_rule_families` | `release_identity`, `artifact_integrity`, `package_contents`, `metadata_license`, `platform_support`, `install_lifecycle`, `publish_state`, `validator_guard` |
| `required_check_families` | affected target, clean full build/test/docs/contract/security, package dry-run, x64·ARM64 runtime/install, update/rollback/uninstall, remote verify |
| `gate_phases` | `release_preflight`, `release_build`, `release_verify`, `release_package`, `release_install_lifecycle`, `release_ready`, publish 시 preflight·verify |
| `artifact_policy` | build/package once, final byte SHA-256와 set digest 봉인, 검증 뒤 같은 byte 승격, 재build 시 새 candidate |
| `platform_policy` | x64·ARM64 build와 native runtime/install evidence; cross-compile-only는 ARM64 support pass 아님 |
| `supply_chain_policy` | SBOM·provenance·signing을 각각 required/not-required/unavailable/incomplete/complete로 판정 |
| `approval_checkpoints` | publish·deploy·withdrawal·remote rollback·paid signing/CI는 action별 exact prompt |
| `completion_policy` | current release Gate와 complete evidence가 있어야 `ready`; after snapshot 전 `published` 금지 |
| `default_stop_state` | remote target이 있어도 기본 `ready`; explicit approval 전 publish 시작 금지 |

Profile 단계 template은 다음 순서다.

1. Task·source·config·Catalog·Tool·Profile identity와 0~9단계 handoff를 current 상태에 다시 bind한다.
2. version·changelog·package metadata·license·notice와 conditional supply-chain applicability를 확인한다.
3. clean x64·ARM64 environment와 locked dependency·network/cache policy를 고정한다.
4. architecture별 build·package를 한 번 수행하고 final artifact set digest를 봉인한다.
5. `target`과 `full` Check가 같은 source와 tool identity를 사용했는지 검증한다.
6. final artifact byte에서 package file list·architecture·path·generated provenance·license를 dry-run 검사한다.
7. native x64·ARM64 clean install, `safe_default` first run, update, rollback과 uninstall data preservation을 검사한다.
8. 모든 required phase가 current·complete하면 ReleaseManifest를 `ready`로 판정한다.
9. exact manifest revision·digest·channel·remote target의 ApprovalRequest가 있을 때만 `approved`로 전이한다.
10. publish·deploy adapter receipt 뒤 remote after snapshot이 exact version·source·artifact digest를 확인할 때만 `published`로 전이한다.
11. 미확정 publication 결과는 top-level `publish_outcome_unknown`, 미확정 deploy 결과는 target action `outcome_unknown`으로 남긴다. install/update/deploy 실패는 `rollback_required`로 남기고 user data retention hold를 건다.

`ready`, `approved`, `published`는 서로 다른 상태다. signing이 final byte를 바꾸면 signing 뒤 artifact가 새 candidate이며 이전 unsigned 검사 결과를 상속하지 않는다. Star-Control은 CI runner, compiler, package manager, installer, signer, artifact registry와 deploy service를 구현하지 않고 registered adapter·공통 Gate·evidence를 사용한다.

이 Profile의 release/evaluation type·Schema·runner·installer·provider adapter는 현재 **10단계 설계 확정·제품 구현 전**이다. 이 문서와 package 예시를 실제 clean build·install·publish evidence로 사용하지 않는다.

### `language_platform_migration`

`language_platform_migration`은 현재 구현의 behavior contract를 고정하고 target language·runtime·SDK·architecture·OS로 단계적으로 공존·전환하는 **compatibility-first Profile**이다. compile 성공과 기능 동등성을 분리하고, 완전 자동 번역을 약속하지 않는다.

적용 trigger는 다음 중 하나다.

- language·runtime·framework·SDK·architecture·OS 이동
- implementation 교체가 public contract·serialization·error·state semantics에 영향을 줌
- old/new 구현을 boundary adapter 뒤에 공존시켜야 함
- consumer 전환과 finite compatibility window가 필요한 provider 변경
- data/config format writer·reader 순서를 동반한 platform cutover

필수 입력은 다음과 같다.

- source/target stack과 toolchain·OS·arch exact fingerprint
- M6 immutable public contract baseline과 current consumer matrix
- input/output, state, error, serialization, concurrency, security, operational behavior contract
- M7 stable ReproductionPack·RegressionRecord와 rollback/restore evidence
- boundary adapter, coexistence phase와 source/consumer/writer 전환 순서
- M4 codegen/codemod Recipe, generator/tool identity와 assurance level
- build·test·contract·differential Check, required이면 PerformanceWorkloadSpec
- compatibility window, cutover approval와 old-path rollback 조건
- claimed platform별 actual local/remote/CI evidence source

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `behavior_baseline`, `boundary_adapter`, `consumer_transition`, `equivalence`, `codegen_codemod`, `platform_evidence`, `cutover_rollback` |
| `required_check_families` | build/compile, unit/integration, contract/differential, serialization/error/state, consumer, platform runtime; declared 중요 경로면 performance |
| `gate_phases` | source Patch의 `patch_pre_apply\|patch_post_apply`, 최종 `language_cutover` |
| `equivalence_policy` | 모든 required dimension current·complete·stable이어야 `equivalent` |
| `compile_policy` | build dimension 하나만 증명; compile-only equivalence 금지 |
| `translation_policy` | assurance 실제값 유지, 미확인 의미는 `HUMAN_REVIEW` |
| `coexistence_policy` | boundary→shadow/differential→reader first→consumer switch→writer cutover→window→removal |
| `platform_policy` | 실제 evidence 없는 OS/arch는 `not_run\|unverified`, cross-compile을 runtime pass로 사용 금지 |
| `cutover_policy` | exact consumer/window/rollback/Gate와 사용자 approval 필요 |
| `default_stop_state` | `awaiting_cutover_approval` |
| `cross_project_policy` | project별 plan·PatchSet만, 9단계 ChangeBundle 전 apply 금지 |

단계 template은 다음과 같다.

1. current behavior를 public surface·I/O·state·error·serialization·concurrency·filesystem/process·security·operational dimension으로 고정한다.
2. 기존 bug/quirk의 보존 여부를 evidence와 사용자 decision으로 분리하고 자동 contract 승격을 금지한다.
3. old/new implementation이 같은 stable contract를 따르는 boundary adapter를 먼저 준비한다.
4. target source를 분리해 구현하고 authoritative Schema/IDL 기반 codegen과 typed codemod를 M4 PatchSet으로 만든다.
5. shadow/differential 단계에서 같은 input·seed·environment로 old/new output·error·state를 비교한다.
6. 새 format/runtime을 읽을 consumer를 먼저 전환하고 low-risk consumer부터 target adapter로 bounded switch한다.
7. 모든 required reader·consumer compatibility 뒤에만 authoritative writer/source cutover를 승인 요청한다.
8. build·test·contract·differential·required performance와 platform matrix를 `EquivalenceReport`에 dimension별로 기록한다.
9. compile/build pass를 별도 field로 유지하고 required behavior missing을 `partial|unverified`로 남긴다.
10. finite compatibility window 동안 old path·fallback과 actual consumer state를 보존한다.
11. old reference 0, complete consumer coverage, rollback readiness와 M3 post Gate 뒤에만 old path를 제거한다.
12. 여러 Project consumer가 있으면 provider compatibility open → consumer transition → provider close 순서와 plan·Gate·rollback을 9단계 handoff로 보낸다. ChangeBundle은 project별 Profile 실행을 조정하지만 language Profile 자체를 cross-repo writer로 만들지 않는다.

reflection, FFI, unsafe, concurrency, numeric/encoding, platform API와 dynamic dispatch처럼 기계적으로 의미를 확정할 수 없는 항목은 `HUMAN_REVIEW`다. Star-Control local runner는 Windows 밖 runtime을 검증했다고 주장하지 않으며 authenticated remote/CI evidence도 exact subject·tool·environment가 있어야 한다.

이 Profile은 현재 **8단계 설계 확정·제품 구현 전**이다. compiler, transpiler, codegen, codemod, target OS runtime과 cutover를 실행·구현하지 않았다.

### `ai_development_validation`

`ai_development_validation`은 Codex가 source·test·docs·config·Schema·Catalog를 생성하거나 수정한 모든 변경의 공통 마감 Profile이다. AI가 작성했다는 사실을 결함으로 취급하지 않지만, 완료 보고를 신뢰 근거로 사용하지 않고 [3단계 공통 Gate](common-validation-gate.md)로 actual change·claim·evidence를 다시 확인한다.

필수 입력은 다음과 같다.

- current TaskSpec·accepted ScopeRevision, project별 ChangePlan과 `readiness=ready` ValidationPlan
- planning-baseline과 current WorkspaceSnapshot에서 재수집한 `observed_after_change` ChangeSet
- CompletionClaim, current Catalog·Validator Registry·Tool Registry·EffectiveConfig snapshot
- compatible Baseline·Suppression revision set과 preexisting user change manifest

Profile metadata 기본값은 다음과 같다.

아래 metadata는 M3 runner가 실행 직전에 새 검사를 고르는 규칙이 아니다. M2 affected selector가 Profile activation과 parent closure를 계산해 required Rule·Check·evidence floor를 ValidationPlan에 materialize한다. M3는 exact `profile_resolution_fingerprint`를 검증하며 actual change class가 다른 Profile을 요구하면 `VALIDATION_PROFILE_CLOSURE_STALE`로 재계획한다.

| metadata | 값 |
|---|---|
| `gate_phases` | `stage_exit`, source 적용이 있으면 `patch_pre_apply`, `patch_post_apply` |
| `required_rule_families` | `change_scope`, `claim_evidence`, `validator_guard`, `secret_dangerous_command` |
| `required_check_families` | M2가 change class에 맞는 test·architecture·docs family를 selected required set에 union. M3 runtime 추가 금지 |
| `baseline_policy` | `ratchet_new_and_worsened`; 기존 unchanged debt를 숨기지 않음 |
| `suppression_policy` | exact fingerprint 또는 bounded Rule+scope, 이유·Rule fingerprint·만료 필수 |
| `claim_policy` | changed file·check execution·compatibility·regression claim을 current evidence에 bind |
| `review_policy.cli_only` | 결정적 검사 뒤 남은 의미 판단은 `human_semantic`; AI 호출 금지 |
| `review_policy.codex_managed` | 위험·정책이 요구할 때 별도 Codex review를 추가 evidence로 허용하되 Gate 대체 금지 |
| `evidence_requirements` | 모든 attempt, actual ChangeSet, DiagnosticEvaluation, GateDecision, EvidenceBundle·ReviewPack |

단계 template은 다음 순서다.

1. ValidationPlan coherence·fingerprint·Tool trust preflight
2. actual add·modify·delete·rename과 preexisting 변경 재수집
3. 작업 계약·ChangePlan·보고된 CompletionClaim 비교
4. validator/test/policy/generated surface two-snapshot guard
5. M2 CheckGraph 실행과 외부 결과 공통 Diagnostic 정규화
6. baseline·suppression·flaky·false-positive 상태 평가
7. `AUTO_PASS|HUMAN_REVIEW|BLOCK` 결정
8. EvidenceBundle·ReviewPack과 blocking ReworkDirective 생성

다른 revision의 evidence, required `not_run|partial|unverified|stale|flaky`, contradicted claim과 out-of-scope change가 있으면 `AUTO_PASS`할 수 없다. CLI-only mode는 AI 독립 검토 부재를 실패로 만들지 않으며, 의미 검토가 필수이면 `HUMAN_REVIEW` 상태에서 사용자를 기다린다.

이 Profile은 현재 **3단계 설계 확정 대상·제품 구현 전**이다. Codex 결과를 검증하는 validator, runner, Corpus나 DB가 이미 구현됐다는 뜻이 아니다.

### `test_correctness`

`test_correctness`는 버그 수정, 핵심 로직, test·fixture·snapshot·test harness 변경에 `ai_development_validation` 공통 Gate를 강화한다. 단순히 test process가 한 번 exit 0인지를 보지 않고 관련 test 선택과 검사의 독립성을 확인한다.

적용 trigger는 다음 중 하나다.

- ChangeSet에 test·fixture·snapshot·assertion helper·runner config가 포함됨
- TaskSpec이 bug fix·regression·correctness를 완료 조건으로 요구함
- ImpactAnalysis의 `tests` edge 또는 `validator-policy` risk path가 match됨
- M2 selected Check에 related test·contract test·regression evidence가 있음

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `test_trust`, `validator_guard`, `regression_evidence` |
| `required_check_families` | M2 related test, owning source regression, changed test trust; contract/public boundary면 contract test 추가 |
| `always_run_for` | test deletion, assertion weakening, skip/ignore/only, timeout/retry change, regression requirement |
| `ratchet_eligible` | lint형 test metadata Diagnostic만 가능; functional test·regression pair는 불가 |
| `stability_policy` | required flaky는 최소 `HUMAN_REVIEW`; validator/regression 핵심이면 `BLOCK` |
| `regression_pair` | 같은 test identity·input·environment의 before failure와 current complete·stable after pass |
| `snapshot_policy` | 변경 item·byte·semantic owner threshold 초과 시 human review |

Profile 단계는 다음과 같다.

1. M2 related test 선택·coverage·fallback 근거 검증
2. test file/case 삭제, assertion·expected value 변화와 owning requirement mapping 비교
3. skip·ignore·only, timeout·retry, fixture·snapshot 대량 변경 탐지
4. before failure evidence의 revision·failure fingerprint compatibility 확인
5. after run을 current subject에서 실행하고 complete·current·stable 여부 확인
6. 구현을 그대로 복제한 oracle·같은 잘못된 가정 가능성은 결정적 evidence가 없으면 의미 검토로 분리
7. raw attempt를 모두 보존하고 마지막 pass만으로 flaky를 숨기지 않음

test framework AST adapter가 없으면 text heuristic은 `suspected`와 실제 confidence를 사용한다. 확정할 수 없는 assertion 의미를 자동 `BLOCK`으로 과장하지 않되 required correctness를 증명하지 못했으면 `AUTO_PASS`도 하지 않는다.

이 Profile의 M3 metadata·Rule·runner 동작은 현재 문서 설계이며 test adapter·Corpus·제품 Gate가 구현됐다는 뜻이 아니다.

### `architecture_quality`

`architecture_quality`는 package·module·layer·공개 경계·정본·generated ownership의 구조 품질을 공통 Gate에 추가한다. 기존 부채가 많은 project에서는 기본 ratchet으로 새 cycle·금지 import·공개 표면 확대·정본 drift를 막고, 과거 전체 위반을 한 번에 차단하지 않는다.

적용 trigger는 다음 중 하나다.

- package/workspace manifest, module boundary, public API·CLI·Schema·config 계약 변경
- ImpactAnalysis의 `depends_on|imports|exposes|implements|generates|generated_from` edge 영향
- architecture policy, Rule, allowlist, layer mapping 또는 canonical docs 변경
- hardcoding candidate가 authoritative config·contract·Schema와 연결됨

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `required_rule_families` | `dependency_boundary`, `cycle`, `public_surface`, `forbidden_import`, `hardcoding_drift`, `generated_drift`, `validator_guard` |
| `required_check_families` | dependency graph, architecture contract, Schema/contract drift, generated consistency; M2 selected build/test는 유지 |
| `required_evidence_tier` | declared/current graph 우선, semantic exact가 가능하면 사용; text-only는 possible/suspected |
| `baseline_policy` | `ratchet_new_and_worsened`; Rule/fingerprint/scope compatible baseline만 사용 |
| `always_block` | 새 confirmed cycle, protected boundary 위반, generated source 직접 편집, validator policy 약화 |
| `human_review` | dynamic/reflection boundary, canonical prose 의미와 hardcoding candidate의 의도 판단 |

단계 template은 다음과 같다.

1. current package·module·contract graph와 architecture policy fingerprint 확인
2. dependency direction, cycle, 공개 표면과 금지 import의 before/after delta 계산
3. hardcoding Finding candidate를 authoritative config·contract·Schema drift evidence와 결합
4. generator input·output·provenance와 actual direct edit 비교
5. compatible baseline으로 existing/new/worsened를 평가하고 suppression 만료·범위를 확인
6. current tier로 확정할 수 없는 dynamic edge를 unverified/possible로 남김
7. common Gate·EvidenceBundle·ReviewPack에 같은 Diagnostic 형식으로 합침

Profile은 범용 parser·graph DB·compiler를 만들지 않는다. Project Catalog·Code Index와 등록된 compiler/LSP/architecture tool 결과를 정규화한다. source graph가 stale·partial이면 architecture 위반 0건을 성공으로 표시하지 않는다.

이 Profile의 M3 metadata·Rule·ratchet 동작은 현재 문서 설계이며 architecture validator·Corpus·제품 Gate가 구현됐다는 뜻이 아니다.

### `project_understanding`

`project_understanding`의 첫 실행은 사용자가 시작한 manual full scan이다. 이후 같은 Profile은 Git revision·file hash 기반 incremental scan을 우선하고, source·config·adapter fingerprint가 달라 재사용할 수 없을 때만 full scan을 요구한다. 출력은 [읽기 전용 Project Catalog와 Code Index](../contracts/project-catalog-and-code-index.md)의 ProjectCatalogSnapshot·CodeIndexSnapshot, tier별 coverage·limitation과 [ContextPack](../contracts/goal-and-stage.md)이다.

이 Profile은 CLI-only·source read-only다. project task·package script를 실행하거나 source를 수정하지 않으며 AI·embedding·LLM 의미 추론 자동화를 요구하지 않는다. semantic adapter가 없거나 parse가 실패하면 syntax·text fallback을 실제 tier로 표시하고, unsupported·partial·stale·no-result 이유를 ContextPack에서 보존한다.

위 Project Catalog·Code Index 동작은 현재 1단계 목표 설계이며 제품 scanner·parser·DB·watcher가 구현됐다는 뜻이 아니다.

### `rust_style_auto_fix`

`rust_style_auto_fix`는 [Rust 코드 스타일 자동 교정 정본](rust-code-style-auto-fix.md)의 `rust_style_v1` fixed pipeline을 사용하는 **CLI-only·single-project mutation Profile**이다. 공식 stable `cargo fmt`/rustfmt와 Clippy를 기존 Tool Registry·M1/M2/M3/M4 경로로 조합하며 별도 formatter·parser·rewrite engine이나 runtime executable을 만들지 않는다.

적용 trigger는 다음 중 하나다.

- Rust package/workspace의 rustfmt drift와 Clippy Diagnostic을 source effect 없이 확인
- routine change의 affected package에 허용된 style correction candidate 준비
- 사용자가 명시한 전체 workspace formatting normalization ChangePlan
- exact policy와 complete 검증 아래 사용자가 terminal에서 시작한 `personal_auto` 적용
- toolchain/config/feature/target drift 뒤 과거 PatchSet이 stale인지 재확인

필수 입력은 다음과 같다.

- current ProjectId·CheckoutId·source/dirty manifest와 trusted-project 판정
- accepted TaskSpec·ScopeRevision, M2 ChangePlan·ImpactAnalysis·`readiness=ready` ValidationPlan
- Cargo workspace/package/target/feature inventory와 handwritten/generated/vendor ownership
- project-pinned stable toolchain source, cargo/rustc/rustfmt/clippy-driver executable identity
- parsing edition, resolved style edition, MSRV, host/required target triple과 rustfmt/Clippy config source
- exact Clippy lint ID fix allowlist, compatible feature/target/cfg coverage policy와 optional `personal_auto` standing grant
- M3 GatePolicy와 M4 isolated preview/PatchSet/SourceMutationPort contract

필수 Profile metadata는 다음과 같다.

| metadata | 값 |
|---|---|
| `pipeline` | `rust_style_v1@1`; ordered step와 adapter definition fingerprint 고정 |
| `tool_roles` | `star.rust.style.rustfmt.check`, `.rustfmt.rewrite`, `.clippy.check`, `.clippy.fix` |
| `scope_policy` | check는 default workspace read-only 가능, prepare/auto-apply는 package 또는 workspace 명시 required |
| `format_policy` | stable `cargo fmt` 우선, resolved stable style edition/config; formatting operation으로 분류 |
| `clippy_fix_policy` | exact lint ID + `MachineApplicable` + actual hunk 대응. group/wildcard와 lint suppression 변경 금지 |
| `coverage_policy` | package/target/feature/triple/cfg/ownership cell. `--all-features` 범용 기본값 금지 |
| `mutation_policy` | isolated preview의 handwritten `.rs` modify만; live checkout external mutator 금지 |
| `auto_policy` | `safe_default` 사용자 승인, `personal_auto` exact PatchSet policy ApprovalDecision + permit 전 candidate/pre `AUTO_PASS` + 성공 전 post `AUTO_PASS` |
| `required_evidence` | RustToolchainBinding·RustStylePolicySnapshot·RustStyleCoverageMatrix·ordered RustStyleStepExecution, complete diff·replay |
| `unknown_policy` | partial/unverified coverage, tool/config/source drift, side effect와 non-idempotence는 auto apply `BLOCK` |

Profile 단계 template은 다음 순서다.

1. Project·Cargo graph·toolchain·config·lint/allowlist policy를 resolve하고 fingerprint한다.
2. current fixed `cargo fmt <typed-scope> -- --check`와 coverage cell별 Clippy JSON Diagnostic을 isolated read-only subject에서 수집한다.
3. M1/M2가 package·target·feature·path scope, generated ownership과 affected Check를 확정한다.
4. exact base/current byte의 Star-Control-owned isolated preview를 만들고 source 밖 `CARGO_TARGET_DIR`을 사용한다.
5. `cargo fmt -> allowlisted cargo clippy --fix -> cargo fmt`를 실행해 step별 diff·Diagnostic·suggestion을 수집한다.
6. final complete filesystem diff에서 `.rs` modify 이외 operation, scope/generated/vendor/public/config/lockfile write와 unmatched hunk를 거부한다.
7. preview ChangeSet으로 M2 영향·검사 계획을 재계산하고 expected-after에서 전체 mutation pipeline replay가 operation 0인지 확인한다.
8. candidate fmt/Clippy 전체 required coverage와 M2 selected build/test/contract Check를 실행한다.
9. immutable PatchSet·reverse 자료와 그 candidate에 묶인 exact ApprovalRequest를 만든다.
10. `safe_default`는 사용자 exact decision을 기다리고 `personal_auto`는 standing grant ceiling과 candidate `AUTO_PASS`를 만족할 때 policy evaluator가 같은 ApprovalRequest를 해소한다. 승인 뒤 M3 `patch_pre_apply`가 `AUTO_PASS`일 때만 single-use permit을 만든다.
11. application이 single-use permit을 소비해 기존 M4 SourceMutationPort로 PatchSet byte만 target에 적용한다.
12. actual-after rescan, fmt/Clippy/affected Check와 `patch_post_apply`, complete EvidenceBundle·ReviewPack을 생성한다.

Profile은 다음을 하지 않는다.

- Cargo/rustfmt/Clippy를 재구현하거나 `star-rust-style.exe`, AI/OpenAI/browser/scheduler를 추가하지 않는다.
- 사용자 shell command를 저장·실행하거나 `cargo fix`, edition/MSRV/dependency migration을 style correction에 섞지 않는다.
- nightly·unstable rustfmt, 전체 Clippy group, `MaybeIncorrect`/`HasPlaceholders` suggestion을 자동 수정하지 않는다.
- `#[allow]`·lint level, Cargo/lock/config/toolchain, generated/vendor, non-`.rs` file과 file lifecycle을 바꾸지 않는다.
- component/target/package를 설치하거나 network download를 허용하지 않는다.
- `cargo clippy --fix --allow-dirty`를 live checkout에서 실행하지 않는다. isolated preview에서도 dirty manifest가 앞 rustfmt step과 정확히 대응되고 staged byte가 0일 때만 사용하며 `--allow-staged`·`--broken-code`는 금지한다.
- partial/stale/unverified/failed result나 post Gate 실패·partial apply를 성공으로 표시하지 않는다.

CLI surface는 `star style rust inspect|check|prepare|auto-apply`이고 Patch 조회·상태·복구는 기존 `patch show|status|recover`를 재사용한다. 현재 상태는 **M11 설계 확정·제품 구현 전**이며 M1→M2→M3→M4 제품 Gate가 실제 통과하기 전 source mutation 구현을 시작하지 않는다.
