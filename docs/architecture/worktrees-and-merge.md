# 병렬 작업과 병합

## 목표

서로 겹치지 않는 단계를 독립 worktree에서 동시에 실행하고, 각 repository 안에서만 검토 가능한 방식으로 통합한다. 여러 Project의 순서·부분 성공·remote 상태는 [9단계 CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md)이 조정한다.

Star-Control은 새 버전 관리 시스템이나 cross-repository transaction을 만들지 않는다. Git의 작업 복사본과 변경 기록 기능을 사용하고, project별 base·dirty·PatchSet·검사·통합·remote observation을 연결한다. 한 repository의 commit/merge 성공은 다른 repository의 성공을 뜻하지 않는다.

## 병렬 실행 조건

다음 조건을 만족할 때만 동시에 실행한다.

- 먼저 끝나야 하는 단계가 없음
- file·rename·range·symbol·contract·generated owner·lockfile이 current·complete evidence에서 `disjoint`
- 같은 전역 설정을 동시에 바꾸지 않음
- 결과를 독립적으로 검사할 수 있음
- worktree·project·process·validation·memory·disk·artifact·시간 reservation이 한도 안에 있음
- 합치는 순서가 결과 의미를 바꾸지 않음
- 같은 Git common repository의 merge queue가 현재 비어 있거나 별도 disjoint mutation slot을 가짐

읽기 전용 조사는 같은 파일을 보더라도 병렬로 진행할 수 있다.

`possible|confirmed` dependency, `ordered_overlap`, stale/partial/unknown Index·dirty manifest와 compatibility window 선행 step이 있으면 병렬 실행하지 않는다. 앞 결과 뒤 새 base에서 prepare한다.

## 작업 복사본

수정 단계마다 별도 Git worktree와 Star-Control owned 작업 브랜치를 만든다. 9단계 cross-repo source effect에서는 선택이 아니라 기본 경계다.

    star/<run-id>/<stage-id>

9단계 기본 bounded form은 `star/<bundle-id>/<participant-id>/<step-id>`다. 실제 ref는 Git adapter가 충돌·escaping을 검사해 receipt에 남기며 이름이 worktree identity를 대신하지 않는다.

각 작업 복사본은 다음 정보를 가진다.

- 기준 변경점
- 담당 단계
- 수정 예상 범위
- Codex 작업 식별자
- 검사 상태
- 변경 기록 상태
- 병합 준비 상태
- ProjectId·CheckoutId·opaque Git common repository fingerprint
- protected root binding, Git registration과 owner token fingerprint
- before/current dirty manifest와 evidence hold

worktree role은 `participant_apply|participant_validation|project_integration|conflict_resolution`이다. 단계마다 `WorktreeId`를 새로 발급하며 directory를 다른 participant·attempt에 재할당하지 않는다. raw absolute path와 `.git` locator는 application 계약·DB·report에 저장하지 않는다.

## 4단계 single-project preview worktree

[안전한 Patch·Refactor·codemod 엔진](../contracts/safe-patch-and-codemod.md)은 P6의 병렬·merge 기능 전체보다 먼저 한정된 worktree capability를 사용할 수 있다. 목적은 사용자 dirty checkout을 자동 병합하는 것이 아니라 external codemod·formatter·generator를 **live target과 분리된 preview root**에서 실행하는 것이다.

`WorktreeDecision`은 `current_checkout|isolated_worktree|blocked` 중 하나이며 다음 input을 fingerprint로 고정한다.

- ProjectId·CheckoutId·Git common repository identity
- exact base commit·ProjectRevision·WorkspaceSnapshot
- staged·unstaged·untracked를 포함한 complete dirty manifest
- Patch target path/range·rename source/destination·generated owner
- Recipe dirty policy·transformer kind·required repository context
- preexisting overlap 결과와 permission·cleanup policy

### 격리 worktree 선택 조건

- external action이 source를 수정하는 codemod·formatter·generator면 preview는 격리 worktree가 기본이다.
- current checkout이 clean이어도 external mutator를 live target에서 실행하지 않는다.
- dirty change가 있지만 target·range·rename·generated owner와 disjoint임을 complete하게 증명하면 exact clean base worktree에서 preview할 수 있다.
- Recipe 결과가 dirty byte의 의미·parse·generator input에 의존하면 clean base worktree는 같은 subject가 아니다. dirty byte를 조용히 복제하지 않고 current exact materialized preview 또는 block을 선택한다.
- dirty overlap, partial status, missing Git object, stale base, linked worktree identity ambiguity와 path/reparse escape는 `blocked`다.

격리 worktree는 한 RecipeExecution·PatchApplication에 소유되고 다음 정보를 가진다.

- opaque WorktreeId, ProjectId와 base revision
- Star-Control owned root binding과 생성 receipt
- before/after manifest·dirty state
- tool process와 RecipeExecution ref
- retention·cleanup eligibility와 evidence hold
- `preview_only|apply_target` 역할

raw absolute path를 DB·MCP·report에 저장하지 않는다. Git adapter가 final path·common-dir·worktree registration을 handle로 확인하고 application layer에는 opaque binding만 반환한다.

### M4에서 하지 않는 것

- worktree 결과를 primary branch에 자동 merge·commit하지 않는다.
- 사용자 dirty change를 worktree로 자동 replay하거나 stash하지 않는다.
- 둘 이상의 Project worktree를 한 PatchSet·PatchApplication으로 묶지 않는다.
- remote branch·PR·push를 만들지 않는다.
- 병렬 Recipe apply와 merge queue를 기본값으로 만들지 않는다.

M4 isolated worktree apply가 post Gate를 통과해도 결과는 그 worktree에만 있다. 사용자는 PatchSet artifact를 검토할 수 있고, primary checkout 통합은 P6의 별도 merge plan·검사를 요구한다.

### M11 Rust mutator 연결

[Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)의 `rust_style_v1`은 위 M4 preview capability를 그대로 사용한다. `cargo fmt` rewrite와 `cargo clippy --fix`는 checkout이 clean이어도 live target에서 실행하지 않는다. `check`의 Clippy도 build script/proc macro를 실행할 수 있으므로 exact current byte의 isolated read subject와 source 밖 Star-Control-owned `CARGO_TARGET_DIR`을 사용한다.

- preview는 exact base/current source manifest, RustToolchainBinding, RustStylePolicySnapshot과 coverage matrix에 소유된다.
- first rustfmt 결과를 base로 coverage cell별 Clippy fix를 독립 fork에서 실행하고 byte-exact compatible hunk만 reconcile한다. 서로 다른 cell이 반대 replacement를 만들면 한쪽을 선택하지 않는다.
- `cargo clippy --fix --allow-dirty`는 staged byte 0과 직전 rustfmt step dirty manifest의 exact 일치가 확인된 preview에서만 허용한다. `--allow-staged`·`--broken-code`는 사용하지 않는다.
- process 전·후 source root complete manifest를 수집한다. `.rs` modify 이외 operation과 generated/vendor/out-of-scope write는 뒤 step에서 사라져도 side-effect violation이다.
- expected-after idempotence는 새 isolated preview와 새 owned target dir에서 전체 mutation pipeline을 replay해 operation 0을 확인한다.
- target apply는 preview worktree를 merge하거나 cargo/rustfmt/Clippy를 재실행하지 않는다. M4 SourceMutationPort가 immutable PatchSet byte만 current target precondition에 적용한다.

preview 실패·coverage partial·tool/config drift는 target source 불변 상태로 worktree를 quarantine/retention 처리한다. `personal_auto`도 worktree 소유권, exact PatchSet, pre/post Gate와 recovery 규칙을 완화하지 않는다.

### 폐기와 rollback

- primary checkout을 바꾸지 않은 preview 실패는 worktree를 `quarantined|discard_ready`로 두고 evidence를 먼저 finalize한다.
- 폐기는 Star-Control owned exact root와 Git registration이 모두 같은지 확인한 뒤 `local_delete` policy로 수행한다.
- 다른 worktree, common Git directory, user-created file과 evidence hold가 있으면 폐기하지 않는다.
- `git reset --hard`, primary checkout 삭제와 broad recursive cleanup은 rollback이 아니다.
- source apply가 isolated worktree 안에서 partial이면 actual manifest를 reconcile하고 reverse PatchSet 또는 discard 중 가능한 전략을 명시한다.

## 실행 흐름

1. ProjectId·CheckoutId·repository fingerprint와 exact base commit을 고정한다.
2. staged·unstaged·untracked를 포함한 complete dirty manifest와 사용자 preexisting ChangeSet을 저장한다.
3. PatchSet·ChangePlan·contract relation·required Gate와 rollback을 project별로 bind한다.
4. file·rename·range·symbol·contract·generated owner·lockfile overlap을 계산한다.
5. dependency DAG와 compatibility window를 적용해 ready step만 선택한다.
6. resource budget을 예약하고 각 step에 별도 participant worktree를 만든다.
7. CLI-only application command 또는 선택적인 Codex Stage가 worktree 안에서 project-local 변경을 수행한다.
8. actual ChangeSet을 수집하고 project `patch_post_apply`/M8 Gate를 실행한다.
9. current base·queue predecessor·overlap을 다시 검사한다.
10. 검사를 통과한 integration unit만 owning repository merge queue에 넣는다.
11. project integration worktree에서 queue를 직렬 실행한다.
12. 충돌이면 양쪽 intent·contract를 가진 MergeConflictRecord를 만들고 queue를 멈춘다.
13. 통합 뒤 새 ChangeSet과 `merge` Gate를 실행해 ProjectMergeResult를 만든다.
14. local target branch update가 필요하면 별도 `git_merge` approval과 current target precondition을 확인한다.
15. 모든 required project 결과를 [ChangeBundle Goal Gate](../contracts/cross-repo-change-bundle.md#project별-검사와-전체-goal-gate)에서 검사한다.

commit 생성은 `git_commit`, local integration/branch update는 `git_merge` action이다. Patch apply 승인이나 검사 통과가 commit·merge 승인을 대신하지 않는다. `validated_worktree` 결과는 local CLI completion level로 보존할 수 있지만 immutable commit이 없으므로 push·PR·10단계 release source 입력이 아니다.

## GitHub와 비슷한 검토

로컬 단계도 검토 요청처럼 다음 정보를 제공한다.

- 변경 요약
- 관련 단계와 목표
- 바뀐 파일
- 검사 결과
- 남은 위험
- 기준 변경점
- 병합 가능 여부
- 충돌 여부

원격 저장소를 사용하는 경우 같은 정보를 실제 검토 요청에 연결할 수 있다.

## 충돌 처리

- 사용자 기존 변경을 자동으로 버리지 않는다.
- 충돌 item과 left/right base·revision·TaskSpec·ChangePlan·PatchSet intent를 함께 보여준다.
- 관련 ManagedDeclaration·API/Schema/config/format contract, consumer와 compatibility window를 표시한다.
- before/after가 독립이고 결과가 유일함을 증명한 기계적 충돌만 자동 해결할 수 있다.
- lockfile, generated output, public contract, delete/rename, binary/submodule와 의미가 다른 symbol edit는 marker가 단순해도 자동 해결하지 않는다.
- 의미 판단은 CLI-only에서 `HUMAN_REVIEW`다. Codex는 선택적으로 resolution PatchSet을 제안할 수 있지만 직접 merge 완료를 쓰지 않는다.
- 해결은 current conflict subject에 대한 새 M4 PatchSet·승인·post/merge Gate로 기록한다.
- 해결 뒤 영향 계산, project 검사와 전체 Goal 검사를 다시 실행한다.
- 안전하게 판단할 수 없으면 병합을 중단한다.

ConflictRecord에는 source byte를 inline하지 않는다. redaction·hash 검증된 conflict artifact와 양쪽 intent/contract ref를 저장하고, resolution 전후 actual ChangeSet을 분리한다.

## 병합 순서

다음 기준으로 순서를 정한다.

- 기반 구조를 바꾸는 단계가 먼저
- 그 기반을 사용하는 단계가 나중
- 문서와 검사는 관련 구현과 함께
- 충돌 가능성이 큰 단계는 앞선 결과를 반영해 다시 실행
- provider의 backward-compatible 경계 개방이 consumer 전환보다 먼저
- consumer coverage와 compatibility window 조건이 provider old path 제거보다 먼저
- reader가 writer cutover보다 먼저, Schema가 codegen보다 먼저
- project-local validation이 queue 진입보다 먼저, local integration이 remote action보다 먼저

같은 provider가 compatibility open과 close에 다시 등장할 수 있으므로 ProjectId만 정렬하지 않고 BundleStep DAG를 사용한다. Project relation이 `possible|unknown`이거나 cycle이면 자동 순서를 만들지 않는다.

## 여러 프로젝트

여러 프로젝트에 걸친 목표는 `MultiProjectGoal`과 `CrossRepoChangeBundle`을 사용하고 프로젝트별 작업 복사본·PatchSet·Gate·Git history·remote state를 따로 관리한다. M4 PatchSet은 계속 한 Project·한 Checkout만 소유한다.

- global bundle에는 ProjectId별 participant ref·fingerprint·summary만 둔다.
- project store와 `.ai-runs`에는 해당 project의 worktree·diff·merge·Diagnostic·remote artifact만 둔다.
- provider compatibility open → consumer transition → provider close를 finite window로 관리한다.
- 한 project effect 뒤 다른 project가 실패하면 bundle은 `partially_applied|rollback_required|held`이며 완료가 아니다.
- resume·roll-forward·compensation은 새 base·approval·effect·evidence를 가진다.
- compensation은 역순을 제안할 수 있지만 cross-repo rollback을 보장하지 않는다.
- 전체 Goal Gate는 project별 current binding과 cross-project invariant를 사용한다.

management `CoordinatedOperation`은 global/project store 상태 commit을 복구할 뿐 Git·remote effect를 하나의 transaction으로 만들지 않는다. 이미 성공한 project effect를 숨은 rollback으로 되돌리거나 실패 project를 성공으로 채우지 않는다.

## base 변경과 stale

다음 stale 축을 분리한다.

- `patch_stale`: PatchSet base·before hash·mode·existence 변화. M2/M4 replan이 필요하다.
- `integration_stale`: target branch tip 또는 앞 queue result 변화. PatchSet이 재현돼도 MergePlan을 새 base에서 다시 만든다.
- `contract_stale`: provider surface·consumer acceptance·compatibility window 변화. dependency order와 Check를 재계획한다.
- `evidence_stale`: source·plan·config·Catalog·Tool·environment binding 변화. current Gate에 사용할 수 없다.
- `remote_stale`: ref·PR head·check subject·snapshot freshness 변화. remote refresh가 필요하다.

Star-Control은 stale PatchSet을 자동 rebase하거나 old approval을 새 base에 적용하지 않는다. rebase·cherry-pick·resolution이 필요하면 별도 operation, actual diff와 새 validation을 만든다.

## merge queue와 resource

queue는 한 Git common repository마다 하나이고 항상 직렬이다. entry는 `queued|blocked_dependency|stale|ready|integrating|conflicted|validating|completed|held|failed`를 가진다. 실행 직전 target tip, input commit/PatchSet, worktree ownership, dependency, overlap, budget과 permission을 다시 확인한다.

ChangeBundle resource budget은 project/worktree/process/validation/local merge/remote write 동시성, CPU·memory·worktree disk·artifact·wall time을 포함한다. EffectiveConfig·Goal·Tool·OS adapter limit 중 가장 강한 상한을 사용하고 `BudgetSnapshot`에 observed·reserved·unknown을 남긴다. required dimension을 측정하지 못하면 무제한으로 추측하지 않고 새 allocation을 멈춘다.

`max_parallel_codex`는 Codex task 수만 제한한다. core worktree·merge·process 한도와 독립이며 CLI-only에서 0이어도 ChangeBundle을 운영할 수 있다.

## local과 remote 상태

- local `validated_worktree|integrated_uncommitted|local_commit|local_branch_updated`
- remote `disabled|snapshot_current|awaiting_approval|pushed|pr_open|checks_pending|checks_failed|merged|stale|outcome_unknown`

위 축을 합쳐 “merged” 하나로 표시하지 않는다. remote 상태는 adapter `RemoteStateSnapshot`만으로 관찰하고 local commit·branch 이름·push response에서 추측하지 않는다. upload, PR 생성/수정, remote merge와 publish는 각각 current action ApprovalRequest가 필요하다. after snapshot이 exact commit·PR·check·merge를 확인하지 못하면 `outcome_unknown`이다.

remote operation·승인·reconcile의 exact contract는 [9단계 정본](../contracts/cross-repo-change-bundle.md#remoteoperationrecord와-승인-경계)이 소유한다.

## 정리

- 완료된 임시 작업 복사본은 보관 정책에 따라 정리한다.
- 삭제는 설정된 승인 정책을 따른다.
- 작업 기록과 핵심 증거는 작업 복사본 삭제 뒤에도 남긴다.
- cleanup 전에 protected root binding, Git registration, owner token, current manifest와 evidence hold를 모두 확인한다.
- `partially_applied|rollback_required|held|outcome_unknown` worktree는 자동 정리하지 않는다.
- primary checkout·common Git directory·user branch·user-created file과 ownership 불명 directory는 삭제하지 않는다.
