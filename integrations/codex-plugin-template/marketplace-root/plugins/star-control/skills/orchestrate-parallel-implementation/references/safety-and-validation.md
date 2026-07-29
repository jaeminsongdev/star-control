# 안전과 검증

## 승인·검증 경계

사용자 명시 승인 없이는 dependency 설치·추가·제거·uninstall, 파일 삭제·대량 이동, system setting/PATH·외부 계정 변경, push/PR/publish/deploy/외부 업로드를 하지 않는다. 일반 구현 요청의 implicit Skill invocation도 새 task/thread 승인으로 해석하지 않는다.

Terra는 Bundle 직접 test를 실행한다. Sol은 exact worker diff/fingerprint와 테스트를 직접 검토하고, combined diff 리뷰 뒤 controller가 final validation을 실행한다. stale evidence·미실행 결과를 pass로 바꾸지 않는다.

## 필수 forward scenario

스킬 또는 제품 계약을 바꿀 때 다음 정확한 1..12를 검증한다.

1. 일반 구현 요청은 새 Codex App thread 0건이며 current-task single-agent로 수행한다.
2. 명시 승인 create_thread bootstrap은 unique bundle_id, BOOTSTRAP_ONLY, prompt, target:{type:"project", projectId, environment:{type:"worktree"}}를 사용한다.
3. direct threadId/hostId도 project/worktree identity를 확인한 뒤 같은 ACTIVATE_BUNDLE protocol로 activation한다.
4. clientThreadId only는 bounded list_threads unique bundle_id + expected projectId + worktree/project identity exactly one resolve이며 timeout/복수 match는 controller BLOCKED다.
5. activation 전에는 Bundle assignment가 아니며 create_goal/commentary/source mutation/test/commit 0건이고 activation ACK/Goal active 뒤에만 진행한다.
6. same file/contract ownership은 한 Bundle로 묶고 shared contract conflict는 mutation 없이 controller에 보고한다.
7. preexisting dirty paths와 owned worktree baseline/head/fingerprint를 보존하고 reset·clean·restore하지 않는다.
8. Terra는 WORKER_COMPLETE 한 번 뒤 Sol review를 polling하지 않고 controller만 wait_threads/read_thread로 관찰한다.
9. 자동 Goal turn 3회 뒤 blocked는 bundle_state=WORKER_COMPLETE, review_state=pending, blocked_reason=awaiting_external_sol_review이며 실패·거절이 아니다.
10. blocked 뒤 correction/approval은 same threadId의 send_message_to_thread로 existing Goal을 EXISTING_GOAL_RESUMED하며 새 create_goal을 만들지 않는다.
11. exact baseline_sha/head_sha/diff_fingerprint Sol 승인 뒤에만 same Goal complete와 INTEGRATED를 허용한다.
12. 승인 없는 dependency 설치·삭제·push와 Sol combined review/final validation 전 VERIFIED 선언을 하지 않는다.

## 완료 증거

report는 bootstrap_state, activation_state, command/expected/result, scope_in/out, depends_on, ownership, approval_boundary, thread_id/host_id, worktree_root, baseline_sha/head_sha/diff_fingerprint, review identity, bundle_state/review_state/goal_status/blocked_reason을 구분한다. `awaiting_external_sol_review`는 implementation failure로 승격하지 않는다.

Sol combined review와 final validation 전에는 `VERIFIED`를 선언하지 않는다.
