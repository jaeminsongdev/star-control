# MCP 구현 동결 계약

## 상태와 범위

이 문서는 MCP 관련 P1 구현자가 추가 설계 판단 없이 type, handler, IPC 변환과 contract test를 작성하기 위한 정본이다. 상태는 **확정**, 계약 version은 `mcp_contract_version = 1`이다.

포함 범위:

- `star-mcp.exe` STDIO MCP server
- 고정 MCP tool 12개
- Controller의 live Tool Registry 조회·호출용 IPC
- descriptor·arguments·snapshot hash
- search·describe·invoke·Operation·approval 상태 전이
- required core Tool package의 MCP 노출

포함하지 않는 범위:

- Goal·Planner·Validation·Git 기능의 내부 구현
- Codex App Server
- HTTP MCP transport
- MCP resources·prompts·sampling·elicitation·tasks
- 외부 EXE의 Windows process 구현 세부사항. 해당 정본은 [Windows Tool Runtime](../architecture/windows-tool-runtime.md)이다.

`MUST`, `MUST NOT`, `SHOULD`는 각각 필수, 금지, 특별한 이유가 없으면 준수를 뜻한다.

## 구현 기술 동결

| 항목 | 결정 |
|---|---|
| 언어·runtime | Rust 2024 edition, Tokio current 1.x, `Cargo.lock` exact pin |
| MCP SDK | 공식 Rust SDK `rmcp = 2.2.0`, 필요한 server·stdio·schemars feature만 활성화 |
| MCP 기준 규격 | `2025-11-25`, 최소 호환 `2025-06-18` |
| Schema | JSON Schema Draft 2020-12, `schemars` 1.x 생성 후 golden Schema 비교 |
| JSON | UTF-8, `serde_json`, duplicate key·invalid Unicode 거부 |
| canonical JSON | RFC 8785 JCS, `serde_json_canonicalizer = 0.3.2` |
| hash | SHA-256, `sha256:` + lowercase hex 64자 |
| ID | ULID 기반 prefix ID, system clock 역행에도 process 안에서 단조 정렬 보장 |

SDK가 기준 MCP protocol을 지원하지 않으면 custom protocol을 덧대지 않고 dependency update와 conformance 검토를 먼저 한다. release build는 `rmcp` protocol fixture가 `2025-11-25` initialize를 통과하지 못하면 실패한다.

## MCP transport와 lifecycle

### STDIO 불변식

1. Codex가 `star-mcp.exe`를 child process로 시작한다.
2. stdin·stdout은 newline-delimited UTF-8 JSON-RPC 2.0 MCP message 전용이다.
3. stdout에는 JSON-RPC message 외 byte를 한 글자도 쓰지 않는다.
4. log는 stderr에 UTF-8 JSONL로만 쓴다.
5. inbound 한 message 상한은 8 MiB, outbound inline 상한은 4 MiB다.
6. raw newline을 포함하는 JSON은 escape되어 한 physical line에 있어야 한다.
7. stdin EOF를 받으면 새 IPC·tool call을 받지 않고 진행 중 변환을 취소한 뒤 5초 안에 종료한다. Controller Operation은 자동 취소하지 않는다.

stderr log 한 line은 최대 64 KiB의 `{timestamp,level,component,event,correlation_id,message,context}` JSON object다. level은 `error,warning,info,debug,trace`, component는 `star-mcp` 고정이다. context는 redacted scalar map이며 raw arguments·secret·absolute path·Controller frame 전문을 넣지 않는다.

### initialize

지원 version의 client 요청에는 같은 version을 반환한다. 그 외 version이면 `2025-11-25`를 제안하고 client가 지원하지 않으면 연결을 끝낸다.

lifecycle은 `new -> initialize_responded -> ready -> closing`이다. initialize는 connection당 한 번만 받고 `notifications/initialized` 뒤에만 ping 외 application request를 처리한다. initialize 전·응답 후 initialized 전·두 번째 initialize의 tools/list·tools/call은 protocol invalid request다. client notification은 response를 만들지 않는다.

ServerInfo 고정값:

| 필드 | 값 |
|---|---|
| `name` | `star-control` |
| `title` | `Star-Control` |
| `version` | 현재 제품 SemVer |
| `description` | `Fixed MCP gateway for the Star-Control live tool registry.` |

server capabilities는 `tools`만 광고한다. `tools.listChanged`는 생략한다. `prompts`, `resources`, `logging`, `completions`, `tasks`, `experimental` capability를 광고하지 않는다. base protocol의 ping과 cancellation notification은 처리한다.

protocol별 initialize 차이:

| 필드 | `2025-11-25` | `2025-06-18` |
|---|---|---|
| `serverInfo.name,title,version` | 포함 | 포함 |
| `serverInfo.description` | 포함 | 해당 version의 Implementation에 없으므로 생략 |
| `serverInfo.icons,websiteUrl` | 생략 | 해당 없음 |
| `capabilities.tools` | 빈 object, `listChanged` 생략 | 동일 |
| `instructions` | 동일 고정 문자열 | 동일 고정 문자열 |

2025-11-25 client가 capability 협상 없이 `tools/call.params.task`를 보내면 `-32602`로 거부한다. tools/list의 `Tool.execution`은 모든 version에서 생략한다.

server instructions 정본:

> 개발 작업과 외부 도구 사용은 먼저 `star_tool_search`로 action을 찾고 `star_tool_describe`로 현재 Schema, 위험 lane과 `descriptor_hash`를 확인한다. 반환된 `required_call_tool`에 `tool_id`, hash와 `arguments`를 전달한다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다. `approval_required`와 `question_required`를 완료로 간주하지 않는다. 장기 실행은 Operation ID로 조회·취소한다.

이 문자열은 Gateway release 상수다. ToolPackageManifest와 project 설명을 결합하지 않는다. 처음 512자 안에 전체 기본 흐름이 들어가야 한다.

## 공통 lexical 제약

| 이름 | 제약 |
|---|---|
| `package_id`, `tool_id`, ActionId | `^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){1,7}$`, 3~128자 |
| descriptor·snapshot·scope·arguments hash | `^sha256:[0-9a-f]{64}$` |
| `request_id` | `req_` + uppercase Crockford ULID |
| `operation_id` | `opn_` + uppercase Crockford ULID |
| `approval_id` | `apr_` + uppercase Crockford ULID |
| tag·task kind | `^[a-z][a-z0-9_-]{0,31}$` |
| 일반 사용자 문자열 | Unicode scalar value, NUL 금지 |
| timestamp | UTC RFC 3339, millisecond precision, `Z` suffix |
| revision | JSON safe integer `0..9007199254740991` |

MCP tool input object는 `additionalProperties=false`다. 단, `arguments`와 result의 `data`는 참조하는 action Schema가 소유한다.

generated Schema는 `$schema = "https://json-schema.org/draft/2020-12/schema"`, `$id = "urn:star-control:schema:<schema-id>:v<schema-version>"`를 사용한다. fixed input ID는 `star.mcp.<mcp-tool-name>.input`, result ID는 `star.mcp.<mcp-tool-name>.result`이고 모두 v1이다. tools/list에는 remote `$ref` 없이 fully resolved object를 넣는다.

## 고정 MCP tool 목록

tools/list는 다음 12개와 정확히 일치해야 한다. 순서는 아래 표 순서로 고정한다.

tools/list request의 cursor가 없으면 12개 전부와 `nextCursor` 없음으로 응답한다. 비어 있지 않은 cursor는 고정 목록에서 의미가 없으므로 `-32602`다.

| 순서 | MCP tool | `readOnlyHint` | `destructiveHint` | `idempotentHint` | `openWorldHint` |
|---:|---|---:|---:|---:|---:|
| 1 | `star_tool_search` | true | false | true | false |
| 2 | `star_tool_describe` | true | false | true | false |
| 3 | `star_tool_registry_status` | true | false | true | false |
| 4 | `star_tool_call_read_closed` | true | false | false | false |
| 5 | `star_tool_call_read_open` | true | false | false | true |
| 6 | `star_tool_call_write_closed` | false | false | false | false |
| 7 | `star_tool_call_destructive_closed` | false | true | false | false |
| 8 | `star_tool_call_write_open` | false | false | false | true |
| 9 | `star_tool_call_destructive_open` | false | true | false | true |
| 10 | `star_tool_operation_get` | true | false | true | false |
| 11 | `star_tool_operation_cancel` | false | true | true | true |
| 12 | `star_approval_resolve` | false | true | true | true |

annotation은 hint이며 Controller authorization을 대신하지 않는다. 실제 action의 risk lane보다 다른 호출 tool을 사용하면 process 시작 전에 거부한다.

title·description 상수:

| MCP tool | title | description |
|---|---|---|
| `star_tool_search` | `Search Star-Control Tools` | `Search the current Star-Control live registry for an action. Call describe before invoking a result.` |
| `star_tool_describe` | `Describe a Star-Control Tool` | `Return the current Schema, risk lane, executable readiness, and descriptor hash for one action.` |
| `star_tool_registry_status` | `Inspect the Tool Registry` | `Inspect live registry revisions, packages, watchers, last-known-good state, and diagnostics.` |
| `star_tool_call_read_closed` | `Run a Local Read Action` | `Invoke the described local read-only action. The descriptor must require this exact lane.` |
| `star_tool_call_read_open` | `Run an External Read Action` | `Invoke the described read-only action that may access external systems.` |
| `star_tool_call_write_closed` | `Run a Local Write Action` | `Invoke the described non-destructive local mutation.` |
| `star_tool_call_destructive_closed` | `Run a Destructive Local Action` | `Invoke the described destructive local action after policy checks.` |
| `star_tool_call_write_open` | `Run an External Write Action` | `Invoke the described non-destructive action that changes or uses an external system.` |
| `star_tool_call_destructive_open` | `Run a Destructive External Action` | `Invoke the described destructive external action after policy checks.` |
| `star_tool_operation_get` | `Get an Operation` | `Read durable status, progress, and result for a Star-Control operation.` |
| `star_tool_operation_cancel` | `Cancel an Operation` | `Request cancellation of a durable operation and return its current state.` |
| `star_approval_resolve` | `Resolve an Approval` | `Record the user's approve or deny decision for the exact approval scope.` |

Tool root `title`은 위 값을 사용하고 `annotations.title`, Tool `_meta`는 생략한다. inputSchema와 outputSchema는 모든 version에서 제공한다.

## 고정 input·result 계약

### `star_tool_search`

입력:

| 필드 | 형식 | 기본값·제약 |
|---|---|---|
| `query` | string | 필수, trim 뒤 1~256자 |
| `namespaces` | string array | `[]`, 최대 16; ToolId dot-segment prefix exact match |
| `tags` | string array | `[]`, 최대 32; 모두 일치 |
| `task_kinds` | string array | `[]`, 최대 16; 하나 이상 일치 |
| `sources` | enum array | 기본 `release,user,project`, 최대 3 |
| `readiness` | enum array | 기본 `ready`; `ready,unavailable,untrusted,incompatible,degraded` |
| `risk_lanes` | enum array | 기본 여섯 lane 전체, 최대 6 |
| `limit` | integer | `10`, 1~50 |
| `cursor` | string 또는 null | null, 최대 1024자 |

결과 data:

| 필드 | 형식 | 규칙 |
|---|---|---|
| `registry_revision` | safe integer | 조회한 active revision |
| `snapshot_hash` | SHA-256 | 조회한 snapshot |
| `items` | array | 최대 `limit` |
| `items[].tool_id` | ToolId | stable action ID |
| `items[].display_name` | string | 1~80자 |
| `items[].summary` | string | 1~240자 |
| `items[].source` | enum | `release`, `user`, `project` |
| `items[].readiness` | enum | 요청 filter의 값 |
| `items[].risk_lane` | enum | 여섯 lane 중 하나 |
| `items[].descriptor_hash` | SHA-256 | describe·invoke에 쓸 hash |
| `items[].matched_fields` | sorted enum array | `tool_id,alias,tag,task_kind,summary,description` |
| `next_cursor` | string 또는 null | 다음 page가 없으면 null |

cursor는 base64url-no-padding JCS object `{snapshot_hash,query_hash,last_score,last_tool_id}`다. `query_hash`는 cursor를 제외한 normalized query·filter·limit object의 JCS SHA-256이다. search의 `snapshot_hash`는 active snapshot과 현재 검색 가능한 candidate descriptor·readiness를 함께 고정하는 discovery hash다. 따라서 candidate-only 변경은 `registry_revision` 대신 discovery hash만 바꿀 수 있으며, 현재 discovery hash 또는 query hash가 다르면 `TOOL_SEARCH_CURSOR_STALE`이다. decoded cursor는 duplicate key·unknown key·비정규 JCS byte를 거부한다.

### `star_tool_describe`

입력은 `tool_id` 하나다. 결과 data는 다음을 모두 포함한다.

- `registry_revision`, `snapshot_hash`, `descriptor_hash`
- `required_call_tool`
- source·trust·readiness
- display name, summary, description, alias, tag, task kind, 사용·비사용 조건
- 완전한 input·output JSON Schema
- permission ActionId, paid 상태, risk lane, isolation, idempotency, concurrency
- backend kind, protocol, executable identity의 redacted 정보
- timeout·output limit·artifact·progress·cancel 동작
- `valid_examples`, `invalid_examples` array. 각각 최대 3개이며 없으면 `[]`

untrusted·unavailable action도 describe할 수 있지만 invoke는 할 수 없다. secret value와 사용자 absolute path 원문은 반환하지 않는다.

### `star_tool_registry_status`

입력:

| 필드 | 형식 | 기본값 |
|---|---|---|
| `package_id` | string 또는 null | null |
| `sources` | enum array | 전체 |
| `include_diagnostics` | boolean | true |
| `limit` | integer | 50, 최대 200 |
| `cursor` | string 또는 null | null |

결과 data는 active·candidate·last-known-good package, watcher 상태, 마지막 demand scan, `registry_revision`, `diagnostic_revision`, source별 오류와 next cursor를 반환한다.

package item은 `package_id`, `package_version`, `source`, `active_state`, `candidate_state`, `active_manifest_hash`, `candidate_manifest_hash`, `trust_state`, `last_probe_at`, `diagnostic_refs`를 가진다. hash·시각이 없으면 null이고 array는 항상 존재한다. status cursor는 base64url-no-padding JCS `{registry_revision,diagnostic_revision,filter_hash,last_package_id}`이며 두 revision 중 하나나 filter가 바뀌면 `TOOL_REGISTRY_CURSOR_STALE`로 첫 page부터 다시 조회한다.

### 여섯 risk lane call

공통 입력:

| 필드 | 형식 | 기본값·제약 |
|---|---|---|
| `tool_id` | ToolId | 필수 |
| `descriptor_hash` | DescriptorHash | 필수 |
| `arguments` | object | 필수, action input Schema, canonical byte 최대 4 MiB |
| `client_request_id` | RequestId 또는 null | null이면 Gateway가 생성 |
| `idempotency_key` | string 또는 null | null, 1~128자 |
| `goal_id` | GoalId 또는 null | core action이 요구할 때 필수 |
| `expected_revision` | safe integer 또는 null | mutation이 요구할 때 필수 |
| `wait_mode` | enum | `auto`, `sync`, `accepted` |
| `requested_timeout_ms` | integer 또는 null | 100~descriptor 상한 |

`auto`는 descriptor의 `expected_duration_ms <= 30000`이고 detachable이 아니면 최대 30초 기다리고, 그 외에는 Operation을 만든다. `sync`도 30초를 넘기지 않으며 계속 실행해야 하면 `accepted`로 전환한다. `accepted`는 dispatch 뒤 5초 안에 OperationId를 반환한다. 관측 duration이 선언과 달라도 현재 call의 mode를 중간에 뒤집는 근거는 30초 sync budget뿐이며 manifest 값을 자동 수정하지 않는다.

`requested_timeout_ms=null`은 ToolDescriptor와 EffectiveConfig의 더 짧은 process timeout을 사용한다. `client_request_id=null`이면 Gateway가 새 RequestId를 만들어 IPC와 결과의 `correlation_id`에 사용한다. action output Schema의 root는 object여야 하며 동기 성공 data는 `{tool_id,descriptor_hash,registry_revision,arguments_hash,output_provenance,result}`다. `output_provenance`는 package·source·executable identity reference와 `external_untrusted_content` boolean을 가진다. process backend output은 항상 true, Controller core result는 false다. accepted data는 같은 식별자와 `operation` 요약을, approval 대기는 같은 식별자와 `approval_request`를 포함한다.

검사 순서:

1. demand scan
2. ToolId·readiness·trust
3. descriptor hash
4. risk lane
5. input Schema
6. arguments hash·idempotency
7. approval·비용·scope
8. concurrency lock
9. backend dispatch

이 순서를 바꾸지 않는다. 2~7의 실패는 process·core command side effect 전에 끝나야 한다.

### `star_tool_operation_get`

입력:

| 필드 | 형식 | 기본값·제약 |
|---|---|---|
| `operation_id` | OperationId | 필수 |
| `after_sequence` | safe integer | 0 |
| `wait_ms` | integer | 0, 최대 30000 |

결과는 Operation 상태, sequence 이후 progress, started·updated timestamp, redacted current phase, final result 또는 ErrorEnvelope를 반환한다.

data는 `{operation,progress,next_after_sequence,has_more,wait_timed_out}`다. `operation`은 `star.operation-snapshot`의 MCP redacted view, progress는 sequence 오름차순 최대 256개다. `next_after_sequence`는 반환한 마지막 sequence 또는 입력값, `has_more`는 저장된 event가 더 있을 때 true, `wait_timed_out`은 long-poll이 event 없이 끝난 경우 true다.

### `star_tool_operation_cancel`

입력은 `operation_id`, optional `reason` 0~512자와 `force_after_ms` 0~30000 또는 null이다. null은 descriptor cancel grace, 0은 즉시 강제 종료 요청이다. 실제 action이 `cancel_mode=none`이면 intent만 기록하고 강제 종료는 timeout·Controller shutdown 정책에서만 수행한다. 같은 Operation에 반복 호출해도 하나의 cancel intent만 남긴다. terminal Operation이면 현재 terminal 결과를 그대로 반환한다.

결과 data는 `{operation,cancel_requested,cancel_effective}`다. cancel intent만 기록됐으면 requested=true/effective=false이며 실제 terminal cancel 뒤에만 둘 다 true다.

### `star_approval_resolve`

입력:

| 필드 | 형식 | 제약 |
|---|---|---|
| `approval_id` | ApprovalId | 필수 |
| `decision` | enum | `approve`, `deny` |
| `scope_hash` | SHA-256 | 필수 |
| `reason` | string 또는 null | 최대 1000자 |
| `conditions` | object 또는 null | ApprovalRequest Schema가 허용한 비용·시간·대상 축소만 |

같은 decision·scope의 반복은 기존 결과를 반환한다. Controller-private durable record에는 최초 decision의 `reason`, 허용된 축소 `conditions`, 인증된 resolver actor를 함께 보존하며 반복 호출이 이를 덮어쓰지 않는다. 다른 decision 또는 stale scope는 `POLICY_APPROVAL_STALE`이다. 이 tool은 승인된 action을 직접 실행하지 않고 대기 Operation을 다시 runnable로 만든다.

결과 data는 `{approval_id,decision,resolved_at,operation}`이다. 연결된 Operation이 없으면 operation은 null이다.

## McpToolResult

모든 tools/call 성공 응답은 `structuredContent`에 다음 object를 반환한다.

| 필드 | 필수 | 형식 |
|---|---:|---|
| `schema_id` | 예 | `star.mcp.<tool-name>.result` |
| `schema_version` | 예 | `1` |
| `status` | 예 | `ok,accepted,question_required,approval_required,blocked,error` |
| `summary` | 예 | 1~1000자 |
| `data` | 아니요 | tool별 object |
| `operation_id` | 아니요 | OperationId |
| `next_actions` | 아니요 | 최대 16개 |
| `artifact_refs` | 예 | array, 기본 `[]` |
| `diagnostic_refs` | 예 | array, 기본 `[]` |
| `error` | 아니요 | ErrorEnvelope |
| `correlation_id` | 예 | RequestId |

`content[0]`은 `type=text`이고 `status`, summary, 주요 ID만 포함한 2000자 이하 요약이다. `status=error`만 `isError=true`다. question·approval·blocked는 `isError=false`다.

`next_actions[]`는 `{tool_name,reason,arguments}`다. `tool_name`은 이 문서의 고정 12개 중 하나, `reason`은 1~240자, `arguments`는 해당 고정 input Schema의 일부 또는 전체 object다. 외부 tool output이 임의 next action을 만들 수 없고 Controller의 상태기계만 이 배열을 생성한다.

- `status=ok`는 `error`와 `operation_id`가 없다.
- `status=accepted`는 `operation_id`가 필수다.
- `status=approval_required`는 `operation_id`, data의 `approval_request`, `next_actions`의 `star_approval_resolve`가 필수다.
- `status=question_required`는 typed question data와 답변 가능한 core action이 필수다.
- `status=blocked`는 diagnostic 또는 정책 근거가 필수다.
- `status=error`는 `error`가 필수이며 `data`에 성공 결과를 넣지 않는다.

JSON-RPC error는 parse error `-32700`, invalid request `-32600`, unknown fixed method·tool `-32601`, 고정 MCP input Schema 위반 `-32602`, SDK·Gateway invariant `-32603`에만 사용한다. Controller·Registry·EXE 오류는 McpToolResult ErrorEnvelope로 반환한다.

Controller pipe가 readiness 안에 나타나지 않으면 `IPC_CONTROLLER_UNAVAILABLE`, HMAC 실패는 `IPC_AUTH_FAILED`, PID·설치 image 불일치는 `IPC_SERVER_IDENTITY_MISMATCH`, IPC major 불일치는 `IPC_PROTOCOL_MISMATCH`로 정규화한다.

## MCP progress·cancellation·tasks 결정

- request `_meta.progressToken`이 있으면 pending sync call에만 `notifications/progress`를 보낸다.
- progress는 단조 증가하고 초당 최대 4회, 같은 phase는 coalesce한다.
- tools/call JSON-RPC request가 진행 중일 때 `notifications/cancelled`를 받으면 해당 request와 아직 응답하지 않은 Operation에 cancel intent를 전달한다.
- initialize·ping·tools/list cancellation은 상태를 바꾸지 않고 무시한다.
- accepted 응답 뒤에는 MCP request가 끝났으므로 `star_tool_operation_cancel`만 사용한다.
- MCP Tasks capability는 광고·구현하지 않는다. durable 실행의 유일한 정본은 Operation이다.
- connection 종료는 Operation 취소로 해석하지 않는다.

## MCP→IPC command 대응

| MCP tool | Local IPC command |
|---|---|
| `star_tool_search` | `tool.search` |
| `star_tool_describe` | `tool.describe` |
| `star_tool_registry_status` | `tool.registry.status` |
| 여섯 `star_tool_call_*` | `tool.invoke` |
| `star_tool_operation_get` | `operation.get` |
| `star_tool_operation_cancel` | `operation.cancel` |
| `star_approval_resolve` | `approval.resolve` |

Gateway가 추가하는 `tool.invoke` 값은 `mcp_tool_name`, 계산된 `mcp_risk_lane`, MCP request ID, progress token 유무와 client info다. Gateway는 arguments를 변형하거나 기본값을 채우지 않는다.

## Hash 정본

### 공통 알고리즘

1. TOML·Rust type을 JSON value로 변환한다.
2. 의미가 set인 array는 stable ID 기준 정렬·중복 제거한다. argv·example처럼 순서가 의미인 array는 유지한다.
3. null과 default가 같은 optional field는 normalized object에 명시된 default value로 넣는다.
4. secret value, timestamp, diagnostic message와 machine absolute path 원문을 제외한다.
5. RFC 8785 JCS UTF-8 bytes로 canonicalize한다.
6. SHA-256을 계산하고 lowercase hex에 `sha256:` prefix를 붙인다.

### `source_content_hash`와 `manifest_hash`

`source_content_hash`는 TOML 원본 byte 그대로의 SHA-256이라 줄바꿈·공백 변화도 검출한다. `manifest_hash`는 parse·default 적용·set 정렬·path separator 정규화가 끝난 ToolPackageManifest semantic object의 JCS hash다. unresolved relative path와 SecretRef ID는 포함하고 source 파일 absolute path, source kind, 실제 secret 값, local Schema content와 resolved EXE identity는 제외한다. Schema content는 별도 relative-path→SHA-256 sorted map으로 보존한다.

### `package_hash`

`{package_id,package_version,source_kind,manifest_hash,schema_hashes,executable_identities,tool_descriptor_hashes,trust_id}`의 JCS hash다. map·array는 stable ID 순이다. candidate·diagnostic·probe 시각과 readiness 문구는 제외하고 실제 active identity·trust가 바뀌면 달라진다.

### `arguments_hash`

action input Schema로 default 적용과 type validation을 끝낸 arguments object의 JCS hash다. 문자열 정규화는 하지 않으므로 사용자가 입력한 Unicode byte 의미를 보존한다.

### `descriptor_hash`

다음을 포함한다.

- package ID·version·source kind·manifest hash
- tool ID와 discovery metadata
- normalized input·output Schema
- backend kind·ref·protocol·argument binding
- permission·paid·risk lane·isolation·idempotency·concurrency
- cwd·env 이름·SecretRef ID·timeout·output·exit·artifact 규칙
- update policy와 실행 시점의 resolved executable identity hash·interface version

description, example 또는 실행 의미가 바뀌면 hash도 바뀐다. readiness message, last probe time, absolute source file path와 secret value는 제외한다.

### `snapshot_hash`

package ID 순으로 정렬한 `{package_id,package_hash}`와 ToolId 순으로 정렬한 `{tool_id,descriptor_hash}`의 JCS hash다. `registry_revision`, timestamp와 diagnostics는 제외한다.

### `scope_hash`

`{approval_id,tool_id,descriptor_hash,arguments_hash,permission_actions,paid_limit,target_refs,expected_revision}`의 JCS hash다.

## 결정적 search

local AI와 embedding을 사용하지 않는다.

1. query·검색 metadata는 Unicode NFKC, invariant lowercase 후 공백·문장부호로 token화한다.
2. 한글·문자·숫자 연속 구간을 token으로 유지한다.
3. 점·하이픈·밑줄은 ID 검색에서 separator와 원문 두 형태로 index한다.
4. score는 exact ToolId 1000, exact alias 800, ID prefix 600, alias prefix 500, exact tag·task kind 300, summary token 일치당 40, description token 일치당 10이다.
5. filter를 먼저 적용하고 score 내림차순, ToolId 오름차순으로 정렬한다.
6. score 0 결과는 반환하지 않는다.

각 distinct normalized query token은 한 metadata field에서 한 번만 점수에 반영한다. exact·prefix와 token 점수는 합산하고 alias가 여러 개 맞아도 가장 높은 alias 하나만 반영한다. query와 filter의 set array는 정렬·중복 제거한 뒤 hash한다.

같은 snapshot·query·filter는 같은 결과·cursor를 만들어야 한다.

## Registry 상태기계

candidate package 상태:

```text
detected -> stabilizing -> parsing -> validating
  -> trust_pending
  -> probing
  -> ready
  -> unavailable
  -> incompatible
  -> invalid
  -> revoked
  -> disabled
```

- `ready` candidate만 active snapshot을 교체한다.
- 기존 active package의 새 candidate가 invalid·unavailable·incompatible이면 active last-known-good를 유지하되 status에 candidate 상태를 함께 표시한다.
- source 파일 삭제는 debounce 뒤 active package를 제거한다. cache로 되살리지 않는다.
- trust revoke는 새 invoke를 즉시 차단하고 새 snapshot을 publish한다.
- required core package가 ready가 아니면 core readiness는 blocked지만 valid external package 조회는 유지한다.

search readiness mapping은 다음과 같다.

| Registry 상태 | search readiness |
|---|---|
| active ready, 새 candidate 없음 | `ready` |
| active last-known-good + 새 candidate invalid·unavailable·incompatible·stabilizing | `degraded` |
| active 없음 + `trust_pending` | `untrusted` |
| active 없음 + `unavailable` | `unavailable` |
| active 없음 + `incompatible` | `incompatible` |
| active 없음 + invalid·revoked·disabled | search에 없음, status에서만 표시 |

두 counter의 최초 값은 0이다. `registry_revision`은 active snapshot hash가 바뀔 때만 1 증가한다. `diagnostic_revision`은 candidate·watcher·probe·trust 진단이 바뀔 때 1 증가한다. 둘은 Controller 재시작 뒤 durable counter에서 이어가며 overflow 전에 새 contract major가 필요하다.

## Invocation·Operation 상태기계

```text
received -> resolving -> approval_wait | queued
approval_wait -> queued | denied | expired
queued -> starting -> running
running -> succeeded | failed | cancelling | outcome_unknown
cancelling -> cancelled | succeeded | failed | outcome_unknown
```

terminal 상태는 `succeeded`, `failed`, `cancelled`, `denied`, `expired`, `outcome_unknown`이다. terminal 뒤 상태는 바뀌지 않는다. 늦게 도착한 process exit는 새 evidence event로 기록하되 terminal 결과를 덮지 않는다.

같은 idempotency key와 같은 descriptor·arguments hash는 기존 Operation을 반환한다. 같은 key에 payload가 다르면 `STATE_IDEMPOTENCY_CONFLICT`다. non-idempotent action은 timeout·disconnect 뒤 자동 재실행하지 않는다.

idempotency record key는 `{current_user_sid_hash,project_id,goal_id,tool_id,idempotency_key}`다. project·Goal이 없으면 null을 명시한다. terminal Operation retention 동안, 최소 24시간 보존하며 만료 시각을 결과에 남긴다. 만료 뒤 같은 문자열은 새 요청이므로 client가 장기 전역 key로 가정하면 안 된다.

## Risk lane 계산

manifest가 lane을 직접 지정하지 않는다. 등록된 Permission ActionId set에서 계산한다.

open set:

`network_read`, `network_download`, `external_write`, `account_change`, `git_push`, `pull_request`, `release_publish`, `paid_action`

destructive set:

`local_delete`, `local_mass_move`, `system_change`, `account_change`, `git_merge`, `release_publish`

write set:

`local_write`, `dependency_change`, `external_write`, `plan_execute`, `git_commit`, `git_push`, `pull_request`와 destructive set 전체

계산 순서:

1. unknown ActionId가 있으면 package invalid
2. destructive set과 교집합이면 `destructive`, 아니면 write set과 교집합이면 `write`, 아니면 `read`
3. open set과 교집합이면 `open`, 아니면 `closed`
4. 두 값을 결합해 여섯 lane 중 하나 선택

`process_run`, `local_read`, `secret_access`만으로는 write가 되지 않지만 각각 실제 PermissionPlan 검사는 유지한다.

## Required core Tool package

release의 `star-control-core.toml`은 다음 action을 정확히 선언한다. `controller_command` backend는 이 package에서만 허용한다.

| ToolId | Controller command | lane |
|---|---|---|
| `star.core.goal.start` | `goal.start` | write_closed |
| `star.core.goal.answer` | `goal.answer` | write_closed |
| `star.core.plan.get` | `plan.get` | read_closed |
| `star.core.plan.update` | `plan.update` | write_closed |
| `star.core.run.continue` | `run.continue` | destructive_open |
| `star.core.status.get` | `goal.status` | read_closed |
| `star.core.goal.pause` | `goal.pause` | write_closed |
| `star.core.goal.resume` | `goal.resume` | write_closed |
| `star.core.goal.cancel` | `goal.cancel` | destructive_open |
| `star.core.evidence.get` | `evidence.get` | read_closed |
| `star.core.merge.status` | `merge.status` | read_closed |
| `star.core.handoff.get` | `handoff.get` | read_closed |
| `star.core.doctor` | `doctor.run` | read_closed |

core action의 input·output 의미는 각 application command의 소유 계약을 그대로 사용한다. MCP package가 필드를 새로 만들거나 축약하지 않는다. build 단계에서 소유 계약의 generated Schema를 가져와 remote `$ref` 없는 fully resolved inline Schema로 만들고, command가 없거나 Schema version mapping이 없으면 release build를 실패시킨다. 이 13개 action의 ToolId·command·lane만 이 문서가 소유한다.

## Codex MCP 설정 정본

기본 설치:

```toml
[mcp_servers.star_control]
command = 'C:\Program Files\Star-Control\star-mcp.exe'
required = true
startup_timeout_sec = 20
tool_timeout_sec = 600
default_tools_approval_mode = "auto"

[mcp_servers.star_control.tools.star_tool_call_destructive_closed]
approval_mode = "prompt"

[mcp_servers.star_control.tools.star_tool_call_write_open]
approval_mode = "prompt"

[mcp_servers.star_control.tools.star_tool_call_destructive_open]
approval_mode = "prompt"

[mcp_servers.star_control.tools.star_tool_operation_cancel]
approval_mode = "prompt"
```

이 외 승인 여부는 Controller PermissionPlan이 결정한다. `personal_auto` 사용자는 위 네 override를 `auto`로 바꿀 수 있으나 `paid_action`의 Controller 승인은 유지된다. `star_approval_resolve`는 이미 대화에서 받은 결정을 기록하므로 MCP 재승인 loop를 만들지 않게 `auto`다.

## Binary 변경 경계

다음은 TOML·Schema·Catalog 변경만으로 처리한다.

- 새 EXE·package·action
- path·flag·subcommand·stdin·output·exit code 변경
- input·output Schema와 설명 변경
- update·probe·timeout·concurrency·permission metadata 변경

다음은 MCP·Controller 계약 변경이다.

- 고정 MCP tool 추가·삭제·annotation 의미 변경
- 새 process protocol
- manifest `format_version=2`
- 새 isolation enforcement
- hash 알고리즘 변경
- IPC major 변경
- MCP protocol major compatibility 변경

## 공식 근거

- [MCP STDIO transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [MCP lifecycle·capability negotiation](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
- [MCP Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
- [MCP cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)
- [MCP progress](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/progress)
- [MCP Tasks](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks)
- [공식 Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Codex MCP 지원 기능과 설정](https://learn.chatgpt.com/docs/extend/mcp)
- [RFC 8785 JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785.html)
