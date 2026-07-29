# 분해와 의존 DAG

## Task Bundle 계약

명시적으로 승인된 parallel Bundle만 별도 Codex App Terra thread/worktree를 사용한다. 각 Bundle은 다음을 고정한다.

- `bundle_id`, `objective`, `scope_in`/`scope_out`, `depends_on`, `ownership.files/contracts/schemas/databases/ports/build_outputs`, `completion_criteria`, `validation [{command, expected/result}]`
- `authorization`: 새 task/thread 또는 parallel delegation의 명시 사용자 승인과 `approval_boundary`
- `thread_identity`: `list_projects` projectId, `list_threads`의 thread_id/host_id/worktree_root, setup-only clientThreadId
- `revision_identity`: baseline_sha, head_sha, diff_fingerprint, preexisting_dirty_paths, owned_paths
- `review_identity`: reviewer, bundle_state, review_state, goal_status, blocked_reason
- `goal_pursuit`: 승인된 Terra Bundle에서만 required

## 분해와 재계획

1. 일반 구현은 새 thread 0건의 current-task single-agent Bundle이다.
2. 명시 승인이 있을 때만 public contract와 consumer의 DAG, 동일 파일/의미 계약 소유권을 먼저 정한다.
3. 공통 계약은 선행 Bundle로 만들고 consumer는 Sol worker whole-diff 승인과 Goal complete 뒤에만 READY가 된다.
4. 읽기·수정·테스트를 microtask로 나누지 않고 한 Bundle에 둔다.
5. shared contract 필요, 소유권 충돌, 사용자 범위 변경은 Terra가 source mutation 없이 `scope_in/out`, ownership, `depends_on`, conflict를 Sol controller에 보고한다. Sol이 이미 유효한 결과를 보존한다.
6. 기존 Bundle의 correction은 same threadId/Goal의 `EXISTING_GOAL_RESUMED`다. 새 Goal을 만들지 않는다. 원 목표 무효화만 `SUPERSEDED_PENDING_GOAL_RESOLUTION`로 격리한다.
