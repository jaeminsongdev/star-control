# 스케줄링과 생명주기

## 상태 기계

```text
PLANNED -> AUTHORIZATION_REQUIRED -> CURRENT_TASK_SINGLE_AGENT
        -> BOOTSTRAP_ONLY -> THREAD_CREATING -> THREAD_IDENTITY_CONFIRMED
        -> ACTIVATE_BUNDLE -> GOAL_ACTIVE -> WORKER_COMPLETE -> SOL_REVIEW_PENDING
                                         ^                          |              |
                                         |                          v              v
                                         +---- EXISTING_GOAL_RESUMED <- CORRECTION / SOL_APPROVAL
GOAL_ACTIVE -> BLOCKED(awaiting_external_sol_review)
SOL_REVIEW_PENDING -> GOAL_COMPLETE -> INTEGRATED
INTEGRATED(all) -> SOL_FINAL_REVIEW -> FINAL_VALIDATION -> VERIFIED
```

- implicit invocation은 `AUTHORIZATION_REQUIRED`를 해소하지 않는다. 새 task/thread 또는 parallel delegation의 사용자 명시 승인이 없으면 `CURRENT_TASK_SINGLE_AGENT`다.
- 승인된 Git project는 `list_projects({})`의 `projectId`/`isGitRepository` 확인 뒤 `create_thread`의 `target:{type:"project", projectId, environment:{type:"worktree"}}`로 bootstrap한다. `startingState`는 명시 요청 때만 허용한다.
- 초기 prompt는 unique `bundle_id`와 `BOOTSTRAP_ONLY`만 담고 Bundle assignment가 아님을 밝힌다. complete Context Pack, Goal, commentary, mutation, test, commit은 activation 전 금지다.
- direct `threadId`/`hostId` 결과도 expected project/worktree identity를 확인해 `THREAD_IDENTITY_CONFIRMED`가 되어야 한다. `clientThreadId`만 주면 `THREAD_CREATING`이다. bounded `list_threads({limit: ...})` polling에서 unique bundle_id + expected projectId + expected worktree/project identity가 정확히 하나인 `id`(threadId), `hostId`, `cwd`를 찾기 전 lifecycle call, wait/read/send, duplicate create를 금지한다. 0건 timeout 또는 복수 match는 controller `BLOCKED`다.
- 두 identity 분기는 같은 confirmed `threadId`의 `send_message_to_thread({threadId, prompt: <complete Context Pack + ACTIVATE_BUNDLE>})`로 합류한다. `wait_threads({targets:[{threadId, hostId, afterCursor}]})`와 `read_thread({threadId, hostId})`가 activation ACK/Goal active를 확인한다. `message`와 `project` top-level 호출은 actual call에서 금지한다.
- Terra activation 뒤 첫 동작은 token budget 없는 `create_goal`이다. worker는 `WORKER_COMPLETE` 한 번 뒤 멈추며 controller만 관찰한다.
- 3회 자동 Goal turn 뒤 blocked면 `bundle_state=WORKER_COMPLETE`, `review_state=pending`, `goal_status=blocked`, `blocked_reason=awaiting_external_sol_review`다.
- Sol correction/approval은 같은 `threadId`에 model/thinking 없이 보내 `EXISTING_GOAL_RESUMED`로 전이한다. 새 `create_goal`을 금지한다.
- Sol 전체 diff 승인 전에는 Goal complete를 허용하지 않는다. same exact `baseline_sha`, `head_sha`, `diff_fingerprint`의 Sol 승인 뒤에만 `update_goal(status="complete")`와 `INTEGRATED`를 허용한다.
