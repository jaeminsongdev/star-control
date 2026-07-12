# MCP 구현 검증 행렬

## 목적

이 문서는 MCP 관련 구현 완료를 판정하는 release gate다. 단위 테스트 몇 개나 mock `tools/list` 성공만으로 완료 처리하지 않는다. 각 ID는 자동 test 이름 또는 수동 Windows smoke 기록에 그대로 사용한다.

범위는 `star-mcp.exe`, live Tool Registry, ToolPackageManifest, Local IPC와 외부 EXE Windows runtime이다.

## Test asset 구조

`cargo run --locked -p star-matrix-check -- --details`는 각 ID에 연결된 모든 Rust test 이름을 출력한다. 하나의 ID를 여러 계층의 test가 검증하면 첫 test만 숨기지 않고 모두 표시하며, `#[ignore]`, `#[should_panic]`, flaky·quarantine 표시는 release gate에서 실패다.

```text
tests/
└─ mcp/
   ├─ conformance/
   ├─ gateway/
   ├─ ipc/
   ├─ registry/
   ├─ manifests/
   ├─ process-runtime/
   ├─ security/
   ├─ recovery/
   └─ codex-e2e/
specs/fixtures/mcp/
├─ manifests/valid/
├─ manifests/invalid/
├─ schemas/
├─ canonicalization/
├─ protocol/
└─ operations/
tools/test-binaries/
├─ fake-argv/
├─ fake-json-stdio/
├─ fake-child-tree/
├─ fake-output-flood/
├─ fake-handle-probe/
└─ fake-appcontainer-adapter/
```

test binary는 실제 Rust child process이며 제품 code에 `if test` backend를 넣지 않는다.

## G. MCP Gateway conformance

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-G001 | valid `2025-11-25` initialize | 같은 version, ServerInfo·tools capability·고정 instructions |
| MCP-G002 | valid `2025-06-18` initialize | 협상 성공, 공통 subset만 사용 |
| MCP-G003 | 지원하지 않는 version | 최신 지원 version 제안 후 client 불수용 시 종료 |
| MCP-G004 | initialize 전 tools/list | protocol 오류, process 생존 |
| MCP-G005 | initialized 뒤 tools/list | 고정 12개, 순서·Schema·annotation golden 일치 |
| MCP-G006 | tools/list 반복 | byte-level canonical tool definition 동일 |
| MCP-G007 | stdout에 log 쓰기 시도 | test에서 실패, stderr만 허용 |
| MCP-G008 | malformed·duplicate-key JSON·8 MiB 초과 line | JSON-RPC 오류 또는 안전 종료, allocation 폭증 없음 |
| MCP-G009 | ping | 정상 응답 |
| MCP-G010 | stdin EOF | 5초 안에 Gateway 종료, Controller Operation 유지 |
| MCP-G011 | unknown fixed tool | JSON-RPC `-32601` |
| MCP-G012 | fixed input Schema 위반 | JSON-RPC `-32602`, Controller 미호출 |
| MCP-G013 | Controller 연결 실패 | `isError=true` structured ErrorEnvelope |
| MCP-G014 | `question_required`·`approval_required` | `isError=false` |
| MCP-G015 | actual error | `isError=true`, text·structured status 일치 |
| MCP-G016 | progress token | 단조 progress, 초당 최대 4회, 완료 뒤 중단 |
| MCP-G017 | sync cancellation | request cancel intent 전달, initialize 취소 무시 |
| MCP-G018 | tools/list changed 여부 | capability·notification 모두 없음 |
| MCP-G019 | resources·prompts·logging·completions·tasks 요청 | 광고하지 않으며 method-not-found |
| MCP-G020 | instructions 길이 | 첫 512자만으로 search→describe→call 흐름 이해 가능 |
| MCP-G021 | 2025-06-18 initialize·tools/list golden | server description·Tool execution만 생략, 나머지 fixed surface 동일 |
| MCP-G022 | task-augmented tools/call 요청 | capability 미광고 상태에서 `-32602`, Operation 생성 0회 |
| MCP-G023 | tools/list cursor 제공 | `-32602`, fixed list pagination 없음 |
| MCP-G024 | initialized notification 전 call·두 번째 initialize | invalid request, connection state 손상 없음 |

공식 MCP Inspector와 protocol JSONL fixture 둘 다 통과해야 한다. SDK 자체 test만으로 대체하지 않는다. 구현자 evidence의 exact `@modelcontextprotocol/inspector@0.22.0` 성공은 당시 release binary에 대한 기록이다. 2026-07-12 독립 감사의 current binary 재실행은 고정 12개 `tools/list`와 status까지 통과했지만 required core가 fail-closed `unavailable`이라 `star_tool_search(query=goal, readiness=ready)`에서 실패했다. 현재 판정은 [독립 감사 보고서](mcp-independent-audit-2026-07-12.md)를 따른다.

## I. Local IPC

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-I001 | current user + valid HMAC handshake | 연결 성공 |
| MCP-I002 | 다른 key·nonce replay | 연결 전 거부 |
| MCP-I003 | remote pipe client | `PIPE_REJECT_REMOTE_CLIENTS` 거부 |
| MCP-I004 | protocol major mismatch | application command 전 실패 |
| MCP-I005 | 0·8 MiB 초과·truncated·duplicate-key frame | 연결 종료, Controller 생존 |
| MCP-I006 | client·server PID/image 확인 | 설치 path와 일치 |
| MCP-I007 | concurrent 16 client | request ID 섞임 없이 응답 |
| MCP-I008 | 17번째 client | bounded backpressure·명확한 오류 |
| MCP-I009 | disconnect 중 accepted Operation | Operation 계속, 재연결 조회 가능 |
| MCP-I010 | same idempotency key·same payload | 기존 Operation 반환 |
| MCP-I011 | same key·different payload | `STATE_IDEMPOTENCY_CONFLICT` |
| MCP-I012 | pipe·key file DACL | current user·LocalSystem만 허용 |
| MCP-I013 | Controller auto-start path 교체 race | install hash와 image identity 불일치 시 시작·연결 거부 |
| MCP-I014 | Gateway outer Job breakaway 허용·거부 | 허용 시 durable Controller, 거부 시 같은 Job 시작 없이 안내 오류 |
| MCP-I015 | controller autostart enable·disable·uninstall | exact HKCU value만 idempotent 관리, 다른 값 보존 |
| MCP-I016 | IPC key 삭제·corrupt·live mismatch | same-key rewrite 또는 no-owner rotation audit, live key split 없음 |

## R. Registry와 live reload

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-R001 | valid user TOML 생성 | 1초 안에 다음 search에 등장 |
| MCP-R002 | TOML description 변경 | descriptor·snapshot hash와 revision 변경 |
| MCP-R003 | whitespace·table order만 변경 | normalized hash 불변 |
| MCP-R004 | editor temp rename 저장 | tool이 중간에 사라지지 않음 |
| MCP-R005 | 5초 동안 쓰는 중인 TOML | stabilizing + last-known-good |
| MCP-R006 | invalid 기존 TOML | active LKG 유지, diagnostic revision 증가 |
| MCP-R007 | invalid 새 TOML | active에 없음, 다른 package 정상 |
| MCP-R008 | required core invalid | core blocked, valid external 조회 유지 |
| MCP-R009 | TOML 실제 삭제 | debounce 뒤 새 call에서 제거, running call 유지 |
| MCP-R010 | watcher buffer overflow | root full scan으로 정확한 snapshot 복구 |
| MCP-R011 | watcher event 강제 누락 | 다음 demand scan이 변경 발견 |
| MCP-R012 | Controller 재시작 + invalid source | durable LKG 복구 |
| MCP-R013 | source 삭제 + cache 존재 | cache로 되살리지 않음 |
| MCP-R014 | trust revoke | 새 invoke 즉시 거부, running policy 적용 |
| MCP-R015 | same ID·same version·different content | conflict, 우선순위 덮기 금지 |
| MCP-R016 | trusted version-bounded replaces·cycle·multiple replacer | valid 교체만 활성, 나머지 conflict·provenance 정확 |
| MCP-R017 | project follow_path | package invalid |
| MCP-R018 | project controller_command | package invalid |
| MCP-R019 | Registry 128 package·512 action | limit 안에서 결정적 search |
| MCP-R020 | limit 초과 | 초과 candidate 거부, existing active 유지 |
| MCP-R021 | status cursor 뒤 Registry·diagnostic revision 변경 | `TOOL_REGISTRY_CURSOR_STALE` |

## H. Hash·search 결정성

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-H001 | RFC 8785 official vector | expected JCS·SHA-256 |
| MCP-H002 | TOML key order·CRLF 차이 | 같은 manifest·descriptor hash |
| MCP-H003 | set array 순서 차이 | 같은 hash |
| MCP-H004 | argv 순서 차이 | 다른 descriptor hash |
| MCP-H005 | description·Schema·permission 변경 | 각각 다른 descriptor hash |
| MCP-H006 | follow_path EXE identity 교체 | 다른 descriptor hash |
| MCP-H007 | timestamp·diagnostic 문구 변경 | snapshot hash 불변 |
| MCP-H008 | secret value 변경 | descriptor hash 불변, SecretRef ID 변경은 변함 |
| MCP-H009 | arguments default 적용 | describe 계약대로 같은 arguments hash |
| MCP-H010 | Unicode 문자열 byte 차이 | 자동 normalization 없이 다른 arguments hash |
| MCP-H011 | 같은 search query·snapshot | 결과·cursor 동일 |
| MCP-H012 | cursor 뒤 snapshot 변경 | `TOOL_SEARCH_CURSOR_STALE` |
| MCP-H013 | 한국어·영문·ID·alias 검색 | score 규칙 golden 일치 |
| MCP-H014 | score 동점 | ToolId 오름차순 |
| MCP-H015 | TOML 공백·CRLF만 변경 | source content hash는 변하고 manifest·package descriptor 의미 hash는 불변 |

canonicalization fixture는 RFC vector와 Star-Control 전용 normalized object를 모두 포함한다.

## M. Manifest validation

각 enum·binding·source·update policy는 최소 valid 1개와 invalid 1개를 가진다.

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-M001 | 완전한 argv manifest | parse·normalize·round-trip 성공 |
| MCP-M002 | 완전한 JSON-STDIO manifest | 성공 |
| MCP-M003 | release core controller_command | release에서만 성공 |
| MCP-M004 | unknown·duplicate key | 거부 |
| MCP-M005 | remote `$ref`·cycle·depth 65 | 거부 |
| MCP-M006 | Schema 총 4 MiB 초과 | 거부 |
| MCP-M007 | 존재하지 않는 parameter binding | 거부 |
| MCP-M008 | stdin binding 두 개 | 거부 |
| MCP-M009 | exit code set 겹침 | 거부 |
| MCP-M010 | paid metadata·ActionId 불일치 | 거부 |
| MCP-M011 | unknown ActionId | 거부 |
| MCP-M012 | raw shell·script·PATH lookup | 거부 |
| MCP-M013 | locator field 조합 위반 | 거부 |
| MCP-M014 | version_compatible without probe | 거부 |
| MCP-M015 | regex compile size·length 초과 | 거부 |
| MCP-M016 | remote·device·ADS·reparse executable | 거부 |
| MCP-M017 | generated Schema drift | build 실패 |
| MCP-M018 | higher future format version | metadata 진단만, 실행 거부 |
| MCP-M019 | project fixed cwd·project location map override | 거부 |
| MCP-M020 | JSON·JSONL output Schema 누락·non-object root·number type | 거부 |
| MCP-M021 | product version constraint without probe | 거부 |
| MCP-M022 | reserved·case-insensitive duplicate environment name | 거부 |
| MCP-M023 | action resolved Schema 합계 1 MiB 초과 | 거부, 다른 package 유지 |
| MCP-M024 | version_compatible without require_subject | 거부, unsigned compatible 자동 채택 없음 |
| MCP-M025 | scaffold disabled draft·enabled zero-action | draft는 process 0회·status only, enabled는 거부 |

## P. Windows process runtime

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-P001 | EXE path·argument에 공백·한글·quote·backslash | fake child argv golden 일치 |
| MCP-P002 | NUL·32767 UTF-16 초과 | process 전 `TOOL_ARGUMENT_INVALID` |
| MCP-P003 | stdin 미사용 | 즉시 EOF, child hang 없음 |
| MCP-P004 | stdout·stderr 동시 hard-limit flood | deadlock 없음, bounded capture 뒤 `TOOL_OUTPUT_LIMIT` |
| MCP-P005 | child tree 생성 | 모두 같은 Job Object |
| MCP-P006 | timeout | grace 뒤 전체 child tree 종료 |
| MCP-P007 | Controller crash | KILL_ON_JOB_CLOSE로 child 종료 |
| MCP-P008 | executable 검증 중 path 교체 | share violation·rescan, 잘못된 byte 실행 없음 |
| MCP-P009 | process 생성 뒤 same-path 교체 | running identity 불변, 새 call만 새 identity |
| MCP-P010 | pinned hash mismatch | unavailable, 실행 없음 |
| MCP-P011 | compatible probe success | 새 identity active |
| MCP-P012 | compatible probe failure | LKG 유지·diagnostic |
| MCP-P013 | follow_path contract-compatible 교체 | 같은 Codex session 재-describe 후 실행 |
| MCP-P014 | JSON-STDIO progress·result | sequence·Schema 정상 |
| MCP-P015 | JSON-STDIO stdout garbage·final 두 개 | `TOOL_PROTOCOL_INVALID` |
| MCP-P016 | JSON-STDIO stdin cancel | ack optional, final 또는 forced Job termination |
| MCP-P017 | argv cancel | Job termination, partial outcome 기록 |
| MCP-P018 | unlisted exit code | non-retryable error |
| MCP-P019 | empty·warning·retryable exit | 계약대로 정규화 |
| MCP-P020 | inline limit 초과·hard limit 이하 | 전체 artifact, 성공 truncate 없음 |
| MCP-P021 | handle probe child | stdin·stdout·stderr 외 inherited handle 0개 |
| MCP-P022 | working directory scope 이탈 | process 전 거부 |
| MCP-P023 | Authenticode unsigned·invalid·offline·subject | record metadata, require 정책 fail-closed, 검증 network 0회 |
| MCP-P024 | integrity DLL mismatch | process 전 거부 |
| MCP-P025 | x64·ARM64 mismatch | incompatible |
| MCP-P026 | Controller가 restrictive outer Job 안 | nested Job 성공 또는 fail-closed, breakaway 없음 |
| MCP-P027 | minimal core environment·allowlist | PATH 미상속, 선언한 값만 child에 존재 |
| MCP-P028 | SecretRef·state directory environment | child에만 값 전달, scope·retention 정확 |
| MCP-P029 | OEM·UTF-16LE·invalid encoding | 선언대로 decode, invalid byte 성공 처리 없음 |
| MCP-P030 | JSON-STDIO unknown·missing·duplicate field·nonzero exit | `TOOL_PROTOCOL_INVALID` |
| MCP-P031 | artifact relative path escape·hash mismatch | result 거부·quarantine |
| MCP-P032 | 같은 file ID·size·last-write로 byte 변경 | invoke full hash가 변경 검출, 이전 descriptor 실행 없음 |

## S. 보안과 개인정보

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-S001 | project manifest가 auto·trusted 주장 | unknown/forbidden key 거부 |
| MCP-S002 | description에 prompt injection | instructions 불변, provenance 표시 |
| MCP-S003 | tool output에 지시문 | untrusted output, 자동 다음 action 없음 |
| MCP-S004 | SecretRef 사용 | child에만 전달, MCP·log·event·hash에 원문 없음 |
| MCP-S005 | secret 포함 stderr·stdout | redaction 또는 quarantine |
| MCP-S006 | project path `..`·junction escape | 거부 |
| MCP-S007 | same ToolId lower-risk manifest replacement | trust·descriptor stale로 거부 |
| MCP-S008 | 다른 risk lane으로 invoke | `TOOL_LANE_MISMATCH`, process 0회 |
| MCP-S009 | describe 뒤 permission 변경 | `TOOL_DESCRIPTOR_STALE` |
| MCP-S010 | paid_action unknown | approval_required |
| MCP-S011 | stale approval scope | `POLICY_APPROVAL_STALE` |
| MCP-S012 | trusted_desktop | describe·CLI·report에 sandbox 아님을 표시 |
| MCP-S013 | appcontainer adapter path escape | OS access denied + Controller 오류 |
| MCP-S014 | appcontainer network attempt·loopback exemption | OS 차단, exemption이면 launch 전 fail-closed |
| MCP-S015 | trust·cache file ACL | 다른 일반 local user 접근 거부 |
| MCP-S016 | log·artifact absolute path | default MCP result에서 redacted |
| MCP-S017 | project·Goal이 user location·trust·IPC auth 완화 | 설정 오류, effective value 불변 |
| MCP-S018 | safe_default user path·location_ref 변경 | 새 code trust 전 process 0회 |

## O. Operation·recovery

| ID | 상황 | 기대 결과 |
|---|---|---|
| MCP-O001 | detachable·expected 30초 초과·sync 실제 30초 초과 | 5초 또는 sync budget 안 accepted + OperationId |
| MCP-O002 | operation long-poll·256 event pagination | sequence 이후 event·cursor 또는 wait timeout |
| MCP-O003 | cancel 반복 | cancel intent 하나, idempotent result |
| MCP-O004 | terminal 뒤 late exit | terminal 불변, evidence만 추가 |
| MCP-O005 | disconnect·Gateway restart | Controller Operation 조회 가능 |
| MCP-O006 | Controller crash before process start | replay에서 안전한 failed/queued 복구 |
| MCP-O007 | crash after process start before final | outcome_unknown, non-idempotent 재실행 없음 |
| MCP-O008 | approval approve·deny·expire | state machine golden 일치 |
| MCP-O009 | queue lock order 반대 요청 | deadlock 없음 |
| MCP-O010 | queue timeout | process 미시작, 명확한 error |
| MCP-O011 | retryable exit | retryable error 한 번, 외부 EXE 자동 재실행 0회 |

## C. 실제 Codex same-session E2E

mock client만으로 이 gate를 대체할 수 없다. ChatGPT desktop app 또는 Codex CLI의 실제 MCP host를 사용한다.

| ID | 순서 | 완료 증거 |
|---|---|---|
| MCP-C001 | Codex 시작→Gateway 연결→search·describe core action | 12개 고정 list와 실제 결과 |
| MCP-C002 | Codex·Gateway·Controller PID 기록→새 user TOML 저장→같은 작업 search·describe·invoke | PID 불변, 새 tool 성공 |
| MCP-C003 | personal_auto user TOML absolute path 변경→같은 작업 invoke | audit trust 갱신, 재시작 없이 새 identity |
| MCP-C004 | follow_path EXE 교체→stale→재-describe→invoke | 이전 hash 거부, 새 hash 성공 |
| MCP-C005 | invalid TOML 저장 | 같은 작업에서 LKG 실행·status 진단 |
| MCP-C006 | paid fake action | 실제 side effect 전 approval_required |
| MCP-C007 | long fake action→Operation 조회·취소 | progress·terminal cancel evidence |
| MCP-C008 | Gateway 강제 종료·Codex reconnect | Controller Operation·Registry 유지 |

각 증거에는 timestamp, 세 PID, MCP server version, Registry revision, descriptor·EXE hash와 restart count를 남긴다. 실제 유료·원격·파괴 동작은 사용하지 않는다.

구현자 evidence의 실제 Codex CLI 결과는 `apps/star-mcp/tests/evidence/codex-same-session-v1.json`에 정규화돼 있고 old raw hash와 일치한다. C001~C007은 한 Codex turn과 동일한 Codex·Gateway·Controller PID, C008은 동일 task resume 기록이다. 다만 이 JSON은 현재 수정 release hash를 가리키지 않는다. 독립 감사에서 current actual Codex C001 release-ready core search는 0개를 반환했으므로 current C001~C008 성공 evidence로 사용하지 않는다. `tools/codex-e2e-fixture.ps1`은 네트워크·유료·원격 동작 없이 fixture 변경과 PID 증거를 재현한다.

## 공식 MCP Inspector 실제 Windows smoke

`tools/mcp-inspector-fixture.ps1`은 exact 0.22.0 npm package-lock integrity와 Node host를 확인하고 새 격리 RunRoot에서 공식 CLI STDIO mode를 실행한다. 고정 tool 순서·title·description·annotation, fully resolved input/output Schema, forbidden capability 부재와 Inspector→Gateway→authenticated Controller IPC를 검증한다. Inspector 0.22.0의 상대 `package.json` 탐색은 호출자 CWD에 잘못 의존하므로 설치 tree를 바꾸지 않고 package의 `cli/build`를 working directory로 지정한다. 이 workaround는 product cwd를 바꾸지 않으며, 독립 감사의 current 실패는 그 뒤 core-ready assertion에서 발생했다.

## 관리 CLI 실제 Windows smoke

`tools/mcp-management-cli-fixture.ps1`은 새 격리 RunRoot에서 release `star.exe`로 validate→trust→list/describe→probe→revoke→scaffold/validate→Controller start/status를 실행한다. 이어서 처음에 없던 exact HKCU `Star-Control` Run 값을 두 번 enable해 idempotency와 owned `REG_SZ` command를 확인하고, 두 번 disable한 뒤 다시 없음까지 검증한다. 정규화 결과는 `apps/star-cli/tests/evidence/management-cli-smoke-v1.json`에 고정하며 fixture 종료 뒤 원래 HKCU 상태를 복구한다.

## 실제 Windows ARM64 smoke

`tools/mcp-arm64-smoke.ps1`과 `.github/workflows/mcp-windows-arm64.yml`은 native GitHub `windows-11-arm` runner용 gate다. 정규화 기록은 `apps/star-mcp/tests/evidence/mcp-arm64-native-smoke-v1.json`에 있다. full run `29188151232`은 workspace test·clippy·drift·release build가 통과했지만 smoke는 `IPC server identity does not match the installed Controller`로 실패했다. 당시 error mapping은 outer Job denial과 identity failure를 구분하지 못했으므로 raw 문구만으로 outer Job 원인을 확정하지 않는다. 후속 `29188629756`은 prestarted Controller의 pipe PID·설치 image·HMAC 검증과 native external EXE를 통과했지만 Gateway verified fallback start coverage가 아니다. 두 run을 하나의 성공 full gate로 합치지 않는다.

관측 환경은 실제 Arm64 PE machine `0xaa64`, Windows 11 25H2 build 26200.8655다. 계약의 최소 build 26100보다 새 환경임은 증명하지만 exact 24H2 baseline 실행을 대체하지 않으며, evidence의 `exact_24h2_baseline_executed`는 명시적으로 false다.

## Fuzz·property 검사

- TOML parser와 local `$ref` resolver: valid seed 기반 arbitrary byte
- length-prefixed IPC frame: length·truncation·UTF-8·JSON mutation
- JSON-STDIO line protocol: frame order·duplicate final·sequence mutation
- Windows CRT argument encoder: arbitrary Unicode OsString round-trip fake child
- Registry state machine: event sequence property test
- JCS hash: key order·set order property

PR gate는 각 target 60초 bounded fuzz smoke, release gate는 각 target 10분이다. crash·panic·allocation 상한 초과 corpus를 회귀 fixture로 승격한다.

## 성능·resource gate

Windows 11 24H2, 4-core·8 GiB 기준:

| 항목 | 기준 |
|---|---:|
| Gateway cold initialize p95, Controller ready | 2초 이하 |
| verified Controller fallback start p95 | 5초 이하 |
| 이미 실행 중 Controller IPC connect p95 | 250 ms 이하 |
| 512 action search p95 | 100 ms 이하 |
| describe p95 | 50 ms 이하 |
| unchanged invoke preflight p95 | 250 ms 이하, child runtime 제외 |
| stable TOML save→search 반영 p95 | 1초 이하 |
| Gateway working set idle | 64 MiB 이하 |
| Controller Registry 512 action 추가 working set | 128 MiB 이하 |
| progress notification | 초당 최대 4개/Operation |

현재 release 바이너리의 반복 측정 원본과 budget assertion은
[`mcp-performance-v1.json`](../../apps/star-mcp/tests/evidence/mcp-performance-v1.json)과
[`performance_evidence.rs`](../../apps/star-mcp/tests/performance_evidence.rs)에 고정한다.
[`mcp-performance-fixture.ps1`](../../tools/mcp-performance-fixture.ps1)은 격리된 release 설치본에서 30회 latency 표본, 512-action pagination, process 전 lane 차단과 working set을 다시 측정해 JSON을 stdout으로 출력한다. 체크인 증거 파일은 측정 결과를 검토한 뒤 별도로 갱신하며 이 도구가 직접 덮어쓰지 않는다.

성능 실패를 해결하기 위해 hash·trust·Schema·permission 검사를 생략할 수 없다.

## 완료 판정

MCP 구현 완료는 다음이 모두 참일 때만 선언한다.

1. 모든 matrix ID가 자동 통과하거나 명시된 실제 Codex smoke 증거를 가짐
2. fixed tools/list·Schema·annotation golden drift 없음
3. `star-control-core.toml`과 fake argv·JSON-STDIO package E2E 통과
4. 새 TOML·path·same-path EXE 교체가 PID 변화 없이 반영됨
5. invalid candidate·watcher overflow·crash 복구 통과
6. secret·path·risk lane·approval·AppContainer 보안 검사 통과
7. x64·ARM64 build와 Windows 11 24H2 smoke 통과
8. Schema·reference docs와 CLI help가 generated output과 일치
9. 미실행·flaky·quarantined test 0개
10. Sol Max 독립 최종 검토에서 blocker 0개

## 연결 문서

- [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)
- [ToolPackageManifest Reference](../contracts/tool-package-manifest-reference.md)
- [Windows Tool Runtime](../architecture/windows-tool-runtime.md)
- [오류와 진단](../contracts/errors-and-diagnostics.md)
