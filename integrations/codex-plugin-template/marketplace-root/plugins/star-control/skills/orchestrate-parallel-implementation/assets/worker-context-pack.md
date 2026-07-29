# Terra Worker Context Pack

초기 bootstrap에는 이 Pack을 보내지 않는다. controller가 identity를 확인한 뒤 같은 `threadId`로 `ACTIVATE_BUNDLE`과 함께 보낸다.

```yaml
bundle_id: "<stable unique id>"
bootstrap_state: "BOOTSTRAP_ONLY|THREAD_IDENTITY_CONFIRMED"
activation_state: "ACTIVATE_BUNDLE|GOAL_ACTIVE"
authorization:
  new_task_or_parallel_delegation: "explicit-user-approved"
  approval_boundary: "no push/install/delete/publish; local commit only when authorized"
  single_agent_opt_out: false
worker_profile:
  model: "gpt-5.6-terra"
  thinking: "high"
  subagents: "forbidden"
goal_pursuit: "required"
objective: "<complete bundle outcome>"
scope_in: []
scope_out: []
depends_on: []
ownership:
  files: []
  contracts: []
  schemas: []
  databases: []
  ports: []
  build_outputs: []
completion_criteria:
  - "implementation, tests, and validation"
  - "Sol approves this exact baseline_sha..head_sha diff fingerprint"
thread_lifecycle:
  project_id: "<list_projects projectId>"
  requested_target: "target:{type:project, projectId, environment:{type:worktree}}"
  thread_id: "<confirmed threadId>"
  host_id: "<confirmed hostId>"
  worktree_root: "<confirmed absolute cwd>"
revision:
  baseline_sha: "<before mutation>"
  head_sha: "<current or pending>"
  diff_fingerprint: "<baseline_sha..head_sha fingerprint or pending>"
  preexisting_dirty_paths: []
  owned_paths: []
review_identity:
  reviewer: "Sol Max"
  expected_baseline_sha: "<baseline_sha>"
  expected_head_sha: "<head_sha>"
  expected_diff_fingerprint: "<fingerprint>"
validation:
  - command: "<exact command>"
    expected/result: "<expected outcome or actual result>"
review:
  bundle_state: "GOAL_ACTIVE|WORKER_COMPLETE"
  review_state: "not_requested|pending|approved|changes_requested"
  goal_status: "active|blocked|complete"
  blocked_reason: "none|awaiting_external_sol_review"
```

`ACTIVATE_BUNDLE` 전에는 이것이 Bundle assignment가 아니며 Goal·commentary·mutation·test·commit을 하지 않는다. activation 뒤 첫 작업은 token budget 없는 `create_goal`이다. shared contract/ownership conflict가 보이면 mutation하지 말고 controller에 `scope_in/out`, ownership, conflict, `depends_on`을 보고한다. Terra는 `WORKER_COMPLETE`를 한 번 보고한 뒤 Sol 승인을 polling하지 않는다. correction과 approval은 같은 `threadId`로 전달되어 기존 Goal을 재개하며 새 Goal을 만들지 않는다.
