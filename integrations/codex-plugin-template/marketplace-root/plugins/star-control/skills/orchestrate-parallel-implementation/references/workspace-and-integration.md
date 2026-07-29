# 작업공간과 통합

별도 worktree는 implicit Skill 호출이 아니라 사용자 명시 승인 뒤에만 bootstrap하며, Git 여부는 `list_projects({})`의 `isGitRepository`로 확인한다. 승인된 Git project는 actual `projectId`와 `environment:{type:"worktree"}`를 사용하고, `startingState`는 사용자 요청이 있을 때만 넣는다.

controller는 direct 결과 또는 unique `bundle_id`/expected `projectId`/worktree identity `list_threads` match가 반환한 `id`(threadId), `hostId`, `cwd`를 Context Pack/report의 `thread_id`, `host_id`, `worktree_root`로 저장한다. `clientThreadId`는 setup 표식이며 lifecycle identity가 아니다. 0건 timeout 또는 multiple match는 BLOCKED이며 재-create하지 않는다. 기존 dirty worktree를 reset, restore, clean, stash하지 않는다.

각 activation된 Bundle은 baseline SHA, head SHA, diff fingerprint와 preexisting dirty paths/owned paths를 고정한다. shared contract/ownership conflict가 발견되면 worker는 source mutation 없이 controller에 보고한다. Sol은 worker `baseline_sha..head_sha` 전체 diff와 fingerprint를 직접 검토하고, 승인된 동일 identity만 combined diff에 통합한다. 승인 없는 push, PR, publish와 별도 worktree의 조용한 shared-worktree 강등을 금지한다.
