# 스케줄링과 생명주기

## 상태 기계

```text
PLANNED -> AUTHORIZATION_REQUIRED -> CURRENT_TASK_SINGLE_AGENT
        -> THREAD_CREATING -> THREAD_READY -> GOAL_ACTIVE -> WORKER_COMPLETE -> SOL_REVIEW_PENDING
                                      ^             |                                  |              |
                                      |             -> BLOCKED(awaiting_external_sol_review)         |
                                      |                                                                v
                                      +-------- EXISTING_GOAL_RESUMED <- CORRECTION / SOL_APPROVAL -----+
SOL_REVIEW_PENDING -> GOAL_COMPLETE -> INTEGRATED
INTEGRATED(all) -> SOL_FINAL_REVIEW -> FINAL_VALIDATION -> VERIFIED
```

- implicit invocation은 `AUTHORIZATION_REQUIRED`를 해소하지 않는다. 새 task/thread 또는 parallel delegation의 사용자 명시 승인이 없으면 `CURRENT_TASK_SINGLE_AGENT`다.
- 승인된 Git project는 `list_projects({})`의 `projectId`/`isGitRepository` 확인 뒤 `target:{type:"project", projectId, environment:{type:"worktree"}}`로 만든다. `startingState`는 명시 요청 때만 허용한다.
- `create_thread`가 `clientThreadId`만 주면 `THREAD_CREATING`이다. `list_threads({limit: ...})`에서 `id`(threadId), `hostId`, `cwd`를 찾을 때까지 lifecycle call, wait/read/send, duplicate create를 금지한다.
- 확인 후 lifecycle 호출은 `wait_threads({targets:[{threadId, hostId, afterCursor}]})`, `read_thread({threadId, hostId})`, `send_message_to_thread({threadId, prompt})`다. `message`와 `project` top-level 호출은 금지한다.
- Terra 첫 동작은 token budget 없는 `create_goal`이다. worker는 `WORKER_COMPLETE` 한 번 뒤 멈추며 controller만 관찰한다.
- 3회 자동 Goal turn 뒤 blocked면 `bundle_state=WORKER_COMPLETE`, `review_state=pending`, `goal_status=blocked`, `blocked_reason=awaiting_external_sol_review`다.
- Sol correction/approval은 같은 `threadId`에 model/thinking 없이 보내 `EXISTING_GOAL_RESUMED`로 전이한다. 새 `create_goal`을 금지한다.
- Sol 전체 diff 승인 전에는 Goal complete를 허용하지 않는다. same exact `baseline_sha`, `head_sha`, `diff_fingerprint`의 Sol 승인 뒤에만 `update_goal(status="complete")`와 `INTEGRATED`를 허용한다.
