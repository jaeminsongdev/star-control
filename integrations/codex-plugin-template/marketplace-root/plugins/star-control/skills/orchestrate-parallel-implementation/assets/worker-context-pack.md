# Terra Worker Context Pack

```yaml
bundle_id: "<stable id>"
worker_profile:
  model: "gpt-5.6-terra"
  thinking: "high"
  subagents: "forbidden"
goal_pursuit: "required"
objective: "<complete bundle outcome>"
completion_criteria:
  - "<implementation criterion>"
  - "<test criterion>"
  - "<validation criterion>"
  - "Sol Max approves this worker's complete baseline_sha..head_sha diff"
scope_in:
  - "<owned path or surface>"
scope_out:
  - "<explicit exclusion>"
depends_on:
  - "<bundle id or none>"
ownership:
  files: []
  contracts: []
  schemas: []
  databases: []
  ports: []
  build_outputs: []
thread:
  thread_id: "<confirmed Codex App threadId>"
  host_id: "<confirmed hostId>"
  client_thread_id: "<setup-only clientThreadId or none>"
  lifecycle_state: "THREAD_READY|GOAL_ACTIVE|WORKER_COMPLETE"
workspace:
  mode: "isolated"
  worktree_root: "<absolute project worktree path>"
  baseline_sha: "<dispatch-time revision>"
  head_sha: "<current revision or pending>"
  diff_fingerprint: "<baseline_sha..head_sha fingerprint or pending>"
  preexisting_dirty_paths: []
  owned_paths: []
validation:
  - command: "<exact command>"
    expected: "<expected result>"
approval_boundary:
  - "<action requiring approval or none>"
report_template: "assets/worker-report.md"
```

`create_thread` 결과에서 `thread_id`와 `host_id`를 확인하기 전에는 `client_thread_id`로 `wait_threads`, `read_thread`, `send_message_to_thread`를 호출하지 않는다. 확인 뒤 첫 동작으로 Bundle 전체 objective와 completion criteria를 token budget 없이 `create_goal`에 등록한다. 범위 안 수정·테스트·검증과 Sol 전체 diff 리뷰가 끝날 때까지 같은 Goal을 active로 유지한다. 구현·직접 검증을 끝내면 `WORKER_COMPLETE`를 보고하되 Sol 승인 전에는 Goal을 complete로 만들지 않는다. shared contract 변경이나 소유권 충돌은 직접 수정하지 말고 즉시 보고한다.
