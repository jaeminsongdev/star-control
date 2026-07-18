# ADR-0013: 고정 Bootstrap Bridge와 Runtime Generation

## 상태

채택, 구현 진행 중 (P-0038 이후).

## 결정

Codex가 실행하는 MCP와 Hook의 진입점은 설치 루트의 `star-mcp.exe`와 `star.exe`로 고정한다. 이 두 파일은 Bootstrap Bridge이며, 변경이 잦은 Controller·CLI Runtime·core catalog·schema는 content-addressed Runtime Generation에 둔다.

고정 MCP 12개는 개발 기능의 목록이 아니라 Gateway protocol이다. 실제 개발 기능은 Controller가 소유하는 live Tool Registry action으로 검색·설명·위험 lane 호출한다. Runtime Generation 변경만으로 고정 MCP 이름이나 schema, Plugin `.mcp.json`, Hook, Codex Plugin cache를 바꾸지 않는다.

활성 generation은 `%LOCALAPPDATA%\\Star-Control\\installation\\active-runtime.v1.json`의 원자적 activation record가 선택한다. Bootstrap Bridge는 이 record를 검증해 Controller를 시작하고, Controller가 잠시 교체되는 동안에는 bounded reconnect 또는 `CONTROLLER_UPDATING`을 반환한다.

## 업데이트 경계

| 종류 | 변경 대상 | Codex/MCP 재시작 |
|---|---|---|
| `tool_hot_reload` | 외부 Tool Registry package | 없음 |
| `runtime_generation` | Controller, CLI Runtime, core handler/catalog/schema | 없음; Controller만 drain/cutover |
| `bridge_update` | 루트 `star-mcp.exe` 또는 `star.exe` | 필요 |
| `plugin_update` | Plugin manifest, MCP config, Hook, Skill contract | restart/new task 및 Hook 검토 필요 |

최초 Bridge v2 migration은 Bridge/Plugin 표면을 바꿀 수 있으므로 Codex 재시작이 필요할 수 있다. 그 뒤의 `runtime_generation` 업데이트는 동일 Codex 작업에서 수행돼야 한다.

## 단일 writer와 rollback

Controller는 운영 상태의 single writer다. Controller가 quiesced된 update lease 동안 Stable Update Supervisor만 activation record의 writer가 된다. 둘이 동시에 writer가 될 수 없다.

업데이트는 `planned → staged → candidate_verified → approval_required → accepted → draining → quiesced → activating → new_controller_started → postcheck_running → committed`를 따른다. 실패 상태는 `aborted`, `rollback_required`, `rolling_back`, `rolled_back`, `rollback_failed`, `outcome_unknown`다. `accepted`와 operation ID는 성공이 아니다.

새 generation은 postcheck 전에는 committed가 아니며, activation·Controller start·postcheck 실패 시 이전 generation으로 rollback한다. rollback 보존 기간 중 이전 generation을 삭제하지 않는다.

## 결과

- 일상 도구/Runtime 업데이트는 Codex MCP 설정과 Plugin cache를 건드리지 않는다.
- 후보 기능, schema, 권한, risk lane, rollback 가능 여부를 apply 전에 검토한다.
- updater는 자동 다운로드·scheduler·package manager를 새로 소유하지 않는다. 초기 입력은 승인된 local artifact다.
- 외부 설치, Bridge/Plugin migration, Codex restart는 별도 승인 경계를 유지한다.

상세 persisted shape와 후보 검토 field는 [Runtime update와 activation 계약](../contracts/runtime-update-and-activation.md)이 소유한다.
