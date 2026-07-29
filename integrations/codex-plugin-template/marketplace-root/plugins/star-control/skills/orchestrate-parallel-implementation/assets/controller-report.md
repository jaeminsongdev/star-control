# Sol Controller Report

```markdown
## 결과

- 요청 결과:
- 최종 상태: verified|blocked|partial|superseded_pending_goal_resolution

## Sol 권한

- 설계: complete|blocked
- worker별 baseline_sha..head_sha 전체 diff 직접 리뷰: <approved/total>
- combined 전체 diff 직접 리뷰: approved|changes_requested|not_run

## Terra Bundle

| Bundle | thread_id / host_id | worktree_root | baseline_sha..head_sha / fingerprint | Goal active through review | Worker complete | Sol review | Goal complete / Integration |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## 검증

- 작업자 검증:
- 최종 프로젝트 검증:
- combined revision/worktree:

## 남은 항목

- 승인 대기:
- 외부 blocker:
- 잔여 위험:
```

Sol worker별 전체 diff 직접 리뷰가 없는 Bundle은 Goal을 complete로 만들거나 `INTEGRATED`로 표시하지 않는다. combined 전체 diff 리뷰와 final validation이 모두 끝나지 않았으면 `verified`로 보고하지 않는다.
