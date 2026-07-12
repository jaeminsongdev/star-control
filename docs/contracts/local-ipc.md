# Windows Local IPC 계약

## 목적과 경계

`star.exe`와 `star-mcp.exe`는 상태를 직접 쓰지 않고 `star-controller.exe`에 같은 application command를 보낸다. 통신은 Windows local named pipe만 사용하며 HTTP server와 network port를 열지 않는다.

Controller는 live Tool Registry의 유일한 owner다. CLI와 MCP 연결은 Registry snapshot을 보관하거나 TOML을 읽지 않는다. 따라서 연결을 유지한 상태에서도 새 TOML, path 변경과 같은 path의 EXE 교체가 반영된다.

정확한 MCP command payload와 hash는 [MCP 구현 동결 계약](mcp-implementation-contract.md), named pipe·HMAC·PID 검증은 [Windows Tool Runtime](../architecture/windows-tool-runtime.md)이 소유한다.

## Endpoint와 접근 통제

- endpoint 형식: `\\.\pipe\star-control-<user-sid-hash>-v1`
- pipe DACL은 Controller를 실행한 현재 사용자 SID와 LocalSystem만 허용한다.
- remote pipe client를 거부하고 같은 사용자 session의 local client만 받는다.
- endpoint에는 사용자 이름, project 경로와 secret을 넣지 않는다.
- 현재 사용자당 Controller 한 개만 endpoint owner가 된다.
- endpoint 발견 자료에는 protocol major, process ID, 시작 시각과 readiness만 둔다.

MCP와 CLI가 자동 시작을 허용받았으면 Controller를 숨김 창으로 시작하고 readiness까지 기다린다. 다른 executable이 endpoint를 차지했거나 owner 확인에 실패하면 연결하지 않는다.

## Wire 형식

IPC frame은 `4-byte unsigned little-endian length + UTF-8 JSON payload`다.

- 기본 최대 frame은 8 MiB이며 EffectiveConfig의 더 낮은 제한을 적용한다.
- 큰 log, diff와 binary는 ArtifactRef로 반환한다.
- 잘못된 UTF-8, 선언 길이 불일치, 최대 크기 초과와 지원하지 않는 schema는 연결 단위 오류다.
- 압축과 암호화는 v1에 넣지 않는다. 현재 사용자 전용 pipe와 DACL이 통신 경계를 담당한다.

## Handshake

연결 직후 client가 server PID·image를 확인하고 Controller가 `IpcChallenge`를 보낸다. client가 `IpcHello`, Controller가 인증된 `IpcWelcome`을 돌려준다. challenge는 unauthenticated이므로 application readiness 근거로 사용하지 않는다. PID·HMAC·nonce 실패는 상세 payload 없이 연결을 닫고 양쪽 local log에서 `IPC_AUTH_FAILED`로 정규화한다.

### IpcChallenge

| 필드 | 의미 |
|---|---|
| `schema_id`, `schema_version` | `star.ipc.challenge`, 1 |
| `protocol_major` | 정확히 1 |
| `controller_instance_id` | process lifetime ID |
| `server_pid` | client가 pipe owner와 대조할 PID |
| `server_nonce` | connection 전용 32-byte random base64url |
| `issued_at` | 5초 만료 검사 시각 |

### IpcHello

| 필드 | 의미 |
|---|---|
| `schema_id`, `schema_version` | `star.ipc.hello`, 1 |
| `protocol_versions` | client가 지원하는 major.minor 목록 |
| `client_kind` | `cli`, `mcp`, `hook`, `internal_test` |
| `client_version` | 실행 파일 version |
| `client_instance_id` | process lifetime의 무작위 ID |
| `client_pid` | server가 image path를 확인할 process ID |
| `client_nonce` | 32-byte random base64url |
| `server_nonce` | 받은 challenge의 exact nonce |
| `auth_tag` | per-user IPC key의 client HMAC |
| `capabilities` | event stream·progress 등 선택 기능 |
| `correlation_id` | handshake 진단 연결 값 |

IpcHello에는 ToolRegistrySnapshot hash를 넣지 않는다. Registry는 MCP process에 고정되지 않으며 Controller가 매 요청에 맞는 live revision을 선택한다.

### IpcWelcome

| 필드 | 의미 |
|---|---|
| `schema_id`, `schema_version` | `star.ipc.welcome`, 1 |
| `protocol_version` | 협상된 version |
| `controller_version` | Controller version |
| `controller_instance_id` | 재시작 감지용 ID |
| `session_id` | 현재 pipe connection ID |
| `server_nonce` | challenge nonce echo |
| `auth_tag` | client nonce와 instance를 묶은 server HMAC |
| `readiness` | `ready`, `degraded`, `recovering`, `blocked` |
| `capabilities` | 지원하는 command·event·operation 기능 |
| `registry_revision` | 응답 시점의 관찰용 live revision |
| `server_time` | 시간 차 진단 값 |

공통 protocol이 없으면 Welcome 대신 `{schema_id:"star.ipc.handshake-error",schema_version:1,code:"IPC_PROTOCOL_MISMATCH",supported_versions:["1.0"],correlation_id,auth_tag}`를 보낸다. `auth_tag`는 welcome과 같은 server HMAC domain으로 handshake-error의 auth_tag 제외 JCS에 계산한다. 이 외 pre-auth 실패는 payload 없이 닫는다.

`registry_revision`은 캐시 key나 실행 권한이 아니다. 다음 요청 전에 Registry가 바뀔 수 있으며 invoke 안전성은 `descriptor_hash`로 보장한다.

protocol major가 다르면 연결을 거부한다. minor 차이는 양쪽이 선언한 capability의 교집합만 사용한다.

최초 구현의 supported list는 `1.0` 하나다. wire string은 `<unsigned-major>.<unsigned-minor>`이며 선행 0을 허용하지 않는다. 새 optional command·field를 추가할 때만 minor를 올린다.

## IpcRequest 계약

| 필드 | 필수 | 의미 |
|---|---:|---|
| `schema_id` | 예 | `star.ipc.request` |
| `schema_version` | 예 | request envelope version |
| `request_id` | 예 | connection 안의 고유 ID |
| `command` | 예 | typed command 이름 |
| `payload` | 예 | command별 JSON object |
| `client_request_id` | 예 | MCP·CLI 재전송 연결 값 |
| `idempotency_key` | 조건부 | effect가 있는 command의 중복 방지 key |
| `deadline` | 조건부 | 새 작업 접수를 중단할 절대 시각 |
| `actor` | 예 | client 종류와 사용자 provenance. 설치 client가 보내는 `project_root`는 현재 요청의 project source·ProjectPathRef·cwd 선택에만 쓰는 private transport 값 |
| `trace_context` | 선택 | event·process 실행 연결 값 |

임의 method 이름, shell string과 자유 형식 payload를 application command로 허용하지 않는다. unknown field 정책과 version 규칙은 [Version과 Migration](versioning-and-migrations.md)을 따른다.

Controller는 autostart 또는 이전 Gateway의 process cwd를 현재 project로 간주하지 않는다. 인증된 설치 client의 `actor.project_root`를 요청마다 final fixed-local directory로 해석하고 reparse point·상대 경로·존재하지 않는 directory를 거부한다. durable trust·Operation evidence와 MCP 결과에는 absolute path 원문을 넣지 않고 project root hash만 남긴다. pending approval의 Controller-private 재개 record만 승인 뒤 같은 root에서 실행하기 위해 원문을 DACL-protected state에 보존한다.

## IpcResponse 계약

| 필드 | 필수 | 의미 |
|---|---:|---|
| `schema_id` | 예 | `star.ipc.response` |
| `schema_version` | 예 | response envelope version |
| `request_id` | 예 | 원 요청과 연결 |
| `status` | 예 | `ok`, `accepted`, `question_required`, `approval_required`, `blocked`, `error` |
| `data` | 선택 | command별 typed 결과 |
| `operation_id` | 선택 | 장기 실행 상태 조회 ID |
| `diagnostics` | 선택 | DiagnosticRef 목록 |
| `error` | 조건부 | `status=error`의 ErrorEnvelope |
| `registry_revision` | 선택 | Registry 관련 응답이 사용한 revision |
| `correlation_id` | 예 | 전체 흐름 연결 값 |

질문·승인 대기는 정상 응답이며 완료로 간주하지 않는다. request deadline이 지나도 이미 시작된 side effect를 성공으로 추측하거나 자동 재실행하지 않는다.

## Command 집합

### Application command

`goal.start`, `goal.answer`, `plan.get`, `plan.update`, `run.continue`, `goal.status`, `goal.pause`, `goal.resume`, `goal.cancel`, `evidence.get`, `merge.status`, `handoff.get`, `approval.resolve`, `doctor.run`을 같은 application layer에 연결한다. CLI는 이 typed command를 직접 사용할 수 있다.

Star-Control core ToolPackageManifest의 action도 실행 시 이 application command로 resolve된다. core manifest는 외부 EXE를 가장하지 않으며 Controller 내부 backend만 사용할 수 있는 release trusted package다.

### Live Tool Registry command

| command | 목적 |
|---|---|
| `tool.search` | 최신 usable descriptor index 검색 |
| `tool.describe` | input·output Schema, risk lane, hash와 실행 조건 조회 |
| `tool.registry.status` | source·package·reload·last-known-good 진단 조회 |
| `tool.invoke` | descriptor hash와 lane을 검증한 뒤 core 또는 process backend 실행 |

### Operation command

| command | 목적 |
|---|---|
| `operation.get` | 상태·progress·부분 진단·최종 결과 조회 |
| `operation.cancel` | 취소 가능한 실행에 취소 signal 전달 |
| `events.subscribe` | 허용된 범위의 상태·progress event 구독 |
| `events.unsubscribe` | 구독 종료 |

## Live Registry 요청 처리

### search·describe·status

Controller는 요청 직전에 demand scan을 수행한다. file watcher 알림이 유실됐거나 timestamp 정밀도가 달라도 manifest·Schema·EXE identity 변경을 확인한다. 검증된 후보가 있으면 immutable RegistrySnapshot을 atomically publish한 뒤 그 revision에서 조회한다.

`search`·`describe`·`status`는 trusted `version_compatible` candidate 하나의 최대 30초 probe를 수행할 수 있고 IPC client는 5초 stable-file 구간까지 합친 40초 response budget을 사용한다. `tool.invoke`는 무관한 candidate를 자동 probe하지 않으며 현재 active descriptor만 검증·lease한다. 수동 `tool.probe`도 같은 40초 budget을 사용한다.

잘못된 optional package는 그 package의 last-known-good를 유지하고 진단을 반환한다. 다른 정상 package의 update까지 막지 않는다.

### `tool.invoke` payload

| 필드 | 필수 | 의미 |
|---|---:|---|
| `tool_id` | 예 | stable action ID |
| `descriptor_hash` | 예 | describe에서 확인한 계약 hash |
| `risk_lane` | 예 | 실제 호출한 고정 MCP lane |
| `arguments` | 예 | descriptor input Schema용 object |
| `goal_id` | 조건부 | Goal 소속 action |
| `expected_revision` | 조건부 | 상태 mutation 충돌 검사 |
| `client_request_id` | 예 | end-to-end 요청 ID |
| `idempotency_key` | 조건부 | 재전송 안전 key |
| `wait_mode` | 예 | Gateway가 정규화한 `auto`, `sync`, `accepted` |
| `requested_timeout_ms` | 조건부 | descriptor·EffectiveConfig 상한 안의 요청값 |
| `mcp_tool_name` | MCP에서 예 | 실제 호출한 고정 risk lane 이름 |
| `mcp_request_id` | MCP에서 예 | 원 JSON-RPC request ID |
| `progress_requested` | 예 | MCP progress token 존재 여부. token 원문은 전달하지 않음 |

Controller는 다음 순서로 처리한다.

1. demand scan과 package 검증을 끝낸 live snapshot을 읽는다.
2. `tool_id`를 찾고 usable·trust·readiness를 확인한다.
3. 현재 descriptor hash와 요청 hash가 같은지 확인한다.
4. descriptor의 risk lane과 요청 lane이 같은지 확인한다.
5. arguments를 현재 input Schema로 검증한다.
6. PermissionPlan, approval scope, 비용, 경로와 project scope를 확인한다.
7. 실행에 사용할 descriptor와 resolved executable identity를 lease한다.
8. core backend 또는 process adapter를 실행하고 결과를 정규화한다.

3번이 다르면 `TOOL_DESCRIPTOR_STALE`, 4번이 다르면 `TOOL_LANE_MISMATCH`다. 둘 다 side effect 전에 실패한다.

실행이 시작된 뒤 새 snapshot이 publish돼도 해당 실행은 lease한 descriptor와 EXE identity를 끝까지 사용한다. 새 호출만 새 revision을 사용한다.

## 비동기 Operation

빠른 command는 한 response에서 끝난다. 장기 실행은 `status=accepted`와 OperationId를 반환하고 Controller가 durable OperationSnapshot을 소유한다.

- progress는 순서 번호와 시각을 가진 event로 저장한다.
- disconnect 뒤 같은 OperationId로 상태를 다시 조회할 수 있다.
- MCP Tasks로 투영하지 않는다. 고정 `star_tool_operation_get` long-polling이 유일한 기준 경로다.
- MCP progress token은 아직 tools/call 응답을 기다리는 동안만 선택적으로 연결한다.

## timeout·취소·재연결

- request deadline은 “응답을 기다리는 시간”과 “실행 process 제한”을 구분한다.
- process timeout은 ToolDescriptor와 상위 EffectiveConfig 중 더 짧은 값을 적용한다.
- MCP cancellation은 Gateway가 `operation.cancel`로 전달하되, process가 이미 끝났다면 최종 결과를 유지한다.
- 취소 요청 뒤에도 process tree 종료, temp file 정리와 부분 effect 확인이 끝날 때까지 `cancelling`일 수 있다.
- 연결이 끊겨도 Controller가 소유한 실행을 임의로 재시작하지 않는다.
- Controller가 재시작되면 event와 OperationSnapshot을 복구하고, 실행 상태를 확인할 수 없으면 `outcome_unknown`으로 표시한다.

## 보안과 진단

- HMAC mutual authentication, PID·installed image 확인과 protocol negotiation이 끝나기 전 application request를 받지 않는다.
- request·response·log에서 secret과 금지 경로 내용을 redaction한다.
- client가 보낸 actor·annotation·trust 주장을 그대로 믿지 않는다.
- path는 Controller가 canonicalize하고 허용 root·reparse point·실행 파일 identity를 확인한다.
- 실행 전후 registry revision, descriptor hash, executable identity, arguments hash, permission 결과와 exit status를 evidence에 남긴다.
- protocol 오류, Registry 오류, tool 오류와 application 오류는 [오류와 진단](errors-and-diagnostics.md)의 namespace로 구분한다.
