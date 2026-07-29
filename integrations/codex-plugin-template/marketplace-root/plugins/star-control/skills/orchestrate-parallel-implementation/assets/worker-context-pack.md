# Terra Worker Context Pack

```yaml
bundle_id: "<stable id>"
authorization:
  new_task_or_parallel_delegation: "explicit-user-approved"
  single_agent_opt_out: false
worker_profile:
  model: "gpt-5.6-terra"
  thinking: "high"
  subagents: "forbidden"
goal_pursuit: "required"
objective: "<complete bundle outcome>"
completion_criteria:
  - "implementation, tests, and validation"
  - "Sol approves this exact baseline_sha..head_sha diff fingerprint"
thread_lifecycle:
  project_id: "<list_projects projectId>"
  requested_target: "target:{type:project, projectId, environment:{type:worktree}}"
  thread_id: "<confirmed list_threads id>"
  host_id: "<confirmed list_threads hostId>"
  client_thread_id: "<setup-only clientThreadId or none>"
  worktree_root: "<confirmed list_threads cwd>"
  state: "THREAD_CREATING|THREAD_READY|GOAL_ACTIVE|WORKER_COMPLETE"
workspace:
  baseline_sha: "<dispatch-time SHA>"
  head_sha: "<current SHA or pending>"
  diff_fingerprint: "<baseline_sha..head_sha fingerprint or pending>"
  preexisting_dirty_paths: []
  owned_paths: []
review:
  bundle_state: "GOAL_ACTIVE|WORKER_COMPLETE"
  review_state: "not_requested|pending|approved|changes_requested"
  goal_status: "active|blocked|complete"
  blocked_reason: "none|awaiting_external_sol_review"
validation: []
```

`clientThreadId`만 있으면 `THREAD_CREATING`에서 멈춘다. lifecycle 도구 호출·중복 create·Goal 시작은 하지 않으며 controller가 `list_threads`로 `threadId`/`hostId`/worktree identity를 확인할 때까지 기다린다. 확인 후 첫 동작은 token budget 없는 `create_goal`이다. Terra는 `WORKER_COMPLETE`를 한 번 보고한 뒤 Sol 승인을 polling하지 않는다. correction과 approval은 같은 `threadId`로 전달되어 기존 Goal을 재개하며 새 Goal을 만들지 않는다.
