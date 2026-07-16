# 검사·완료·증거

## 목표

검사는 많이 하는 것이 목적이 아니다. 결과가 맞는지 판단하는 데 실제로 필요한 검사만 선택하고, 작업 흐름을 불필요하게 늦추지 않아야 한다.

Project·ScanRun·Rule·Finding·Occurrence·ChangePlan·PatchSet·RecipeExecution·PatchApplication·ValidationResult의 공통 필드와 identity는 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md), ProjectCheckout·ProjectCatalogSnapshot·CodeIndexSnapshot과 tier·freshness 의미는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)이 소유한다. 이 문서는 그 자료가 검사·완료·증거 판단으로 연결되는 방식을 소유한다.

Star-Control 3단계의 실행 순서·rule family·ratchet·validator guard·Patch 적용 전후 의미는 [공통 검증·품질 Gate 상세 설계](../features/common-validation-gate.md)가 소유한다. 4단계의 Recipe selector·rewrite assurance·dry-run·idempotence·apply·복구 알고리즘은 [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md), 5단계 identity·lifecycle·binding·consumer는 [Managed Registry](managed-symbol-registry.md), 6단계 compatibility·docs/config/environment 판정은 [계약 호환성·환경](contract-compatibility-and-environment.md), 7단계 failure·supply-chain·dependency·Radar 의미는 [실패 재현·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md), 8단계 migration·performance·language/platform 의미는 [Migration·성능·언어·플랫폼 계약](migration-performance-and-platform.md), 9단계 project/worktree/merge/remote coordination은 [CrossRepo ChangeBundle](cross-repo-change-bundle.md), 10단계 release/evaluation 상태·Gate·판정은 [CI·Release·평가 정본](ci-release-evaluation-and-product-completion.md), 11단계 Rust style pipeline·coverage·policy 의미는 [Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)이 소유한다. 이 문서는 M3~11단계가 사용하는 evidence wire 계약의 유일한 정본이다. 아래 target field와 enum 확장은 **설계 확정 대상·제품 구현 전**이며 현재 Rust type·Schema·DB에 이미 존재한다고 해석하지 않는다.

## 공통 실행 계약

검사와 일반 도구 실행은 shell 문자열이 아니라 `TaskInvocation`으로 기록한다.

M3 validation에서 사용하는 `star.task-invocation` writer version은 `schema_version=2`이며 `executable_binding_fingerprint`가 필수다. v1 invocation은 historical display에는 사용할 수 있지만 current Check 실행 binding으로 자동 승격하지 않는다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `invocation_id` | typed ID | 한 실행 요청 |
| `tool_ref` | Catalog ref | 검증된 ToolDescriptor ID와 version |
| `executable` | logical string | v1 descriptor-owned executable label. 사용자 절대 path나 shell text 금지 |
| `executable_binding_fingerprint` | SHA-256 | M3 target ToolDescriptor locator·trust·protocol binding identity |
| `args` | string array | shell 재해석 없이 전달할 인자 |
| `cwd` | ProjectPathRef | Project root 안의 작업 위치 |
| `env_refs` | map<name, value/SecretRef> | 허용된 환경 이름과 값 reference |
| `stdin_ref` | optional ArtifactRef | 큰 입력 또는 민감하지 않은 입력 자료 |
| `timeout_ms` | positive integer | 강제 종료 전 한도 |
| `permission_action` | action ID | PermissionPlan에서 확인할 행동 |
| `idempotency_key` | string | 같은 side effect 중복 실행 방지 |
| `expected_exit_codes` | integer set | 성공으로 해석할 종료 code |
| `output_limits` | object | stdout·stderr와 artifact 크기 상한 |

Controller는 ToolDescriptor locator를 process memory에서 final executable path로 해석하고 `args`와 함께 process API에 전달한다. persisted `executable`은 descriptor-owned logical label이며 사용자 절대 path가 아니다. shell 또는 script host로만 표현된 검사는 직접 실행하지 않는다. 이미 등록·신뢰된 native EXE, package-manager EXE의 typed task ID 또는 `star_json_stdio_v1` adapter EXE로 표현할 수 있어야 하며 그렇지 않으면 unresolved다. 실행 시 opaque executable file identity·hash·version과 redacted locator label을 결과에 남기며 raw 개인 path는 저장하지 않는다.

## ValidationPlan 계약

ValidationPlan은 무엇을 왜 검사할지 실행 전에 정한다.

아래 `task_spec_ref`부터 `readiness`까지의 확장은 2단계 **목표 계약**이며 현재 P0 관리 Slice나 test runner에 구현됐다는 뜻이 아니다. 2단계 계산 규칙은 [변경 계획·영향 분석 정본](change-planning-and-impact.md)이 소유한다.

P-0031은 같은 schema ID의 v1 `capability_level=tracked_path_precursor`만 구현했다. 이 bounded contract는 changed file/source/class, direct unit, reverse consumer, adaptive profile과 이유, exact planned command, cache key 구성요소·execute/reuse 판정, uncertainty, independent-review trigger와 canonical evidence flow를 담는다. cache reuse와 AI 압축은 ValidationRun·GateDecision·EvidenceBundle·Diagnostic의 immutable ID·revision/sequence·canonical hash를 대조하며, 자유 문자열로 명령·종료 코드·소요시간을 바꾸지 않고 ValidationRun에서 파생한다. 아래 full M2 target field를 모두 구현했거나 `readiness=ready` full M2 plan을 만든다는 뜻은 아니다.

precursor의 목표 evidence 순서는 `ValidationPlan -> ValidationRun/Diagnostic/ValidationResult -> GateDecision -> EvidenceBundle -> AI compressed summary`로 고정한다. AI summary는 EvidenceBundle과 GateDecision의 exact ref가 검증된 뒤 명령, 종료 코드, 소요시간, 실패 요약과 남은 위험 수만 노출하며 raw log나 artifact 내용을 재판정하지 않는다. P-0035의 bounded `validation.run`은 exact tracked `scripts/validate.ps1`을 실행해 sealed plan과 native report를 대조하고, `evidence.get`은 그 immutable report path·hash를 재검증한다. 이 둘은 ready precursor지만 persisted cache, Diagnostic·ValidationResult normalization, GateDecision·EvidenceBundle writer는 아직 unavailable이다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `goal_id`, `run_id`, `stage_id` | optional typed ID | 관리 Goal/Run/Stage에 연결할 때의 범위. 독립 TaskSpec planning이면 모두 생략 가능 |
| `task_spec_ref` | DocumentRef | 사용자 입력 revision |
| `scope_revision` | integer | 검증할 변경·계획 revision |
| `scope_revision_ref` | DocumentRef | requested/analysis/change/validation scope의 immutable ref |
| `phase` | enum | M3 v2 `during_stage`, `stage_exit`, `goal_exit`, `patch_pre_apply`, `patch_post_apply`, `merge`, `release`; M8 v3 `migration_pre_execute`, `migration_post_execute`, `migration_post_rollback`, `performance_compare`, `language_cutover`; 9단계 v4 `change_bundle_prepare`, `change_bundle_goal_exit`; M10 v5 `release_preflight`, `release_build`, `release_verify`, `release_package`, `release_install_lifecycle`, `release_ready`, `release_publish_preflight`, `release_publish_verify`; M11 v6 `rust_style_candidate` 목표 추가 |
| `phase_subject_expectations` | PhaseSubjectExpectation array | target Project별 현재 또는 Patch 전·후에 기대하는 exact source/ChangeSet identity |
| `change_summary_ref` | ArtifactRef | 선택 근거가 된 변경 목록 |
| `change_set_refs` | DocumentRef array | project별 actual dirty ChangeSet |
| `impact_analysis_ref` | DocumentRef | direct/transitive·confirmed/possible과 risk path 근거 |
| `risk_level` | enum | 검사 강도를 정한 위험 등급 |
| `profile_refs` | ResolvedProfileRef array | M2가 해석한 planning·validation Profile ID/version/content hash, role과 activation evidence. ID byte-order 정렬 |
| `profile_resolution_fingerprint` | SHA-256 | parent closure, trigger input, 병합한 required Rule·Check·evidence와 policy floor의 canonical hash |
| `affected_scope` | AffectedScope array | project·package·workspace·full과 선택·promotion 근거 |
| `candidate_checks` | CheckCandidate array | 조사한 모든 family·descriptor와 applicability 결과 |
| `required_checks` | CheckPlan array | 실패하면 gate를 막는 검사 |
| `optional_checks` | CheckPlan array | 정보 보강용 검사 |
| `check_graph` | CheckGraph | CheckPlan ID의 requires edge, parallel group과 failure propagation |
| `omitted_checks` | OmittedCheck array | 가능한 검사 중 실행하지 않는 이유와 대체 증거 |
| `unresolved_checks` | UnresolvedCheck array | 필요하지만 descriptor·tool·permission을 해결하지 못한 검사 |
| `previous_success_comparisons` | PreviousSuccessComparison array | 과거 pass와 current delta의 compatibility 판정 |
| `fallback_decisions` | FallbackDecision array | package→workspace→project full 승격 또는 유지 근거 |
| `manual_observations` | ManualObservationPlan array | 사람이 실제 흐름을 확인해야 하는 항목 |
| `independent_review` | ReviewRequirement | 실행 context별 사람 또는 별도 Codex 검토 필요 여부와 범위 |
| `gate_policy` | GatePolicy | 결과를 완료·검토·차단으로 바꾸는 규칙 |
| `config_fingerprint` | SHA-256 | 선택에 사용한 EffectiveConfig |
| `catalog_snapshot_ref` | DocumentRef | Check·Tool·RiskPath 정의 근거 revision·hash |
| `managed_registry_expectations` | ManagedRegistryExpectation array | M5 변경일 때 before/expected-after snapshot, declaration·binding·consumer·lifecycle 기대값 |
| `selection_fingerprint` | SHA-256 | candidate, outcome, scope, fallback과 descriptor hash의 canonical fingerprint |
| `readiness` | enum | `draft`, `ready`, `blocked`, `invalidated` |

`CheckCandidate`는 check family, optional CheckDescriptor ref, applicability `applicable|not_applicable|unknown`, matched impact·risk·user evidence, 가능한 coverage unit, resolution outcome과 reason code를 가진다.

`CheckPlan`은 stable plan item ID, CheckDescriptor·ToolDescriptor ref, outcome `selected_required|selected_optional`, 선택 이유와 ImpactEdge·RiskPath ref, project·package·workspace scope, typed TaskInvocation template, 기대 결과, timeout, retry, cache/reuse precondition, fallback floor와 생성할 evidence 종류를 가진다. invocation의 scope argument는 descriptor가 선언한 binding으로만 만들며 package 이름을 shell text에 삽입하지 않는다.

`CheckGraph`는 CheckPlan ID node, `requires|provides_input|must_run_after` edge, parallel group, group 동시성 상한과 `stop_dependents|continue_independent` failure policy를 가진다. cycle과 존재하지 않는 node reference는 plan 생성 전에 거부한다. optional Check가 required Check의 선행 입력이면 그 실행은 required로 승격한다.

`OmittedCheck`는 candidate ref, `not_applicable|compatible_previous_success|duplicate_coverage|user_waived`, 근거와 대체 evidence, remaining risk와 gate 영향을 가진다. `not_applicable`은 complete metadata로 descriptor expression이 false인 경우에만 허용한다.

`UnresolvedCheck`는 family, `descriptor_not_found|tool_unavailable|untrusted|permission_blocked|scope_unbindable`, searched Catalog scope, 필요한 coverage, 대체 수동 관찰과 readiness 영향을 가진다. required family의 unresolved 상태를 omitted로 옮기지 않는다.

`AffectedScope`와 `FallbackDecision`은 requested scope, selected scope, `package|workspace|project_full` level, trigger, graph closure·risk floor evidence와 limitation을 가진다. `project_full`은 affected Project 하나의 full Check이지 등록된 모든 Project의 전체 검사가 아니다.

`PreviousSuccessComparison`은 이전 ValidationResult·GateDecision·CheckPlan·source revision, ancestor/manifest lineage, descriptor·tool·config·scope compatibility, `previous_success_delta` ChangeSet ref, invalidation rule 결과와 `reusable|incompatible|unknown` 판정을 가진다.

`ReviewRequirement`는 `required`, `review_kind=none|human_semantic|codex_independent`, 적용 risk·scope, required evidence, allowed executor set과 `absence_behavior=human_review|block`을 가진다. `execution_context=cli_only`에서는 `codex_independent`를 요구하거나 Codex·AI adapter를 시작하지 않는다. 결정적 검사 뒤 의미 판단이 필요하면 `review_kind=human_semantic`, GateDecision `human_review`로 남긴다. Codex-managed context의 독립 review도 보조 evidence이며 결정적 Check·GateDecision을 대체하지 않는다.

`ResolvedProfileRef`는 Profile ID·version·descriptor content hash, `planning|validation` role, parent closure, activation reason `explicit|default|task_kind|change_class|impact_risk|mandatory_auto_mutation`, 그 reason을 증명한 TaskSpec·ChangeSet·ImpactAnalysis ref를 가진다. M2는 Profile closure를 먼저 확정한 뒤 그 required Rule·Check family를 candidate set에 합치고 `profile_resolution_fingerprint`를 계산한다. M3 runner는 이 closure를 추가·삭제·재평가하지 않는다. actual ChangeSet이 새로운 Profile trigger를 활성화하거나 trigger evidence가 달라지면 plan은 stale이며 M2 재계획 대상이다.

`PhaseSubjectExpectation`은 ProjectId·CheckoutId, `expectation_kind=exact_current|patch_before|patch_expected_after`, base ProjectRevision·before WorkspaceSnapshot ref, expected workspace content fingerprint, expected ChangeSet 또는 PatchSet operation-manifest fingerprint와 collection scope를 가진다. `exact_current|patch_before`는 계획 시 관찰된 byte snapshot과 같아야 한다. `patch_expected_after`는 immutable PatchSet의 각 operation before/after hash·mode·existence를 canonical 적용해 계산한 expected after fingerprint이며 실제 apply 성공을 뜻하지 않는다. after snapshot은 적용 뒤 별도로 수집하고 expected fingerprint와 exact 비교해야 한다. patch가 binary·generator·도구 side effect처럼 expected byte를 결정할 수 없으면 post expectation을 `unverified`로 만들지 않고 post ValidationPlan readiness를 `blocked` 또는 `human_review`로 두며 자동 apply/완료에 사용하지 않는다.

### 계획 불변식

1. 모든 필수 Check와 Tool reference는 같은 CatalogSnapshot에서 해석되어야 한다.
2. 위험을 낮게 적어 필수 검사를 피할 수 없다. 위험 결정과 검사 선택 근거를 함께 남긴다.
3. `not_run`은 `pass`가 아니며 대체 증거가 있어도 원래 검사 상태는 바꾸지 않는다.
4. 변경 revision이 달라지면 영향받는 ValidationPlan을 다시 계산한다.
5. 검사 자체가 파일·외부 상태를 바꾸면 별도 Permission action과 변경 증거를 가진다.
6. `ready` plan은 required candidate가 모두 selected 또는 valid waiver로 설명되고 `unresolved_checks`가 gate policy상 실행을 막지 않아야 한다.
7. `not_found`는 `not_required`가 아니다. required test·build·contract family를 찾지 못하면 `blocked` 또는 `human_review`다.
8. ChangePlan과 ValidationPlan의 TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet fingerprint는 같아야 한다.
9. 사용자 override는 자동 선택보다 우선하지만 required Check 생략은 Waiver·remaining risk와 human review를 남긴다.
10. `scope_revision` integer는 `scope_revision_ref.revision`과 같아야 하며 legacy 숫자 field가 다른 ScopeRevision을 암시하면 plan을 거부한다.
11. CheckGraph는 acyclic이어야 하고 모든 required Check는 실행 가능한 dependency closure를 가져야 한다.
12. `patch_pre_apply`와 `patch_post_apply`는 같은 ChangePlan·PatchSet lineage를 사용하고 phase별 subject WorkspaceSnapshot을 명시해야 한다.
13. CLI-only plan의 `independent_review`가 Codex·AI executor를 required로 선언하면 plan은 invalid다. human semantic review는 실행 Check가 아니라 Gate 대기 조건이다.
14. `profile_refs`의 parent closure·activation evidence·required family union이 CatalogSnapshot과 일치해야 한다. Profile closure가 바뀌거나 actual change class가 미계획 Profile을 요구하면 runner가 Check를 추가하지 않고 plan을 `invalidated`로 만든다.
15. 모든 target Project에는 phase와 맞는 `PhaseSubjectExpectation`이 정확히 하나 있어야 한다. pre-apply는 `patch_before`, post-apply는 같은 PatchSet lineage의 `patch_expected_after`만 허용하며 actual mismatch를 새 정상 plan으로 자동 흡수하지 않는다.

## ChangeSet 계약

ChangeSet은 사용자의 기존 변경과 Star-Control이 만든 변경을 분리해 영향 분석, 검사 선택과 병합이 같은 자료를 보게 한다.

2단계 planning baseline에서 ChangeSet은 source를 수정하지 않고 ProjectRevision과 실제 dirty WorkspaceSnapshot의 차이를 관찰한다. project·checkout마다 별도 ChangeSet을 만들며 multi-project 상위 문서는 reference만 묶는다.

PatchSet과 ChangeSet은 같은 뜻이 아니다. PatchSet은 한 ChangePlan이 **제안한 immutable preview**이고, ChangeSet은 적용 전부터 존재한 사용자 변경을 포함해 workspace에서 **실제로 관찰한 전체 변경**이다. `patch.prepare`는 source를 바꾸지 않고 `patch.apply`는 patch fingerprint와 permission에 대한 명시적 승인을 요구한다. PatchSet을 적용한 뒤 새 WorkspaceSnapshot과 ChangeSet을 다시 수집해야 하며 두 fingerprint가 다르면 실제 상태를 우선하고 차이를 진단한다.

| 필드 | 의미 |
|---|---|
| `change_set_id` | 문서 ID |
| `task_spec_ref`, `scope_revision_ref` | 이 actual comparison을 분류한 사용자 의도와 scope revision |
| `project_id`, `run_id`, `stage_id` | Project는 필수. Run·Stage는 관리 실행에 연결할 때만 사용하며 독립 TaskSpec planning이면 생략 |
| `checkout_id` | 실제 working copy identity |
| `change_set_kind` | `planning_baseline`, `previous_success_delta`, `recipe_preview`(M4 target), `observed_after_change`, `merge_result` |
| `base_revision` | 작업 시작 기준 ProjectRevisionId·commit·filesystem fingerprint |
| `base_workspace_snapshot_id` | 비교 시작 dirty snapshot이 따로 있을 때의 ref |
| `observed_revision` | 수집 시점 WorkspaceSnapshotId와 entries fingerprint |
| `comparison_scope` | 수집한 Project path/class 범위와 exclusion·ignore provenance |
| `entries` | add, modify, delete, rename, mode, binary, submodule 변경 |
| `preexisting_entries` | 시작 전 사용자 변경 reference |
| `classifications` | source, test, docs, config, schema, migration, generated, vendor 등 |
| `scope_relation` | planned, necessary_expansion, unrelated, unknown |
| `collection_limits` | 도구 미지원·미확인 영역 |
| `collection_state` | `complete`, `partial`, `unverified` |
| `change_set_fingerprint` | source comparison·entry·classification·origin의 canonical hash |

각 entry는 stable entry ID, ProjectPathRef, 전·후 hash·mode·존재 상태, 변경 종류, staged·unstaged·untracked metadata, 가능한 line/range 통계, rename source와 근거, binary 여부, source class/facet, ownership·생성 주체를 가진다. `origin=preexisting|task_declared|tool_applied|unknown`과 `scope_relation`은 별도 축이다. 사용자가 preexisting dirty entry를 이번 task에 포함해도 실제 origin은 바꾸지 않는다.

`recipe_preview`는 live target checkout의 실제 변경이 아니다. exact before snapshot을 materialize한 Controller preview root 또는 isolated worktree에서 관찰한 before/after이며 `preview_workspace_ref`, RecipeExecution ref와 original target binding을 필수로 가진다. 이 ChangeSet은 M2 impact·Profile·affected Check reconciliation input으로만 쓰고 target `observed_after_change`나 적용 성공으로 재분류하지 않는다.

전체 diff는 ArtifactRef로 분리한다. source byte, secret, 개인 path와 민감 literal을 entry에 넣지 않는다. `unrelated`나 `unknown`을 자동으로 Star-Control 변경으로 덮거나 되돌리지 않는다. `entries=[]`는 collection scope와 `collection_state=complete`가 있을 때만 유효하며 “계획된 변경이 필요 없음”을 뜻하지 않는다.

`change_set_fingerprint`는 TaskSpec·ScopeRevision ref, ProjectId·CheckoutId, base ProjectRevision, base/observed WorkspaceSnapshot, normalized comparison scope, 정렬된 entry의 before/after identity·origin·scope relation·classification과 collection limitation을 포함한다. timestamp·rendered diff·line 통계만의 변화는 identity에서 제외한다. ImpactEdge와 RiskPathFinding은 이 immutable ChangeSet을 입력으로 계산하는 ImpactAnalysis output이며 ChangeSet에 나중에 backfill하지 않는다.

dirty workspace에서 PatchSet을 적용하려면 대상마다 exact before hash와 mode가 일치하고 기존 ChangeSet과 byte range·rename·delete overlap이 없어야 한다. 대상 밖 pre-existing change는 새 ChangeSet에서도 동일해야 한다. overlap이면 별도 worktree 또는 block이며, 자동 rollback은 exact reverse operation과 적용 직후 hash가 모두 증명될 때만 허용한다.

## CompletionClaim과 actual 비교

`CompletionClaim`은 사용자·Codex·도구가 보고한 완료 주장을 typed record로 보존한다. 주장은 source truth가 아니며 actual ChangeSet·ValidationRun·contract evidence와 비교할 입력이다. M3 target에서는 EvidenceBundle과 ReviewPack이 정렬된 claim·evaluation을 포함한다.

| 필드 | 의미 |
|---|---|
| `claim_id` | claim set 안의 stable ID |
| `kind` | `change`, `check_executed`, `bug_fixed`, `compatibility`, `generated_current`, `docs_current`, `registry_current`, `other` |
| `subject` | Project·path·CheckPlan·Contract·Schema·Finding 같은 typed ref |
| `assertion` | add·modify·delete·rename, pass, fixed 같은 typed assertion. 자유 형식 text는 표시 설명으로만 분리 |
| `reported_evidence_refs` | 주장자가 제시한 ChangeSet·ValidationRun·ArtifactRef |
| `reported_subject_binding` | evidence가 가리킨 revision·config·Catalog·Tool identity |
| `source_actor` | 사용자·Codex·tool ActorRef |
| `created_at` | 표시·audit 시각, identity에서는 제외 |

`ClaimEvaluation`은 `claim_ref`, current `EvidenceSubjectBinding`, actual evidence refs, `status=verified|contradicted|unverified|stale|not_applicable`, Diagnostic refs와 evaluation fingerprint를 가진다.

- actual add·modify·delete·rename 종류와 after identity가 다르면 `contradicted`다.
- required scope를 관찰하지 못했으면 `unverified`이며 `verified`로 추정하지 않는다.
- reported evidence가 다른 source·plan·config·Catalog·Tool binding이면 `stale`이다.
- `not_applicable`은 current·complete applicability evidence가 있을 때만 허용한다.
- `contradicted` required completion claim은 `auto_pass`를 막는다.
- claim text를 parsing해 사실을 만들지 않는다. typed assertion이 없으면 `unverified`다.

### ImpactEdge 계약

ImpactEdge는 ImpactAnalysis의 project-scoped detail record다. 별도 top-level document를 만들지 않으며 ChangeSet에는 포함하지 않는다. ChangePlan·ValidationPlan과 전역 ImpactAnalysis summary는 owning project detail의 fingerprinted ref만 사용한다.

| 필드 | 의미 |
|---|---|
| `edge_id` | from·relation·to·evidence set의 content-derived ID |
| `project_id` | detail을 소유한 Project |
| `from`, `to` | typed IndexEntity key 또는 unresolved target |
| `relation` | contains, defines, references, tests, documents, depends_on, exposes, generates 등 |
| `direction` | 영향 전파에 사용한 source→target 방향 |
| `impact_kind` | `direct`, `transitive` |
| `distance` | seed에서 허용 relation 수. seed 자체는 0 |
| `certainty` | `confirmed`, `possible` |
| `confidence` | `high`, `medium`, `low` |
| `resolution` | `resolved`, `ambiguous`, `unresolved`, `external` |
| `tier` | 실제 `text`, `syntax`, `semantic`, `declared` evidence tier |
| `freshness` | 사용 partition의 current·stale·partial·unverified state |
| `evidence_refs` | source range·manifest·descriptor·IndexEdge ref |
| `path_edge_ids` | transitive 대표 path의 ordered edge ID |
| `limitations` | fallback, excluded scope, dynamic/reflection, resource limit |
| `content_fingerprint` | 의미 field의 canonical hash |

confirmed edge는 current·complete required partition, resolved target과 relation별 minimum evidence를 모두 요구한다. text literal equality, ambiguous target와 stale graph는 possible만 만들 수 있다. 같은 literal도 ProjectId·owning Symbol/Contract key가 다르면 서로 다른 node·edge다.

## Scan·Finding·Patch 관계

```text
ProjectCatalogSnapshot + ProjectCheckout
  -> ProjectRevision + WorkspaceSnapshot + EffectiveConfig + adapter set + Rule set
  -> ScanRun
       -> CodeIndexSnapshot(inventory, text, syntax, semantic, graph, finding)
       -> Occurrence -> Finding
             + Baseline
             + Suppression
             + Disposition
       -> ChangeRecipe -> ChangePlan -> RecipeExecution* -> PatchSet
            -> patch_pre_apply Gate -> PatchApplication
            -> 적용 뒤 WorkspaceSnapshot + ChangeSet
            -> ValidationRun* -> ValidationResult
            -> patch_post_apply GateDecision
```

관계 불변식은 다음과 같다.

1. Occurrence는 정확히 하나의 ScanRun과 Finding을 가리키며 source content hash와 WorkspaceSnapshotId를 가진다.
2. Finding은 관찰 사실이고 suppression·baseline·disposition은 별도 decision이다. decision 때문에 Finding이나 Diagnostic 원문을 삭제·수정하지 않는다.
3. Baseline은 complete ScanRun을 검토한 뒤 명시적으로 생성하는 `existing/new` 비교 기준이지 pass나 suppression이 아니다. shared Baseline은 Git 선언이 정본이다.
4. ChangePlan은 Finding fingerprint, target WorkspaceSnapshot, Recipe fingerprint와 config fingerprint를 precondition으로 고정한다.
5. PatchSet의 큰 diff·binary delta는 ArtifactRef이며 DB에는 file operation, 전·후 hash와 artifact metadata만 저장한다.
6. patch 적용 뒤 source hash가 예상과 다르면 `partially_applied` 또는 실패이며 성공 ValidationResult를 만들지 않는다.
7. Rule이 만든 Finding과 Check 실행이 만든 Diagnostic은 같은 stable RuleId를 공유할 수 있지만, Diagnostic instance를 Finding aggregate로 대체하지 않는다.
8. scan이 `incomplete`이면 관찰되지 않은 기존 Finding을 resolved로 바꾸지 않으며 해당 범위의 GateDecision은 자동 통과할 수 없다.
9. Suppression 기본 만료는 90일이며 permanent flag·justification·승인이 없는 무기한 suppression은 invalid다. Disposition은 local state이고 공유하려면 Baseline 또는 Suppression PatchSet을 만든다.
10. secret·사용자 이름·raw 절대 경로·민감 literal 때문에 occurrence를 안전하게 저장하지 못하면 원문과 hash를 폐기하고 ScanRun·ValidationResult completeness를 낮춘다. redaction 누락을 pass로 해석하지 않는다.
11. ScanRun은 사용한 ProjectCatalogSnapshot, CheckoutId, requested/effective scan mode, source 분석용 `analysis_input_fingerprint`, 판정 join용 `decision_projection_fingerprint`와 결과 CodeIndexSnapshot을 함께 고정한다. 두 fingerprint는 같은 값으로 합치지 않는다.
12. dirty working tree에서는 default branch나 HEAD가 아니라 실제로 읽은 tracked modification·staged·untracked byte가 WorkspaceSnapshot과 Finding의 source 근거다.
13. parse 실패, unsupported language, limit 초과, adapter unavailable과 no-result는 각각 상태·count·limitation으로 남기며 0건 성공 하나로 합치지 않는다.
14. hardcoding Finding은 자동 결함 판정이 아니다. Rule이 만든 관찰과 `candidate`, `warning`, `review`, `allowed` assessment를 분리하고 assessment 변경은 원래 evidence를 수정하지 않는다.
15. M4 PatchSet은 immutable preview이며 실제 적용·partial·recovery 상태는 PatchApplication에 기록한다. historical v1 PatchSet status를 current apply evidence로 사용하지 않는다.
16. RecipeExecution은 exact Recipe·input·selector·transformer·Tool identity와 preview output을, PatchApplication은 실제 operation receipt·after snapshot·recovery를 소유한다. 둘의 evidence를 하나의 성공 boolean으로 합치지 않는다.
17. ManagedRegistrySnapshot은 Git manifest의 derived Index다. source와 다르면 stale이며 더 최근 DB timestamp나 과거 valid snapshot을 current Registry evidence로 사용하지 않는다.

## 4단계 Recipe·Patch evidence 계약

M4 target은 `star.recipe-execution` v1, `star.patch-set` v2와 `star.patch-application` v1을 사용한다. full transform·apply state machine은 [4단계 엔진 계약](safe-patch-and-codemod.md#document-graph)이 소유하며 이 절은 Gate와 EvidenceBundle이 요구하는 최소 binding만 고정한다.

### RecipeExecution evidence

RecipeExecution은 최소 다음을 가진다.

- Recipe ID·SemVer·definition fingerprint와 CatalogSnapshot source
- ChangePlan·resolved TargetSelector binding과 base EvidenceSubjectBinding
- normalized redacted input fingerprint와 input ArtifactRef
- rewrite kind·assurance, built-in/language adapter ID·version·hash 또는 ToolDescriptor·Registry·executable version/full hash
- external이면 exact TaskInvocation, process start/outcome/termination/completeness와 stdout·stderr·result ArtifactRef
- materialized/isolated preview workspace opaque ref와 before manifest
- actual `recipe_preview` ChangeSet, output manifest와 expected postcondition evaluation
- preview·idempotence replay attempt relation, limitation과 execution fingerprint

timeout, cancellation, process start failure, malformed output, output limit, undeclared file effect와 outcome unknown은 successful RecipeExecution이 아니다. 실패한 isolated diff를 PatchSet으로 승격하지 않는다.

### PatchApplication evidence

PatchApplication은 최소 다음을 가진다.

- immutable PatchSet v2 ref·fingerprint, Project·Checkout과 WorktreeDecision
- pre-apply GateDecision·subject binding set과 persisted permit binding fingerprint. in-memory permit token은 저장하지 않음
- effect intent, target mutation lock과 operation별 started/completed receipt
- actual operation manifest, applied WorkspaceSnapshot과 `observed_after_change` ChangeSet
- post-apply GateDecision, selected Check evidence와 EvidenceBundle ref
- `failed_before_effect|partially_applied|outcome_unknown|recovery_required`의 raw 상태
- exact reverse PatchSet 또는 owned isolated worktree discard eligibility, recovery attempt·result

multi-file atomicity를 주장하지 않는다. partial/outcome unknown은 성공 result가 아니며 actual source reconciliation 전 같은 apply를 재실행하지 않는다.

## 5단계 Managed Registry evidence 계약

Registry 변경 evidence는 일반 M2·M4·M3 chain에 다음 정보를 추가한다. field의 full wire 의미와 drift vocabulary는 [Managed Registry 정본](managed-symbol-registry.md)이 소유한다.

- before/expected-after/actual-after ManagedRegistrySnapshot ref와 authoritative manifest hash
- 대상 ManagedDeclaration ID·item version·before/after definition fingerprint
- namespace claim, alias와 removed/reserved tombstone set fingerprint
- definition/reference·Schema·documentation·generated output BindingSpec/observation
- consumer Project, minimum supported version, accepted declaration version·required binding과 transition status
- codegen/codemod 선택, generator/Recipe/Tool identity와 declared/actual output manifest
- RegistryConsistencyRecord set, compatibility result와 blocking Diagnostic refs

candidate promotion decision은 분류·owner를 승인한 actor와 exact source proposal을 남기되 scanner literal 원문을 DB 정본으로 저장하지 않는다. local implementation constant는 exclusion evidence로 참조할 수 있지만 Registry-owned change target이 아니다. large rendered manifest diff와 consumer migration matrix는 ArtifactRef로 분리한다.

Registry evidence completeness는 manifest·namespace·binding·consumer·lifecycle 각각을 `complete|partial|unverified`로 가진다. 한 축이라도 required인데 current·complete하지 않으면 `registry_current`와 removal compatibility를 verified로 만들 수 없다.

## 6단계 compatibility·문서·config·환경 evidence 계약

6단계 evidence는 M3 결과 모델을 재정의하지 않고 [계약 호환성·문서·설정·개발 환경 관리](contract-compatibility-and-environment.md)의 산출물을 exact subject에 결합한다.

| evidence 축 | required ref·fingerprint | complete 조건 |
|---|---|---|
| contract source | `ProjectContractManifest`, baseline approval·artifact, current source refs | baseline이 immutable·승인됨, 모든 required surface source가 current |
| comparison | baseline/current `ContractSurfaceSnapshot`, `CompatibilityReport` | kind별 rule·evidence·consumer·migration·companion set이 모두 평가됨 |
| Registry | `ManagedRegistrySnapshot`, declaration/consumer refs, consistency set | M5 manifest·binding·consumer·lifecycle required 축 complete |
| documentation | `DocumentationSnapshot` | required link·anchor·command·snippet·config example·Schema/generated ref·assumption이 빠짐없이 관찰됨 |
| config | `ConfigKeyTrace` set, EffectiveConfig fingerprint | declaration·Schema·docs·semantic reader·override provenance·consumer/lifecycle required 축 complete |
| environment | `EnvironmentSnapshot`, `ProjectDoctorReport` | registered read-only probe만 사용하고 constraint별 result·limitation이 있음 |
| clean-room | applicable `CleanRoomSpecification`, readiness와 selected Check result | 명세 필수값, immutable source·lockfile·registered command·금지 행동이 확인됨 |
| 7단계 handoff | `DependencySecurityInputManifest` | manifest·lockfile·toolchain·package manager·environment provenance/coverage/freshness가 있음 |

각 축은 `complete|partial|unverified|not_required`를 가진다. `not_required`는 Profile·surface·ChangePlan rule ref가 있어야 하고 단순 누락을 뜻하지 않는다. raw environment variable 값, secret, username, home/temp path와 전체 command output은 inline evidence나 fingerprint에 넣지 않는다.

compatibility `unknown`은 두 원인을 구분한다.

- required evidence가 missing/stale/partial/unverified: `BLOCK` 또는 재계획·재수집
- evidence는 complete하지만 API/Schema/CLI 의미를 결정적 rule로 확정 불가: exact 질문과 영향이 있는 `HUMAN_REVIEW`

둘을 같은 manual pass로 합치지 않는다. doctor가 network/download/install/system/source mutation을 필요로 하거나 시도하면 해당 Check는 `not_run|tool_error`, side-effect Diagnostic과 `BLOCK`을 남긴다.

## 7단계 failure·security·dependency evidence 계약

7단계는 공통 Finding·Diagnostic·ArtifactRef를 재사용하며 도구별 DB나 별도 완료 모델을 만들지 않는다. persisted 의미는 [7단계 정본](failure-security-and-dependency-maintenance.md)이 소유하고, 이 절은 M3 evidence 결합만 소유한다.

| evidence 축 | required ref·fingerprint | complete 조건 |
|---|---|---|
| failure identity | `FailureRecord`와 normalization/fingerprint rule | family·occurrence fingerprint, root candidate/cascade, exact subject가 있음 |
| reproduction | `ReproductionPack`과 attempt/artifact refs | command·structured args·environment·input/seed·expected/actual·redaction·limitation이 있음 |
| regression | `RegressionRecord` | compatible before failure·after success 또는 unverified 이유가 명시됨 |
| recovery | `RecoveryPlan`과 optional `RecoveryAttempt` | rollback·roll-forward·restore가 분리되고 validation·stop condition이 있음 |
| dependency | `DependencySnapshot` | manifest·lockfile·package manager·relation·status·coverage가 exact subject에 묶임 |
| supply chain | `SupplyChainSnapshot` | secret/redaction, sensitive change, workflow, release와 dependency evidence가 있음 |
| external data | `ExternalDataSnapshot` set | source/query/schema, tool, coverage, freshness와 valid_until이 있음 |
| update | `DependencyUpdatePlan`, PatchSet과 before lockfile ref | approval state, actual preview diff, replan, rollback과 Gate lineage가 있음 |
| maintenance | `MaintenanceRadarSnapshot` | input refs, evaluation_time, valid_until과 deterministic sort key가 있음 |

일반 ValidationRun log와 ReproductionPack은 같은 artifact를 참조할 수 있지만 `artifact_role=general_log|reproduction_required`를 구분한다. `quarantined|unknown` redaction 상태는 default ReviewPack에 포함하지 않으며, required external data가 `stale|unknown|unavailable`이면 current security pass evidence가 아니다.

scanner·debugger·package manager output은 producer evidence다. adapter의 success·exit 0·“no findings”·“update complete”는 GateDecision을 만들지 못하며 M3가 subject, coverage, freshness, permission과 required evidence를 다시 판정한다.

## 8단계 migration·performance·language/platform evidence 계약

M8은 domain result를 공통 Gate에 결합하며 별도 성공/완료 DB를 만들지 않는다. persisted document field와 상태기계는 [8단계 정본](migration-performance-and-platform.md)이 소유한다.

| evidence 축 | required ref·fingerprint | complete 조건 |
|---|---|---|
| migration declaration | `ProjectMigrationManifest`, target/version source | manifest·target·current/target vector·coverage가 current |
| chain·plan | `MigrationPlan` | unique continuous chain, invariant, strategy, permission·rollback과 plan fingerprint가 있음 |
| dry-run | `MigrationAttempt(phase=dry_run)` | live target write 0, expected change/loss/unknown/resource/consumer 결과가 complete |
| backup·restore | backup manifest와 `RestoreVerificationRecord` | consistent backup integrity와 required restore rehearsal/behavior가 별도 상태로 verified |
| rehearsal | restore/migration rehearsal attempt·checkpoint | execute plan과 같은 chain·tool·compatible environment에서 complete·stable |
| execute·resume | `MigrationAttempt`, `MigrationCheckpoint` set | actual receipt·before/after probe와 ordered durable prefix가 일치 |
| migration validation | `MigrationValidationReport` | target version, required invariant·consumer Check와 actual active state가 current |
| rollback | M7 RecoveryPlan과 rollback/restore attempt | before-compatible state와 post-rollback invariant·Gate가 있음 |
| performance spec | `PerformanceWorkloadSpec` | explicit activation, workload/input/tool/environment/mode·metric/noise protocol이 고정됨 |
| performance samples | baseline/candidate `PerformanceRun` set | cohort 내부 exact revision, numeric unit·collector, warmup/measured·raw attempt가 있음 |
| performance comparison | `PerformanceComparison` | comparability, noise/outlier, correctness·trade-off와 metric decision이 complete |
| language plan | `LanguageMigrationPlan` | behavior baseline, boundary/coexistence/consumer/cutover/window/rollback이 있음 |
| equivalence | `EquivalenceReport` | required dimension·platform·consumer가 current complete stable 또는 exact review 질문을 가짐 |
| 9단계 handoff | `CrossProjectMigrationHandoff` | project별 plan·PatchSet·Gate·restore/rollback과 dependency edge가 있고 실행 권한은 없음 |

각 축은 `complete|partial|unverified|not_required`를 가진다. migration `partially_succeeded|outcome_unknown|rollback_failed`, performance `no_measurement|not_comparable|noisy|inconclusive`, language equivalence `partial|unverified|not_equivalent`를 success/pass로 정규화하지 않는다.

backup byte 존재와 restore 검증을 하나의 boolean으로 만들지 않는다. `created_unverified|integrity_verified|restore_rehearsed|restore_validated`의 exact source record를 보존하고 required 수준보다 낮으면 pre-execute Gate를 만족하지 못한다.

## Project Catalog·Code Index freshness 근거

이 절은 1단계 read-only scan이 “현재 source를 얼마나 보았는가”를 검증 가능한 증거로 만드는 규칙을 소유한다. 구체적인 entity·partition 필드는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)을 반복하지 않고 참조한다.

아래 항목은 1단계 목표 evidence 계약이며 현재 ScanRun Schema나 DB에 구현된 것으로 읽지 않는다.

### ScanRun evidence 최소 집합

| 묶음 | 필수 근거 |
|---|---|
| 대상 | ProjectId, CheckoutId, ProjectCatalogSnapshotId, ProjectRevisionId, WorkspaceSnapshotId |
| 계획 | `requested_mode`, `effective_mode`, full 승격 이유, normalized scope, required/max tier, limit과 required/optional expected partition |
| 의미 입력 | EffectiveConfig, discovery·classification·scan·index·Rule·adapter set fingerprint, source 분석용 `analysis_input_fingerprint`, 판정 join용 `decision_projection_fingerprint` |
| source 관찰 | Git object format·HEAD if any, porcelain status fingerprint, tracked·untracked actual content manifest, non-Git manifest fingerprint |
| 결과 | CodeIndexSnapshotId, partition별 status·input/content fingerprint·count·coverage, 전체 completeness |
| 품질 | 언어별 requested/used tier, parse·resolution count, ambiguous·unresolved·unsupported·excluded·quarantined count |
| 실행 | started/finished time, adapter identity·version, batch ordinal·fingerprint, cache reuse provenance |
| 실패 | stable error code, affected scope, previous current generation 유지 여부, retry 또는 full-scan requirement |

`cache_hit=true`, process exit code 0 또는 Finding 0건만으로 complete를 증명하지 않는다. expected source와 partition을 모두 관찰했고 batch·reference 무결성, redaction과 limit 조건을 통과해야 `complete`다. incomplete·failed scan은 이전 complete generation의 current pointer를 바꾸지 않는다. 사용자가 incomplete snapshot을 조회하면 그 snapshot 자체와 이전 current snapshot을 구분해 반환한다.

### freshness proof

current 판정은 저장 시각의 나이만 비교하지 않고 index input과 **지금 관찰한 source**를 대조한다.

```text
indexed input
  = checkout identity
  + ProjectRevisionId
  + WorkspaceSnapshotId와 entries fingerprint
  + discovery·classification·scan·index·Rule·adapter fingerprints

current probe
  = 같은 checkout의 현재 HEAD/object format/status
  + 영향 대상 file content hash 또는 bounded filesystem manifest
  + 현재 effective fingerprint set
```

모든 의미 입력이 같고 probe가 complete일 때만 `current`다. 불일치는 각각 `stale_source`, `stale_config`, `stale_adapter`, `stale_catalog`로 분류한다. 일부 partition만 일치하면 snapshot 전체를 current로 올리지 않고 `partial`과 partition별 상태를 반환한다. root 접근 실패·Git status 실패·한도 초과처럼 비교 자체를 끝내지 못하면 `unverified`, attached checkout이 없으면 `unavailable`이다.

stale evidence에는 최소 `indexed_value_ref`, redaction한 `observed_value_ref`, `detected_at`, affected partition, `required_action=incremental_scan|full_scan|reattach|adapter_restore`를 둔다. timestamp 차이만으로 stale reason을 만들지 않는다. 반대로 source/config가 달라졌는데 최근에 실행했다는 이유로 current를 유지하지 않는다.

### query와 Finding 근거

index query 결과는 `snapshot_ref`, `freshness`, `requested_tier`, `used_tier`, `coverage`, `resolution`, `confidence`, `limitations`를 항상 가진다. semantic이 syntax나 text로 fallback하면 실제 tier를 보존하고 semantic 결과처럼 gate·영향 분석에 사용하지 않는다. definition/reference 0건은 `confirmed_empty`, `partial`, `unsupported_language`, `semantic_unavailable`을 구분하며 query 실행 자체가 실패하면 빈 결과가 아니라 stable `query_error`를 반환한다.

hardcoding Occurrence는 다음을 모두 가진다.

- stable Rule ID·version·definition·parameter fingerprint
- source class와 fixture·generated·vendor·docs-example facet, 그 classification provenance
- secret을 제거한 source range·evidence kind·주변 symbol/config relation
- candidate category, confidence, false-positive control과 limitation
- assessment와 assessor·reason·revision; `allowed`도 관찰 evidence를 삭제하지 않음

source literal이 secret·개인 path·민감 값일 수 있으면 원문과 그 hash를 저장하지 않는다. redacted category·위치·관계 근거만으로 identity를 만들 수 없으면 occurrence를 `quarantined`로 계수하고 scan completeness를 낮춘다.

### gate 사용 규칙

- gate policy가 요구한 scope·tier에서 `current + complete`인 required partition만 자동 gate의 positive evidence가 될 수 있다.
- required evidence의 `stale_*`, `partial`, `unverified`, `unavailable`, fallback tier와 unresolved edge는 `human_review` 또는 재scan 요구 근거이며 자동 통과 근거가 아니다. optional tier limitation은 숨기지 않고 remaining risk로 남기되 policy가 요구하지 않았다면 그것만으로 전체 gate를 block하지 않는다.
- hardcoding `candidate`는 그 자체로 block하지 않는다. 정책 threshold를 넘긴 `warning`, 명시적 `review`, 또는 별도 validator가 확정한 위반만 해당 gate policy에 따라 판단한다.
- 2단계 영향 분석이 stale graph를 받으면 confirmed impact를 만들지 않고 최신 scan을 요구하거나 possible impact와 limitation만 반환한다.

## Evidence subject binding

`EvidenceSubjectBinding`은 실행 evidence가 정확히 무엇을 검증했는지 고정하는 M3 target nested contract다. ValidationRun·ValidationResult·CompletionClaim과 GateDecision은 이 binding 또는 그 exact fingerprint를 사용한다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `project_id`, `checkout_id` | 예 | source ownership과 working copy |
| `project_revision_id`, `workspace_snapshot_id` | 예 | 실제 검증한 source revision·dirty byte snapshot |
| `workspace_content_fingerprint` | 예 | scope 안 actual entry의 canonical fingerprint |
| `task_spec_ref`, `scope_revision_ref` | M2/M3 plan 실행 시 | 사용자 입력과 accepted scope |
| `impact_analysis_ref`, `change_set_refs` | M2/M3 plan 실행 시 | 영향·actual change 입력 |
| `change_plan_refs` | source 변경 검증 시 | planned unit·postcondition |
| `patch_set_ref` | patch phase일 때 | immutable patch ID·revision·fingerprint |
| `recipe_execution_refs` | M4 preview/apply일 때 | exact Recipe·transformer·input/output lineage |
| `patch_application_ref` | apply/post/recovery일 때 | actual operation·receipt·recovery lineage |
| `managed_registry_snapshot_refs`, `managed_declaration_refs` | M5 Registry 검증 시 | before/expected-after/actual-after Registry와 대상 declaration lineage |
| `registry_consistency_fingerprint` | M5 Registry 검증 시 | binding·consumer·alias·lifecycle·drift record 집합 |
| `project_contract_manifest_ref`, `baseline_approval_ref` | M6 compatibility 검증 시 | surface 선언과 immutable baseline activation |
| `contract_surface_snapshot_refs`, `compatibility_report_ref` | M6 compatibility 검증 시 | baseline/current observation과 consumer·migration 판정 |
| `documentation_snapshot_ref`, `config_trace_fingerprint` | M6 B07 검증 시 | docs/config/generated/assumption 관찰 집합 |
| `environment_snapshot_ref`, `project_doctor_report_ref` | M6 doctor 검증 시 | redacted read-only environment와 constraint 판정 |
| `clean_room_spec_ref`, `clean_room_result_ref` | clean-room required일 때 | readiness 또는 disposable environment의 selected Check 결과 |
| `failure_record_refs`, `reproduction_pack_ref`, `regression_record_ref` | M7 failure 검증 시 | family/occurrence, curated reproduction, before/after·재발 lineage |
| `recovery_plan_ref`, `recovery_attempt_refs` | M7 recovery 검증 시 | rollback·roll-forward·restore 계획과 검증 |
| `dependency_snapshot_ref`, `supply_chain_snapshot_ref` | M7 dependency/security 검증 시 | exact manifest·lockfile relation과 supply-chain observation |
| `external_data_snapshot_refs` | M7 외부 자료 사용 시 | source·coverage·freshness·valid_until |
| `dependency_update_plan_ref` | M7 update 준비·적용 시 | candidate·approval·PatchSet·previous lockfile·rollback lineage |
| `maintenance_radar_snapshot_ref` | M7 Radar 생성 시 | input set·evaluation_time·deterministic ordering |
| `project_migration_manifest_ref`, `migration_plan_ref` | M8 migration 검증 시 | target/version source, exact chain·strategy·approval plan |
| `migration_attempt_refs`, `migration_checkpoint_ref`, `migration_validation_report_ref` | M8 execute/resume/rollback 시 | actual phase·durable prefix·invariant/active result |
| `restore_verification_record_ref` | M8 backup/restore가 required일 때 | backup integrity와 실제 restore/behavior 수준 |
| `performance_workload_spec_ref`, `performance_run_refs`, `performance_comparison_ref` | M8 performance 시 | protocol, baseline/candidate raw cohort와 comparison |
| `language_migration_plan_ref`, `equivalence_report_ref` | M8 language/platform 시 | behavior/coexistence/cutover plan과 dimension result |
| `cross_project_migration_handoff_ref` | 9단계 인계 시 | project participant·dependency·Gate/rollback ref |
| `multi_project_goal_ref`, `change_bundle_ref`, `change_bundle_participant_ref` | 9단계 시 | exact goal·bundle·owning project participant revision |
| `worktree_record_ref`, `merge_plan_ref`, `merge_queue_record_ref` | 9단계 local integration 시 | owned worktree·project-local queue lineage |
| `project_merge_result_ref`, `merge_conflict_refs` | merge/해결 시 | actual local result와 양쪽 intent·contract conflict lineage |
| `compatibility_window_refs` | provider/consumer 변경 시 | open/close·consumer state evidence |
| `remote_state_snapshot_ref`, `remote_operation_refs` | remote observation/effect 시 | adapter before/after snapshot과 승인된 effect lineage |
| `change_bundle_release_handoff_ref` | 10단계 인계 시 | project별 immutable revision·artifact input |
| `rust_toolchain_binding_ref`, `rust_style_policy_snapshot_ref` | M11 Rust style 시 | resolved stable toolchain/config/style edition과 exact allowlist·scope·auto policy fingerprint |
| `rust_style_coverage_matrix_ref`, `rust_style_step_execution_refs` | M11 Rust style 시 | required cell completeness와 fixed pipeline ordered process/diff/Diagnostic lineage |
| `validation_plan_ref`, `gate_phase` | 예 | plan revision과 v2 phase, M8 v3 phase 또는 9단계 v4 `change_bundle_prepare\|change_bundle_goal_exit` |
| `profile_resolution_fingerprint` | M2/M3 plan 실행 시 | 계획에 materialize된 Profile closure와 required family floor |
| `effective_config_fingerprint`, `gate_policy_fingerprint` | 예 | 실행·판정 설정 |
| `catalog_snapshot_ref`, `validator_registry_fingerprint` | 예 | Rule·Check·Profile·Gate metadata |
| `check_descriptor_ref`, `rule_refs` | ValidationRun이면 | Check identity와 결과를 생산할 Rule set |
| `tool_registry_snapshot_ref`, `tool_descriptor_ref`, `observed_tool_fingerprint` | process 실행 시 | live Registry revision, 선언과 실제 executable identity |
| `invocation_fingerprint`, `execution_environment_fingerprint`, `normalizer_fingerprint` | process 실행 시 | typed args·scope, OS·arch·toolchain·runtime·nonsecret env와 parser/mapping 의미 |
| `binding_fingerprint` | 예 | 위 의미 field의 JCS SHA-256 |
| `probed_at` | 예 | 표시·audit 시각, identity에서는 제외 |

필요하지 않은 optional ref는 생략한다. 값이 없다는 이유로 required binding을 partial default로 채우지 않는다.

current 판정은 binding과 Gate 직전 probe를 비교해 `current|stale_source|stale_plan|stale_config|stale_catalog|stale_tool|stale_environment|unverified`를 만든다. source·plan·Profile closure·config·Catalog·Tool·execution environment 중 하나라도 다르면 해당 evidence는 positive required evidence가 아니다. Profile closure·activation evidence 불일치는 `stale_plan`, Profile descriptor content 불일치는 `stale_catalog`로 분류한다. probe를 끝내지 못하면 `unverified`이며 최근 timestamp, 같은 branch 이름, 같은 command text 또는 exit code 0으로 current를 추정하지 않는다.

binding fingerprint에는 timestamp, display name, raw absolute path, secret, stdout·stderr와 render order를 넣지 않는다. path는 ProjectPathRef 또는 opaque binding으로만 참조한다.

`execution_environment_fingerprint`는 CheckPlan이 요구한 environment constraint와 실제 OS·arch·runtime/toolchain identity의 결합이다. remote/CI evidence도 authenticated registered ToolDescriptor와 exact source/input binding을 가지면 사용할 수 있지만 Gate host 환경과 같다고 가장하지 않는다. freshness는 각 CheckPlan의 expected environment와 비교하고, 회귀 before/after는 descriptor가 선언한 compatibility mapping 없이는 서로 다른 environment를 같은 pair로 묶지 않는다.

## ValidationRun 계약

ValidationRun은 CheckPlan 한 항목의 실제 시도다. M3 writer version은 `star.validation-run` `schema_version=2`이며 v1 reader compatibility와 섞어 쓰지 않는다. M8 EvidenceSubjectBinding·phase ref를 쓰는 writer는 [version 계약](versioning-and-migrations.md#m8-evidence-계약-version-전이)의 `schema_version=3` 목표를 사용하며 historical v2 run을 current M8 Gate로 자동 승격하지 않는다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `validation_run_id` | typed ID | 검사 실행 ID |
| `validation_plan_ref` | document ref | 원래 계획과 revision |
| `check_ref`, `tool_ref` | Catalog ref | 사용한 정의 |
| `subject_binding` | EvidenceSubjectBinding | source·plan·config·Catalog·Tool exact identity |
| `attempt` | positive integer | 같은 Check의 시도 번호 |
| `invocation` | TaskInvocation | 실제 인자와 제한 |
| `started_at`, `finished_at` | UTC timestamp | 실행 구간 |
| `process_start_state`, `process_started_at` | enum, optional UTC | M3 `not_started\|started\|unknown`과 실제 생성 시각 |
| `outcome` | enum | `pass`, `fail`, `not_run`, `error`, `cancelled` |
| `completeness` | enum | `complete`, `partial`, `unverified` |
| `exit_code` | optional integer | process가 시작된 경우 |
| `termination_reason` | enum | `exited`, `timeout`, `cancelled`, `launch_error`, `outcome_unknown` |
| `diagnostic_refs` | ref array | 정규화된 진단 |
| `stdout_ref`, `stderr_ref` | optional ArtifactRef | redaction한 원문 |
| `result_artifact_refs` | ArtifactRef array | report, trace, screenshot 등 |
| `observed_tool` | object | ToolRegistry revision, opaque executable file identity, version·hash와 redacted locator label. raw 개인 path 금지 |
| `cache` | object | hit 여부, cache key와 원래 run |
| `raw_result_fingerprint` | SHA-256 | exit·termination·normalized raw artifact manifest hash |

`outcome=pass`이려면 실행이 시작되어 기대 종료 code와 Check 의미를 모두 만족해야 한다. parser 실패, 출력 잘림이나 결과 일부만 확인한 경우 `complete`로 만들 수 없다.

`not_run`은 process가 시작되지 않았다는 실행 사실이며 `dependency_unsatisfied|permission_blocked|tool_unavailable|preflight_invalidated|cancelled_before_start|launch_error` reason을 가진다. `not_run`을 cache hit, manual evidence 또는 waiver로 `pass`로 바꾸지 않는다. cache를 사용했다면 compatible original ValidationRun ref와 current binding comparison을 남기며 stale·partial cache는 사용할 수 없다.

required Check의 기본 Gate effect는 `dependency_unsatisfied|tool_unavailable|cancelled_before_start|launch_error`이면 `block`, `permission_blocked`이면 ActionPolicy `prompt`에서 `human_review`, `deny` 또는 승인 불가능 상태에서 `block`이다. `preflight_invalidated`는 Gate success/failure로 소비하지 않고 해당 orchestration을 `invalidated`로 끝내 재계획한다. 명시적 Waiver가 있으면 raw `not_run`을 유지한 채 `RunSatisfaction=waived_for_review`와 `gate_effect=human_review`다. optional Check도 미실행 사실과 remaining risk를 보존하되 policy가 required로 승격하지 않았다면 그것만으로 전체 Gate를 막지 않는다.

| outcome | `process_start_state` | 허용 termination | completeness 규칙 |
|---|---:|---|---|
| `pass` | `started` | `exited` | `complete\|partial\|unverified`; complete가 아니면 Gate positive evidence 아님 |
| `fail` | `started` | `exited` | parser/coverage에 따라 세 값; raw fail 유지 |
| `not_run` | `not_started` | `launch_error\|cancelled\|outcome_unknown`과 typed not-run reason | `unverified` 고정 |
| `error` | `started\|unknown` | `exited\|timeout\|outcome_unknown`; adapter/parser/harness 오류 | `partial\|unverified`, complete 금지 |
| `cancelled` | `started` | `cancelled` | `partial\|unverified`, complete 금지 |

`process_start_state=not_started`이면 exit code·stdout/stderr·result artifact를 성공 근거로 합성하지 않는다. process 생성 여부 자체를 확인할 수 없으면 `unknown`이며 false로 축약하지 않는다.

## ValidationResult 계약

ValidationResult는 한 WorkspaceSnapshot, ScanRun 또는 PatchSet을 검증한 여러 ValidationRun의 immutable 정규화 결과다. M3 writer version은 `star.validation-result` `schema_version=2`이며 상세 field는 [공통 개발 관리 계약](development-management.md)을 따른다.

M3 target은 기존 field에 다음을 추가한다.

| 필드 | 의미 |
|---|---|
| `subject_binding` | result 전체의 exact EvidenceSubjectBinding |
| `freshness` | `current\|stale_source\|stale_plan\|stale_config\|stale_catalog\|stale_tool\|stale_environment\|unverified` |
| `stale_reasons` | indexed/current 값 ref와 required action을 가진 reason array |
| `stability` | `stable\|flaky\|not_evaluated` |
| `attempt_summaries` | CheckPlan별 전체 attempt ref·outcome sequence·selected result와 선택 근거 |
| `coverage_summary` | required/observed scope·partition·item count와 limitation |
| `normalizer_fingerprint` | result parser·Diagnostic mapping contract identity |

- 같은 Check의 retry는 각각 ValidationRun으로 남기고 ValidationResult가 최종 선택과 전체 시도를 함께 참조한다.
- `pass`에는 모든 required ValidationRun의 `outcome=pass`, `completeness=complete`, 같은 subject revision과 config fingerprint가 필요하다.
- optional Check 실패를 숨기지 않고 result의 Diagnostic과 limitation에 남긴다.
- ValidationResult 생성 뒤 subject WorkspaceSnapshot, ValidationPlan, ToolDescriptor 또는 EffectiveConfig fingerprint가 달라지면 stale이다.
- GateDecision은 process exit code를 다시 해석하지 않고 ValidationResult와 policy를 입력으로 사용한다.
- 같은 EvidenceSubjectBinding·CheckDescriptor·Tool identity·input에서 pass/fail outcome이 섞이면 `stability=flaky`다. 마지막 pass만 선택해 `stable`로 만들지 않는다.
- `outcome=fail`인 결과를 baseline이나 suppression으로 `pass`로 다시 쓰지 않는다. Gate의 ratchet 만족 여부는 별도 `RunSatisfaction`이다.
- `freshness != current`, `completeness != complete` 또는 `stability=flaky`인 required result는 `auto_pass`의 clean positive evidence가 아니다.

stability attempt group key는 `subject_binding.binding_fingerprint + CheckDescriptor content hash + observed Tool fingerprint + invocation_fingerprint + normalizer_fingerprint`다. group을 넘는 revision·environment·args·parser 결과를 같은 retry로 합치지 않는다. 분류 순서는 다음과 같다.

1. started·complete·current한 comparable attempt가 없으면 `not_evaluated`다.
2. group 안에 `pass`와 `fail`이 모두 있으면 contract mode와 관계없이 `flaky`다.
3. `single_attempt`은 comparable attempt가 하나 이상이고 모두 같은 outcome이면 `stable`이다.
4. `repeat_on_failure|sampled`는 descriptor의 minimum comparable attempts를 채우고 outcome이 모두 같을 때만 `stable`이다.
5. minimum 미달, `error|cancelled|not_run`, partial/unverified attempt만 있거나 group identity를 증명할 수 없으면 `not_evaluated`이며 raw failure는 그대로 Gate를 막는다.

historical attempt를 stability 계산에 쓰려면 cache/reuse contract가 current binding과 exact compatibility를 증명하고 원래 attempt ref를 포함해야 한다. 시간 범위만 같거나 command text가 같다는 이유로 group에 넣지 않는다.

## GateDecision 계약

GateDecision은 여러 ValidationRun과 Diagnostic을 완료 판단으로 모은다. M3 writer version은 `star.gate-decision` `schema_version=2`다. M8 phase와 domain result ref를 쓰는 writer는 `schema_version=3`, 9단계 ChangeBundle ref·phase를 쓰는 writer는 `schema_version=4` 목표다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `gate_id` | typed ID | gate 판단 ID |
| `scope` | object | goal, stage, patch, merge, release 또는 M8 migration/performance/language scope와 revision |
| `subject_fingerprints` | object | v1 read compatibility용 ProjectRevision, WorkspaceSnapshot, ChangeSet, PatchSet과 config fingerprint |
| `subject_bindings` | non-empty EvidenceSubjectBinding array | M3에서 평가한 project·Check별 exact binding. `(project_id, checkout_id, gate_phase, check_descriptor_ref, binding_fingerprint)` byte-order |
| `subject_binding_set_fingerprint` | SHA-256 | 정렬된 `subject_bindings[].binding_fingerprint`의 JCS SHA-256 |
| `decision` | enum | `auto_pass`, `human_review`, `block` |
| `required_check_plan_ids` | CheckPlanId array | M2 plan이 선택한 required Check. byte-order, 중복 금지 |
| `required_run_refs` | ref array | v1 read compatibility와 audit용 실제 required attempt ref |
| `satisfied_run_refs` | ref array | v1 clean-pass projection. M3 충족 여부의 정본으로 사용하지 않음 |
| `run_satisfactions` | RunSatisfaction array | raw outcome과 Gate 충족을 분리한 M3 평가 |
| `validation_result_refs` | ref array | subject별 normalized 결과 |
| `domain_result_refs` | v3 optional ref array | MigrationValidationReport·PerformanceComparison·EquivalenceReport와 rollback/restore result |
| `finding_refs` | ref array | unresolved·new·changed Finding과 fingerprint |
| `diagnostic_evaluations` | DiagnosticEvaluation array | baseline·suppression·gate effect M3 평가 |
| `claim_evaluations` | ClaimEvaluation array | 완료 주장과 current evidence 대조 |
| `decision_input_refs` | ref array | 적용한 Baseline·Suppression·Disposition revision |
| `blocking_diagnostic_refs` | ref array | 차단 원인 |
| `waivers` | WaiverRef array | 사용자가 명시적으로 수용한 예외 |
| `omissions` | OmittedCheck array | 미실행 검사와 영향 |
| `remaining_risks` | RiskRef array | 통과 뒤에도 남은 위험 |
| `policy_snapshot` | object | 적용한 gate threshold와 출처 |
| `decided_by` | ActorRef | engine 또는 사용자 |
| `evaluation_time`, `time_source_state` | UTC timestamp, enum | pure engine에 주입한 semantic 시각과 `verified\|unverified` |
| `valid_until` | optional UTC timestamp | suppression·approval·external DB freshness 등 가장 이른 semantic 재평가 경계 |
| `time_boundary_inputs` | ref array | valid_until을 만든 expiry/updated_at+max_age policy 근거 |
| `supersedes_gate_ref` | optional GateRef | 새 evidence·user decision으로 대체한 이전 판단 |
| `decision_fingerprint` | SHA-256 | subject·policy·평가·decision의 canonical hash |

M3 `GateScope` v2는 다음 tagged union이다.

| kind | 필수 field | 허용 phase |
|---|---|---|
| `independent_task` | TaskSpec ref, accepted ScopeRevision ref | `during_stage\|stage_exit\|goal_exit` 중 plan에 고정한 phase |
| `goal` | GoalId, RunId, goal revision | `goal_exit` |
| `stage` | GoalId, RunId, StageId, stage revision | `during_stage\|stage_exit` |
| `patch` | TaskSpec·ScopeRevision, 정렬된 ChangePlan ref, PatchSet ref, optional Goal/Run/Stage context | `patch_pre_apply\|patch_post_apply` |
| `merge` | project set, MergePlan ref·revision | `merge` |
| `release` | project set, ReleaseManifest input ref·revision | `release` |

M8 `GateScope` v3는 위 kind를 유지하며 다음 kind를 추가한다.

| kind | 필수 field | 허용 phase |
|---|---|---|
| `migration` | TaskSpec·ScopeRevision, ProjectMigrationManifest ref, MigrationPlan ref, target ID, 해당 phase attempt/checkpoint/result ref | `migration_pre_execute\|migration_post_execute\|migration_post_rollback` |
| `performance` | TaskSpec·ScopeRevision, PerformanceWorkloadSpec ref, baseline/candidate subject, PerformanceComparison ref | `performance_compare` |
| `language_migration` | TaskSpec·ScopeRevision, LanguageMigrationPlan ref, EquivalenceReport ref, cutover revision | `language_cutover` |

9단계 `GateScope` v4는 위 kind를 유지하며 다음 kind를 추가한다.

| kind | 필수 field | 허용 phase |
|---|---|---|
| `change_bundle` | MultiProjectGoal ref, CrossRepoChangeBundle ref·revision, 정렬된 required participant ref/fingerprint, completion target | `change_bundle_prepare\|change_bundle_goal_exit` |

participant source apply는 기존 `patch`/M8 scope, project-local integration은 기존 `merge` scope를 사용한다. `change_bundle` scope는 이 project-local Gate를 대체하지 않고 첫 cross-repo effect 전 readiness와 전체 Goal completion만 집계한다.

모든 `subject_bindings[].gate_phase`는 scope phase와 같아야 한다. `patch`의 pre/post decision은 같은 ChangePlan·PatchSet lineage지만 서로 다른 WorkspaceSnapshot을 사용한다. migration pre/post/rollback도 같은 MigrationPlan lineage를 유지하되 attempt·active target·version binding은 phase별 actual 값을 사용한다. CLI-only 독립 실행에 가짜 GoalId·RunId를 발급하지 않는다.

- 필수 검사 `RunSatisfaction=unsatisfied|waived_for_review`, 실행 오류와 확인되지 않은 중대한 결과는 `auto_pass`가 될 수 없다.
- incomplete ScanRun·ValidationResult, stale PatchSet 또는 다른 config fingerprint의 결과는 `auto_pass`가 될 수 없다.
- waiver는 실패 결과를 통과로 변조하지 않고 GateDecision에만 적용한다.
- waiver 대상, revision, 만료와 사용자가 본 evidence hash가 달라지면 새 승인이 필요하다.
- `human_review`는 차단도 성공도 아니며 RunSnapshot에 대기 상태로 나타난다.
- `ratchet_satisfied` raw failure는 ValidationRun·ValidationResult에서 계속 `fail`로 보존하고 GateDecision에 기존 unchanged debt만으로 만족한 근거를 명시한다.
- `not_run`, `partial`, `unverified`, stale와 flaky required evidence는 `clean_pass|ratchet_satisfied`가 될 수 없다.
- 다른 source·plan·config·Catalog·Tool binding의 evidence는 history reference일 뿐 `satisfied_run_refs`에 넣지 않는다.

M3 writer는 `required_check_plan_ids`마다 정확히 하나의 `RunSatisfaction`을 만들고 둘의 ID 집합이 같음을 검증한다. `RunSatisfaction`이 참조하는 모든 ValidationRun의 binding은 `subject_bindings`에 있어야 한다. 같은 Project라도 Check·Tool·normalizer가 다르면 별도 binding이다. 여러 Project를 한 Gate가 다루면 project별 binding을 정렬해 하나의 `subject_binding_set_fingerprint`를 만들며 임의의 대표 Project fingerprint로 축약하지 않는다.

`valid_until`은 적용된 active Suppression expiry, approval/waiver expiry, Tool trust expiry와 external database `updated_at + maximum_age` 중 가장 이른 시각이다. 영구적 time boundary가 하나도 없을 때만 생략한다. Gate pure engine은 system clock을 직접 읽지 않고 application service가 고정한 `evaluation_time`을 입력으로 받는다. time boundary가 있는데 `time_source_state=unverified`이면 `auto_pass`가 아니다. consumer는 현재 시각이 `valid_until` 이상이면 decision을 current로 사용하지 않고 새 probe·GateDecision을 요구한다. 표시용 `created_at`은 decision identity에서 제외하지만 `evaluation_time`, `valid_until`과 계산 input은 semantic field이므로 `decision_fingerprint`에 포함한다.

`time_source_state=verified`는 Controller가 UTC clock을 읽고 같은 store의 마지막 committed evaluation/event보다 허용되지 않은 역행이 없음을 확인했다는 local 의미다. 외부 NTP·시간 서비스를 호출하거나 암호학적 시간을 주장하지 않는다. clock read 실패·범위 오류·역행이면 `unverified`다.

v1 `satisfied_run_refs`는 실제 pass run만 표현할 수 있으므로 v2 writer는 `clean_pass` attempt만 이 projection에 넣는다. `ratchet_satisfied` raw fail은 `run_satisfactions`에만 만족으로 기록한다. v1 reader가 ratchet 의미를 알지 못하므로 M3 GateDecision v2를 v1로 downgrade하거나 v1 `validate_against`로 자동 통과 판정하지 않는다.

### DiagnosticEvaluation

`DiagnosticEvaluation`은 immutable Diagnostic 관찰과 Gate policy 결정을 분리한다.

| 필드 | 의미 |
|---|---|
| `evaluation_subject` | tagged union `current_diagnostic`의 Diagnostic ID·fingerprint 또는 `baseline_entry`의 Baseline ID·revision·entry fingerprint |
| `subject_binding_fingerprint` | current 판단 subject |
| `baseline_relation` | `new\|existing_unchanged\|worsened\|improved\|not_observed\|incompatible\|unbaselined` |
| `baseline_ref` | 비교한 Baseline ID·revision·set fingerprint |
| `suppression_state` | `none\|active\|expired\|stale\|revoked\|invalid` |
| `suppression_ref` | 적용·검토한 Suppression ID·revision |
| `disposition_ref` | false positive·accepted risk 등 local 판단이 있을 때 |
| `gate_effect` | `none\|remaining_risk\|requires_review\|blocks` |
| `reason_codes` | baseline·suppression·threshold·protected invariant 근거 |
| `evaluation_fingerprint` | 위 의미 input/output의 canonical hash |

`current_diagnostic`은 현재 관찰된 issue의 `new|existing_unchanged|worsened|improved|incompatible|unbaselined` 평가에 사용한다. `baseline_entry`는 complete current coverage에서 해당 entry가 관찰되지 않은 `not_observed` 평가에만 사용하며 `suppression_state=none`이어야 한다. current Diagnostic 없이 가짜 resolved Diagnostic을 만들지 않는다.

Baseline·Suppression은 Diagnostic severity·confidence·observation status·evidence를 수정하지 않는다. Rule definition·fingerprint contract·scope/config constraint가 다르면 이전 decision은 `incompatible|stale`이며 자동 적용하지 않는다.

### RunSatisfaction

`RunSatisfaction`은 CheckPlan ID, `requirement=required|optional`, 전체 ValidationRun/ValidationResult refs, raw outcome sequence, `satisfaction=clean_pass|ratchet_satisfied|unsatisfied|waived_for_review`, `gate_effect=none|human_review|block`, stable `reason_code`, DiagnosticEvaluation refs, policy reason과 content fingerprint를 가진다.

- `clean_pass`: required run이 실제 실행됐고 `outcome=pass`, `complete`, `current`, `stable`이다.
- `ratchet_satisfied`: CheckDescriptor가 명시적으로 ratchet eligible이고 실행·parsing·coverage가 complete/current/stable이며 모든 실패 Diagnostic이 `existing_unchanged` 또는 policy가 허용한 active suppression이다. raw outcome은 바꾸지 않는다.
- `unsatisfied`: fail/error/not_run/cancelled, partial/unverified/stale/flaky, new/worsened blocking Diagnostic 또는 실행 invariant 실패다.
- `waived_for_review`: 사용자가 required Check 생략·실패를 검토 대상으로 수용했다. `auto_pass` 근거가 아니다.

`gate_effect`는 raw outcome을 재분류하지 않고 Gate aggregation 우선순위만 고정한다. protected invariant, required functional failure, tool/launch/dependency 실패와 policy deny는 `block`이다. permission/approval prompt, 명시적 waiver, required flaky 또는 semantic manual observation은 policy가 더 엄격하게 막지 않는 한 `human_review`다. `clean_pass|ratchet_satisfied`만 `none`을 가질 수 있고, required `unsatisfied|waived_for_review`가 `none`이면 Schema/invariant 오류다. 전체 Gate는 `block` 하나라도 있으면 `block`, 없고 `human_review`가 하나라도 있으면 `human_review`, 둘 다 없을 때만 다른 AUTO_PASS 조건을 평가한다.

functional test, build·compile, regression before/after, validator guard, secret critical, migration invariant, required performance budget·equivalence dimension과 release artifact identity Check는 기본 `ratchet_eligible=false`다. project Catalog가 이를 true로 넓힐 수 없다.

GateDecision은 모든 selected Check에 RunSatisfaction을 만들되 `required_check_plan_ids`와 `requirement=required` ID 집합이 정확히 같아야 한다. optional `unsatisfied`는 숨기지 않고 policy에 따라 remaining risk·review·block으로 평가하지만 required satisfaction 집합을 채우는 데 사용하지 않는다.

### 공개 구현과 소비 경계

`crates/foundation/star-contracts`가 `ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle`, `ReviewPack`, `Diagnostic`, `ArtifactRef`와 M3 nested `ResolvedProfileRef`, `PhaseSubjectExpectation`, `EvidenceSubjectBinding`, `SubjectBindingRecord`, `CompletionClaim`, `ClaimEvaluation`, `DiagnosticEvaluation`, `RunSatisfaction`, `EvidenceRefSet`, M11 nested `RustToolchainBinding`, `RustStylePolicySnapshot`, `RustStyleCoverageMatrix`, `RustStyleStepExecution` 및 지원 ref·enum의 Rust·JSON Schema 정본을 소유한다. schema ID는 각각 `star.validation-plan`, `star.validation-run`, `star.gate-decision`, `star.evidence-bundle`, `star.review-pack`, `star.diagnostic`, `star.artifact-ref`로 고정하고 nested type은 owning Schema의 `$defs`에서 한 번만 정의한다. Rust style 4개 type도 별도 top-level persisted document·DB table을 만들지 않고 owning RecipeExecution/PatchSet/EvidenceBundle Schema의 공통 `$defs`를 참조한다. 공통 개발 관리 계층도 이 `evidence` module 타입을 직접 사용하며 동명의 병렬 wire type을 정의하지 않는다.

하위 adapter는 `GateDecision::authoritative_state()`가 반환한 상태만 완료 판정으로 소비한다. `ValidationRun` 목록을 다시 집계해 `GateDecision`을 대체하거나 `not_run`을 통과로 바꿀 수 없다. `validate_against`는 이미 만들어진 결정의 참조와 불변식을 검증할 뿐 새 결정을 계산하지 않는다. adapter는 `StarValidationResult`, `CompletionEvidence` 같은 호환 DTO를 만들지 않고 이 공개 계약을 직접 사용한다.

## ArtifactRef 계약

큰 출력과 파일은 계약에 복사하지 않고 ArtifactRef로 연결한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `artifact_id` | ArtifactId | 안정 ID |
| `kind` | stable enum/string | log, report, diff, screenshot, trace 등 |
| `project_id` | optional ProjectId | 프로젝트 소속 자료 |
| `relative_path` | normalized path | artifact root 기준 위치 |
| `media_type` | string | MIME type |
| `size_bytes` | non-negative integer | 저장 크기 |
| `sha256` | lowercase hex | 저장 byte의 hash |
| `created_at` | UTC timestamp | 생성 시각 |
| `producer` | ProducerRef | 생성 component·tool |
| `redaction_status` | enum | `not_needed`, `redacted`, `quarantined`, `unknown` |
| `retention_class` | enum | `temporary`, `run`, `evidence`, `hold` |
| `source_artifact_ref` | optional ArtifactRef | redaction·변환 전 자료 |

경로는 artifact root를 벗어날 수 없고 소비자는 읽기 전에 size와 hash를 확인한다. `quarantined` 또는 `unknown` 자료는 기본 report와 MCP 응답에 포함하지 않는다.

`source_artifact_ref`는 저장이 허용된 비민감 원본의 변환 관계에만 사용한다. secret, 사용자 이름, raw 개인 절대 경로와 민감 literal이 포함된 byte는 quarantined 상태라도 저장·hash하지 않고 source reference를 생략한다.

관리 DB에는 ArtifactRef와 subject relation만 저장한다. 다음 자료의 byte는 DB blob이나 event payload에 넣지 않는다.

- source·workspace entries manifest
- 전체 diff와 PatchSet patch
- stdout·stderr와 parser raw output
- trace, profile, screenshot과 재현 bundle
- Markdown·HTML·JSON 대형 report

artifact를 DB에서 참조하기 전 temp write, redaction, size·hash 검증과 atomic finalize를 끝낸다. DB transaction이 실패한 artifact는 orphan이며 evidence로 노출하지 않고 retention이 격리한다.

## EvidenceBundle과 ReviewPack

EvidenceBundle은 실행 사실을 기계가 읽는 정본으로 묶고 ReviewPack은 사람이 판단하기 쉬운 순서로 참조한다.

참조 방향은 순환하지 않는다.

```text
ValidationRun·Diagnostic·ValidationResult
  -> GateDecision
  -> EvidenceBundle
  -> ReviewPack
```

GateDecision은 EvidenceBundle이나 ReviewPack을 역참조하지 않는다. EvidenceBundle은 GateDecision을, ReviewPack은 EvidenceBundle을 참조한다. 따라서 각 document의 byte hash를 이전 단계부터 순서대로 확정할 수 있다.

### EvidenceBundle

M3 target `star.evidence-bundle` v2는 CLI-only와 multi-project 실행을 위해 다음 field를 정본으로 사용한다. Goal에 연결되지 않은 독립 실행에서는 Goal/Stage ref를 생략하고 `task_spec_ref`를 반드시 둔다. M8 ref와 subject role을 포함하는 bundle은 v3, 9단계 ChangeBundle·merge·remote·release handoff ref를 포함하는 bundle은 v4, 10단계 release ref는 v5, M11 Rust style binding/ref를 포함하는 bundle은 v6 목표다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `evidence_bundle_id`, `revision`, 공통 envelope | 예 | immutable export revision과 producer |
| `task_spec_ref`, `scope_revision_ref` | 예 | 요청과 accepted scope |
| `goal_spec_ref`, `stage_graph_ref`, `stage_evidence` | Goal 실행 시 | 관리 실행 context |
| `change_plan_refs`, `change_set_refs`, `patch_set_refs` | 해당 시 | 계획, before/current/preview actual change와 patch lineage |
| `recipe_execution_refs`, `patch_application_refs` | M4일 때 | preview·idempotence와 actual apply·recovery lineage |
| `managed_registry_snapshot_refs`, `managed_declaration_refs`, `registry_consistency_records` | M5일 때 | source Registry, 변경 대상과 binding·consumer·drift EvidenceRefSet |
| `project_contract_manifest_ref`, `contract_surface_snapshot_refs`, `compatibility_report_ref` | M6 B04일 때 | baseline/current surface, consumer·migration·companion change EvidenceRefSet |
| `documentation_snapshot_ref`, `config_key_trace_set` | M6 B07일 때 | docs/config/generated/assumption evidence |
| `environment_snapshot_ref`, `project_doctor_report_ref`, `clean_room_refs` | M6 B07일 때 | read-only doctor, environment와 clean-room readiness/result |
| `dependency_security_input_manifest_ref` | 7단계 인계가 적용될 때 | dependency manifest·lockfile·toolchain·environment discovery evidence |
| `failure_record_refs`, `reproduction_pack_refs`, `regression_record_refs` | M7 failure일 때 | occurrence, curated reproduction, before/after·재발 EvidenceRefSet |
| `recovery_plan_refs`, `recovery_attempt_refs` | M7 recovery일 때 | rollback·roll-forward·restore와 rehearsal/actual evidence |
| `dependency_snapshot_refs`, `supply_chain_snapshot_refs` | M7 dependency/security일 때 | relation·state·workflow·release·redaction evidence |
| `external_data_snapshot_refs` | M7 외부 자료 사용 시 | source·query·schema·coverage·freshness·valid_until |
| `dependency_update_plan_refs` | M7 update일 때 | candidate·affected Project·approval·PatchSet·rollback 상태 |
| `maintenance_radar_snapshot_ref` | M7 Radar를 포함할 때 | derived input refs와 deterministic priority·time boundary |
| `project_migration_manifest_ref`, `migration_plan_refs` | M8 migration일 때 | target/version source와 project별 plan EvidenceRefSet |
| `migration_attempt_refs`, `migration_checkpoint_refs`, `migration_validation_report_refs` | M8 migration일 때 | dry-run·rehearsal·execute/resume/rollback과 invariant·active result |
| `restore_verification_record_refs` | M8 backup/restore일 때 | backup integrity·restore rehearsal·behavior 수준 |
| `performance_workload_spec_refs`, `performance_run_refs`, `performance_comparison_refs` | M8 performance일 때 | protocol·raw cohort·comparability/noise/trade-off |
| `language_migration_plan_refs`, `equivalence_report_refs` | M8 language/platform일 때 | behavior·coexistence·consumer·cutover와 dimension result |
| `cross_project_migration_handoff_ref` | 9단계 인계가 적용될 때 | project별 plan·PatchSet·Gate·rollback과 dependency edge |
| `multi_project_goal_ref`, `change_bundle_ref` | 9단계일 때 | global relation·step graph와 exact bundle revision |
| `change_bundle_participant_refs` | 9단계일 때 | project별 participant EvidenceRefSet; global bundle에 detail inline 금지 |
| `worktree_record_refs`, `merge_plan_refs`, `merge_queue_record_refs` | 9단계 local integration 시 | project-local ownership·order·base lineage |
| `merge_conflict_refs`, `project_merge_result_refs` | conflict/merge 시 | 양쪽 intent·contract와 actual result·Gate |
| `compatibility_window_refs` | provider/consumer 변경 시 | open/consumer/close evidence |
| `remote_state_snapshot_refs`, `remote_operation_refs` | remote 관찰/effect 시 | project별 adapter snapshot과 approval/receipt/after probe |
| `change_bundle_release_handoff_ref` | 10단계 인계 시 | immutable project source·artifact·Gate mapping |
| `rust_toolchain_binding_refs`, `rust_style_policy_snapshot_refs` | M11일 때 | check/preview/replay/actual-after가 사용한 exact toolchain·config·policy EvidenceRefSet |
| `rust_style_coverage_matrix_refs`, `rust_style_step_execution_refs` | M11일 때 | phase별 coverage와 ordered step/Diagnostic/suggestion/diff/side-effect EvidenceRefSet |
| `subject_binding_records`, `authoritative_subject_binding_set_fingerprint` | 예 | preflight·run·final probe의 역할별 binding과 authoritative Gate set hash |
| `project_catalog_snapshot_ref`, `code_index_snapshot_refs` | source 영향 검사 시 | project/index coverage와 freshness |
| `catalog_snapshot_ref`, `validator_registry_fingerprint`, `tool_registry_snapshot_ref` | 예 | Rule·Check·Tool resolution 정본 |
| `effective_config_fingerprint`, `gate_policy_fingerprint` | 예 | 실행·판정 설정 |
| `validation_plan_refs`, `validation_run_refs`, `validation_result_refs` | 예 | 계획과 모든 attempt·정규화 결과 |
| `diagnostic_refs`, `diagnostic_evaluations`, `run_satisfactions` | 예 | raw issue와 baseline·suppression·Gate 평가 |
| `completion_claims`, `claim_evaluations` | 예 | 보고된 완료와 actual 비교. 주장이 없으면 complete empty set |
| `decision_input_refs` | 예 | Baseline·Suppression·Disposition·Waiver exact revision |
| `gate_decision_refs` | 예 | patch pre/post를 포함한 관련 decision. 중복 금지 |
| `authoritative_gate_decision_ref` | 예 | 이 bundle이 보고하는 최종 phase decision이며 위 배열 원소여야 함 |
| `event_ranges`, `cost_record_refs`, `merge_result_ref`, `remaining_risks`, `handoff_ref` | 해당 시 | 운영·비용·이어하기 근거 |
| `artifact_manifest` | 예 | 모든 큰 자료의 ID·hash·size·redaction 상태 |
| `completeness`, `missing_reasons` | 예 | `complete\|partial\|unverified`; complete이면 missing은 empty |
| `bundle_fingerprint` | 예 | timestamp·render 순서를 제외한 위 의미 field와 manifest의 JCS SHA-256 |

큰 ref/evaluation 집합은 `EvidenceRefSet`으로 외부화할 수 있다. 이 nested type은 `item_kind`, `item_count`, `content_fingerprint`와 `storage=inline|artifact`를 가지며, `inline`이면 정렬된 `items`만, `artifact`이면 redaction·hash 검증이 끝난 JSON ArtifactRef만 허용한다. 둘을 동시에 두거나 둘 다 생략하지 않는다. externalized collection도 bundle fingerprint에 item content fingerprint로 참여한다.

`SubjectBindingRecord`는 v2 `role=preflight|recipe_preview|idempotence_replay|validation_run|final_probe|patch_before|patch_after|rollback_after`를 가진다. M8 v3는 `migration_before|migration_rehearsal_after|migration_after|restore_after|performance_baseline|performance_candidate|language_baseline|language_candidate|language_cutover_after`, 9단계 v4는 `bundle_prepare|participant_apply_after|project_merge_before|project_merge_after|remote_before|remote_after|bundle_goal_exit|release_handoff`를 추가한다. M10 v5 release role은 release phase와 artifact subject를 사용하고, M11 v6 Rust role은 아래 11단계 절에서 추가한다. record는 optional owning RecipeExecution·PatchApplication·MigrationAttempt·PerformanceRun·ValidationRun·GateDecision·ChangeBundleParticipant·ProjectMergeResult·RemoteOperation ref와 exact EvidenceSubjectBinding을 가지며 `(role, project_id, checkout_id, check_descriptor_ref, binding_fingerprint)`로 정렬한다. `authoritative_subject_binding_set_fingerprint`는 authoritative GateDecision의 값과 exact 일치해야 하며 preview/history/stale record를 final authoritative set에 섞지 않는다.

- GoalSpec, StageGraph와 최종 revision reference
- 각 Stage의 RouteDecision, PermissionPlan, 결과와 Checkpoint
- 변경 전·후 ProjectRevision·WorkspaceSnapshot·ChangeSet fingerprint와 actual add·modify·delete·rename 목록
- ProjectCatalogSnapshot·CodeIndexSnapshot과 freshness proof, ScanRun·Finding decision, ChangePlan·RecipeExecution·PatchSet·PatchApplication, ValidationPlan·ValidationRun·ValidationResult, Diagnostic·DiagnosticEvaluation·RunSatisfaction과 GateDecision reference
- ManagedRegistrySnapshot before/expected-after/actual-after, declaration·namespace·tombstone, binding·consumer compatibility와 RegistryConsistencyRecord reference
- ProjectContractManifest, baseline/current ContractSurfaceSnapshot, CompatibilityReport와 consumer migration·companion change reference
- DocumentationSnapshot, ConfigKeyTrace set, EnvironmentSnapshot, ProjectDoctorReport와 applicable CleanRoomSpecification/result reference
- dependency·security 입력 manifest, FailureRecord·ReproductionPack·RegressionRecord·RecoveryPlan, Dependency/SupplyChain/ExternalData snapshot, DependencyUpdatePlan과 MaintenanceRadarSnapshot reference
- 외부 자료의 source·coverage·freshness·valid_until과 package manager·previous lockfile·approval lineage
- CompletionClaim·ClaimEvaluation, preflight/final EvidenceSubjectBinding과 stale comparison
- 적용한 Baseline·Suppression·Disposition·Waiver의 exact revision·fingerprint와 active/expired/stale 상태
- CatalogSnapshot·ValidatorRegistrySnapshot·ToolRegistrySnapshot·EffectiveConfig·GatePolicy fingerprint
- approval, retry, escalation, pause와 recovery event 구간
- CostRecord와 측정되지 않은 usage 항목
- merge 결과, remaining risk와 Handoff
- bundle manifest에 포함 artifact의 ID, hash, size와 redaction 상태

EvidenceBundle은 원문 로그를 inline으로 넣지 않고 ArtifactRef만 가진다. `complete`, `partial`, `unverified` 중 bundle completeness를 표시하고 빠진 이유를 적는다.

### ReviewPack

M3 target `star.review-pack` v1은 derived report지만 hash가 있는 독립 document다.

9단계 ChangeBundle·project local/remote·partial 상태를 typed summary로 포함하는 writer는 `schema_version=4` 목표다. 아래 9개 stable section key와 순서는 그대로 유지하고, 새 사실을 자유 형식 section으로 추가하지 않고 해당 section의 typed item과 evidence ref로 표현한다. v1~v3 ReviewPack은 current ChangeBundle 전체 성공 근거로 자동 승격하지 않는다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `review_pack_id`, `revision`, 공통 envelope | 예 | report identity와 producer |
| `evidence_bundle_ref` | 예 | 검증한 EvidenceBundle ID·revision·byte hash |
| `authoritative_gate_decision_ref` | 예 | bundle의 authoritative ref와 exact 일치 |
| `section_order` | 예 | 아래 9개 stable section key의 고정 순서 |
| `sections` | 예 | 각 section의 typed summary와 원본 evidence ref; 원문 사실을 새로 만들지 않음 |
| `questions` | 예 | `human_review`에 필요한 질문·선택지·영향. 없으면 empty |
| `required_actions` | 예 | ReworkDirective 또는 approval·replan 요구. 없으면 empty |
| `rendered_artifact_refs` | 예 | optional Markdown/HTML report. 구조화 document가 정본 |
| `completeness`, `missing_reasons` | 예 | bundle보다 높을 수 없음 |
| `review_pack_fingerprint` | 예 | bundle hash·decision ref·structured section 내용의 JCS SHA-256 |

`section_order` v1은 다음 stable key를 정확히 이 순서로 가진다.

1. `request_and_completion_criteria`
2. `planned_vs_actual_changes`
3. `completion_claims`
4. `check_results`
5. `diagnostic_relations`
6. `quality_security_highlights`
7. `gate_decision`
8. `remaining_risks_and_questions`
9. `evidence_identity`

unknown section key, 중복, 누락과 순서 변경은 Schema 오류다. 새 section은 ReviewPack schema version을 올려 추가한다. 권한·비용·독립 검토는 각각 관련 quality/risk section의 typed item으로 포함하며 별도 자유 형식 section을 만들지 않는다.

- 사용자가 요청한 목표와 완료 조건
- 계획 대비 actual add·modify·delete·rename, missing·unexpected·out-of-scope와 preexisting 변경 보존 요약
- 중요한 diff·설계 결정·permission·approval
- 완료 주장을 `verified`, `contradicted`, `unverified`, `stale`로 나눈 표
- 검사 결과를 pass, fail, not_run, partial, unverified, stale, flaky로 분리하고 raw outcome과 `clean_pass|ratchet_satisfied|unsatisfied|waived_for_review`를 함께 보인 표
- Diagnostic을 new, worsened, existing unchanged, active suppression, expired/stale suppression으로 나눈 표
- 독립 검토 결과와 아직 남은 위험
- 비용 발생과 사용량
- 다음 선택지, 이어하기 또는 rollback 위치
- EvidenceBundle ID와 hash

ReviewPack은 evidence를 새로 해석해 사실을 바꾸지 않는다. 사람이 읽는 Markdown과 같은 내용을 가진 구조화 JSON을 함께 만들 수 있다.

CLI-only execution에서 별도 AI review가 없으면 “검토 통과”로 합성하지 않는다. 의미 검토가 필요하면 ReviewPack 질문과 GateDecision `human_review`를 남긴다.

commit 순서는 `ValidationRun/Diagnostic/ValidationResult -> GateDecision -> EvidenceBundle -> ReviewPack -> Run/Stage completion projection`이다. artifact byte는 각 document commit 전에 finalize한다. EvidenceBundle 또는 ReviewPack 생성이 실패하면 이미 commit된 GateDecision을 바꾸지 않지만 자동 완료 projection은 만들지 않고 packaging failure Diagnostic과 incomplete evidence 상태를 남긴다.

## ReproductionPack과 CostRecord

### ReproductionPack

M7 target `star.reproduction-pack`은 일반 ValidationRun log가 아니라 실패를 다시 만들거나 재현 불가를 검증하는 데 필요한 최소 immutable manifest다. 상세 identity·state는 [7단계 정본](failure-security-and-dependency-maintenance.md)이 소유한다.

최소 필드는 다음과 같다.

| 필드 | 의미 |
|---|---|
| `reproduction_pack_id`, `failure_record_ref` | pack과 failure family/occurrence identity |
| `subject_binding` | exact Project·Checkout·ProjectRevision·WorkspaceSnapshot·ChangeSet |
| `invocation` | registered Task/Check/Tool ref, structured args, logical cwd, timeout·resource limit |
| `environment` | compatibility class, exact redacted fingerprint, toolchain/runtime/package manager, manifest·lockfile refs |
| `input_refs`, `seed` | content fingerprint가 있는 input/generator와 deterministic seed |
| `expected`, `observed` | 예상·실제 result와 failure family |
| `attempt_refs` | 모든 rerun·reduce·bisect·debug/trace attempt와 variance |
| `artifact_refs` | stdout·stderr·dump·trace 등 role·redaction·retention이 있는 ArtifactRef |
| `external_conditions` | service/device/clock/network 등 조건과 verification state |
| `reproduction_state` | `reproduced\|partially_reproduced\|not_reproduced\|blocked_external\|unverified` |
| `limitations` | 누락·stale·permission·호환성 제한 |

pack은 전체 repository 사본, raw secret·token·개인정보, username·home/temp 절대 경로를 포함하지 않는다. 같은 artifact를 일반 run과 공유해도 `artifact_role=reproduction_required`를 명시한다. `quarantined|unknown` artifact는 default ReviewPack에서 제외하며, bytes를 안전하게 가릴 수 없으면 `dropped_sensitive` metadata만 남긴다.

`not_reproduced`는 fixed나 pass가 아니다. 재현할 수 없는 외부 조건은 `blocked_external` 또는 `unverified`로 기록하고, 수정 전 failure·수정 후 success는 compatible command·input·seed·environment와 complete·stable result가 있는 `RegressionRecord`로만 연결한다.

### CostRecord

| 필드 | 의미 |
|---|---|
| `scope` | goal, stage, attempt 또는 외부 동작 |
| `source` | Codex, tool, 외부 서비스 등 측정 주체 |
| `usage` | token, duration, invocation count처럼 실제 받은 단위 |
| `monetary_cost` | provider가 검증 가능한 금액을 제공한 경우만 기록 |
| `currency` | 금액이 있을 때의 ISO currency |
| `price_source_ref` | 가격·청구 근거 artifact |
| `estimated` | 추정 여부. 금액은 기본적으로 `false`만 허용 |
| `paid_action` | PermissionPlan의 유료 동작 판정 |

가격을 알 수 없을 때 0원으로 기록하지 않고 금액 필드를 생략하며 `measurement_unavailable` 이유를 남긴다.

### BudgetSnapshot

BudgetSnapshot은 한 시점의 실행 가능 한도를 표현한다.

| 필드 | 의미 |
|---|---|
| `scope` | goal, stage 또는 attempt |
| `limits` | 시간, attempt, 병렬 수, artifact, paid action과 검증된 금액 한도 |
| `observed` | CostRecord에서 실제 측정한 양 |
| `reserved` | 이미 시작했지만 끝나지 않은 operation의 보수적 예약량 |
| `remaining` | 같은 단위로 계산 가능한 잔여량 |
| `unknown_measurements` | 측정할 수 없는 비용·usage와 영향 |
| `decision` | `within_budget`, `approval_required`, `exhausted`, `unknown` |
| `config_fingerprint` | 한도 출처 |
| `evaluated_at` | 계산 시각 |

측정 단위가 다른 값은 합산하지 않는다. 비용을 모르면 `remaining=0`으로 만들지 않고 `unknown` 또는 승인 필요로 판단한다.

## RemoteStateSnapshot 계약

원격 Git·PR·check·release 상태는 로컬 상태와 분리해 snapshot으로 기록한다. 9단계 writer는 adapter provenance와 exact commit subject를 가진 `schema_version=2` 목표를 사용한다.

| 필드 | 의미 |
|---|---|
| `remote_snapshot_id`, `revision` | immutable snapshot ID·revision |
| `project_id`, `remote_kind`, `adapter_descriptor_ref` | 대상과 provider adapter identity |
| `remote_identity` | secret을 제거한 host·repository identity |
| `local_subject` | ProjectRevision·commit OID·optional ChangeBundleParticipant ref |
| `query_scope` | refs·PR·checks·release 중 실제 조회 범위 |
| `refs` | branch/tag/commit provider ref와 observed object ID |
| `pull_requests` | head/base/merge commit·state·updated revision |
| `checks` | check identity·subject commit·status·conclusion |
| `releases` | tag/source/artifact identity와 provider status |
| `capabilities` | adapter가 관찰한 조회·push·PR·merge 지원; permission 아님 |
| `captured_at`, `valid_until` | 조회 완료와 실행 전 재확인 경계 |
| `completeness` | `complete\|partial\|unverified` |
| `limitations` | 권한 부족·부분 조회·provider 차이 |
| `raw_artifact_ref` | redaction한 adapter 응답 |
| `snapshot_fingerprint` | adapter/query/result/limitation의 canonical hash |

원격 변경 command는 이 snapshot과 대상 revision을 precondition으로 사용한다. stale·partial·unverified이면 push·PR·merge·release 전에 다시 조회한다. branch 이름, local commit, adapter capability와 이전 success response로 현재 remote 상태를 추측하지 않는다.

remote write는 [9단계 `RemoteOperationRecord`](cross-repo-change-bundle.md#remoteoperationrecord와-승인-경계)가 exact before snapshot·local commit·target·ApprovalRequest·adapter receipt·after snapshot을 연결한다. adapter call이 성공해도 after snapshot이 target result를 확인하지 못하면 `outcome_unknown|unverified`다.

## EvaluationRun 계약

EvaluationRun은 Router, Rule, Check, Profile, ChangeRecipe와 정책 후보가 실제로 1인 개발자의 작업을 개선하는지 비교한다. 10단계 target은 `schema_version=2`다. 비교 알고리즘·recommendation·Catalog lifecycle은 [10단계 정본](ci-release-evaluation-and-product-completion.md#evaluationrun-v2-평가-단위)이 소유한다.

| 필드 | 의미 |
|---|---|
| `evaluation_run_id` | 문서 ID |
| `subject_kind`, `subject` | route/policy, Rule, Check, Profile, Recipe 중 하나의 stable ID·version·definition fingerprint |
| `evaluation_context` | `cli_only` 또는 `codex_integrated`; 다른 context를 같은 cohort로 합치지 않음 |
| `baseline`, `candidate` | 비교 대상 definition·resolved closure·policy snapshot |
| `mode` | `offline`, `replay`, `shadow` |
| `corpus_ref` | 실제 사례를 비식별화한 평가 자료와 version |
| `case_selection`, `measurement_protocol` | case filter·sample floor·attempt·timeout·retry·metric·threshold의 사전 고정 값 |
| `case_result_refs` | 사례별 subject binding·run·Diagnostic/Finding·adjudication·rework·outcome artifact |
| `ground_truth_summary` | confirmed defect, false positive, unresolved, not-applicable 수와 denominator |
| `finding_metrics` | Rule/Check·severity·new/existing/worsened별 finding, false negative, flaky와 suppression 상태 |
| `efficiency_metrics` | first-result·total·review·rework duration, retry·failure·rollback·revert·acceptance |
| `usage_and_cost_refs` | 실제 단위와 provider가 검증 가능한 금액만 가진 CostRecord |
| `comparability` | case/source/config/Catalog/Tool/environment/protocol dimension별 compatible·not-comparable |
| `protected_metric_results` | validator guard, required Check·severity·ratchet·Corpus·freshness 약화 여부 |
| `limitations` | 표본·측정·외부 조건의 한계 |
| `comparison` | 개선·악화·불확실한 항목 |
| `recommendation` | `keep`, `trial`, `accept`, `reject`, `needs_review` |
| `decision_ref` | 실제 규칙 변경 승인과 ADR·config change |
| `radar_item_refs` | Maintenance Radar와 deprecation·next review 연결 |

case adjudication은 `confirmed_defect|false_positive|unresolved|not_applicable`을 구분한다. unresolved를 defect나 false positive에 넣지 않고 denominator 0은 100%가 아니라 `not_computable`이다. suppression은 raw finding을 삭제하거나 false positive를 대신하지 않는다.

baseline/candidate는 case·source·config·Catalog·Tool·environment·measurement protocol이 같을 때만 비교 가능하다. 실행 시간 단축이 false negative·new/worsened finding·rollback 증가 또는 validator weakening을 상쇄하지 않는다. monetary cost는 provider가 검증 가능한 값과 price source를 제공한 경우만 기록하고, 없으면 0이 아니라 `measurement_unavailable`이다.

shadow mode는 실제 작업의 route, permission, 검사와 파일을 바꾸지 않는다. EvaluationRun의 recommendation만으로 Catalog나 설정을 자동 갱신하지 않는다. `accept`도 review된 source change·migration·M3 Gate의 입력일 뿐 자동 적용 명령이 아니다.

## ReleaseManifest 계약

ReleaseManifest는 Star-Control 자체 또는 대상 프로젝트 release 후보의 신원과 준비 증거를 묶는다. 10단계 target은 `schema_version=2`다. build-once·검사 계층·상태 전이·설치 수명주기는 [10단계 정본](ci-release-evaluation-and-product-completion.md)이 소유한다.

| 필드 | 의미 |
|---|---|
| `release_manifest_id` | 문서 ID |
| `revision`, `supersedes` | immutable manifest revision과 이전 revision |
| `product_id`, `version`, `channel` | release identity |
| `task_spec_ref`, `scope_revision_ref` | 모든 local/CI/release 계층이 공유하는 Task·scope identity |
| `source_revisions` | project별 immutable source revision. 여러 Project release는 current ChangeBundleReleaseHandoff의 ProjectReleaseInput과 exact 연결 |
| `identity_binding` | config, Catalog, logical Tool ID/version/descriptor set, resolved Profile fingerprint와 environment별 ToolRegistrySnapshot/executable identity |
| `verification_layers` | local_quick·target·full·release ValidationPlan/Run/Gate ref와 reuse/invalidation 이유 |
| `build_invocation_refs` | architecture별 typed clean build/package invocation과 builder provenance |
| `artifacts` | logical name·role·architecture·크기·media type·SHA-256과 ArtifactRef |
| `artifact_set_digest` | 정렬된 final artifact entry 집합의 canonical digest |
| `included_files_manifest_ref` | package에 실제 포함한 file entry·owner·license 목록 |
| `metadata_refs` | version source, changelog, package policy, license·third-party notice |
| `supply_chain_applicability` | SBOM·provenance·signing별 required/not-required/unavailable/incomplete/complete와 policy ref |
| `sbom_ref`, `provenance_ref`, `signature_refs` | applicable할 때 exact final artifact set에 대한 공급망 자료 |
| `compatibility` | Windows baseline·architecture, Plugin/runtime, config·state·schema와 install·update 범위 |
| `validation_refs` | clean build·test·package, native runtime, install·safe_default·update·rollback·uninstall 결과 |
| `release_gate_refs` | release_preflight부터 release_ready까지 phase별 GateDecision |
| `remote_actions` | publish·deploy·withdraw·rollback별 target, immutable subject와 `planned\|approved\|running\|verified\|outcome_unknown\|rollback_required\|rolled_back\|withdrawn` 상태 |
| `approval_request_refs` | action ID별 exact manifest revision·digest·channel·provider·destination 승인 |
| `remote_operation_refs`, `before_remote_snapshot_refs`, `after_remote_snapshot_refs` | action/target별 external effect와 실제 결과 확인; 한 target 결과로 다른 target을 채우지 않음 |
| `rollback_plan_ref`, `rollback_artifact_ref`, `user_data_policy` | 실패 시 돌아갈 byte·state와 보존 정책 |
| `remaining_risks`, `external_gates` | 미구현·환경·provider·승인 한계 |
| `status` | `draft`, `candidate`, `blocked`, `ready`, `approved`, `publishing`, `publish_outcome_unknown`, `published`, `rollback_required`, `withdrawn` |

`candidate`가 되려면 final artifact set digest가 있어야 한다. artifact byte, source, version, config, Tool, Profile 또는 package file list가 바뀌면 같은 candidate를 수정하지 않고 새 revision을 만든다. verification과 promotion은 같은 byte를 사용하며 rebuild·재압축·signing으로 byte가 달라지면 새 candidate다.

`ready`는 publish됐다는 뜻이 아니고, `approved`는 remote effect가 성공했다는 뜻이 아니다. top-level `published`는 주 publication action의 exact provider after snapshot이 version·source/tag·artifact digest·channel을 확인한 event가 있을 때만 기록한다. deploy는 role별 remote action `verified`와 `deployed_verified` projection을 사용하며 top-level status를 되감거나 가짜 published를 만들지 않는다. adapter success response, local tag, branch name과 이전 snapshot만으로 `published`를 만들지 않는다.

필요하지 않은 SBOM·provenance·signing은 field를 조용히 생략하지 않고 applicability=`not_required`, versioned policy·이유·decision ref를 둔다. required인데 unavailable/incomplete이면 release Gate는 block한다.

### 10단계 release evidence phase

`ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle`, `ReviewPack` v5는 다음 phase와 subject role을 추가한다.

- `release_preflight`
- `release_build`
- `release_verify`
- `release_package`
- `release_install_lifecycle`
- `release_ready`
- `release_publish_preflight`
- `release_publish_verify`

release EvidenceSubjectBinding은 ReleaseManifest revision, Task ID, project source revision set, artifact set digest, config·Catalog·Tool·Profile fingerprint, architecture·environment와 phase를 포함한다. project별 M3/M8/M9 Gate가 current여야 하며 release Gate가 이를 대체하거나 예전 version을 자동 승격하지 않는다.

EvidenceBundle v5는 artifact entry·included-files manifest·metadata/license·supply-chain applicability·install lifecycle·approval·remote before/after ref를 role별로 연결한다. ReviewPack의 기존 section order는 유지하고 release summary를 `checks`, `artifacts`, `permissions`, `risks`, `next_actions` section의 typed subsection으로 넣는다.

### 11단계 Rust style evidence binding

M11 writer는 `ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle`, `ReviewPack` v6 목표를 사용한다. 기존 `patch_pre_apply|patch_post_apply` phase를 유지하고 candidate 검증에는 상세 phase `rust_style_candidate`를 추가한다. current check는 `during_stage`, apply 후 검증은 `patch_post_apply`로 축약하지 않고 각각의 exact subject role을 함께 기록한다.

v6 `SubjectBindingRecord.role`은 다음을 추가한다.

- `rust_style_current_check`: original current byte의 fmt/Clippy Diagnostic과 coverage
- `rust_style_preview_candidate`: final isolated candidate와 candidate ValidationRun
- `rust_style_idempotence_replay`: expected-after fresh preview의 full-pipeline no-op
- `rust_style_actual_after`: SourceMutationPort 적용 뒤 actual target와 post Check

각 record는 owning RecipeExecution/PatchApplication/ValidationRun/GateDecision과 exact EvidenceSubjectBinding을 가진다. 같은 tool/config/policy fingerprint라도 workspace byte가 다르면 role을 합치지 않는다. `rust_style_preview_candidate` evidence를 actual-after로 재사용하거나 check-only result를 apply 성공으로 승격하지 않는다.

M11 evidence의 complete 조건은 다음과 같다.

| evidence 축 | required ref·artifact | `complete` 조건 |
|---|---|---|
| toolchain | RustToolchainBinding | cargo/rustc/rustfmt/clippy-driver version·opaque executable file identity·redacted locator·full hash, pin/config source, parsing/style edition, MSRV, host/target와 component 상태가 있음 |
| policy | RustStylePolicySnapshot | formatting/lint/config source hash, exact fix allowlist, scope/coverage/auto policy와 definition fingerprint가 있음 |
| coverage | RustStyleCoverageMatrix | required package/target/feature/triple/cfg/ownership cell 전체가 executed이고 frontier/conflict가 비어 있음 |
| current | current fmt check, raw/normalized Clippy Diagnostic | current source/config/tool binding과 parser completeness가 있음 |
| steps | ordered RustStyleStepExecution set | `rust_style_v1` 모든 applicable step의 ToolDescriptor/TaskInvocation/result, before/after manifest와 step diff가 있음 |
| suggestions | selected/nonselected suggestion manifest | exact lint ID, span/replacement, applicability, selection/skip reason과 source fingerprint가 있음 |
| hunk mapping | suggestion-to-hunk artifact | 모든 Clippy fix hunk가 허용 `MachineApplicable` suggestion 하나 이상에 byte-exact 대응하고 unmatched hunk 0 |
| side effect | step/final complete filesystem diff | handwritten in-scope `.rs` modify 이외 operation 0; 숨긴 cleanup 없음 |
| impact | preview ChangeSet·ImpactAnalysis·ValidationPlan | M2 scope/Profile/affected Check reconciliation이 current·ready |
| replay | replay RecipeExecution/step set | 같은 binding의 전체 mutation pipeline operation 0 |
| candidate/post | fmt·Clippy·affected ValidationRun·Gate | required run current·complete·stable, candidate/pre/post policy 요구 충족 |
| apply | ApprovalRequest·permit binding·PatchApplication | exact PatchSet, actual receipt/after manifest와 recovery 상태가 사실대로 있음 |

Clippy Diagnostic 0건만으로 coverage complete를 만들지 않는다. allowlist 밖 또는 non-MachineApplicable suggestion도 raw/normalized Diagnostic과 skip reason을 EvidenceBundle에서 삭제하지 않는다. tool output parser가 모르는 item을 만났거나 stdout/stderr/JSON이 truncated면 `unverified`다.

EvidenceBundle v6의 Rust ref는 phase별 `EvidenceRefSet`으로 정렬하며 RustToolchainBinding·policy·coverage·step content fingerprint를 `authoritative_subject_binding_set_fingerprint`와 `bundle_fingerprint`에 포함한다. toolchain/config/Catalog/source/coverage drift는 기존 bundle을 수정하지 않고 새 run을 요구한다. DB projection은 이 ref를 조회용으로 저장할 수 있지만 Git config·Catalog policy를 역으로 생성·수정하지 않는다.

ReviewPack의 기존 stable section 순서는 유지한다. Rust summary는 `planned_vs_actual_changes`에 step/final diff, `check_results`에 fmt/Clippy/coverage/affected Check, `diagnostic_relations`에 selected/skipped suggestion, `gate_decision`에 candidate/pre/post, `remaining_risks_and_questions`에 partial cfg/target·tool drift·recovery, `evidence_identity`에 toolchain/policy/coverage/step/Patch fingerprint를 typed subsection으로 넣는다. 별도 자유 형식 section으로 사실을 재해석하지 않는다.

## 검사 선택 기준

affected 선택의 계산 순서와 promotion trigger는 [변경 계획·영향 분석 정본](change-planning-and-impact.md#affected-검사-선택)이 소유한다. ValidationPlan은 그 계산 결과를 다음 근거로 설명해야 한다.

- actual ChangeSet과 user-intended change scope
- direct/transitive·confirmed/possible ImpactEdge와 risk path
- CheckDescriptor applicability·coverage·scope-binding·invalidation metadata
- Project가 제공하는 trusted Task·Tool·Check와 실제 availability
- package·workspace·project full closure의 soundness
- compatible previous success와 current revision·dirty delta
- 실행 side effect, permission, 시간과 대체 evidence

검사 수가 적은 것과 sound한 범위가 같은 뜻은 아니다. 가장 좁은 sound scope를 선택하고 증명할 수 없을 때만 package→workspace→project full로 승격한다. possible edge가 있다는 사실만으로 무조건 full을 선택하지 않고, 그 edge가 selected Check coverage 밖 closure를 가리키는지와 risk path floor를 함께 본다.

각 candidate family는 `selected_required`, `selected_optional`, `omitted_not_applicable`, `unresolved_not_found`, `blocked_unavailable`, `user_waived` 중 하나여야 한다. 관련 검사 후보를 찾지 못한 `unresolved_not_found`를 “검사가 필요 없음”으로 렌더링하지 않는다.

## 항상 수행할 가벼운 확인

- 작업 시작 전 기존 변경 상태 기록
- 실제 변경 파일 목록 확인
- 계획과 변경 목적 비교
- 명령 종료 결과 확인
- 비밀정보 노출 확인
- 검사 실패 누락 여부 확인
- 완료 증거와 이어하기 기록 생성 여부 확인

이 확인도 작업 종류와 무관한 파일을 모두 읽는 방식으로 구현하지 않는다.

## 필요할 때만 수행할 검사

| 변경 종류 | 필요한 검사 예시 |
|---|---|
| 문서 | 내부 연결, 중복 기준, 맞춤법, 명령 예시 |
| 코드 | 형식, 코드 검사, 빌드, 관련 테스트 |
| 공개 사용 약속 | 이전 사용 방식과의 호환, 예제, 문서 |
| 공개 API·Schema·contract | 호환 diff, contract test, provider·consumer 검사, migration guide |
| 새 의존 항목 | 필요성, 잠금 파일, 보안, 라이선스 |
| lockfile·workspace manifest | lock consistency, affected workspace build·test |
| 설정 | 읽기, 잘못된 값, 기본값, 이전 설정 호환 |
| validator·policy | negative fixture, self-test, 검증기 보호와 gate regression |
| migration | forward·rollback rehearsal, invariant, backup·restore |
| generated source | generator input/output 일치, regeneration diff, consumer 검사 |
| workflow·release | workflow syntax, package dry-run, release readiness와 rollback |
| 파일 저장 | 중단 복구, 동시 접근, 손상 방지 |
| 화면 | 실제 실행, 주요 흐름, 오류 표시 |
| 원격 작업 | 대상, 권한, 상태, 실행 결과 |
| 병합 | 충돌, 변경 겹침, 통합 검사 |
| 배포 | 버전, 산출물, 설치, 되돌리기 |

## 검사 단계

### 작업 중 빠른 검사

현재 고친 부분과 직접 관련된 가장 빠른 검사를 실행한다.

### 단계 종료 검사

단계 목적과 관련된 모든 필수 검사를 실행한다.

### 목표 종료 검사

여러 단계가 합쳐진 최종 상태에서 필요한 전체 검사를 실행한다. 전체 검사가 지나치게 비싸고 같은 증거를 반복한다면 생략 이유와 대체 증거를 기록한다.

## 테스트가 없는 프로젝트

- 현재 프로젝트가 제공하는 실행 방법과 예제를 먼저 사용한다.
- 변경 위험이 크면 필요한 최소 테스트를 추가한다.
- 테스트 추가 자체가 과도하면 수동 확인 방법과 남은 위험을 기록한다.
- 테스트가 없다는 이유로 검증한 것처럼 보고하지 않는다.

## 실제 동작 확인

화면, 서버, 외부 도구 연동처럼 코드 검사만으로 알 수 없는 기능은 실제 경로를 실행한다. 가능하면 가짜 입력만 확인하지 않고 실제 사용자 경로를 확인한다.

## 4단계 Patch engine Gate 계약

`patch_pre_apply` GateDecision은 accepted ChangePlan, immutable PatchSet, current planning-baseline/observed ChangeSet, WorkspaceSnapshot, ValidationPlan·config·Catalog·Tool fingerprint를 project별 `EvidenceSubjectBinding`과 binding set fingerprint로 고정한다. 자동 apply는 decision이 `auto_pass`이고 apply 직전 binding set probe가 같을 때만 허용한다. `human_review`는 exact PatchSet fingerprint에 대한 별도 사용자 승인과 policy가 있을 때만 수동 apply 경로로 진행할 수 있으며 자동 통과로 바꾸지 않는다. `block`은 source effect를 시작할 수 없다.

M4 pre-apply binding에는 RecipeExecution preview·idempotence replay, resolved selector, `recipe_preview` ChangeSet, reconciled ImpactAnalysis·ValidationPlan, WorktreeDecision, preexisting manifest, forward/reverse artifact hash와 `idempotence=proved`를 포함한다. preview completeness가 complete가 아니거나 Recipe/tool/config/Index fingerprint가 바뀌면 apply permit을 만들지 않는다.

pre-apply 뒤 source, PatchSet, plan, config, Catalog, Rule, Tool 또는 approval scope가 바뀌면 기존 GateDecision은 stale이다. Patch engine은 기존 decision을 재사용하지 않고 M2 replan 또는 새 pre-apply Gate를 요구한다.

`patch_post_apply`는 적용 뒤 새 ProjectRevision·WorkspaceSnapshot·`observed_after_change` ChangeSet과 PatchSet actual operation manifest를 subject로 사용한다. required Check evidence는 이 after binding과 exact 일치해야 한다. 자동 완료는 post-apply `auto_pass`에서만 허용한다. `human_review`는 적용된 workspace를 보존한 대기 상태, `block`은 실패·복구 상태이며 기존 사용자 변경을 자동 rollback하지 않는다.

PatchApplication이 `partially_applied|outcome_unknown|recovery_required`이면 post-apply Gate는 실제 상태를 evidence로 보존하되 성공을 계산하지 않는다. reverse PatchSet은 별도 current precondition·PermissionPlan·Gate를 요구하며 completed operation receipt의 역순으로만 실행한다. primary checkout hard reset이나 사용자 변경 삭제는 rollback evidence가 아니다.

pre/post GateDecision은 같은 ChangePlan·PatchSet lineage와 서로 다른 phase subject binding을 가진다. 각 phase는 별도 ValidationPlan revision/ref를 사용할 수 있지만 Profile closure·selected Check lineage와 PatchSet은 호환돼야 한다. pre plan은 `patch_before`, post plan은 `patch_expected_after` PhaseSubjectExpectation을 가지며 runner가 post-apply에 새 검사를 즉석 추가하지 않는다. EvidenceBundle과 ReviewPack은 두 plan·decision, 중간 apply event와 before/after fingerprint를 모두 연결한다.

### 5단계 Managed Registry 추가 Gate

Registry 변경도 위 Patch Gate를 그대로 사용하며 별도 DB mutation Gate를 만들지 않는다. `patch_pre_apply`는 authoritative Git manifest hash, current ManagedRegistrySnapshot, 대상 declaration의 before/expected-after fingerprint, namespace claim·tombstone, M1 binding, M2 consumer impact·compatibility table, alias window와 M4 RecipeExecution·PatchSet을 exact binding으로 고정한다. snapshot이 stale/partial/unverified이거나 duplicate ID, namespace collision, ID reuse, unresolved required binding, consumer transition 누락이 있으면 자동 apply를 허용하지 않는다.

`patch_post_apply`는 actual manifest와 source definition/reference/generated output을 다시 scan해 actual-after ManagedRegistrySnapshot을 만든다. required binding·consumer coverage가 complete이고 `binding-drift`, `removed-reference`, `alias-window-expired`, `generated-output-stale`, `docs-schema-drift` blocking Diagnostic이 0이며 actual-after가 기대와 같을 때만 `registry_current`를 verified로 평가할 수 있다. generated source 직접 편집은 generator output이 우연히 일치해도 Gate를 차단한다.

source와 DB Index가 다르면 Git manifest를 authoritative input으로 삼고 DB snapshot은 `stale_registry_index` evidence다. stale row를 source로 되쓰거나 기존 GateDecision에 맞추지 않는다. cross-project consumer는 M2 read-only impact와 ReviewPack에 포함하지만 9단계 전 다른 Project PatchApplication은 `REGISTRY_CROSS_PROJECT_APPLY_UNSUPPORTED`로 거부한다.

### 6단계 compatibility·documentation·environment 추가 Gate

`api_contract_change`와 `docs_config_environment` Profile이 활성화되면 M2 ValidationPlan은 위 6단계 evidence 축과 B04/B07 Check를 required set에 materialize한다. M3 runner, doctor와 comparator가 실행 직전에 새 검사를 고르거나 문서 command를 raw shell로 추가하지 않는다.

`patch_pre_apply`는 explicit baseline approval, baseline/current-before surface, Registry snapshot, consumer coverage, compatibility/window/migration requirement, `contract_change_group_id`, expected public expansion과 companion change set을 exact binding으로 고정한다. 같은 Project의 companion은 한 PatchSet이어야 하고 cross-project companion은 별도 read-only ChangePlan 상태로 연결한다. baseline 부재·mutable ref, required consumer partial, breaking migration guide 누락, 의도되지 않은 public 확대, generated output direct edit와 doctor side-effect 요구가 있으면 apply permit을 만들지 않는다.

`patch_post_apply`는 actual-after source에서 current surface·Registry·documentation·config·environment evidence를 다시 만든다. public source, Schema/file descriptor, generated reference, docs, compatibility metadata와 required migration guide가 같은 lineage에서 충족되고 blocking Diagnostic이 없을 때만 contract change가 verified다. before report, DB latest row, timestamp가 최근인 doctor report를 actual-after evidence로 재사용하지 않는다.

docs-only 또는 environment 진단처럼 source 적용이 없는 실행은 `stage_exit|goal_exit` Gate를 사용한다. local link·anchor·Schema/config example은 pure check로, command/snippet/doctor는 exact registered read-only descriptor로만 실행한다. unsafe/unregistered command, missing toolchain 또는 설치 필요는 자동 수정하지 않고 `not_run`·Diagnostic·manual remediation으로 보존한다.

clean-room readiness는 실제 clean-room pass가 아니다. reproducibility claim이 required면 이미 준비된 disposable environment에서 exact `CleanRoomSpecification`과 selected Check로 얻은 current result가 있어야 한다. network/package/system mutation 금지 때문에 prerequisite가 없으면 그 조건을 완화하거나 설치하지 않고 block/review한다.

### 7단계 failure·security·dependency 추가 Gate

`debug_recovery`, `security_supply_chain` 또는 `dependency_upgrade` Profile이 활성화되면 M2는 M7 rule family와 required evidence 축을 ValidationPlan에 materialize한다. runner·scanner·debugger·package manager adapter는 실행 중 required set을 축소하거나 직접 GateDecision을 만들지 않는다.

failure Gate는 family/occurrence fingerprint rule version, exact subject, root candidate/cascade evidence, ReproductionPack redaction과 RegressionRecord before/after compatibility를 검사한다. required pack이 `blocked_external|unverified`, after result가 다른 subject·input·environment 또는 flaky이면 fixed/pass로 판정하지 않는다. rollback·roll-forward·restore는 각각 plan·attempt·validation을 가져야 한다.

security Gate는 external source/query/schema, tool identity, coverage와 freshness를 검사한다. required 자료가 `stale|unknown|unavailable`, secret redaction이 `unknown|quarantined`인데 default report에 포함됨, workflow permission 확대·mutable external action·release manifest 누락이 unresolved이면 policy floor에 따라 `HUMAN_REVIEW|BLOCK`이다. “scanner finding 0건”은 current complete coverage가 없으면 clean evidence가 아니다.

dependency `patch_pre_apply`는 current DependencySnapshot, update candidate, affected Project, package manager owner, approved effect, actual isolated preview diff, replan, immutable PatchSet, previous manifest·lockfile와 rollback plan을 고정한다. network/download/dependency change 승인 누락, lockfile 직접 편집, out-of-scope write, preview 뒤 replan 누락이면 apply permit을 만들지 않는다. 기본 상태는 `awaiting_apply_approval`이다.

dependency `patch_post_apply`는 actual source·manifest·lockfile을 재수집하고 registered package manager의 locked/appropriate verification, M2 selected Check와 M3 Gate를 실행한다. adapter의 update 성공이나 새 lockfile 존재만으로 `validated`를 설정하지 않는다. 실패하면 before lockfile을 보존하고 rollback evidence를 별도 Gate subject로 남긴다.

Maintenance Radar는 Gate 입력을 바꾸지 않는 derived view다. 같은 input refs와 `evaluation_time`에서 deterministic sort가 같아야 하며, `valid_until`을 지나면 current dashboard로 표시하지 않는다.

### 8단계 migration·performance·language/platform 추가 Gate

`data_config_db_migration`, `performance_build` 또는 `language_platform_migration` Profile이 활성화되면 M2는 M8 rule/check/evidence floor와 v3 phase를 ValidationPlan에 materialize한다. migration tool, benchmark, profiler, build analyzer, compiler·test runner와 codegen/codemod adapter는 required set을 줄이거나 GateDecision을 직접 만들지 않는다.

#### `migration_pre_execute`

다음 조건을 모두 확인한다.

- exact `ProjectMigrationManifest`, current/target version source와 unique continuous chain
- current plan fingerprint, source·target·config·Catalog·Tool·environment precondition
- live target을 쓰지 않은 complete dry-run과 expected destructive/loss/unknown field scope
- consistent backup의 integrity 수준과 policy가 요구한 `restore_rehearsed|restore_validated`
- 같은 chain·tool·compatible environment의 complete stable migration rehearsal
- required M4 source PatchSet·post Gate, M6 consumer/window와 M7 RecoveryPlan
- live/destructive effect의 exact PermissionDecision과 irreversible boundary

chain gap/ambiguity, unknown version, unpreserved unknown field, backup/restore/rehearsal 부족, stale plan, outcome unknown과 approval 누락이면 execute permit을 만들지 않는다. backup path·checksum 하나만으로 restore readiness를 만족시키지 않는다.

PermissionDecision은 미래 GateDecision을 참조하지 않고 exact MigrationPlan, ValidationPlan·GatePolicy, subject/tool/environment와 expiry를 고정한다. Controller는 이 pre Gate가 허용된 뒤 PermissionDecision과 actual GateDecision fingerprint를 함께 결합한 single-use in-memory execute permit을 발급한다. persisted approval 또는 Gate ID 하나만으로 migration target port를 열지 않는다.

#### `migration_post_execute`

actual target을 다시 읽어 target version, ordered step receipt, checkpoint prefix, active/candidate pointer, required invariant, consumer·contract·startup Check와 actual before/after를 평가한다. tool exit 0, target version header 하나 또는 일부 record count만으로 `succeeded`를 만들지 않는다.

`partially_succeeded|outcome_unknown|failed|rollback_required|rollback_failed`는 `AUTO_PASS`할 수 없다. isolated candidate partial은 live active target 유지 근거와 함께 failure로 보존하고, live partial은 protected invariant `BLOCK`이다.

#### `migration_post_rollback`

rollback·restore·roll-forward attempt를 구분하고 before-compatible version·data scope·consumer behavior, active pointer와 required invariant를 새 subject에서 확인한다. reverse command exit 0 또는 이전 파일 존재만으로 `rolled_back`을 만들지 않는다. rollback 뒤에도 unknown field loss·consumer incompatibility 또는 unverified active state가 남으면 block한다.

#### `performance_compare`

explicit `PerformanceWorkloadSpec`, 같은 workload·input·driver/collector·environment·build/cache mode, cohort 내부 exact revision과 intent별 `allowed_delta_axes`를 확인한다. source/migration 비교가 아니면 양쪽 revision도 같아야 하고, toolchain/config/platform intent가 아니면 해당 axis도 같아야 한다. 여러 axis가 달라지면 선언된 factorial plan 없이는 causal claim을 만들지 않는다. numeric value·unit·collector, warmup/measured 분리, minimum sample, predeclared noise/outlier rule와 raw attempt가 있어야 한다.

required metric이 `no_measurement|not_comparable|noisy|inconclusive|unmeasured`이면 performance pass/regression/improvement를 만들지 않는다. optional Profile이면 remaining risk로 보고할 수 있지만 required budget·migration downtime·language equivalence이면 `HUMAN_REVIEW|BLOCK` floor를 적용한다. candidate correctness·contract·test Gate가 실패하면 수치가 개선돼도 최적화 완료가 아니다.

#### `language_cutover`

baseline behavior contract, boundary adapter, reader-first·consumer transition order, compatibility window, M4 source/codegen/codemod lineage, exact platform evidence, rollback/data compatibility와 user approval을 확인한다. `EquivalenceReport`의 모든 required dimension이 current·complete·stable `equivalent`여야 automatic cutover 후보가 된다.

cutover PermissionDecision도 exact LanguageMigrationPlan, ValidationPlan·GatePolicy와 subject에 먼저 결합한다. Controller는 current `language_cutover` GateDecision 뒤에만 approval과 Gate를 함께 묶은 single-use in-memory permit을 발급한다.

compile/build pass는 build dimension만 만족한다. required runtime behavior·error·serialization·state·concurrency·security·consumer·platform 또는 declared performance가 partial/not_run/unverified면 전체 equivalence는 pass가 아니다. evidence는 complete하지만 reflection·FFI·platform API 같은 의미를 결정할 수 없으면 CLI-only `HUMAN_REVIEW`다. 실제 실행하지 않은 OS·architecture는 `unverified`이며 cross-compile을 runtime pass로 바꾸지 않는다.

#### 9단계 handoff

`CrossProjectMigrationHandoff`는 read-only evidence다. project별 plan·PatchSet·Gate·backup/restore·rollback과 dependency edge를 검증할 수 있지만 이를 `ChangeBundle`, cross-project approval 또는 apply success로 해석하지 않는다. 9단계가 current participant를 다시 bind하고 새 coordination plan/Gate를 만들어야 한다.

### 9단계 CrossRepo ChangeBundle 추가 Gate

9단계는 project-local Patch/migration/merge Gate를 그대로 유지하고 bundle 전용 Gate를 두 번만 추가한다.

#### `change_bundle_prepare`

첫 participant source effect 전에 다음을 모두 확인한다.

- exact MultiProjectGoal·CrossRepoChangeBundle revision과 acyclic BundleStepGraph
- 모든 required participant의 current Project/Checkout/base/dirty snapshot, M2 plan, M4 PatchSet 또는 M8 plan
- project별 pre Gate·recovery plan과 current M5/M6 provider/consumer·compatibility evidence
- file·range·rename·symbol·contract·generated owner·lockfile overlap 결과와 parallel/serial 결정
- worktree ownership 계획, merge order와 BudgetSnapshot reservation
- local action PermissionPlan. future remote action은 별도 승인 없이는 readiness를 execution permit으로 만들지 않음

stale·partial·unverified participant, dependency cycle, overlap unknown, missing rollback, exhausted/unknown required budget과 open outcome-unknown effect가 있으면 `auto_pass`가 아니다. 이 GateDecision은 project Patch apply permit을 대체하지 않는다.

#### project apply·merge

각 Project source apply는 기존 `patch_pre_apply|patch_post_apply` 또는 M8 phase를 사용한다. Star-owned integration worktree 결과는 `phase=merge`, project-local MergePlan v2·ProjectMergeResult를 사용한다. 한 Project pass를 다른 Project required run에 재사용하지 않는다.

base tip·PatchSet·dirty state·compatibility relation이 바뀌면 해당 participant와 downstream edge는 stale이다. old Gate를 새 base에 rebase하지 않고 새 plan·PatchSet·MergePlan·Gate를 만든다.

#### `change_bundle_goal_exit`

다음 조건을 모두 만족할 때만 전체 Goal `auto_pass` 후보가 된다.

- requested completion target에 필요한 모든 required participant가 완료됨
- project별 post/merge Gate와 EvidenceBundle이 current·complete함
- required compatibility window·consumer minimum version·cross-project invariant가 충족됨
- `partially_applied|rollback_required|held|outcome_unknown` participant가 없음
- 열린 effect와 required pending approval이 없음
- `remote_merged` target이면 exact project commit과 PR/check/merge after snapshot이 current함
- `release_handoff_ready` target이면 project별 immutable commit·artifact subject·Gate가 ChangeBundleReleaseHandoff에 exact 연결됨

global GateDecision은 participant별 binding을 정렬해 set fingerprint를 만들고 representative repository revision으로 축약하지 않는다. `CoordinatedOperation=completed`, local commit, push response 또는 PR open은 전체 Goal success evidence가 아니다.

## 자동 수정과 재검사

- 자동 수정은 [4단계 엔진](safe-patch-and-codemod.md)의 dry-run RecipeExecution과 immutable PatchSet을 만들고 pre-apply Gate를 통과한 경우에만 수행할 수 있다.
- 같은 실패를 무한 반복하지 않는다.
- 검사를 통과시키기 위해 테스트를 삭제하거나 기준을 약화하면 안 된다.
- 새로운 위험이나 유료 동작이 생기면 새 단계 또는 승인으로 전환한다.
- external codemod·formatter·generator를 live target checkout에서 직접 실행하지 않고 격리 preview diff를 PatchSet으로 정규화한다.
- format·build·test·contract Check는 Recipe shell text가 아니라 M2 ValidationPlan이 선택하고 M3 post Gate가 실행한다.
- project doctor와 clean-room readiness는 install/download/system setting/source/config/generated output을 수정하는 자동 fix를 제공하지 않는다.

## 독립 검토

다음 변경은 실행 context에서 허용하는 독립 검토를 기본으로 한다. 결정적 Check를 먼저 수행하고, Codex-managed 실행에서는 처음 작업한 Codex와 분리된 review를 보조 evidence로 사용할 수 있다. CLI-only에서는 Codex·AI review를 요구하거나 호출하지 않고 사람이 판단해야 하는 항목을 `human_review`로 남긴다.

- 공개 사용자와의 약속
- 권한과 승인 정책
- 비밀정보 처리
- 파일 손상 또는 복구
- 병합과 배포
- 중요한 모델 배정 규칙
- 반복 실패 뒤의 최종 변경

독립 검토가 없다는 이유로 결정적 실패를 통과시키지 않으며, 독립 검토가 통과했다는 이유로 stale·partial·not_run 결과를 성공으로 바꾸지 않는다.

## 자동 완료 조건

다음 조건을 만족하면 별도 사람 승인 없이 완료 처리할 수 있다.

- 목표의 완료 조건 충족
- 필수 단계 모두 완료
- 모든 required Check가 `clean_pass` 또는 policy가 허용한 `ratchet_satisfied`
- required evidence가 current·complete·stable이고 final subject binding과 일치
- 실패와 생략된 검사가 숨겨지지 않음
- blocking new·worsened Diagnostic, contradicted claim, expired·stale blocking suppression과 pending human review가 없음
- 병렬 변경이 모두 통합됨
- 완료 증거 생성
- 남은 위험 기록
- 비용 한도 위반 없음

사용자는 설정으로 최종 사람 승인을 추가할 수 있다.

여기서 완료 증거 생성은 GateDecision 뒤 EvidenceBundle과 ReviewPack이 위 단방향 순서로 모두 commit되고 `completeness=complete`라는 뜻이다. `auto_pass` GateDecision만 존재하고 packaging이 실패한 상태는 자동 완료가 아니다.

## 증거 묶음

최종 보고에는 다음을 포함한다.

- 무엇을 요청받았는지
- 어떤 단계로 처리했는지
- 실제로 무엇을 바꿨는지
- 어떤 검사를 실행했는지
- 통과, 실패, 생략 결과
- 범위가 어떻게 달라졌는지
- 모델과 실행 방식을 왜 선택했는지
- 재시도와 승급 내역
- 사용량과 유료 동작
- 남은 위험
- 다음에 이어갈 위치

## 로그 원칙

- 사용자에게는 핵심 요약과 실패 위치를 먼저 보여준다.
- 전체 원문이 재현에 필요하면 별도 파일로 저장한다.
- 반복되는 출력은 압축한다.
- 비밀정보는 저장 전에 가린다.
- 검사를 실행하지 않았으면 미실행이라고 기록한다.
