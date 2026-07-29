# Sol Controller Report

```markdown
## Result
- final_state: verified|blocked|partial|superseded_pending_goal_resolution
- thread-creation authorization: explicit|not-granted|not-needed-single-agent

## Identity and review

## Terra Bundle
| Bundle | thread_id / host_id | worktree_root | baseline_sha..head_sha / fingerprint | bundle_state | review_state | goal_status | blocked_reason |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

- worker whole-diff direct review: <approved/total>
- combined whole-diff direct review: approved|changes_requested|not_run

## Validation and remaining risk
- worker / final commands and results:
- pending external approval or blocker:
```

controller만 `wait_threads`/`read_thread`로 worker를 관찰한다. Sol은 worker worktree의 exact `baseline_sha..head_sha`와 fingerprint를 직접 확인하고, 일치할 때만 같은 `threadId`에 completion을 보낸다. `awaiting_external_sol_review`는 구현 실패·거절로 분류하지 않는다.
