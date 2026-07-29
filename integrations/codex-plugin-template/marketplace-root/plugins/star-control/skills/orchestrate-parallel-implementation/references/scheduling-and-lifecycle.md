# 스케줄링과 생명주기

## 상태 기계

```text
PLANNED -> READY -> THREAD_CREATING -> THREAD_READY -> GOAL_ACTIVE -> WORKER_COMPLETE
                                                              -> BLOCKED
                                                              -> SUPERSEDED_PENDING_GOAL_RESOLUTION
WORKER_COMPLETE -> SOL_REVIEW -> CORRECTION -> GOAL_ACTIVE
SOL_REVIEW -> GOAL_COMPLETE -> INTEGRATED
INTEGRATED(all) -> SOL_FINAL_REVIEW -> FINAL_VALIDATION -> VERIFIED
```

- `THREAD_CREATING`: Codex App이 project worktree를 설정 중이며 `clientThreadId`만 있을 수 있는 상태
- `THREAD_READY`: `thread_id`, `host_id`, absolute `worktree_root`, `baseline_sha`를 확인한 상태
- `GOAL_ACTIVE`: Terra가 Bundle 전체를 token budget 없는 `create_goal`로 등록하고 추진 중
- `WORKER_COMPLETE`: Terra가 구현·자체 검증을 끝냈지만 Goal은 Sol 리뷰를 위해 active로 유지됨
- `SOL_REVIEW`: Sol이 해당 Terra worktree의 `baseline_sha..head_sha` 전체 diff를 직접 검토 중
- `CORRECTION`: Sol 지적을 같은 Terra thread와 같은 Goal로 교정 중
- `GOAL_COMPLETE`: Sol 승인 뒤 같은 Terra가 활성 Goal을 `update_goal(status="complete")`로 종료함
- `INTEGRATED`: Sol 전체 diff 승인과 Terra Goal 완료를 모두 확인한 Bundle
- `VERIFIED`: combined 전체 diff 리뷰와 final validation까지 성공
- `SUPERSEDED_PENDING_GOAL_RESOLUTION`: 사용자 scope 변경으로 원 목표가 무효화됐지만 Goal API에 cancel/replace가 없어 변경을 격리하고 소유권 재사용을 막은 상태

## thread와 Goal Pursuit 규칙

Terra는 별도 Codex App thread와 project worktree로 dispatch한다.

```text
create_thread(model="gpt-5.6-terra", thinking="high", project=<isolated worktree>, message=<Context Pack>)
```

- `clientThreadId`는 setup 중인 client 식별자다. `thread_id`와 `host_id`가 확인되기 전에는 `wait_threads`, `read_thread`, `send_message_to_thread`의 대상으로 쓰지 않는다.
- `THREAD_READY` 뒤 Terra의 첫 동작은 `create_goal({ objective: "<Bundle objective와 모든 completion criteria>" })`다. 임의의 `token_budget`을 설정하지 않는다.
- 중간 턴 종료, 일부 파일 변경, 자체 테스트 일부 성공은 목표 완료가 아니다.
- 확인된 `host_id`와 cursor로 `wait_threads`를 bounded하게 사용하고, 상세 상태는 확인된 `thread_id`로 `read_thread`에서 읽는다.
- 교정은 `send_message_to_thread`로 동일 Terra thread에 보낸다. model/thinking override 없이 같은 활성 목표를 유지한다.
- 구현·직접 검증 기준을 충족하면 Goal을 active로 유지한 채 `WORKER_COMPLETE`를 보고한다.
- Sol 전체 diff 승인 전에는 `update_goal(status="complete")`를 호출하지 않는다.
- Sol 승인 뒤 controller가 동일 Terra thread에 완료를 지시한 경우에만 같은 Terra가 `update_goal(status="complete")`를 호출한다.
- 같은 blocker가 Goal 도구의 규정상 충분히 반복되고 더 진행할 수 없을 때만 `blocked`를 사용한다.

## 탄력적 thread 풀

- 고정 lane 수를 정의하지 않는다.
- ready 집합에서 충돌 없는 Bundle을 현재 Codex App thread capacity만큼 dispatch한다.
- `wait_threads`로 완료나 주의 필요 상태를 bounded하게 기다린다.
- 완료된 thread slot이 생기면 다음 ready Bundle을 즉시 dispatch한다.
- 단순 진행 확인 때문에 실행 중인 Terra를 중단시키지 않는다.
- 범위 안 질문과 교정은 `send_message_to_thread`로 같은 thread에 보낸다.
- thread/worktree setup 실패는 Bundle을 `READY`에 보존하고 capacity 회복 뒤 재시도한다. 정확한 Terra High profile 부재는 재시도 가능한 capacity 부족과 구분해 `BLOCKED`로 보고한다.

## Sol 리뷰 루프

1. 확인된 Terra thread의 보고와 worker worktree `baseline_sha..head_sha` 실제 전체 diff를 모은다.
2. Sol에 원 수용 기준, Bundle 계약, 전체 diff, fingerprint, 테스트 결과를 제공한다.
3. Sol이 승인하면 동일 Terra thread에 active Goal 완료를 지시한다. Goal 도구의 complete 상태를 확인한 뒤 `INTEGRATED`로 전이한다.
4. 지적이 있으면 동일 Terra thread에 교정시키고 1단계부터 반복한다.
5. 전 Bundle 통합 후 combined 전체 diff를 Sol이 다시 직접 검토한다.
6. final validation 성공 후에만 `VERIFIED`로 전이한다.
