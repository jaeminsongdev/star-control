# Terra Worker Report

```markdown
## Bundle identity
- bundle_id:
- worker_profile: gpt-5.6-terra/high
- thread_id:
- host_id:
- worktree_root: <absolute path>
- baseline_sha:
- head_sha:
- diff_fingerprint:
- goal_id:
- goal_status: active|blocked|complete

## Review state
- bundle_state: WORKER_COMPLETE
- review_state: pending|approved|changes_requested
- blocked_reason: none|awaiting_external_sol_review
- exact diff binding verified: yes|no

## Change and validation
- changed files / baseline_sha..head_sha summary:
- commands / exit codes:
- shared-contract impact:

## Handoff
- Sol worker whole-diff direct review: pending
- worker action after this report: stop; do not poll Sol
- risks / required approval:
```

`WORKER_COMPLETE`는 구현 실패나 거절이 아니다. 자동 Goal turn 3회 뒤 `goal_status: blocked`여도 `bundle_state: WORKER_COMPLETE`, `review_state: pending`, `blocked_reason: awaiting_external_sol_review`로 유지한다. Sol correction/approval은 동일 Terra thread와 기존 Goal을 재개한다.
