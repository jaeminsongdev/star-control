# Changelog

## Unreleased

- Windows 전용 `star`, `star-controller`, `star-mcp` 구조와 typed foundation·IPC workspace로 재구성했다.
- 고정 12-tool MCP surface, live Tool Registry, authenticated IPC, 외부 EXE Runtime과 관리 CLI를 구현했다.
- 실제 Codex same-session evidence를 포함한 170개 MCP 검증 matrix와 Windows full CI gate를 추가했다.
- `ValidationRun`, `GateDecision`, `EvidenceBundle`, `Diagnostic` 공개 계약과 생성 JSON Schema를 추가하고 `not_run` 및 권위 있는 gate 판정 불변식을 고정했다.

## 0.1.0-scaffold - 2026-06-28

- Star-Control monorepo 초기 scaffold를 생성했다.
