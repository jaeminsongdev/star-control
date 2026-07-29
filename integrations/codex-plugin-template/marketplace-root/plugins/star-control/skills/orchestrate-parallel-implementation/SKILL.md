---
name: orchestrate-parallel-implementation
description: Orchestrate explicitly approved Codex App Terra High project-worktree bundles and Sol Max review threads. Ordinary implementation remains in the current task unless the user explicitly approves new tasks or parallel delegation.
---

# Orchestrate Parallel Implementation

이 Skill은 `Sol Max 관제 + 별도 Codex App Terra High thread/worktree`를 **사용자가 새 task/thread 또는 병렬 task 위임을 명시적으로 승인한 경우에만** 사용한다. implicit invocation은 새 task/thread 생성 승인이나 parallel delegation이 아니다. 일반 구현 요청은 현재 task에서 single-agent로 수행하며, `single-agent` 명시는 언제나 우선한다.

## 권한과 Bundle 경계

- 중앙 작업 자체를 `create_goal`로 등록하지 않는다. `goal_pursuit: required`는 명시적으로 승인된 Terra Bundle에만 적용한다.
- `Sol Max`는 요구 해석, DAG, 소유권, worker별 complete diff 직접 리뷰, combined diff 직접 리뷰를 소유한다. `Terra High`는 하나의 Bundle 구현·검증·교정만 수행한다.
- 정확한 역할 분리가 필요하지만 새 task/thread 승인이 없으면 controller는 승인을 요청하고 현재 task를 조용히 분리·모델 강등·새 worktree로 바꾸지 않는다.
- 동일 파일, public contract, Schema, DB migration, port, fixture namespace, build output은 동시에 여러 Bundle에 소유시키지 않는다.

## 실제 Codex App thread 생성 계약

새 thread가 명시적으로 승인되면 먼저 `list_projects({})`로 후보의 `projectId`와 `isGitRepository`를 확인한다. Git project는 worktree를 기본으로 하고, non-Git project는 local만 사용한다. 승인된 Terra와 필요 시 Sol 모두 `prompt`를 필수로 제공한다.

```text
create_thread({
  model: "gpt-5.6-terra",
  thinking: "high",
  prompt: <complete Context Pack>,
  target: {
    type: "project",
    projectId: <list_projects projectId>,
    environment: { type: "worktree" }
  }
})
```

Sol thread도 `model: "gpt-5.6-sol"`, `thinking: "max"`, `prompt`, 동일한 `target` shape를 사용한다. `startingState`는 사용자가 특정 기존 Git state를 명시 요청한 경우에만 `environment`에 넣는다. prompt와 target 밖의 legacy alias는 실제 Schema가 아니며 사용하지 않는다.

`create_thread`가 `clientThreadId`만 반환하면 `THREAD_CREATING`으로 fail-closed한다. 그 값으로 lifecycle call, `wait_threads`, `read_thread`, `send_message_to_thread`, 중복 `create_thread`를 하지 않는다. controller는 `list_threads({limit: ...})`에서 실제 `id`(threadId), `hostId`, `cwd` worktree identity를 resolve한 뒤에만 다음 호출을 한다.

```text
wait_threads({ targets: [{ threadId, hostId, afterCursor }], timeoutMs: <bounded> })
read_thread({ threadId, hostId })
send_message_to_thread({ threadId, prompt: <correction or approval>; /* model/thinking 생략 */ })
```

API lifecycle 예시는 `threadId`, `hostId`, `afterCursor` camelCase만 쓴다. Context Pack과 report의 저장 필드는 `thread_id`, `host_id`, `worktree_root`, `baseline_sha`, `head_sha`, `diff_fingerprint`처럼 snake_case여도 된다.

## 실행 순서

1. 사용자 범위, 승인, dirty worktree, 정본, 검증 명령을 고정한다.
2. 새 task/thread 승인이 없으면 현재 task single-agent로 구현한다. 승인된 경우에만 Sol 설계와 위 Schema의 Terra thread/worktree를 만든다.
3. Terra의 첫 동작은 token budget 없는 `create_goal({ objective: <Bundle 전체 objective와 completion criteria> })`다.
4. Terra는 구현·직접 검증 뒤 정확한 `baseline_sha..head_sha` diff와 fingerprint를 한 번 `WORKER_COMPLETE`로 보고하고 멈춘다. Terra는 Sol 승인을 polling하지 않는다.
5. controller만 확인된 identity로 `wait_threads`/`read_thread`를 관찰한다. `WORKER_COMPLETE`는 `SOL_REVIEW_PENDING`으로 전이하며 Sol 리뷰가 구현 실패·거절을 뜻하지 않는다.
6. 자동 Goal turn이 외부 Sol 승인을 기다리다 3회 후 blocked가 되면 `bundle_state=WORKER_COMPLETE`, `review_state=pending`, `goal_status=blocked`, `blocked_reason=awaiting_external_sol_review`를 분리해 기록한다.
7. Sol 지적 또는 승인은 같은 `threadId`의 `send_message_to_thread`로 전달한다. 이것은 `EXISTING_GOAL_RESUMED`이며 새 thread·새 `create_goal`·model/thinking override를 만들지 않는다.
8. Sol이 같은 exact `baseline_sha`, `head_sha`, `diff_fingerprint`의 worker 전체 diff를 승인한 경우에만 같은 Terra Goal을 `update_goal(status="complete")`로 끝내고 `INTEGRATED`로 전이한다.
9. 모든 Bundle 통합 뒤 Sol은 combined 전체 diff와 interaction을 직접 검토하고 final validation 뒤에만 `VERIFIED`를 선언한다.

## 금지 사항

- 사용자 승인 없는 새 task/thread, parallel dispatch, project worktree 생성
- `clientThreadId`를 lifecycle 도구에 전달하거나 setup 중복 생성
- Sol 승인 전 `update_goal(status="complete")`, worker의 Sol review polling, 검증 전 `INTEGRATED`/`VERIFIED` 승격
- 기존 dirty 변경 reset, clean, restore, stash와 승인 없는 dependency 설치·파일 삭제·push·publish·deploy
