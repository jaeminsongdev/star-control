# Star-Control MCP 도구 계약

## 역할과 불변식

`star-mcp.exe`는 Codex와 `star-controller.exe` 사이의 고정형 STDIO MCP Gateway다. Gateway는 실제 EXE, 실제 action, TOML, Tool Registry, 권한 정책과 실행 상태를 알지 않는다. 고정된 MCP 요청을 [Windows Local IPC](local-ipc.md)로 전달하고 Controller의 결과를 MCP 결과로 바꾸는 일만 한다.

다음 불변식을 지킨다.

1. 새 EXE·action·Schema·path를 추가해도 MCP tool 목록과 `star-mcp.exe`가 바뀌지 않는다.
2. `star-mcp.exe`는 TOML을 읽거나 EXE를 실행하지 않는다.
3. Tool Registry의 유일한 live owner는 Controller다.
4. Codex가 연결된 동안 TOML 또는 EXE가 바뀌어도 MCP 재등록·재시작과 Codex 재시작을 요구하지 않는다.
5. 실제 실행 직전의 trust·권한·비용·경로·descriptor 검사는 Controller가 다시 수행한다.
6. local STDIO만 제공하며 HTTP listener를 열지 않는다.

외부 도구를 동적인 MCP tool 이름으로 공개하지 않는 이유와 전체 reload 규칙은 [무재시작 외부 Tool Registry](external-tool-registry.md)가 소유한다.

이 문서는 사람이 읽는 surface 설명이다. exact field·Schema·annotation·timeout·hash·state machine은 [MCP 구현 동결 계약](mcp-implementation-contract.md)이 소유하며 구현은 그 값을 바꿔 해석하지 않는다.

## 고정 Server instructions

instructions는 모든 설치에서 같은 짧은 사용 순서만 설명한다. release·user·project ToolPackageManifest는 server-wide instructions를 추가하거나 덮어쓸 수 없다.

핵심 의미는 다음과 같다.

> 개발 작업은 먼저 `star_tool_search`로 필요한 Star-Control action을 찾고 `star_tool_describe`로 현재 입력 계약과 실행 lane을 확인한다. 반환된 `descriptor_hash`와 `arguments`를 지정된 `star_tool_call_*` 도구에 전달한다. descriptor가 바뀌었다는 오류가 나면 다시 설명을 조회한다. 질문·승인 대기를 성공으로 간주하지 않는다.

Plugin Skill과 Hook도 같은 흐름을 사용한다. MCP나 Controller가 준비되지 않았으면 일반 개발 도구로 조용히 우회하지 않고 readiness 오류를 보여준다.

## 고정 MCP surface

### 발견·설명·상태

| MCP tool | 입력 | 결과 | annotation |
|---|---|---|---|
| `star_tool_search` | 검색어, namespace·tag·task kind·source·readiness·risk lane filter, cursor·limit | 후보 tool의 ID, 짧은 설명, risk lane, descriptor hash | read-only, closed-world |
| `star_tool_describe` | `tool_id` | 실제 input·output Schema, 예시, side effect, 비용·승인 정보, 정확한 lane, `descriptor_hash` | read-only, closed-world |
| `star_tool_registry_status` | optional package·source filter | live revision, source 상태, last-known-good, 변경·거부 진단 | read-only, closed-world |

검색 결과는 호출에 필요한 Schema 전체를 싣지 않는다. Codex는 후보를 고른 뒤 반드시 `star_tool_describe`를 호출한다. 설명 결과의 `descriptor_hash`는 “어떤 계약을 보고 arguments를 만들었는지”를 고정한다.

### 실행 risk lane

실제 action마다 MCP tool을 만들 수 없으므로, MCP 표준 annotation을 보수적으로 유지하는 여섯 개의 고정 호출 도구를 둔다.

| MCP tool | `readOnlyHint` | `destructiveHint` | `openWorldHint` | 용도 |
|---|---:|---:|---:|---|
| `star_tool_call_read_closed` | `true` | `false` | `false` | 로컬 상태를 바꾸지 않는 조회 |
| `star_tool_call_read_open` | `true` | `false` | `true` | 인터넷·원격 서비스 조회 |
| `star_tool_call_write_closed` | `false` | `false` | `false` | 로컬의 되돌릴 수 있는 변경 |
| `star_tool_call_destructive_closed` | `false` | `true` | `false` | 로컬 삭제·대량 교체 등 |
| `star_tool_call_write_open` | `false` | `false` | `true` | 원격 상태 생성·수정 |
| `star_tool_call_destructive_open` | `false` | `true` | `true` | 원격 삭제·배포·병합 등 큰 영향 |

여섯 도구의 `idempotentHint`는 실제 action이 서로 다르므로 `false`로 고정한다. 실제 idempotency와 retry 가능성은 descriptor와 Controller가 판단한다. MCP annotation은 사용자 경험을 위한 hint이며 authorization 근거로 사용하지 않는다.

`star_tool_describe`가 지정한 lane과 호출한 MCP tool이 다르면 실행 전에 `TOOL_LANE_MISMATCH`로 거부한다. 낮은 위험 lane으로 바꿔 호출해 승인을 피할 수 없다.

### 고정 control 도구

| MCP tool | 역할 | annotation |
|---|---|---|
| `star_tool_operation_get` | 장기 실행 Operation 상태·progress·결과 조회 | read-only, closed-world |
| `star_tool_operation_cancel` | 취소 가능한 Operation의 중단 요청 | destructive, open-world |
| `star_approval_resolve` | 사용자에게 이미 제시한 ApprovalRequest의 승인·거부 기록 | destructive, open-world |

승인 해소와 Operation 제어는 Registry 자체가 고장 난 경우에도 접근해야 하므로 고정 control surface에 둔다. Goal 시작·질문 답변·계획·상태·이어하기 같은 Star-Control 업무 action은 required core ToolPackageManifest에 등록하며 검색→설명→lane 호출 흐름을 그대로 사용한다.

Gateway는 MCP `tools.listChanged` capability를 광고하지 않는다. MCP 표준은 tool 목록 변경 알림을 정의하지만 이 설계의 MCP 목록은 처음부터 바뀌지 않으므로 알림에 의존할 이유가 없다.

Gateway는 resources, prompts, logging, completions와 MCP Tasks도 광고하지 않는다. progress token과 cancellation notification은 응답 전 tools/call에만 사용하며 accepted 이후에는 Operation 도구만 사용한다.

## 공통 호출 입력

여섯 risk lane은 같은 고정 input Schema를 사용한다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `tool_id` | 예 | `star_tool_describe`가 반환한 stable ID |
| `descriptor_hash` | 예 | Codex가 확인한 정확한 descriptor hash |
| `arguments` | 예 | descriptor의 input Schema를 만족하는 JSON object |
| `client_request_id` | 아니요 | 생략하면 Gateway가 만드는 MCP call 단위 correlation ID |
| `idempotency_key` | 조건부 | 재전송 가능한 mutation을 같은 효과로 묶는 key |
| `goal_id` | 조건부 | 활성 Goal에 속한 action일 때의 GoalId |
| `expected_revision` | 조건부 | 상태 mutation의 낙관적 충돌 검사 값 |
| `wait_mode` | 아니요 | `auto`, `sync`, `accepted`; 기본 `auto` |
| `requested_timeout_ms` | 아니요 | descriptor 상한 안에서 요청하는 process timeout |

Gateway는 arguments를 해석하거나 CLI argument로 바꾸지 않는다. JSON 크기·기본 MCP Schema만 검사하고 그대로 IPC에 전달한다. Controller는 demand scan 뒤 최신 descriptor를 찾아 hash·lane·Schema·permission을 다시 확인한다.

설명 이후 descriptor가 바뀌었으면 Controller는 이전 계약으로 추측 실행하지 않고 `TOOL_DESCRIPTOR_STALE`과 현재 revision을 반환한다. Codex는 다시 describe한 뒤 새 arguments를 만들어야 한다.

## 발견→설명→실행 예시

1. `star_tool_search({"query":"프로젝트에서 문자열 찾기"})`
2. 후보 `dev.ripgrep.search` 선택
3. `star_tool_describe({"tool_id":"dev.ripgrep.search"})`
4. 반환된 input Schema, `descriptor_hash`, `required_call_tool=star_tool_call_read_closed` 확인
5. 지정된 lane에 `tool_id`, hash와 typed `arguments` 전달
6. 즉시 끝나면 결과를 받고, 오래 걸리면 OperationId를 받은 뒤 상태 조회

TOML이나 같은 path의 EXE가 3번과 5번 사이에 바뀌면 5번이 stale 오류로 끝나며 이전과 새 계약이 섞이지 않는다.

## 공통 결과

모든 MCP tool은 JSON object를 `structuredContent`의 정본으로 반환하고 고정 `outputSchema`로 검사한다. 호환을 위해 `content`에는 status·summary·주요 ID만 담은 짧은 TextContent를 함께 제공하되 큰 JSON을 중복하지 않는다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `schema_id` | string | 해당 고정 MCP 결과 계약 ID |
| `schema_version` | integer | 결과 계약 version |
| `status` | enum | `ok`, `accepted`, `question_required`, `approval_required`, `blocked`, `error` |
| `summary` | string | 사용자에게 바로 보여줄 짧은 결과 |
| `data` | optional object | 고정 결과 또는 descriptor output Schema를 만족한 결과 |
| `operation_id` | optional string | 비동기 실행을 조회·취소할 ID |
| `next_actions` | optional array | 호출 가능한 다음 동작과 필요한 입력 |
| `artifact_refs` | array | 큰 결과·log·report reference |
| `diagnostic_refs` | array | 정규화된 문제 reference |
| `error` | optional ErrorEnvelope | `status=error`일 때의 기계 오류 |
| `correlation_id` | string | MCP·IPC·event·process log 연결 값 |

`question_required`와 `approval_required`는 오류도 완료도 아니므로 `isError`로 표시하지 않는다. `status=error`만 `isError=true`와 ErrorEnvelope를 함께 사용해 성공으로 오해하지 않게 한다. 큰 stdout·diff·binary와 secret은 inline 결과에 넣지 않는다.

## 승인과 신뢰 경계

- `star_tool_describe`는 실제 side effect, 외부 접속, 비용 가능성, 승인 ActionId를 보여준다.
- Codex MCP approval 설정, Star-Control PermissionPlan과 운영체제 제한 중 더 강한 제한을 적용한다.
- project manifest가 `auto`, `trusted=true`를 주장해도 승인과 trust가 되지 않는다.
- approval은 descriptor hash, arguments hash, 대상, 비용 상한과 상태 revision에 묶는다.
- 승인 뒤 이 값이 바뀌면 stale로 처리한다.
- 실제 EXE identity와 update policy 검사는 매 실행 직전에 Controller가 수행한다.

## 준비 상태와 장애 처리

1. 설치 설정에서 MCP server를 required로 등록한다.
2. Gateway가 Controller pipe에 연결하고 protocol 호환성을 확인한다.
3. 허용된 경우 Gateway가 현재 사용자 Controller를 숨김 창으로 시작하고 readiness를 기다린다.
4. `star_tool_registry_status`와 core `doctor` action은 source, TOML, Schema, EXE, trust와 last-known-good 상태를 분리해 보여준다.
5. Gateway가 종료돼도 Controller의 Goal, Operation과 실행 중 process 상태는 사라지지 않는다.
6. 재연결한 Codex는 Operation·Goal 상태를 조회하며 같은 side effect를 추측해 다시 실행하지 않는다.
7. TOML·Schema·EXE 변경은 Controller의 watcher와 demand scan이 반영하며 Gateway 연결을 끊지 않는다.

## 제공하지 않는 우회 도구

- 문자열을 그대로 `cmd /c` 또는 PowerShell에 넘기는 범용 shell
- 대상 제한 없는 범용 파일 읽기·쓰기·삭제
- manifest Schema를 건너뛰는 raw argv
- OpenAI API 직접 호출이나 다른 AI provider 선택
- 자체 scheduler, HTTP API와 browser UI

복잡하거나 대화형인 기존 CLI는 Gateway에 예외 parser를 넣지 않고 `star_json_stdio_v1` adapter EXE로 정규화한다.

## 공식 근거

- [MCP Tools 규격](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) — tool Schema, structured result, 목록 변경 capability
- [MCP Schema Reference](https://modelcontextprotocol.io/specification/2025-11-25/schema) — tool annotation wire 형식
- [MCP Cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)
- [MCP Tasks](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks)
- [Codex MCP 문서](https://learn.chatgpt.com/docs/extend/mcp) — STDIO, server instructions와 Codex 설정
