# 9단계 여러 프로젝트 변경·worktree·병합·원격 상태 연결

## 목적과 현재 상태

이 문서는 하나의 사용자 목표가 여러 Git repository의 source·contract·data·검사·원격 상태를 함께 바꿀 때 사용하는 9단계 정본이다. 1단계 Project Catalog, 2단계 영향 graph, 3단계 Validation Gate, 4단계 single-project PatchSet, 5단계 managed consumer, 8단계 migration·rollback 결과를 **project별 실행 단위로 유지한 채** 조정한다.

이번 반영은 문서 설계뿐이다. 실제 worktree·branch·commit·merge를 만들거나 push·PR·remote merge·publish를 실행하지 않았고, 아래 contract type·Schema·state projection·CLI·Git/remote adapter도 아직 구현됐다고 보지 않는다.

핵심 경계는 다음과 같다.

```text
M1 current Project·Checkout·revision
  + M2 project별 ChangePlan·dependency impact
  + M3 project별 Gate·evidence
  + M4 project별 immutable PatchSet·recovery
  + M5/M6 provider·consumer·compatibility window
  + M8 project별 migration·rollback handoff
  -> MultiProjectGoal
  -> CrossRepoChangeBundle
  -> project별 worktree·apply·validation·local integration
  -> 선택적인 remote snapshot·승인된 remote operation
  -> 전체 Goal Gate
  -> 10단계 ChangeBundleReleaseHandoff
```

CLI-only 경로가 기본이다. Codex 병렬 실행은 같은 application command를 호출할 수 있는 선택 소비자일 뿐, ChangeBundle contract·dependency order·worktree·merge·Gate·remote adapter의 필수 dependency가 아니다.

## 범위와 제외 범위

### 포함

- 여러 Project를 가진 `MultiProjectGoal`과 provider·consumer·data owner·tooling 관계
- project별 exact base revision, dirty snapshot, PatchSet, Gate와 evidence
- project·단계별 독립 worktree와 project-local integration worktree
- file·rename·symbol·contract·generated owner·lockfile 겹침 사전 검사
- dependency·compatibility 순서, merge queue와 병렬·resource 한도
- 부분 적용, 검증 대기, rollback 필요, 보류, 재개와 명시적 compensation
- local integration과 remote push·PR·check·merge 상태의 분리
- adapter가 관찰한 `RemoteStateSnapshot`과 승인된 `RemoteOperationRecord`
- project별 source revision·artifact·Gate를 10단계로 넘기는 release handoff

### 제외

- 여러 repository를 하나의 ACID transaction 또는 원자적 Git history로 만드는 기능
- 사용자 checkout의 자동 stash, reset, clean, checkout, force update 또는 untracked 삭제
- PatchSet을 새 base에 자동 rebase하거나 stale 승인을 재사용하는 기능
- 승인 없는 commit, target branch update, push, PR 생성·수정, remote merge와 publish
- provider별 Git hosting API를 core contract에 직접 노출하는 기능
- remote 상태를 branch 이름, local commit 또는 adapter exit code만으로 추측하는 기능
- Codex가 별도 ChangeBundle·merge engine 또는 direct Git writer가 되는 기능
- 자체 scheduler, background merge queue, browser UI와 release publish

## 선행 계약과 책임

| 선행 단계 | 9단계가 재사용하는 것 | 9단계가 다시 정의하지 않는 것 |
|---|---|---|
| 0단계 | Controller 단일 Writer, global/project store, `CoordinatedOperation`, ArtifactRef | DB backend·cross-store transaction 구현 |
| 1단계 | ProjectId, ProjectCheckout, Git common repository identity, revision·dirty snapshot | repository discovery·path binding |
| 2단계 | TaskSpec·ScopeRevision, project별 ChangePlan, ImpactEdge와 dependency graph | 자동 source 계획 생성 |
| 3단계 | ValidationPlan·Run·GateDecision·EvidenceBundle | 검사 실행·완료 판정 engine |
| 4단계 | 한 Project·Checkout의 PatchSet·PatchApplication·reverse/discard recovery | cross-project PatchSet 또는 merge |
| 5·6단계 | owner·provider·consumer, contract baseline, compatibility·deprecation window | Registry/source 정본 변경 |
| 7단계 | RecoveryPlan·ReproductionPack·dependency update rollback | package manager·debugger 구현 |
| 8단계 | project별 MigrationPlan·Gate·restore/rollback과 `CrossProjectMigrationHandoff` | handoff를 실행 승인으로 승격 |

`CoordinatedOperation`은 global/project **관리 store**의 event·projection commit을 복구하는 계약이다. Git repository의 source effect, local commit, remote PR 또는 merge를 원자적으로 commit한다는 뜻이 아니다. ChangeBundle은 이를 상태 기록에 사용할 수 있지만 source effect의 성공은 각 participant receipt와 actual probe로 따로 판정한다.

## 9단계 계약 Inventory

새 top-level 목표 계약은 다음 9개다. `ProjectRelation`, `BundleStep`, `CompatibilityWindow`, `OverlapItem`, `MergeQueueEntry`, `ProjectReleaseInput`은 owning Schema의 `$defs`에 두며 별도 top-level Schema로 복제하지 않는다.

| 계약 | schema ID | 저장 범위 | 역할 |
|---|---|---|---|
| `MultiProjectGoal` | `star.multi-project-goal` | global store | GoalSpec을 project relation·step DAG로 정규화 |
| `CrossRepoChangeBundle` | `star.cross-repo-change-bundle` | global store | participant ref·순서·정책·집계 상태 |
| `ChangeBundleParticipant` | `star.change-bundle-participant` | project store | 한 Project의 base·dirty·Patch·Gate·복구 상태 |
| `WorktreeRecord` | `star.worktree-record` | project store | Star-Control owned worktree identity·lifecycle |
| `MergeQueueRecord` | `star.merge-queue-record` | project store | 한 repository의 직렬 integration queue |
| `MergeConflictRecord` | `star.merge-conflict-record` | project store | 양쪽 의도·contract·resolution·재검사 |
| `ProjectMergeResult` | `star.project-merge-result` | project store | local integration actual revision과 Gate |
| `RemoteOperationRecord` | `star.remote-operation-record` | project store + global summary ref | push·PR·merge 등 한 remote effect의 사실 |
| `ChangeBundleReleaseHandoff` | `star.change-bundle-release-handoff` | global store + project refs | 10단계 project별 source·artifact 입력 |

기존 `MergePlan`은 project-local v2로, `RemoteStateSnapshot`은 adapter-bound v2로 확장한다. 9단계 evidence ref를 포함하는 `ValidationPlan`, `ValidationRun`, `GateDecision`, `EvidenceBundle`은 v4 목표다. 이전 version은 history 조회에는 사용할 수 있지만 current ChangeBundle Gate로 자동 승격하지 않는다.

모든 top-level document는 공통 envelope, `schema_version`, immutable `revision`, 이전 revision ref와 JCS SHA-256 fingerprint를 가진다. current 상태는 event를 접은 projection이며 같은 revision byte를 덮어쓰지 않는다.

## 식별과 경로 경계

- Project 연결 key는 stable `ProjectId`다. display name, repository folder name과 절대 경로를 identity로 쓰지 않는다.
- local checkout은 `CheckoutId`, Git repository는 adapter가 산출한 opaque common-repository fingerprint, worktree는 `WorktreeId`와 protected root binding으로 가리킨다.
- persisted document·event·DB·CLI JSON·ReviewPack에는 raw root, `.git` path, 사용자 이름과 credential URL을 넣지 않는다.
- source 위치는 `ProjectPathRef`, symbol은 stable Index entity key, contract는 ManagedDeclaration·ContractSurface·Schema ID로 표현한다.
- remote repository는 credential을 제거한 canonical `remote_identity`와 provider-owned opaque ref로 연결한다. remote URL text를 ProjectId 대신 쓰지 않는다.
- 한 project의 private path·symbol·Diagnostic detail을 다른 project participant나 global bundle에 inline 복제하지 않는다. global record는 DocumentRef·fingerprint·summary만 가진다.

## MultiProjectGoal

`MultiProjectGoal`은 기존 `GoalSpec`을 대체하지 않는다. 사용자가 승인한 GoalSpec·TaskSpec revision을 여러 Project 실행에 필요한 typed relation과 step graph로 정규화한 immutable document다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `multi_project_goal_id`, `revision` | 예 | stable ID와 immutable revision |
| `goal_spec_ref` | 예 | exact GoalSpec revision·hash |
| `task_spec_refs`, `scope_revision_refs` | 예 | project target과 accepted scope 입력 |
| `participants` | 예 | ProjectId, required 여부, role set과 source-of-truth summary |
| `project_relations` | 예 | provider·consumer·data/tooling 관계와 evidence |
| `step_graph` | 예 | project-local 실행 node와 cross-project edge DAG |
| `compatibility_windows` | 예 | provider introduction부터 consumer 전환·provider removal까지의 window |
| `cross_project_invariants` | 예 | 여러 Project evidence를 함께 대조할 조건 |
| `completion_target` | 예 | `validated_participants\|local_integrated\|remote_merged\|release_handoff_ready` |
| `resource_budget` | 예 | 동시성·worktree·process·disk·artifact·시간 상한 |
| `permission_floor_ref` | 예 | local Git·remote action의 최소 정책 |
| `source_snapshot_refs` | 예 | M1 Catalog·project별 Index/revision freshness |
| `unknowns`, `questions` | 예 | unresolved relation·순서·compatibility 판단 |
| `goal_fingerprint` | 예 | 위 의미 field의 canonical hash |

participant role은 `provider|consumer|data_owner|tooling|validation_only`의 non-empty set이다. 한 Project가 provider와 consumer를 함께 가질 수 있으며, role 하나만 보고 실제 적용 순서를 정하지 않는다. `required=false` participant는 전체 완료에 필수는 아니지만 해당 project의 실패·미실행·remaining risk를 숨기지 않는다.

### ProjectRelation

`ProjectRelation`은 다음 field를 가진다.

| 필드 | 의미 |
|---|---|
| `relation_id` | stable relation ID |
| `provider_project_id`, `consumer_project_id` | 방향이 있는 ProjectId |
| `relation_kind` | `api\|schema\|format\|config\|error_code\|artifact\|dependency\|data\|tooling\|runtime` |
| `contract_refs` | ManagedDeclaration·ContractSurface·Schema·artifact contract ref |
| `accepted_versions` | consumer가 현재 읽거나 호출할 수 있는 범위 |
| `minimum_provider_version` | consumer 전환 뒤 필요한 최소 provider revision/version |
| `certainty` | `confirmed\|possible\|unknown` |
| `evidence_refs` | M1/M2/M5/M6 observation |
| `freshness` | current·stale·partial·unverified |
| `limitations` | dynamic lookup·unregistered consumer 등 |

자동 순서는 `confirmed`이며 current·complete한 relation만 사용한다. `possible|unknown`, stale·partial relation과 cycle은 무시하지 않고 `human_review|block`으로 보낸다.

### BundleStepGraph

Project relation graph와 실행 graph를 분리한다. 같은 provider가 compatibility 시작과 종료에 두 번 나타날 수 있으므로 ProjectId만 node로 쓰면 안전한 순서를 표현할 수 없다.

`BundleStep`은 `step_id`, ProjectId, optional StageId, `step_kind`, project-local input/output ref, expected effect, required Gate와 completion condition을 가진다. `step_kind`는 최소 다음을 지원한다.

- `provider_compatibility_open`
- `project_patch_apply`
- `project_migration`
- `project_validate`
- `consumer_transition`
- `project_local_integrate`
- `provider_compatibility_close`
- `remote_push`, `remote_pr`, `remote_merge`
- `bundle_goal_validate`

edge는 `requires|provider_before_consumer|schema_before_codegen|reader_before_writer|consumer_before_provider_removal|validation_before_integration|local_before_remote` 중 하나와 reason·evidence를 가진다. graph는 DAG여야 하며 존재하지 않는 node, self edge와 의미 없는 중복 edge를 거부한다.

source effect를 내는 StageSpec은 정확히 한 ProjectId·CheckoutId를 가진다. 여러 Project를 가진 coordinator Stage는 read-only plan·status·Gate aggregation만 수행한다.

### provider·consumer compatibility window

`provider-before-consumer`는 breaking provider removal을 먼저 배포하라는 뜻이 아니다. 기본 순서는 다음과 같다.

1. provider가 old/new consumer를 함께 허용하는 additive·alias·dual-read 경계를 연다.
2. provider의 project-local Gate와 declared completion level을 충족한다.
3. consumer를 dependency order에 따라 전환하고 consumer별 compatibility evidence를 수집한다.
4. finite window 동안 actual consumer state와 minimum accepted version을 관찰한다.
5. required consumer coverage, old reference 0, rollback readiness와 별도 승인을 확인한다.
6. provider의 deprecated path·alias·old writer를 마지막 step에서 제거한다.

`CompatibilityWindow` 최소 field는 window ID, contract ref, provider ProjectId, required consumer set, open/close step, old/new accepted version, opened revision, deadline 또는 evidence-based close condition, current consumer state refs, rollback trigger와 status `planned|open|closing_ready|closed|expired_unresolved|blocked`다.

provider가 compatibility window를 제공할 수 없는 경우 이를 simultaneous transaction이라고 부르지 않는다. downtime/cutover plan, partial success 영향, reverse order 가능성, project별 rollback 한계와 exact 사용자 승인을 가진 별도 high-risk graph가 필요하다.

## CrossRepoChangeBundle

`CrossRepoChangeBundle`은 여러 project-local 계획을 실행 순서와 완료 기준으로 묶는 global coordinator document다. source byte·Patch operation·Diagnostic detail을 소유하지 않는다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `change_bundle_id`, `revision` | 예 | bundle identity와 immutable revision |
| `multi_project_goal_ref` | 예 | exact goal relation·step graph |
| `task_spec_refs`, `scope_revision_refs` | 예 | 사용자 의도와 accepted scope |
| `input_handoff_refs` | 예 | optional M8 handoff 등; 없으면 empty |
| `participant_refs` | 예 | ProjectId 정렬 `ChangeBundleParticipant` ref·fingerprint |
| `step_graph` | 예 | 실행할 BundleStep DAG revision |
| `compatibility_window_refs` | 예 | current window set |
| `merge_policy` | 예 | project별 target completion·strategy·protected branch policy |
| `remote_policy` | 예 | `disabled\|observe_only\|approved_actions_only`; 기본 `disabled` |
| `resource_budget`, `budget_snapshot_ref` | 예 | resolved limit와 현재 예약·관찰량 |
| `permission_plan_ref`, `gate_policy_fingerprint` | 예 | local/remote effect floor |
| `prepare_gate_ref`, `goal_gate_ref` | 해당 시 | bundle 시작 전·전체 완료 Gate |
| `state` | 예 | 아래 bundle state |
| `completion_level_reached` | 예 | 현재 실제 수준; requested target과 분리 |
| `open_effect_refs`, `pending_approval_refs` | 예 | 재개 전 확인할 effect·approval |
| `remaining_risks`, `hold_reasons` | 예 | partial·remote·compatibility 위험 |
| `supersedes_bundle_ref` | revision > 1 | base·scope·order 변경 전 revision |
| `bundle_fingerprint` | 예 | participant ref·graph·policy·budget의 canonical hash |

M8 handoff는 current participant를 발견하는 seed일 뿐이다. ChangeBundle 생성 시 모든 Project·Checkout·base·dirty·PatchSet·Gate·rollback을 다시 probe하고 bind한다. handoff의 approval·success·freshness를 복사하지 않는다.

## ChangeBundleParticipant

한 participant는 정확히 한 Project·Git repository·target checkout을 소유한다. 여러 repository의 PatchSet 또는 evidence를 한 participant에 넣지 않는다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `participant_id`, `revision` | 예 | bundle 안 project participant identity |
| `change_bundle_ref`, `project_id`, `required` | 예 | global bundle과 owning Project |
| `roles`, `step_ids` | 예 | provider/consumer 등과 담당 step |
| `checkout_id`, `repository_fingerprint` | 예 | local checkout과 Git common repository identity |
| `base_project_revision_ref`, `base_commit_oid` | 예 | Patch·worktree를 만든 exact committed base |
| `baseline_workspace_snapshot_ref` | 예 | staged·unstaged·untracked를 포함한 시작 관찰 |
| `dirty_manifest_ref`, `dirty_state` | 예 | `clean\|dirty_complete\|dirty_partial\|unverified` |
| `preexisting_change_set_ref` | 예 | 사용자 기존 변경; empty도 complete evidence 필요 |
| `change_plan_refs`, `patch_set_refs` | 예 | project-local M2/M4 계획 |
| `migration_plan_refs` | 예 | M8 target effect가 있으면 ref, 없으면 empty |
| `worktree_record_refs` | 예 | apply·integration worktree |
| `merge_plan_ref`, `merge_queue_ref` | 해당 시 | project-local integration plan |
| `validation_plan_refs`, `gate_decision_refs` | 예 | pre/post/merge Gate |
| `evidence_bundle_refs` | 예 | project-local evidence |
| `project_merge_result_ref` | local integrate 뒤 | actual local source revision |
| `remote_snapshot_refs`, `remote_operation_refs` | 예 | remote를 쓰지 않으면 empty |
| `recovery_plan_ref`, `compensation_refs` | 예 | reverse·roll-forward·hold 전략 |
| `state`, `pending_action` | 예 | participant 상태와 다음 effect |
| `actual_subject_binding_ref` | 적용 뒤 | current actual source/effect binding |
| `participant_fingerprint` | 예 | base·dirty·plans·policy·state input hash |

`base_commit_oid`는 Git object format과 함께 해석한다. branch 이름, default branch 또는 “최신 main”은 base revision을 대신하지 않는다. non-Git Project는 9단계 source mutation participant가 될 수 없고 `validation_only` read-only participant로만 둘 수 있다.

### participant 상태

| 상태 | 의미 | 허용되는 대표 다음 상태 |
|---|---|---|
| `preparing` | current base·dirty·plan·worktree를 확인 중 | `prepared`, `held`, `failed` |
| `prepared` | effect 없는 plan·overlap·budget·recovery가 완성됨 | `awaiting_apply`, `held` |
| `awaiting_apply` | exact PatchSet/migration/local Git 승인을 기다림 | `applying`, `held`, `cancelled` |
| `applying` | project-local effect 진행 중 | `awaiting_validation`, `partially_applied`, `outcome_unknown` |
| `partially_applied` | 일부 operation만 확인됨 | `rollback_required`, `held`, `awaiting_validation` |
| `awaiting_validation` | actual subject를 수집했고 required Check 대기 | `validating`, `rollback_required`, `held` |
| `validating` | project-local post/merge Gate 실행 중 | `merge_ready`, `local_completed`, `rollback_required` |
| `merge_ready` | validated integration unit이 queue를 기다림 | `merging`, `held` |
| `merging` | Star-owned integration worktree에 통합 중 | `local_completed`, `rollback_required`, `outcome_unknown` |
| `local_completed` | requested local integration level 충족 | `remote_pending`, `completed`, `held` |
| `remote_pending` | 별도 remote approval·snapshot·check/merge 대기 | `completed`, `held`, `outcome_unknown` |
| `rollback_required` | 자동 계속 진행이 안전하지 않고 recovery 결정 필요 | `held`, `applying`, `failed` |
| `held` | 사용자가 재개·roll-forward·rollback·abandon을 선택할 때까지 effect 중지 | precondition을 다시 확인한 새 revision |
| `outcome_unknown` | side effect 결과를 확정할 수 없음 | reconcile 뒤 새 상태; 자동 retry 금지 |
| `completed` | participant의 declared target과 Gate 충족 | terminal for revision |
| `failed`, `cancelled` | 현재 revision이 완료되지 못함 | terminal; 새 revision/attempt로 재개 |

`partially_applied`, `local_completed`, `remote_pending`을 bundle 전체 `completed`로 투영하지 않는다. local completion과 remote completion은 다른 축으로 계속 표시한다.

## bundle 집계 상태와 비원자성

bundle state는 participant·step·effect·Gate에서 결정적으로 계산한다. 사용자가 summary field를 직접 바꾸지 않는다.

| 상태 | 집계 조건 |
|---|---|
| `preparing` | 하나 이상의 required participant가 current prepare 중 |
| `prepared` | 모든 required participant가 effect 없는 prepare 완료 |
| `awaiting_apply` | 다음 dependency-ready effect가 승인 대기 |
| `applying` | 하나 이상의 effect가 열려 있고 unknown이 아님 |
| `partially_applied` | 적어도 한 participant effect가 확인됐지만 required graph가 완료되지 않음 |
| `awaiting_validation` | 적용된 required participant의 검사·Gate가 남음 |
| `validating` | project 또는 goal Gate 실행 중 |
| `rollback_required` | required participant가 rollback_required 또는 protected invariant block |
| `held` | 새 effect를 시작하지 않는 명시적 보류 |
| `outcome_unknown` | 하나 이상의 required effect 결과가 미확정 |
| `completed` | requested completion target, 모든 required step, current Gate와 evidence 충족 |
| `failed`, `cancelled` | current bundle revision terminal이지만 완료 아님 |

집계 우선순위는 `outcome_unknown > rollback_required > partially_applied > applying > validating/awaiting_validation > awaiting_apply > preparing/prepared > completed`다. `held`는 effect를 멈추는 제어 상태이며 underlying partial/rollback/unknown reason을 별도 field에 보존한다.

다음 불변식은 설정으로 완화할 수 없다.

1. repository마다 Git history, lock, commit, merge와 remote 결과가 독립이다.
2. 첫 participant effect 뒤에는 부분 성공이 가능하며 이를 transaction rollback으로 숨기지 않는다.
3. compensation은 원래 effect를 삭제하는 동작이 아니라 새 plan·승인·effect·evidence를 가진 후속 operation이다.
4. 한 participant rollback 성공이 다른 participant의 호환성을 자동 복구했다고 뜻하지 않는다.
5. global state는 project-local receipt·actual probe·Gate ref 없이 성공을 만들지 않는다.
6. management `CoordinatedOperation=completed`는 상태 기록 commit 성공일 뿐 Git·remote bundle 성공이 아니다.

## base revision·dirty state·stale 판정

### prepare 시 고정할 것

- ProjectId·CheckoutId·repository fingerprint와 object format
- exact base commit·ProjectRevision
- complete staged·unstaged·untracked manifest와 preexisting ChangeSet
- M1 Catalog·Index partition fingerprint와 freshness
- M2 TaskSpec·ScopeRevision·ChangePlan·dependency edge
- M4 PatchSet operation before/after hash와 reverse/discard 전략
- M3 ValidationPlan·Gate policy·Catalog·Tool·config fingerprint
- M5/M6 contract owner·consumer·compatibility window
- M8 plan·backup/restore·rollback ref
- worktree·merge target·remote snapshot precondition과 resource reservation

### stale 종류

| 종류 | 조건 | 처리 |
|---|---|---|
| `patch_stale` | PatchSet target base/before hash·mode·existence가 달라짐 | apply 금지, M2/M4 replan·새 PatchSet·pre Gate |
| `integration_stale` | integration target tip 또는 앞 queue result가 달라짐 | MergePlan/overlap을 새 base에서 다시 계산 |
| `contract_stale` | provider surface·window·consumer acceptance가 달라짐 | dependency graph와 required Check 재계획 |
| `evidence_stale` | source·plan·config·Catalog·Tool·environment binding 변화 | current Gate positive evidence에서 제외 |
| `remote_stale` | snapshot expiry, ref/PR/check head 변화 또는 partial 조회 | remote action 금지, adapter refresh |

base branch tip이 움직였다고 immutable PatchSet byte가 자동 변환되지는 않는다. old base worktree에서 PatchSet이 여전히 재현돼도 새 integration target에는 `integration_stale`일 수 있다. Star-Control은 암묵 rebase·cherry-pick·conflict resolution을 하지 않고 새 MergePlan revision과 필요한 재검사를 만든다.

승인은 bundle·participant·PatchSet·base·dirty manifest·action·target·expiry fingerprint에 결합한다. 어느 축이든 달라지면 stale이며 새 승인이 필요하다.

## worktree 계약

### 역할

9단계 worktree는 다음 role을 가진다.

- `participant_apply`: 한 BundleStep의 PatchSet·migration source 변경을 적용하고 검사
- `participant_validation`: source write가 없는 별도 검사 환경이 필요한 경우
- `project_integration`: 같은 repository의 validated integration unit을 순서대로 합침
- `conflict_resolution`: 충돌 해결용 새 M4 PatchSet을 준비하는 격리 환경

단계별 worktree는 서로 다른 `WorktreeId`를 사용한다. 같은 filesystem directory를 다른 participant나 attempt에 재할당하지 않는다.

### WorktreeRecord

| 필드 | 의미 |
|---|---|
| `worktree_id`, `project_id`, `participant_id`, `step_id` | ownership |
| `repository_fingerprint`, `base_commit_oid` | exact Git identity·base |
| `root_binding_id` | raw path가 아닌 protected locator |
| `role` | 위 worktree role |
| `branch_ref` | optional Star-owned local branch ref; credential/path 없음 |
| `creation_receipt_ref` | Git adapter actual registration evidence |
| `before_manifest_ref`, `current_manifest_ref` | source 상태 |
| `owner_token_fingerprint` | cleanup 전에 확인할 Star-Control ownership |
| `state` | `planned\|creating\|ready\|dirty\|validating\|merge_ready\|retained\|discard_ready\|discarded\|orphaned\|ownership_unknown` |
| `retention`, `evidence_hold` | 자동 정리 가능 여부 |
| `last_probe_ref` | Git registration·filesystem identity current probe |

Star-owned branch 이름 기본 형식은 `star/<bundle-id>/<participant-id>/<step-id>`의 bounded·escaped form이다. adapter는 기존 ref 충돌을 확인하고 실제 ref를 receipt에 남긴다. 표시 규칙이 identity를 대신하지 않으며 이미 존재하는 user branch를 재사용하지 않는다.

### 사용자 변경 보존

- 사용자 primary checkout은 apply·integration target이 아니다. clean이어도 cross-repo bundle은 Star-owned worktree를 기본으로 한다.
- 사용자 dirty byte를 자동 stash·copy·replay하지 않는다. task 의미가 dirty byte에 의존하면 명시적 materialization plan과 origin-preserving ChangeSet을 만들거나 block한다.
- `git reset --hard`, `git clean`, broad checkout, primary branch 강제 이동과 untracked 삭제를 rollback으로 사용하지 않는다.
- target branch가 사용자 checkout에 열려 있으면 별도 integration branch에서 결과를 만든다. user branch update는 별도 `git_merge` action과 current precondition을 요구한다.
- cleanup은 `root_binding_id`, Git registration, owner token, current manifest, evidence hold가 모두 일치할 때만 가능하다. ownership이 불명확하면 보존한다.

## 겹침과 충돌 사전 검사

`OverlapAnalysis`는 prepare, parallel dispatch 직전, queue 진입, merge 직전에 다시 계산한다.

| 축 | 비교 대상 |
|---|---|
| file | normalized ProjectPathRef, add/delete/mode/binary/submodule |
| rename | source와 destination 양쪽, case-only rename와 Windows collision |
| range | Patch operation byte/line range와 before hash |
| symbol | semantic definition/reference key, syntax-only candidate limitation |
| contract | ManagedDeclaration, API/CLI/Schema/config/error/format ID |
| generated | authoritative input, generator ID, declared output owner |
| dependency | manifest·lockfile·toolchain owner와 package relation |
| repository policy | workflow·release·validator·global config·migration manifest |

각 pair의 판정은 `disjoint|ordered_overlap|conflict_possible|conflict_confirmed|unknown`이다.

- `disjoint`: current·complete evidence에서 겹침이 없음. dependency도 없을 때만 병렬 가능하다.
- `ordered_overlap`: 같은 대상이지만 선행 결과 뒤 다시 prepare하면 순차 실행 가능하다.
- `conflict_possible`: dynamic/reference limitation 등으로 의미 충돌 가능성이 남는다. 자동 병렬 금지다.
- `conflict_confirmed`: 같은 before를 다른 after로 바꾸거나 contract intent가 모순된다. replan 필요다.
- `unknown`: scan/Index/dirty manifest가 partial·stale·unverified다. 병렬·merge 준비를 block한다.

다른 repository의 같은 상대 path 이름은 file 충돌이 아니다. 반대로 path가 달라도 같은 stable contract·generated owner·package identity를 바꾸면 cross-project semantic overlap이다.

## 병렬 실행과 resource 한도

병렬 실행은 step graph dependency가 없고 OverlapAnalysis가 `disjoint`이며 participant별 precondition·budget reservation이 current할 때만 허용한다.

`resource_budget`은 최소 다음 dimension을 가진다.

- `max_parallel_projects`
- `max_active_worktrees`
- `max_parallel_mutations_per_repository`
- `max_parallel_validations`
- `max_parallel_local_merges`
- `max_parallel_remote_writes`
- `max_processes`, `cpu_weight_limit`, `memory_limit_bytes`
- `worktree_disk_limit_bytes`, `artifact_limit_bytes`
- `wall_time_limit_ms`

effective 값은 StarConfig·Goal·Profile·ToolDescriptor·OS adapter limit의 가장 강한 상한으로 계산하고 `BudgetSnapshot`에 observed·reserved·remaining·unknown을 둔다. 단위를 측정하지 못하면 0이나 무제한으로 추측하지 않는다. 새 allocation은 `unknown|exhausted`에서 시작하지 않고 실행 중 작업을 checkpoint한 뒤 `held`로 전환한다.

한 repository의 merge queue는 항상 직렬이다. mutation worktree는 disjoint할 때 병렬일 수 있지만 각 apply와 merge는 repository-local lock, expected base와 operation receipt를 사용한다. remote write도 Project별 직렬이며 provider/consumer edge를 건너뛰지 않는다.

`max_parallel_codex`는 Codex consumer 수만 제한한다. core worktree/process/check/merge 한도를 대신하거나 늘리지 않는다. CLI-only에서는 Codex reservation이 0이어도 ChangeBundle을 끝까지 운영할 수 있어야 한다.

## project-local MergePlan v2와 merge queue

`MergePlan` v2는 정확히 한 Project·repository의 local integration을 소유한다.

| 필드 | 의미 |
|---|---|
| `merge_plan_id`, `revision`, `change_bundle_ref`, `participant_ref` | identity·scope |
| `project_id`, `repository_fingerprint` | owning repository |
| `integration_worktree_ref` | Star-owned project integration worktree |
| `target_ref`, `target_base_commit_oid` | local integration target; remote target 아님 |
| `inputs` | validated PatchSet application 또는 local commit ref |
| `strategy` | `fast_forward_only\|merge_commit\|squash\|apply_patch`; project policy가 허용한 값 |
| `order`, `dependency_refs` | queue 순서와 근거 |
| `overlap_analysis_ref` | latest overlap result |
| `conflict_policy` | deterministic block·human/Codex proposal 경계 |
| `validation_plan_ref` | merge phase 검사 |
| `rollback_plan_ref` | discard·revert·roll-forward 가능성 |
| `permission_plan_ref` | commit·merge·branch update action |
| `status` | `draft\|ready\|queued\|stale\|integrating\|conflicted\|validating\|completed\|held\|failed` |
| `plan_fingerprint` | base·input·order·policy hash |

commit 생성은 `git_commit`, integration branch update·merge commit은 `git_merge` permission을 각각 따른다. PatchSet validation이 commit 승인을 대신하지 않는다. commit을 만들지 않은 결과는 `validated_worktree` completion level로 남을 수 있지만 remote push와 10단계 immutable source revision 입력이 될 수 없다.

### MergeQueueRecord

queue는 ProjectId, repository fingerprint, integration target, current base, ordered `MergeQueueEntry`, active entry, repository lock, resource reservation과 queue fingerprint를 가진다. entry 상태는 `queued|blocked_dependency|stale|ready|integrating|conflicted|validating|completed|held|failed`다.

한 entry를 실행할 때 다음 순서를 지킨다.

1. 앞 dependency와 participant post Gate를 확인한다.
2. target ref와 current tip을 다시 읽어 `integration_stale`을 검사한다.
3. input commit/PatchSet·worktree·dirty manifest와 ownership을 확인한다.
4. overlap·contract window·resource budget을 다시 평가한다.
5. exact `git_merge` 또는 apply permission을 확인한다.
6. integration worktree에서 effect를 실행하고 actual Git/manifest receipt를 기록한다.
7. 새 ChangeSet과 `merge` ValidationPlan을 실행한다.
8. Gate가 `auto_pass`이고 evidence packaging이 complete일 때만 entry를 `completed`로 만든다.

## MergeConflictRecord

충돌은 Git marker만 저장하지 않고 양쪽 목적과 관련 contract를 함께 보여준다.

| 필드 | 의미 |
|---|---|
| `conflict_id`, `project_id`, `merge_plan_ref`, `queue_entry_refs` | conflict scope |
| `base_commit_oid`, `left_revision`, `right_revision` | three-way identity |
| `conflict_items` | path/range/rename/mode/binary/symbol/contract/generated/lockfile/policy item |
| `left_intent`, `right_intent` | TaskSpec·Stage·ChangePlan·PatchSet ref와 typed desired outcome |
| `contract_refs` | 관련 owner·baseline·compatibility window·consumer evidence |
| `raw_conflict_artifact_ref` | redaction·hash 검증된 project-local artifact |
| `resolution_class` | `mechanical_safe\|requires_replan\|human_review\|blocked` |
| `resolution_decision_ref` | 사용자 또는 optional Codex proposal의 채택 결정 |
| `resolution_patch_set_ref` | 실제 해결은 M4 새 PatchSet으로만 표현 |
| `revalidation_refs` | resolved source의 impact·post/merge Gate |
| `state` | `open\|proposed\|resolved_pending_validation\|resolved\|blocked` |

automatic resolution은 두 operation의 before/after가 독립이고 결과가 유일함을 증명할 수 있는 기계적 경우로 제한한다. lockfile, generated output, public contract, delete/rename, 의미가 다른 symbol edit와 policy file은 text marker가 단순해도 자동 해결하지 않는다.

CLI-only에서는 결정적 해결이 없으면 `HUMAN_REVIEW`다. Codex는 ConflictRecord를 읽고 새 resolution PatchSet을 제안할 수 있지만 conflict marker를 직접 저장하거나 Gate 없이 merge 완료를 만들지 않는다.

## ProjectMergeResult와 local 완료

`ProjectMergeResult`는 다음을 고정한다.

- ProjectId·repository fingerprint·MergePlan/queue entry
- integration before/after commit OID와 working tree snapshot
- actual merge strategy, commit parent set과 adapter receipt
- preexisting change 보존 비교
- actual merge ChangeSet과 scope deviation
- `merge` ValidationPlan·GateDecision·EvidenceBundle
- local branch update 여부와 별도 approval ref
- rollback/discard/revert 가능성
- `result=validated_worktree|integrated_uncommitted|local_commit|local_branch_updated|conflicted|failed|outcome_unknown`
- result fingerprint

`local_commit` 또는 `local_branch_updated`는 remote push·PR·merge를 뜻하지 않는다. local adapter receipt만으로 remote 상태를 변경하지 않는다.

## project별 검사와 전체 Goal Gate

검사는 두 층으로 나눈다.

### project-local Gate

- M4 source apply는 기존 `patch_pre_apply`·`patch_post_apply`를 그대로 사용한다.
- migration은 M8 phase Gate를 사용한다.
- project integration은 `phase=merge`, project-local MergePlan v2와 ProjectMergeResult를 subject로 사용한다.
- ValidationRun·Diagnostic·EvidenceBundle은 owning project store와 해당 project `.ai-runs`에 둔다.
- project-local `auto_pass`가 다른 project의 missing/failed Gate를 보충하지 않는다.

### ChangeBundle Gate

v4는 `change_bundle_prepare`와 `change_bundle_goal_exit` phase를 추가한다.

`change_bundle_prepare`는 첫 cross-repo effect 전에 다음을 확인한다.

- 모든 required participant의 current base·dirty·PatchSet·recovery와 pre Gate
- acyclic step graph와 current provider/consumer relation
- overlap result가 병렬/순차 policy와 일치함
- resource reservation과 local action permission
- remote action이 plan에 포함돼도 아직 별도 approval이 없으면 실행되지 않음

`change_bundle_goal_exit`는 다음을 project별 binding으로 평가한다.

- requested completion target에 필요한 ProjectMergeResult 또는 remote merged state
- project별 required Gate·EvidenceBundle completeness
- compatibility window·consumer minimum version과 cross-project invariant
- partial/rollback/outcome unknown/held participant 부재
- 열린 effect·pending required approval 부재
- release handoff target이면 immutable project commit과 artifact binding

global GateDecision은 project별 `EvidenceSubjectBinding`을 정렬해 set fingerprint를 만든다. 임의의 대표 repository revision, “모두 main” 같은 문자열 또는 global 평균 상태로 축약하지 않는다.

## evidence v4 확장

`EvidenceSubjectBinding` v4는 해당 시 다음 ref를 추가한다.

- `multi_project_goal_ref`, `change_bundle_ref`, `change_bundle_participant_ref`
- `worktree_record_ref`, `merge_plan_ref`, `merge_queue_record_ref`
- `project_merge_result_ref`, `merge_conflict_refs`
- `compatibility_window_refs`
- `remote_state_snapshot_ref`, `remote_operation_refs`
- `change_bundle_release_handoff_ref`

`SubjectBindingRecord.role`은 `bundle_prepare|participant_apply_after|project_merge_before|project_merge_after|remote_before|remote_after|bundle_goal_exit|release_handoff`를 추가한다.

EvidenceBundle v4는 global bundle summary에서 project detail을 inline하지 않는다. global bundle은 participant EvidenceBundle ref·fingerprint, overall Gate, compatibility window, remote summary와 release handoff만 가진다. 각 project bundle은 자신의 diff·log·Diagnostic·merge/remote artifact만 가진다.

전체 완료 보고에는 최소 다음이 있어야 한다.

- project별 planned/actual change와 preexisting change 보존 상태
- project별 base·after revision, PatchSet/PatchApplication/MergeResult
- project별 ValidationPlan·GateDecision·EvidenceBundle
- provider/consumer 순서와 open/closed compatibility window
- local completion level과 remote status 축
- partial·rollback·outcome unknown·held participant
- 전체 Goal Gate와 remaining risk

## local 상태와 remote 상태 분리

participant는 최소 다음 두 축을 동시에 보존한다.

| 축 | 상태 예 |
|---|---|
| local | `not_prepared\|prepared\|applied\|validated\|merge_ready\|integrated_uncommitted\|local_commit\|local_branch_updated\|failed\|outcome_unknown` |
| remote | `disabled\|not_observed\|snapshot_current\|awaiting_approval\|pushed\|pr_open\|checks_pending\|checks_failed\|merge_ready\|merged\|failed\|outcome_unknown\|stale` |

`local_commit`은 `pushed`가 아니고, `pushed`는 `pr_open`이 아니며, PR open은 check pass나 merged가 아니다. remote target이 completion target에 포함되지 않으면 local bundle은 remote axis `disabled`로 완료될 수 있다. 반대로 `completion_target=remote_merged`이면 모든 required project의 current remote merged evidence가 필요하다.

## RemoteStateSnapshot v2

RemoteStateSnapshot은 adapter observation이며 원격 정본을 대신하지 않는다.

| 필드 | 의미 |
|---|---|
| `remote_snapshot_id`, `schema_version`, `revision` | snapshot identity; v2 target |
| `project_id`, `remote_kind`, `adapter_descriptor_ref` | owning Project와 provider adapter |
| `remote_identity` | credential 없는 canonical repository identity |
| `local_subject` | ProjectRevision·commit OID·optional bundle participant ref |
| `query_scope` | refs·PR·checks·release 중 실제 조회 범위 |
| `refs` | branch/tag/commit의 provider ref와 observed object ID |
| `pull_requests` | head/base/merge commit·state·updated revision |
| `checks` | check identity·subject commit·status·conclusion |
| `releases` | tag/source/artifact identity와 provider status |
| `capabilities` | adapter가 관찰한 지원 기능; permission이 아님 |
| `captured_at`, `valid_until` | 조회 완료와 재확인 경계 |
| `completeness` | `complete\|partial\|unverified` |
| `limitations` | auth scope·pagination·provider 차이·rate limit |
| `raw_artifact_ref` | redaction한 adapter response |
| `snapshot_fingerprint` | adapter/query/result/limitation hash |

remote status는 authenticated adapter response의 typed subject로만 만든다. PR check가 다른 head commit을 가리키면 current participant pass가 아니다. adapter가 API call success를 반환해도 after snapshot이 target ref/PR/merge result를 확인하지 못하면 `outcome_unknown|unverified`다.

`capabilities.push=true` 같은 값은 실행 권한이나 user approval이 아니다. stale·partial snapshot에서 remote effect를 시작하지 않는다.

## RemoteOperationRecord와 승인 경계

`RemoteOperationRecord`는 원격 effect 한 건의 요청·실행·재확인을 기록한다.

| 필드 | 의미 |
|---|---|
| `remote_operation_id`, `project_id`, `change_bundle_ref`, `participant_ref` | scope |
| `action` | `push\|create_pr\|update_pr\|merge_pr\|close_pr\|publish` |
| `before_snapshot_ref` | current v2 snapshot precondition |
| `local_source_revision` | push/PR head가 될 exact commit OID |
| `target` | canonical remote/ref/PR/release typed target |
| `expected_remote_precondition` | remote ref OID·PR head/base·check set |
| `permission_plan_ref`, `approval_request_ref` | exact action 승인 |
| `idempotency_key` | provider 재시도 key와 local correlation |
| `request_fingerprint` | redacted canonical request hash |
| `adapter_receipt_ref` | provider response; 성공 정본 아님 |
| `after_snapshot_ref` | actual remote result 재관찰 |
| `state` | `planned\|awaiting_approval\|executing\|succeeded\|failed\|outcome_unknown\|reconciled` |
| `diagnostic_refs`, `operation_fingerprint` | 실패·audit evidence |

다음 action은 `safe_default`와 `personal_auto` 모두에서 **현재 bundle action별 명시적 `ApprovalRequest decision=approved`** 없이는 실행하지 않는다.

- remote upload·`git_push`
- PR 생성·수정·닫기
- remote merge 또는 protected ref update
- release publish·deploy

user config의 `RemoteWriteScope`는 host·repository·action이 승인 요청 후보인지 제한할 뿐 per-bundle 승인을 대체하지 않는다. 한 action의 승인을 다른 project, commit, branch, PR, merge, publish에 재사용하지 않는다. push 승인으로 PR 생성이나 merge를 실행하지 않으며, PR 생성 승인으로 publish를 실행하지 않는다.

force push, remote history rewrite, protected branch bypass, account/permission 변경은 기본 `deny`다. 별도 제품 범위와 정책이 명시되지 않는 한 9단계가 제안하지도 실행하지도 않는다.

remote write 실패·timeout 뒤 자동 재시도하지 않는다. 같은 idempotency contract로 provider 상태를 조회해 reconcile할 수 있을 때만 기존 operation을 이어가며, 확인할 수 없으면 `outcome_unknown`과 hold를 유지한다.

## 부분 성공·재개·rollback·보류

일부 repository만 성공했을 때 사용자는 다음 전략 중 하나를 선택한다.

| 전략 | 의미 |
|---|---|
| `resume_remaining` | 완료 participant를 보존하고 remaining participant를 current base에 다시 bind |
| `roll_forward` | 실패/partial participant에 새 PatchSet·migration·merge plan을 적용 |
| `compensate` | 이미 성공한 participant를 reverse/revert/compatibility restore하는 새 operation |
| `hold` | 새 effect를 시작하지 않고 worktree·evidence·remote 상태를 보존 |
| `abandon_partial` | 완료 아님을 유지한 terminal 기록과 운영 위험 인계 |

재개 전에는 모든 remaining participant뿐 아니라 이미 성공한 provider/consumer contract와 compatibility window도 다시 probe한다. dependency edge·base·remote state가 달라지면 기존 bundle을 수정하지 않고 superseding revision을 만든다.

compensation 기본 순서는 완료된 dependency edge의 역순이지만 안전성과 가능성을 보장하지 않는다. 다음 규칙을 지킨다.

- reverse PatchSet은 exact after hash·current precondition과 새 approval/Gate가 필요하다.
- local merge rollback은 primary branch reset이 아니라 owned integration branch discard, 새 revert commit 또는 roll-forward다.
- remote rollback은 force push가 아니라 provider가 지원하고 사용자가 승인한 새 revert PR/merge/release withdrawal operation이다.
- provider를 되돌리면 이미 전환한 consumer의 compatibility를 별도 평가한다.
- compensation 실패·outcome unknown은 original success를 지우지 않고 새 failure evidence를 추가한다.

`held` worktree·backup·remote evidence는 retention hold 후 자동 정리하지 않는다. 보류 해제는 current ownership·revision·budget·approval을 다시 확인한다.

## CLI-only command 계약

CLI는 Controller의 typed application service만 호출한다. DB·Git executable·remote API를 직접 호출하지 않는다. 모든 command는 stable ID를 사용하고 `--json`에서 raw absolute path를 반환하지 않는다.

### read-only

| command | 결과 |
|---|---|
| `star change-bundle plan --task <task-id> --scope <scope-revision-id>` | MultiProjectGoal·bundle draft, participant·edge·unknown |
| `star change-bundle import-handoff --handoff <handoff-id>` | M8 ref를 current participant 후보로 변환한 draft; apply 권한 없음 |
| `star change-bundle show <bundle-id>` | state·completion level·participant summary |
| `star change-bundle preflight <bundle-id>` | base·dirty·overlap·budget·permission·Gate readiness |
| `star change-bundle conflicts <bundle-id>` | 양쪽 intent·contract·required decision |
| `star change-bundle status <bundle-id>` | local/remote 축, open effect·approval·hold |
| `star change-bundle remote refresh <bundle-id> --project <project-id>` | RemoteStateSnapshot v2; network/secret policy 적용 |
| `star change-bundle release-handoff plan <bundle-id>` | project별 missing immutable revision·artifact·Gate |

### local effect

| command | 필수 precondition |
|---|---|
| `star change-bundle worktree create <bundle-id> --step <step-id>` | current prepare·budget·owned root permission |
| `star change-bundle apply <bundle-id> --participant <participant-id>` | project Patch/migration Gate와 exact approval |
| `star change-bundle validate <bundle-id> --participant <participant-id>` | actual subject binding·ready ValidationPlan |
| `star change-bundle merge enqueue <bundle-id> --project <project-id>` | validated integration unit·current MergePlan |
| `star change-bundle merge run <bundle-id> --project <project-id>` | serial queue lock·base probe·`git_merge` permission |
| `star change-bundle hold <bundle-id>` / `resume <bundle-id>` | current state·reason·rebind plan |
| `star change-bundle recovery plan <bundle-id>` | actual partial/unknown probe; effect 없음 |
| `star change-bundle recovery apply <bundle-id> --strategy <...>` | exact compensation plan·approval·Gate |

### remote effect

`star change-bundle remote prepare`는 request와 ApprovalRequest만 만든다. `remote apply`는 approved action ID·current before snapshot·exact local commit에서만 실행한다. push, PR, merge와 publish는 각각 별도 operation이다.

`--yes`, `--force`, environment variable 또는 MCP/Codex 전달값으로 required approval을 합성하지 않는다. noninteractive 실행에서 approval이 없으면 `awaiting_approval`과 machine-readable next action을 반환한다.

CLI-only dependency graph에는 Codex, App Server, 다른 AI provider와 OpenAI API client가 없어야 한다. 사람이 필요한 의미 판단은 `HUMAN_REVIEW`와 질문으로 남는다.

## Codex 선택 소비자 경계

Codex는 다음에만 선택적으로 참여한다.

- 독립 Stage worktree에서 사용자가 요청한 변경을 수행해 M4 PatchSet 후보를 생성
- ConflictRecord와 두 intent를 읽고 resolution PatchSet 후보 제안
- ReviewPack을 읽고 사람 검토를 돕기

Codex가 참여해도 다음은 core가 결정한다.

- ProjectId·CheckoutId·base·dirty·overlap·dependency order
- worktree ownership과 resource reservation
- PatchSet apply permit, merge queue와 Git/remote adapter effect
- GateDecision, bundle aggregate state와 release handoff eligibility

Codex thread·parallel count·model·reasoning effort는 RouteDecision evidence다. ChangeBundle fingerprint, ProjectRelation 또는 remote truth에 넣지 않는다. Codex 작업 실패는 participant evidence로 남지만 deterministic local CLI를 사용할 수 없게 만들지 않는다.

## event와 projection

9단계는 다음 event family를 추가한다.

- bundle: `change_bundle.created|revised|prepared|state_changed|held|resumed|completed|failed`
- participant: `change_bundle.participant_prepared|apply_started|partially_applied|validation_waiting|completed|rollback_required|outcome_unknown`
- worktree: `worktree.planned|created|probed|retained|discarded|ownership_mismatch`
- merge: `merge.enqueued|stale|started|conflicted|resolved|validated|completed|failed`
- remote: `remote.snapshot_recorded|operation_requested|approval_waiting|operation_started|operation_finished|operation_unknown|operation_reconciled`
- release handoff: `change_bundle.release_handoff_created|invalidated`

global run/management event에는 bundle ref, participant ref/fingerprint와 summary만 둔다. project source effect, worktree, merge, validation과 remote detail event는 owning project store에 둔다. global projection은 project receipt가 없거나 fingerprint가 다르면 participant를 성공으로 만들지 않는다.

crash 뒤에는 open local/remote effect를 먼저 reconcile한다. process 종료, lock expiry 또는 missing heartbeat를 failure/success로 추측하지 않는다. `outcome_unknown`이 하나라도 있으면 해당 dependency downstream effect를 시작하지 않는다.

## 저장과 evidence 위치

| 자료 | 위치 | 큰 byte |
|---|---|---|
| MultiProjectGoal·CrossRepoChangeBundle·global state | global management store | 없음; project ref만 |
| ChangeBundleParticipant·Worktree·MergeQueue·Conflict·MergeResult | owning project store | diff·conflict·Git report는 project `.ai-runs` ArtifactRef |
| RemoteStateSnapshot·RemoteOperationRecord | owning project store + global summary ref | redacted provider response ArtifactRef |
| project Validation/Gate/EvidenceBundle | owning project store | 해당 project `.ai-runs` |
| ChangeBundleReleaseHandoff | global small document + project release input refs | artifact byte는 project ArtifactRef |

worktree source byte, `.git` directory, remote credential과 project root는 management backup·EvidenceBundle에 복사하지 않는다. worktree는 source 정본도 evidence store도 아니며 삭제 뒤에도 PatchSet·receipt·Gate·artifact ref가 남아야 한다.

## 10단계 ChangeBundleReleaseHandoff

`ChangeBundleReleaseHandoff`는 release를 실행하거나 publish를 승인하지 않는다. [10단계 CI·Release·평가·최종 제품 완성](ci-release-evaluation-and-product-completion.md)이 project별 source와 artifact를 정확히 연결할 수 있는 immutable 입력이다.

| 필드 | 의미 |
|---|---|
| `release_handoff_id`, `change_bundle_ref`, `multi_project_goal_ref` | source bundle identity |
| `completion_target`, `completion_level_reached` | local/remote 완료 수준 |
| `project_inputs` | ProjectId 정렬 `ProjectReleaseInput` |
| `dependency_order` | release/build/install 순서와 contract edge |
| `compatibility_windows` | open/closed/remaining window와 consumer state |
| `overall_gate_ref` | current `change_bundle_goal_exit` Gate |
| `remaining_risks`, `limitations` | release가 다시 확인할 항목 |
| `handoff_fingerprint` | project input·Gate·artifact manifest hash |

`ProjectReleaseInput`은 최소 다음을 가진다.

- ProjectId와 provider/consumer/data/tooling role
- exact Git object format·commit OID·ProjectRevision ref
- ProjectMergeResult와 project Gate/EvidenceBundle ref
- source revision에 binding된 build/package ArtifactRef ID·size·SHA-256·provenance
- local branch state와 remote merged commit/snapshot ref가 있으면 둘 다 별도 field
- migration/rollback·compatibility state와 unresolved risk

uncommitted worktree, stale Gate, artifact subject가 다른 commit, remote merged commit 불일치와 partial participant는 release-ready project input이 아니다. 10단계는 handoff를 current source·artifact·remote 상태에 다시 bind하고 자체 release Gate·publish approval을 만들어야 한다.

## stable 오류·Diagnostic

아래 표는 상태 reducer와 CLI가 반드시 구분할 최소 code다. 전체 9단계 ErrorEnvelope와 required Diagnostic namespace는 [오류·진단 정본](errors-and-diagnostics.md#9단계-crossrepo-changebundle-대표-오류)이 소유한다.

| code | 조건 | 처리 |
|---|---|---|
| `CHANGE_BUNDLE_DEPENDENCY_CYCLE` | step graph cycle | prepare Gate·첫 effect 금지 |
| `CHANGE_BUNDLE_PARTICIPANT_STALE` | base·dirty·plan·Gate 변화 | participant reprepare |
| `CHANGE_BUNDLE_OVERLAP_UNKNOWN` | complete overlap proof 없음 | parallel/merge block |
| `CHANGE_BUNDLE_PARTIAL` | 일부 effect만 확인 | 전체 성공 금지, recovery decision |
| `CHANGE_BUNDLE_OUTCOME_UNKNOWN` | local/remote effect 불명 | 자동 retry 금지, reconcile |
| `WORKTREE_OWNERSHIP_MISMATCH` | root binding·Git registration·owner token 불일치 | cleanup·apply 금지 |
| `MERGE_TARGET_STALE` | target tip·queue predecessor 변화 | 새 MergePlan |
| `MERGE_CONFLICT_REVIEW_REQUIRED` | 기계적 유일 해 없음 | ConflictRecord·HUMAN_REVIEW |
| `REMOTE_SNAPSHOT_STALE` | freshness/precondition 불충족 | remote refresh |
| `REMOTE_APPROVAL_REQUIRED` | action별 승인 없음 | effect 시작 금지 |
| `REMOTE_RESULT_UNVERIFIED` | after snapshot이 결과를 확인 못함 | outcome_unknown/held |
| `RELEASE_HANDOFF_INCOMPLETE` | immutable revision·artifact·Gate 누락 | release-ready handoff 발행 금지 |

Git/provider native code와 message는 cause evidence로 보존하되 문자열 parsing만으로 위 상태를 만들지 않는다.

## 구현 순서

제품 구현 승인이 난 뒤 다음 순서를 지킨다.

1. 9개 top-level contract, MergePlan v2·RemoteStateSnapshot v2와 evidence v4 version/fixture
2. pure ProjectRelation·BundleStep DAG·compatibility window·bundle state reducer
3. fake Git repository/worktree port의 identity·ownership·dirty·stale conformance
4. file/range/symbol/contract/generated/lockfile OverlapAnalysis corpus
5. project-local MergePlan v2·serial MergeQueue·conflict/result state machine
6. project post/merge Gate와 global prepare/goal-exit Gate pure aggregation
7. global/project event·projection·crash reconciliation과 partial/hold/resume
8. CLI-only `plan|show|preflight|status` read-only Slice
9. approval-gated worktree create·participant apply·validate·local integration Slice
10. explicit compensation·roll-forward·revert/discard recovery corpus
11. fake Remote Git adapter의 snapshot·push/PR/check/merge observation conformance
12. action별 explicit approval이 있는 remote operation과 after-snapshot reconciliation
13. ChangeBundleReleaseHandoff 생성·invalidation과 10단계 소비 fixture
14. 선택적인 Codex Stage/Conflict proposal adapter 통합

실제 Git hosting provider adapter는 fake conformance, permission·redaction·pagination·rate/error·outcome-unknown corpus를 통과한 뒤 하나씩 연결한다. core package가 provider SDK type을 알지 않는다.

## Fixture와 Corpus

### graph·compatibility

- provider open → consumer A/B → provider close 성공
- breaking provider first 제거 차단
- reader-before-writer·schema-before-codegen 순서
- relation possible/stale/partial과 dependency cycle
- optional participant 실패와 required participant 실패 차이

### dirty·worktree·overlap

- clean/dirty staged·unstaged·untracked complete manifest
- user dirty disjoint/overlap/unknown과 silent copy 금지
- linked worktree common repository identity, case collision·reparse escape
- same file range disjoint/overlap, rename/delete, binary/submodule
- 다른 path의 같은 symbol/contract/generated owner/lockfile 충돌
- owner token·Git registration mismatch cleanup 차단

### stale·merge

- PatchSet before hash 변화와 base branch tip 변화 구분
- queue predecessor merge 뒤 ordered overlap reprepare
- fast-forward/merge commit/squash/apply patch 정책
- conflict 양쪽 TaskSpec·contract 표시와 resolution PatchSet 재검사
- project Gate pass 뒤 goal Gate fail
- 사용자 checkout·branch·untracked byte 보존

### partial·recovery

- provider 성공/consumer 실패 뒤 hold·resume
- consumer 일부 성공 뒤 provider rollback compatibility 위험
- reverse PatchSet 불가와 roll-forward
- local merge success/remote push unknown
- compensation 자체 실패·outcome unknown
- abandon partial이 completed가 아님

### remote

- snapshot complete/partial/stale, PR head commit mismatch
- adapter call success지만 after snapshot missing
- push/PR/merge/publish approval 분리
- standing RemoteWriteScope만 있고 current ApprovalRequest 없음
- provider rate limit·pagination·auth limitation
- force push/protected bypass 기본 deny

### release handoff

- project별 서로 다른 commit·artifact hash 정상 연결
- uncommitted worktree, stale Gate, 다른 revision artifact 거부
- local integrated와 remote merged source revision 구분
- compatibility window open 상태의 remaining risk

## 완료 조건

- 여러 repository 변경은 각 Project의 Git history·PatchSet·Gate·EvidenceBundle을 따로 유지한다.
- 일부 participant 성공·부분 적용·outcome unknown·rollback 필요를 전체 성공으로 표시하지 않는다.
- 사용자 checkout·dirty·untracked·branch를 자동 reset·stash·삭제·강제 이동하지 않는다.
- provider compatibility open → consumer migration → provider close 순서를 step DAG와 window로 표현한다.
- file·symbol·contract·generated owner·lockfile 겹침과 stale base를 병렬·merge 전에 차단한다.
- worktree·process·validation·merge·remote 동시성 및 disk/memory/artifact/time 한도가 budget evidence에 남는다.
- local validated/commit/branch update와 remote pushed/PR/check/merged가 서로 다른 상태다.
- remote 상태는 current adapter snapshot으로만 판정하고 upload·PR·merge·publish는 action별 명시적 승인 없이는 실행되지 않는다.
- 부분 성공 뒤 resume·roll-forward·compensate·hold·abandon을 project별 precondition과 evidence로 선택할 수 있다.
- persisted 계약과 report는 stable ProjectId·opaque binding을 사용하고 여러 project 절대 경로를 복제하지 않는다.
- CLI-only local ChangeBundle이 Codex 없이 plan·preflight·apply·validate·merge·recovery·status를 수행할 수 있다.
- Codex 병렬 실행은 선택 소비자이며 core ChangeBundle·Git·remote·Gate 계약과 분리된다.
- 10단계가 project별 immutable source revision, artifact hash, Gate와 compatibility 상태를 `ChangeBundleReleaseHandoff`에서 연결할 수 있다.
