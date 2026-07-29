# Terra Worker Report

```markdown
## Bundle identity
- bundle_id:
- bootstrap_state: BOOTSTRAP_ONLY|THREAD_IDENTITY_CONFIRMED
- activation_state: ACTIVATE_BUNDLE|GOAL_ACTIVE
- worker_profile: gpt-5.6-terra/high
- thread_id:
- host_id:
- worktree_root: <absolute path>
- baseline_sha:
- head_sha:
- diff_fingerprint:
- goal_id:
- goal_status: active|blocked|complete

## Context and ownership
- scope_in / scope_out / depends_on:
- ownership.files/contracts/schemas/databases/ports/build_outputs:
- approval_boundary:
- preexisting_dirty_paths / owned_paths:
- review identity:

## Review state
- bundle_state: WORKER_COMPLETE
- review_state: pending|approved|changes_requested
- blocked_reason: none|awaiting_external_sol_review
- exact diff binding verified: yes|no

## Change and validation
- changed files / baseline_sha..head_sha summary:
- validation [{command, expected/result}] / exit codes:
- shared-contract impact:

## Handoff
- Sol worker whole-diff direct review: pending
- worker action after this report: stop; do not poll Sol
- risks / required approval:
```

`WORKER_COMPLETE`는 구현 실패나 거절이 아니다. 자동 Goal turn 3회 뒤 `goal_status: blocked`여도 `bundle_state: WORKER_COMPLETE`, `review_state: pending`, `blocked_reason: awaiting_external_sol_review`로 유지한다. Sol correction/approval은 동일 Terra thread와 기존 Goal을 재개한다.
