# Terra Worker Context Pack

```yaml
bundle_id: "<stable id>"
worker_profile:
  model: "gpt-5.6-terra"
  reasoning_effort: "high"
  fork_turns: "none"
  subagents: "forbidden"
goal_pursuit: "required"
objective: "<complete bundle outcome>"
completion_criteria:
  - "<implementation criterion>"
  - "<test criterion>"
  - "<validation criterion>"
  - "Sol Max approves this worker's complete diff"
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
workspace:
  mode: "shared|isolated"
  root: "<absolute path>"
  baseline_revision: "<dispatch-time revision>"
  preexisting_dirty_paths: []
  owned_paths: []
validation:
  - command: "<exact command>"
    expected: "<expected result>"
approval_boundary:
  - "<action requiring approval or none>"
report_template: "assets/worker-report.md"
```

첫 동작으로 Bundle 전체 objective와 completion criteria를 토큰 예산 없이 `create_goal`에 등록한다. 범위 안 수정·테스트·검증과 Sol 전체 diff 리뷰가 끝날 때까지 같은 목표를 active로 유지한다. 구현·직접 검증을 끝내면 `WORKER_COMPLETE`를 보고하되 Sol 승인 전에는 Goal을 complete로 만들지 않는다. shared contract 변경이나 소유권 충돌은 직접 수정하지 말고 즉시 보고한다.
