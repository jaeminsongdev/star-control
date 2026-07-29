# 작업공간과 통합

## Codex App project worktree

각 Terra Bundle은 별도 Codex App thread와 isolated project worktree를 사용한다. dispatch 결과에서 확인한 absolute `worktree_root`, `baseline_sha`, pre-existing dirty paths, owned paths를 Context Pack에 기록한다. setup 중의 `clientThreadId`는 상태 표식일 뿐 worktree identity나 lifecycle 대상이 아니며, 확인된 `thread_id`와 `host_id`가 없는 Bundle은 `THREAD_READY`가 아니다.

각 Terra는 자신의 소유 범위만 수정한다. 다른 Bundle 변경을 reset, restore, clean, stash하지 않는다. baseline은 dispatch 시점의 정확한 SHA로 고정하고, `head_sha`와 `baseline_sha..head_sha` diff fingerprint를 `WORKER_COMPLETE`와 Sol 리뷰 때 다시 산출한다.

## 공유 경계와 통합

- 파일뿐 아니라 public API, Schema, DB migration sequence, port, fixture namespace, build output도 단일 소유자로 지정한다.
- shared contract 변경은 선행 Bundle로 만들고 consumer는 그 결과에 의존시킨다.
- Terra는 다른 Bundle의 shared contract를 직접 고치지 않고 필요성을 보고한다.
- Sol은 각 worker worktree에서 `baseline_sha..head_sha` 전체 diff를 직접 검토한다. 승인 전에는 commit 후보나 통합 완료로 간주하지 않는다.
- 통합 뒤 실제 combined diff, Git 상태, 생성 파일을 다시 확인하고 Sol이 combined 전체 diff를 직접 검토한다.
- commit에는 요청 범위 파일만 stage한다. push, PR, publish는 사용자 승인 없이는 실행하지 않는다.

## 실패와 승인 경계

별도 격리 worktree setup이 사용자 승인이나 플랫폼 준비 때문에 불가능하면 공유 worktree로 조용히 강등하지 않는다. 정확한 경계와 blocker를 `BLOCKED`로 보고한다. 기존 linked worktree를 폐기 가능한 cache로 보지 않으며, 생성·삭제·이동은 저장소 지침의 승인 경계를 따른다.
