# Changelog

## Unreleased

- Windows 전용 `star`, `star-controller`, `star-mcp` 구조와 typed foundation·IPC workspace로 재구성했다.
- 고정 12-tool MCP surface, live Tool Registry, authenticated IPC, 외부 EXE Runtime과 관리 CLI를 구현했다.
- 실제 Codex same-session evidence를 포함한 170개 MCP 검증 matrix와 Windows full CI gate를 추가했다.
- `ValidationRun`, `GateDecision`, `EvidenceBundle`, `Diagnostic` 공개 계약과 생성 JSON Schema를 추가하고 `not_run` 및 권위 있는 gate 판정 불변식을 고정했다.
- Code Health 기능과 Runtime update 경로를 Codex operations Skill에 연결하고, main session 종료를 관찰하는 `SessionEnd` Hook 및 integration-only 후보 봉인 절차를 추가했다.
- required core package 1.5.0에 project register·scan·index·Finding·Diagnostic 6개 action을 additive로 공개하고, Hook review surface를 Codex CLI `/hooks`로 명시했다.
- validation-manifest shape의 `.star-control/project.toml`을 손상된 shared identity로 오인하지 않고 local ProjectId로 등록하도록 두 manifest 계약의 fail-closed 분기를 추가했다.
- required core package 1.5.1에서 `scan.run`의 CodeIndexSnapshot을 bounded reference summary로 축약해 8 MiB IPC frame을 넘지 않게 하고, malformed IPC 응답을 인증 실패가 아닌 `IPC_FRAME_INVALID`로 분류한다.
- required core package 1.5.2에서 source별 index limitation 1,377건을 count와 unique stable code 5개로 집계해 64 KiB action 결과 상한 안에서 terminal receipt를 반환한다.
- Runtime activation의 installed doctor postcheck를 15초/5초에서 45초/15초로 늘리고 timed-out child를 종료해 cold Registry 준비를 false rollback으로 오인하지 않게 했다.
- Git history Radar item ID를 정본 token 문법에 맞춰 생성해 `maintenance radar git-history`가 실제 snapshot을 publish할 수 있게 했다.

## 0.1.0-scaffold - 2026-06-28

- Star-Control monorepo 초기 scaffold를 생성했다.
