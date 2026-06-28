# Provider Capability

Router는 provider 이름이 아니라 capability 조건으로 실행자를 선택한다.

```text
needs.file_edit
needs.shell_command
needs.structured_output
needs.private_code
needs.offline
needs.cost_cap
```

Provider manifest는 `can.edit_files`, `can.run_shell`, `can.read_repo`, `can.apply_patch`, `can.return_json`, `can.work_offline` 같은 능력을 선언한다.
