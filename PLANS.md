# PLANS.md

## 목적

이 문서는 새 Star-Control의 현재 작업 상태를 짧게 유지하는 원장이다. 상세 설계는 docs/에 두고 이 문서에는 현재 판단과 다음 조치만 남긴다.

## Context Pack

### 현재 목표

- adapter가 직접 소비할 validation·gate·evidence·diagnostic 공개 계약을 `star-contracts`에 구현하고 schema와 불변식을 검증한다.

### 반드시 지켜야 할 제약

- Codex 전용, Windows 전용이다.
- 로컬 AI, 다른 AI 제공자, OpenAI API 직접 호출을 제외한다.
- 사용자 입력 화면은 Codex 앱이고 제품 조작 화면은 터미널이다.
- 브라우저 UI와 자체 예약 실행 기능을 만들지 않는다.
- legacy/는 로컬 읽기 전용이며 새 설계의 기준이 아니다.
- 코드는 문서 설계가 확정된 뒤 작성한다.
- 모든 최종 범위 기능을 구현 대상으로 삼되 단계적으로 완성한다.

### 이미 끝난 것

- 기존 프로젝트를 `legacy/`에 보존하고 새 문서만으로 이해되는 설계 구조로 전환했다.
- 레거시 163개 기능과 외부 개발·검증 자료를 판정해 23개 구현 기능과 15개 작업 Profile로 확정했다.
- 3개 실행 파일, 21개 내부 Package와 기능·Profile 소유권을 repository 구조에 배치했다.
- RouteDecision 분리와 전체 데이터·설정 정본을 ADR-0001·0002에 고정했다.
- 상태·검증·MCP·IPC·오류·migration과 외부 Tool Registry를 포함한 현재 계약 Inventory 49개 항목을 확정했다.
- 고정 generic MCP surface, 여섯 risk lane, Controller 단일 live Registry, watcher+demand scan, descriptor hash와 EXE update policy를 ADR-0004에 고정했다.
- MCP contract v1의 고정 12 tool, exact TOML 문법, authenticated IPC, Win32 process·격리, 170개 검증 matrix를 ADR-0005에 고정했다.
- 41개 Markdown, 49개 계약 Inventory 항목, TOML·JSON 예제, 내부 링크·anchor, 표 형식, 170개 matrix ID, `git diff --check`와 `legacy/` 무변경을 검증했다.

### 아직 남은 것

- P-0011 Star 공개 검증·증거 계약 구현과 검증

### 건드리면 안 되는 것

- legacy/
- 사용자가 이동한 기존 파일
- 원격 저장소와 외부 계정

### 먼저 확인할 파일

- docs/README.md
- docs/contracts/goal-and-stage.md
- docs/contracts/routing.md
- docs/contracts/config-and-catalog.md
- docs/contracts/README.md
- docs/contracts/external-tool-registry.md
- docs/contracts/mcp-implementation-contract.md
- docs/contracts/tool-package-manifest-reference.md
- docs/contracts/events-and-state.md
- docs/contracts/mcp-tools.md
- docs/contracts/local-ipc.md
- docs/architecture/windows-tool-runtime.md
- docs/testing/mcp-verification-matrix.md
- docs/contracts/errors-and-diagnostics.md
- docs/contracts/versioning-and-migrations.md
- docs/architecture/repository-layout.md
- docs/decisions/ADR-0001-최종-설계-기준.md
- docs/decisions/ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md

### 먼저 실행할 명령

- 문서 링크 검사
- 금지 기능 용어 검사
- git diff --check

### 현재 차단 요소

- 없음

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|
| P-0011 | 진행 중 | ValidationRun·GateDecision·EvidenceBundle·Diagnostic 공개 계약과 고정 schema를 제공 | crates/foundation/star-contracts/src/evidence.rs, specs/schemas/v1 | 전체 locked gate와 독립 변경 검토 |

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | Codex 제품 기능과 설정 이름은 앞으로 바뀔 수 있음 | 고정된 모델 이름과 실행 방식이 낡을 수 있음 | 실행 시 App Server에서 지원 기능을 조회하도록 설계 |
| R-0002 | Plugin과 Hook은 사용자가 끄거나 신뢰하지 않을 수 있음 | 모든 작업이 Star-Control을 거친다는 보장이 약해짐 | 활성화·신뢰 상태를 시작 전 검사하고 실패 시 닫힌 상태로 중단 |
| R-0003 | 공개 배포용 기본 승인 정책과 개인용 자동 정책이 다름 | 공개 사용자에게 과도한 자동 권한을 줄 수 있음 | 안전 기본값과 개인 자동 프로필을 분리 |
| R-0005 | 외부 EXE 등록은 임의 코드 실행 경계를 넓힘 | 악성 프로젝트 설정·경로 바꿔치기 위험 | manifest·실행 파일 hash, trust store, project 설정 격리 |
| R-0006 | generic invoke는 실제 도구별 MCP Schema·annotation을 직접 노출하지 못함 | 모델 입력 실수와 승인 표시 약화 | describe hash와 고정 risk lane별 invoke tool로 보완 |
| R-0007 | Windows watcher 알림 유실·편집 중 파일·같은 path 교체가 race를 만들 수 있음 | 이전·새 descriptor와 EXE가 섞일 수 있음 | demand scan, stable-file 확인, immutable snapshot과 실행 lease 회귀 검사 |
| R-0008 | `rmcp`·Codex MCP host 동작은 update될 수 있음 | 고정 Gateway conformance가 새 version에서 깨질 수 있음 | dependency exact pin, protocol golden과 실제 Codex E2E 뒤에만 update |
| R-0009 | AppContainer adapter·ARM64 경계는 아직 제품 code와 실제 Windows 환경에서 실행되지 않음 | 설계와 OS 실제 동작 차이가 구현 때 드러날 수 있음 | P1에서 fake adapter, x64·ARM64와 loopback·path escape matrix를 먼저 통과 |

## 다음 작업 시작점

- 공개 계약 전체 locked gate와 generated schema drift 검사
- `not_run` 비통과 및 adapter 완료 판정 재계산 금지 회귀 확인
- 변경 독립 검토와 PR CI

### P-0010 완료 검증 상태

- 시작 전: `git status --short`에서 대규모 기존 사용자 변경(이전 제품 파일의 삭제·문서 재구성)을 확인했다. 해당 변경은 되돌리거나 `legacy/`를 수정하지 않는다.
- 시작 전: `git diff --check` 통과.
- 완료: `star-contracts` MCP·IPC·Registry·Manifest·Runtime type, JCS/SHA-256, fixed 12 tool metadata, JSON Schema generator, valid·invalid·IPC compatibility fixture를 추가했다.
- 완료: `star-ipc`에 8 MiB bounded frame과 HMAC challenge-response primitive를 추가했다. Named Pipe/DPAPI/PID identity는 다음 수직 slice에서 Windows adapter로 구현한다.
- 통과: `cargo fmt --check`; `cargo test -p star-contracts -p star-ipc` (8 tests); `cargo run -p star-schema-gen`; `cargo test -p star-contracts`; `git diff --check`.
- 완료: `rmcp = 2.2.0` STDIO-only `star-mcp`을 추가하고 Controller 추상화로 고정 12 tool을 IPC command로만 변환하도록 구현했다. TOML·EXE·Registry state를 읽지 않는다.
- 통과: `cargo test -p star-contracts -p star-ipc -p star-mcp` (10 tests). 실제 STDIO `initialize`에서 `2025-11-25`, 고정 serverInfo·instructions·tools capability만 응답하는 것을 확인했다.
- 완료: `list_tools`를 직접 구현해 계약 표의 12개 순서를 고정하고 non-empty cursor를 `-32602`으로 거부했다. 실제 STDIO JSON-RPC smoke에서 두 동작을 확인했다.
- 완료: Local IPC frame bound를 정본의 8 MiB로 정정하고 zero-length frame을 닫도록 했다. HMAC input도 단순 field join 대신 문서의 JCS challenge·unsigned hello/welcome 방식으로 변경했다.
- 완료: `star-ipc`에 Windows `CreateNamedPipeW` transport를 추가했다. byte pipe, `PIPE_REJECT_REMOTE_CLIENTS`, 16 instance, 64 KiB buffer와 owner+LocalSystem DACL을 raw `SECURITY_ATTRIBUTES`로 적용하며 실제 same-user pipe round-trip test를 통과했다.
- 완료: `star-ipc`에 UI 없는 current-user DPAPI protect/unprotect를 추가하고 실제 Windows user profile round-trip을 검증했다. key file atomic persistence·key rotation과 PID/image identity는 다음 IPC session slice에 남아 있다.
- 완료: DPAPI key file의 `%LOCALAPPDATA%\\Star-Control\\secrets\\ipc-key.v1` 경로 계산, 32-byte 검증, sibling temp-file flush 후 atomic replace, existing corrupt blob fail-closed를 추가했다. key-file DACL과 live Controller rotation 판단은 session/Controller slice에 남아 있다.
- 완료: one-shot `ServerHandshake`를 추가해 fixed `1.0` negotiation, challenge nonce echo, JCS client HMAC, authenticated Welcome과 server HMAC, replay rejection을 test로 검증했다. 5초 expiry·PID/image identity·pipe request loop는 다음 slice에 남아 있다.
- 완료: challenge expiry를 wire clock이 아닌 process-local monotonic `Instant`로 5초 강제했고 만료 fixture를 추가했다. PID/image identity·pipe request loop는 다음 slice에 남아 있다.
- 완료: `GetNamedPipeServerProcessId`와 `QueryFullProcessImageNameW` 기반 pipe server PID·image verification helper를 추가하고 현재 process image Windows smoke를 통과했다. pipe request loop와 Controller composition은 다음 slice에 남아 있다.
- 완료: `apps/star-controller` composition root를 추가했다. DPAPI key·DACL pipe·authenticated handshake 뒤 typed IPC request를 단일 writer Controller에서 수신하며, 아직 구현되지 않은 command는 `CONTROLLER_HANDLER_UNAVAILABLE`로 fail-closed 한다. Registry/Runtime handler 연결은 다음 slice다.
- 완료: contract SID SHA-256 앞 16 hex endpoint 생성과 Gateway-owned authenticated IPC client를 추가했다. client는 server PID/image, challenge protocol·nonce, JCS HMAC Welcome과 request/response ID·correlation binding을 확인하며, Gateway error를 `IPC_CONTROLLER_UNAVAILABLE`·`IPC_AUTH_FAILED`·`IPC_SERVER_IDENTITY_MISMATCH`·`IPC_PROTOCOL_MISMATCH`로 정규화한다.
- 통과: `cargo fmt --check`; `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-controller` (19 tests); `cargo build -p star-mcp -p star-controller`; `git diff --check`. 실제 Windows same-session에서 `star-mcp.exe`의 `star_tool_registry_status`가 SID-hash pipe의 `star-controller.exe`와 인증 후 `status=ok`으로 응답했다.
- 완료: Controller가 Hello `client_pid`와 pipe peer PID를 대조하고 installed `star-mcp.exe`/`star.exe` image만 허용하도록 했으며 `Local\Star-Control.Controller.<sid-hash>.v1` mutex로 단일 writer를 보장한다. 실제 second Controller process가 exit 0으로 종료하는 smoke를 통과했다.
- 완료: demand scan을 재귀·deterministic scan으로 확장했다. duplicate PackageId는 source precedence로 덮지 않고 conflict로 차단하며, invalid same-file 교체만 LKG를 유지하고 delete·disabled·rename은 active를 제거한다. package-only JCS snapshot hash와 diagnostic revision을 추가했고 MCP registry/search 결과가 snapshot hash·action ID·risk lane·descriptor hash를 반환하도록 보강했다.
- 통과: `cargo fmt --check`; `cargo test -p star-controller` (3 tests: LKG/delete, duplicate conflict, stable snapshot); `git diff --check`.
- 완료: `IpcHandshakeError` contract·generated schema를 추가하고, common protocol이 없을 때 client nonce에 묶인 server HMAC `IPC_PROTOCOL_MISMATCH` wire를 반환·검증하도록 했다.
- 통과: `cargo run -p star-schema-gen`; `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-controller` (22 tests); `git diff --check`.
- 완료: Controller internal direct-EXE adapter를 추가했다. absolute `.exe`·existing working directory만 받고 `env_clear`/typed argv로 shell·PATH·script host를 배제하며 stdin close, stdout·stderr 동시 drain, hard output cap 이후 drain, strict UTF-8/UTF-16LE decode, timeout kill 및 artifact relative-path 검사를 수행한다. `star_json_stdio_v1` request/result typed contract와 4 MiB argument·8 MiB line·single final-result validation도 추가했다.
- 의존성: user가 허용한 Rust dependency 범위에서 `tokio`의 `process` feature를 활성화했다. lockfile의 `errno`, `signal-hook-registry`는 해당 async process support의 transitive resolution이다.
- 통과: `cargo run -p star-schema-gen`; `cargo test -p star-contracts -p star-controller` (14 tests); `cargo fmt --check`; `git diff --check`. 생성 전 schema drift test가 한 번 실패했으나 generator 실행 후 재검증해 통과했다.
- 완료: Controller package를 reusable library로 분리하고 test-only `star-fake-exe`를 추가했다. 실제 child EXE로 typed argv 전달, fake JSON-STDIO final response binding, output flood drain/limit을 integration test로 검증했다.
- 통과: `cargo test -p star-controller` (11 tests, fake argv·JSON-STDIO·flood 포함); `git diff --check`.
- 완료: `star` 관리 CLI를 별도 package로 추가했다. 고정 `tools list|describe|status|validate|probe|trust|revoke|scaffold`, `controller start`, `controller autostart enable|disable|status` syntax/help를 parse하고 authenticated Controller IPC만 사용한다. Controller에는 static manifest validation command를 연결했다.
- 통과: `cargo test -p star-cli -p star-controller` (12 tests); `cargo build -p star-controller -p star-cli`. 실제 same-session `star.exe tools status --json`과 valid fixture 대상 `star.exe tools validate <path> --source user --json`이 authenticated Controller IPC `status=ok`으로 응답했다.
- 완료: candidate manifest hash에 정확히 묶이는 durable `ToolTrustRecord`/TrustStore를 추가했다. trust expiry·candidate hash mismatch를 fail-closed로 처리하고 revoke는 record를 제거해 즉시 새 invoke readiness를 `untrusted`로 되돌린다. Controller의 `tool.trust`·`tool.revoke`와 Registry status/search readiness에 연결했다.
- 통과: `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-cli -p star-controller` (32 tests); `cargo run -p star-schema-gen`; generated Schema contract 재검증; `git diff --check`.
- 완료: Registry action lookup과 manifest candidate-bound descriptor hash를 추가했고 Controller `tool.describe`가 live descriptor·required lane·trust/readiness를 반환한다. `tool.invoke`는 Runtime 이전에 trust→descriptor hash→MCP lane 순으로 검사해 `TOOL_EXECUTABLE_UNTRUSTED`, `TOOL_DESCRIPTOR_STALE`, `TOOL_LANE_MISMATCH`를 fail-closed 한다. MCP actor에 fixed call-tool 이름을 전달하도록 IPC client를 확장했다.
- 통과: `cargo test -p star-controller -p star-mcp` (15 tests; descriptor live candidate 변경 포함); `git diff --check`.
- 완료: `ArgvBinding` contract를 `flag`, condition, joined, stdin JSON/text fields까지 확장하고 typed argv/stdin builder를 추가했다. authorized `argv_v1` invoke는 backend reference, pinned absolute executable, launch 직전 SHA-256 재검사, typed binding, timeout/output cap, exit/output encoding을 통과해야 direct Runtime으로 실행된다. non-pinned·JSON-STDIO backend는 아직 fail-closed 한다.
- 통과: 관련 workspace test 실행 중 Schema generator 전 `tool-package-manifest.schema.json` drift가 한 번 검출되었고, `cargo run -p star-schema-gen` 뒤 `cargo test -p star-contracts --test contracts`를 다시 실행해 통과했다. 나머지 `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-cli -p star-controller`, `git diff --check`도 통과했다.
- 완료: authorized `star_json_stdio_v1` invoke도 current descriptor hash와 per-request Operation ID를 포함한 typed frame으로 Runtime에 연결했다. IPC key atomic store 뒤에는 current owner+LocalSystem만 허용하는 protected DACL을 적용한다.
- 통과: `cargo test -p star-ipc` (11 tests; actual DPAPI/key store round-trip 포함); `git diff --check`.
- 완료: Controller-only autostart lifecycle을 추가했다. `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`의 `Star-Control` value에 quoted absolute `star-controller.exe --background` exact value만 enable/disable/status하며 foreign value는 conflict로 보존한다. service/Task Scheduler를 쓰지 않는다.
- 통과: `cargo test -p star-controller -p star-cli` (16 tests); `cargo build -p star-controller -p star-cli`. 실제 hidden Controller와 `star.exe controller autostart status --json` IPC smoke가 `state=disabled`/`status=ok`으로 응답했다.
- 완료: `tool.search`에 deterministic score→ToolId ordering, snapshot/query hash-bound base64url cursor, next cursor와 `TOOL_SEARCH_CURSOR_STALE` snapshot invalidation을 추가했다. search cursor encoding/decoding unit test를 넣었다.
- 통과: `cargo test -p star-controller` (16 tests); `git diff --check`.
- 완료: Registry demand scan이 candidate file metadata(size·last-write)를 250 ms 간격으로 두 번 확인한다. 저장 중 candidate는 `TOOL_MANIFEST_STABILIZING`으로 기록하고 이전 LKG를 유지한다.
- 통과: `cargo test -p star-controller registry_runtime -- --nocapture` (5 tests; concurrent write stabilizing/LKG 포함); `git diff --check`.
- 완료: Runtime executable lease가 `CreateFileW(GENERIC_READ, FILE_SHARE_READ)`로 final absolute EXE를 열어 write/delete share를 거부하고, pinned hash 재검사부터 child Runtime 완료까지 handle을 유지한다.
- 통과: `cargo test -p star-controller -p star-ipc` (26 tests); current executable lease Windows smoke; `git diff --check`.
- 완료: Runtime child마다 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION` Job Object를 생성하고 process handle assignment가 실패하면 unjobbed 실행 대신 fail-closed 한다.
- 통과: `cargo test -p star-controller process_runtime -- --nocapture` (7 tests; direct EXE path now Job assigned); `git diff --check`.
- 남은 제한: Tokio public launch는 `CREATE_SUSPENDED`를 노출하지 않아 Job assignment가 spawn 뒤에 일어난다. suspended-before-first-child invariant은 dedicated `CreateProcessW` launcher로 교체할 때까지 미완료다.
- 완료: `tool.scaffold` Controller command가 EXE를 실행하지 않고 byte SHA-256을 계산해 `enabled=false`, `pinned_hash`, action 0개 manifest draft를 sibling temp-file flush+atomic rename으로 생성한다. existing output은 덮어쓰지 않는다.
- 통과: `cargo test -p star-controller` (18 tests); `cargo fmt --check`; `git diff --check`.
- 완료: `star-matrix-check` release-gate tool을 추가했다. 정본 matrix 170 ID를 actual Rust test source의 `// matrix: MCP-*` marker와 대조하며, missing ID가 있으면 non-zero로 실패한다. 기존 증거가 있는 24개 ID를 marker로 연결했다.
- 통과: `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-controller` (35 tests); `git diff --check`.
- 완료: matrix gate가 marker가 실제 `#[test]` 또는 `#[tokio::test]` 함수 바로 앞에 붙어 있는지를 확인하도록 강화했다. 단순 주석의 false-positive mapping은 허용하지 않는다.
- 완료: Manifest validation이 unsafe raw shell/script/PATH-style locator, invalid locator field combination, probe 없는 `version_compatible`, missing parameter binding, 복수 stdin binding 및 overlapping exit code set을 parse 단계에서 거부하도록 보강했다.
- 통과: `cargo test -p star-contracts --test contracts` (7 tests; M007/M008/M009/M012/M013/M014 regressions); `cargo run -p star-schema-gen`; generated Schema 재검증; `git diff --check`.
- 미완료 증거: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=24`, 146 missing으로 exit 1이다. 이 실패는 숨기지 않으며 전체 완료 gate가 아직 열려 있음을 뜻한다.
- 완료: Controller Registry root를 정본 경로로 정정했다. release는 `<install>\\catalog\\tool-packages`, user는 `%APPDATA%\\Star-Control\\tools.d`, project는 현재 project의 `.star-control\\tools.d`를 각각 독립 source로 scan한다. project source가 active snapshot으로 들어가는 회귀 검사(`MCP-R001`)를 추가했다.
- 통과: `cargo test -p star-controller --lib` (15 tests), `cargo test -p star-controller --test process_runtime` (3 tests), `cargo fmt --check`, `git diff --check`.
- 완료: Windows Runtime은 public Tokio spawn 대신 narrow `CreateProcessW` launcher를 사용한다. stdin/stdout/stderr 세 pipe end만 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`에 넣고 `CREATE_SUSPENDED`로 만들며, Operation Job 할당과 reader 준비 뒤에만 `ResumeThread`한다. timeout/launch failure는 Job-bound process를 종료하고 minimal environment는 `SystemRoot`, `WINDIR`, `TEMP`, `TMP`와 명시 allowlist만 전달한다.
- 통과: `cargo test -p star-controller --test process_runtime` (4 tests, fake argv/JSON-STDIO/flood와 `MCP-P002` oversized command line 사전 거부); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=26`, 144 missing으로 exit 1이다.
- 통과: `cargo run -p star-schema-gen`; `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-controller` (40 tests); `cargo fmt --check`; `git diff --check`.
- 완료: `RegistryWatcher`가 source root마다 `ReadDirectoryChangesW`를 사용해 file name/directory/size/last-write 변경을 invalidation으로 전달한다. overflow 또는 watcher 오류는 full demand scan과 unavailable 진단으로 처리하며, Controller가 시작 뒤 생긴 user/project root도 다음 request에 watcher를 등록한다. event는 증분 mutation이 아니므로 LKG와 atomic snapshot 경계는 demand scan이 계속 소유한다.
- 통과: `cargo test -p star-controller --lib` (16 tests, 실제 Windows directory-change smoke `MCP-R003` 포함); `cargo check -p star-controller`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=27`, 143 missing으로 exit 1이다.
- 완료: Controller-private durable `OperationStore`를 추가했다. 상태 전이는 `received → resolving/queued → starting → running → terminal`로 제한하고, restart 중 nonterminal record는 자동 재실행 없이 `outcome_unknown`으로 고정한다. 반복 cancel은 하나의 intent만 남기며 terminal 결과를 바꾸지 않는다. `accepted` 또는 detachable invoke는 background Runtime과 연결되고 `operation.get`/`operation.cancel`은 IPC control surface에서 durable snapshot·event를 조회한다.
- 제한: 현재 cancel은 durable intent를 기록하고 cancellable 상태를 `cancelling`으로 바꾸지만 Runtime process handle에 연결된 강제 종료는 아직 구현하지 않았다. approval 대기 record/resolve state machine도 아직 없으며 `approval.resolve`는 stale scope를 명시적으로 반환한다.
- 통과: `cargo test -p star-contracts -p star-ipc -p star-mcp -p star-controller` (43 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=29`, 141 missing으로 exit 1이다.
- 완료: fake EXE Runtime 회귀에 `MCP-P006` timeout(Job-bound process termination)과 `MCP-P015` JSON-STDIO garbage result 거부를 추가했다.
- 통과: `cargo test -p star-controller --test process_runtime` (6 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=31`, 139 missing으로 exit 1이다.
- 완료: `MCP-P003` stdin 미사용 시 child가 EOF를 받고 hang하지 않는 실제 fake EXE 회귀를 추가했다.
- 통과: `cargo test -p star-controller --test process_runtime` (7 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=32`, 138 missing으로 exit 1이다.
- 완료: `RuntimeCancellation`을 Controller Operation token map과 연결했다. `operation.cancel`은 cancellable argv Operation의 Job-bound Win32 child tree를 종료하며, `TOOL_CANCELLED` 결과는 durable Operation의 `cancelled`와 `cancel_effective=true`로 commit된다.
- 제한: `star_json_stdio_v1` adapter의 cancellation frame/forced Job termination과 approval 대기 state machine은 아직 미완료다.
- 완료: `star_json_stdio_v1`도 shared `RuntimeCancellation`을 Win32 launcher에 전달해 cancel 시 Job-bound adapter process를 강제 종료한다. protocol-level optional cancel ack frame은 아직 구현하지 않았지만 forced termination과 terminal Operation 기록은 공통이다.
- 통과: `cargo test -p star-controller --test process_runtime` (9 tests, argv `MCP-P017` 및 JSON-STDIO `MCP-P016` cancel 포함); `cargo check -p star-controller`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=34`, 136 missing으로 exit 1이다.
- 완료: manifest parser가 `paid_action=yes|unknown`과 permission `paid_action`의 동반 선언, `paid_action=no`의 금지를 fail-closed로 검사한다. 이 policy 불일치 검증의 matrix marker를 실제 test attribute에 연결했다.
- 완료: fake child가 stdout/stderr를 동시에 flood하는 `MCP-P004`를 추가했고, 두 stream을 끝까지 drain한 뒤 hard limit으로 거부하는 것을 확인했다.
- 통과: `cargo run -p star-schema-gen`; `cargo test -p star-contracts --test contracts` (7 tests); `cargo test -p star-controller --test process_runtime` (9 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=42`, 128 missing으로 exit 1이다.
- 완료: asynchronous invoke의 idempotency conflict·Operation store 오류가 Controller process를 종료하지 않고 request-local `STATE_IDEMPOTENCY_CONFLICT` 또는 `OPERATION_STORE_UNAVAILABLE`로 반환되도록 보정했다.
- 통과: `cargo check -p star-controller`.
- 완료: 문서에 정의된 Action `expected_duration_ms`(기본 1000)를 manifest type과 generated Schema에 추가했다. `wait_mode=auto`이고 값이 30초를 초과하면 synchronous budget을 넘기지 않고 durable Operation 경로를 사용한다.
- 통과: `cargo run -p star-schema-gen`; `cargo test -p star-contracts --test contracts` (7 tests); `cargo check -p star-controller`; `cargo fmt --check`; `git diff --check`.
- 완료: parser가 process action의 `process_run`, output, executable backend reference, protocol별 argv/exit-code/cancel-mode, JSON/JSONL output Schema, parameter/input schema 배타, `expected_duration_ms` 상한을 fail-closed로 검사한다.
- 통과: `cargo test -p star-contracts --test contracts` (7 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=43`, 127 missing으로 exit 1이다.
- 완료: fake EXE가 실제 child `sleep` process를 생성한 뒤 cancellation Job Object가 parent와 child tree를 함께 종료하는 `MCP-P005` Windows PID wait 회귀를 추가했다.
- 통과: `cargo test -p star-controller --test process_runtime` (10 tests).
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=44`, 126 missing으로 exit 1이다.
- 완료: unknown permission ActionId를 manifest parser에서 거부하는 `MCP-M011` fixture mutation regression을 추가했다.
- 통과: `cargo test -p star-contracts --test contracts` (7 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=45`, 125 missing으로 exit 1이다.
- 완료: Registry descriptor와 snapshot hash가 raw TOML source hash가 아니라 parsed semantic manifest hash를 사용하도록 수정했다. raw hash는 trust/provenance 용도로 별도 유지하며 whitespace-only candidate 변경은 stale/snapshot churn을 만들지 않는다.
- 통과: `cargo test -p star-controller --lib` (19 tests, `MCP-H015` 포함); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=46`, 124 missing으로 exit 1이다.
- 완료: semantic hash가 backend kind·architecture·isolation·alias·tag·task kind·permission의 set order를 정규화하고 argv/parameter/example 등 실행 순서는 보존하도록 보강했다. `MCP-H003`은 permission order 변경에도 descriptor/snapshot이 유지됨을 검증한다.
- 통과: `cargo test -p star-controller --lib` (20 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=47`, 123 missing으로 exit 1이다.
- 완료: argv binding 순서 변경이 descriptor hash를 반드시 바꾸는 `MCP-H004` 회귀를 추가해 set 정규화가 process argument contract를 약화하지 않음을 확인했다.
- 통과: `cargo test -p star-controller --lib` (21 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=48`, 122 missing으로 exit 1이다.
- 완료: Controller invoke가 process/Operation 생성 전에 inline parameter contract를 검증한다. referenced input Schema는 resolver가 구현되기 전 parameter validation으로 대체하지 않고 fail-closed이며, unknown/missing/type/length/number/array bound 위반을 거부한다.
- 보정: `MCP-P005` child-tree 회귀가 종료된 PID의 `OpenProcess` race에서 flaky하지 않도록, handle을 얻으면 signaled wait를 확인하고 handle이 없으면 이미 종료됨으로 판정한다.
- 통과: `cargo test -p star-controller` (33 tests); `cargo fmt --check`; `git diff --check`.
- 완료: `tool.registry.status`가 package/source filter, diagnostics 선택, 1..200 limit와 base64url JCS status cursor를 처리한다. cursor는 registry/diagnostic revision 및 filter hash에 결합되고 변경 시 `TOOL_REGISTRY_CURSOR_STALE`로 거부한다.
- 통과: `cargo check -p star-controller`; `cargo test -p star-controller --bin star-controller` (3 tests, `MCP-R021` 포함); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=49`, 121 missing으로 exit 1이다.
- 완료: invalid 신규 TOML candidate가 diagnostic만 남기고 같은 source의 다른 valid active package를 유지하는 `MCP-R007` 회귀를 추가했다.
- 통과: `cargo test -p star-controller --lib` (22 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=50`, 120 missing으로 exit 1이다.
- 완료: active Registry snapshot을 protected atomic `registry-cache.v1.json`으로 저장·복구해 restart 뒤 invalid source에서도 LKG를 유지한다. cache corrupt/write failure는 Controller 시작/요청을 중단하지 않고 Registry diagnostic으로 노출한다.
- 통과: `cargo test -p star-controller` (36 tests, durable LKG `MCP-R012` 포함); `cargo check -p star-controller`; `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=51`, 119 missing으로 exit 1이다.
- 완료: restart 뒤 manifest source가 실제 삭제된 경우 durable cache가 active package를 되살리지 않는 `MCP-R013` 회귀를 추가했다.
- 통과: `cargo test -p star-controller --lib` (24 tests); `cargo fmt --check`; `git diff --check`.
- 보정: contract type 변화로 생긴 generated Schema drift를 `cargo run -p star-schema-gen`으로 갱신했고, 이후 contracts/Controller 전체 회귀를 재실행했다.
- 통과: `cargo test -p star-contracts --test contracts` (8 tests); `cargo test -p star-controller` (37 tests); `cargo fmt --check`; `git diff --check`.
- 완료: TOML top-level key order와 CRLF 변경이 descriptor/snapshot semantic hash를 바꾸지 않는 `MCP-H002` Registry regression을 추가했다.
- 통과: `cargo test -p star-controller --lib` (25 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=52`, 118 missing으로 exit 1이다.
- 통과: `cargo test -p star-controller --lib` (18 tests); `cargo test -p star-controller --test process_runtime` (8 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=33`, 137 missing으로 exit 1이다.
- 완료: Manifest v1의 JSON-STDIO process package와 release-only core Controller command를 실제 parser regression으로 고정했다. future format, project fixed working directory, probe 없는 product version constraint를 fail-closed로 확인했고, `enabled=false`·action 0개 draft는 executable을 실행하지 않는 상태로 허용하도록 contract 예외를 구현했다.
- 보정: manifest resource `$ref:"#"` fixture의 Rust raw-string delimiter와 JSON Value type inference을 수정해 Controller resource validator가 다시 빌드되도록 했다. generated Schema는 `cargo run -p star-schema-gen`으로 갱신·검사했다.
- 통과: `cargo test -p star-contracts --test contracts` (16 tests); `cargo test -p star-controller` (40 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=67`, 103 missing으로 exit 1이다. 실제 Codex same-session C001~C008 및 approval·AppContainer·Windows 보안/복구 matrix는 아직 완료 증거가 없다.
- 완료: release Registry source용 `catalog/tool-packages/star-control-core.toml`을 추가하고, 정본의 13개 ToolId·Controller command·risk lane이 정확히 선언되는 parser regression으로 고정했다. fixed Gateway에는 tool별 handler를 추가하지 않았다.
- 통과: `cargo test -p star-contracts --test contracts` (17 tests); `cargo test -p star-mcp` (2 tests); `cargo run -p star-schema-gen -- --check`; `cargo fmt --check`; `git diff --check`; `legacy/` 변경 없음.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=68`, 102 missing으로 exit 1이다.
- 완료: external EXE identity가 바뀌면 descriptor hash가 바뀌고, diagnostic 문구·diagnostic revision은 execution snapshot hash를 바꾸지 않는 Registry identity 경계를 `MCP-H006`, `MCP-H007` 회귀로 고정했다.
- 통과: `cargo test -p star-controller --lib` (29 tests); `cargo fmt --check`; `git diff --check`.
- 미완료 증거 갱신: `cargo run -p star-matrix-check`는 `expected=170`, `mapped=70`, 100 missing으로 exit 1이다.
- 다음 작업: Win32 `ReadDirectoryChangesW` watcher와 overflow full scan을 demand scan에 연결하고, dedicated suspended CreateProcess/handle allowlist launcher를 추가한다. 이어 CLI의 남은 mutation/probe handler, IPC key rotation·verified Controller start, matrix rows를 실제 test에 순차 연결한다.

## Archive References

### P-0010 최신 상태 (2026-07-12)

- Registry는 root 단위 two-pass stability scan, temp-rename debounce, cache-restart deletion 경계, watcher overflow·event-loss demand scan, 128 package·512 action capacity를 구현·회귀 검증했다.
- Operation은 typed `OperationCreate`, exact idempotency reuse/conflict, 256-event pagination, terminal late completion 불변성을 검증했다. argv exit-code table은 empty·warning·retryable을 정규화하고 자동 EXE retry를 하지 않는다.
- 고정된 `rmcp 2.2.0`의 `client` feature를 test transport에만 사용해 Gateway 최신·`2025-06-18` initialize, fixed tools/list, cursor 거부와 ping을 duplex JSON-RPC로 검증했다. stdio supervisor는 8 MiB bounded JSONL, duplicate-key JSON 거부, initialize→initialized notification 순서, unknown tool `-32601`, fixed input/task augmentation `-32602`을 Controller 호출 전에 강제한다. `question_required`·`approval_required`는 `isError=false`, 실제 error는 `isError=true`로 정규화한다.
- 통과: `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --workspace`; `cargo run -p star-schema-gen -- --check`; `cargo fmt --check`; `git diff --check`; `legacy/` 변경 없음.
- 이후 보강: Gateway는 unsupported protocol·bounded duplicate-key JSONL·progress/cancel·fixed input을 실제 wire test로 검증했고, Runtime은 PE architecture, full invoke hash, integrity file, strict working-directory, child-only SecretRef/state directory, artifact hash/path, OEM decode와 inline-overflow artifact를 fail-closed로 보강했다. generated Schema도 artifact response type 변경에 맞춰 갱신했다.
- 이후 보강: Windows executable lease·running-image identity·Job-close termination, strict project path, fixed MCP lane/descriptor stale, SecretRef hash/provenance, inline overflow artifact, full-hash/integrity/PE architecture를 실제 Windows regression으로 추가했다.
- 이후 보강: 실행 직전 Authenticode offline/cache-only 검증을 probe·invoke에 연결했고, SecretRef가 stdout/JSON 결과에 섞이면 redaction 처리했다. artifact는 Controller-private SHA-256 reference만 공개해 절대 경로를 숨기며, `trusted_desktop`은 describe 결과에서 sandbox 아님을 명시한다. trust·cache private state는 owner+LocalSystem DACL 회귀로 다시 확인했다.
- 이후 보강: `paid_action=yes|unknown` invoke는 process 생성 전에 durable Operation을 `approval_wait`로 전이하고 ApprovalStore에 JCS scope를 기록한다. resolve는 approval ID·scope·decision 재사용, live descriptor·trust·revision을 다시 확인하며 stale이면 fail-closed한다.
- 이후 보강: ApprovalStore는 normalized arguments와 actor reference(SecretRef 원문 제외)를 controller-private state로 보존한다. approve는 arguments hash·descriptor·trust·revision을 재확인한 뒤 기존 concurrency gate와 per-Operation Job Runtime으로 재-dispatch하며, deny는 terminal `denied`로 끝낸다.
- 이후 보강: adapter-only manifest에는 `StarControl.Tool.<package SHA-256 앞 32 hex>` AppContainer profile을 선택하고, launcher가 suspended `CreateProcessW` 이전에 profile SID·capability 없는 `SECURITY_CAPABILITIES`와 loopback exemption을 확인한다. profile-local `USERPROFILE`·`LOCALAPPDATA`·`APPDATA`·`TEMP/TMP`를 제공하고, 실제 Windows fake adapter가 broker 밖 파일·127.0.0.1 loopback에 접근하지 못함을 검증했다.
- 완료 증거: WindowsApps의 실행 ACL을 우회하도록 동일 SHA-256의 번들 `codex-cli 0.144.0-alpha.4`를 격리된 `target/` install로 복사해 실제 Codex MCP host를 실행했다. C001~C007은 한 turn에서 PID 불변으로 통과했고, C008은 Gateway 강제 종료 뒤 동일 Codex task ID를 resume해 새 Gateway PID와 동일 Controller instance·Registry revision·terminal Operation을 확인했다. 정규화 증거는 `apps/star-mcp/tests/evidence/codex-same-session-v1.json`, 재현 helper는 `tools/codex-e2e-fixture.ps1`에 있다.
- 보존 증거: staged 삭제 1,167개 각각에 대응하는 `legacy/<원래 경로>`가 존재하며 `git hash-object --no-filters` raw blob ID가 `HEAD` blob과 1,167개 모두 동일하다. `legacy/` 자체의 Git diff는 없다.
- 최종 gate: `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo run -p star-schema-gen -- --check`, `cargo fmt --all -- --check`, `cargo run -p star-matrix-check` (`expected=170`, `mapped=170`, `missing=[]`), `git diff --check`, `legacy/` 무변경이 2026-07-12에 모두 통과했다.

| 항목 | 위치 |
|---|---|
| 로컬 레거시 | legacy/ |
| 새 설계 기준 | docs/README.md |
| 레거시 기능 조사 | docs/history/legacy-feature-catalogue.md |
| 최종 구현 대상 기능 | docs/features/README.md |
| 최종 repository 구조 | docs/architecture/repository-layout.md |
| D0 최종 설계 결정 | docs/decisions/ADR-0001-최종-설계-기준.md |
| 전체 데이터 계약 | docs/contracts/README.md |
| 데이터 계약·설정 결정 | docs/decisions/ADR-0002-데이터-계약과-설정-정본.md |
| 외부 Tool Registry 계약 | docs/contracts/external-tool-registry.md |
| 과거 동적 MCP Gateway 결정 | docs/decisions/ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md |
| 무재시작 고정 MCP 결정 | docs/decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md |
| MCP 구현 동결 계약 | docs/contracts/mcp-implementation-contract.md |
| ToolPackageManifest exact 문법 | docs/contracts/tool-package-manifest-reference.md |
| Windows Tool Runtime | docs/architecture/windows-tool-runtime.md |
| MCP 구현 검증 행렬 | docs/testing/mcp-verification-matrix.md |
| MCP 구현 계약 결정 | docs/decisions/ADR-0005-MCP-구현-계약-동결.md |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-07-11 | 사용자 결정과 공식 Codex 기능을 반영한 새 전체 설계 문서 작성 | docs/README.md |
| P-0002 | 2026-07-11 | 레거시 자료의 개념 기능과 근거를 범위 판단 없이 카탈로그화 | docs/history/legacy-feature-catalogue.md |
| P-0003 | 2026-07-11 | 세 자료군을 1인 개발자 효용성 기준으로 선별해 23개 구현 기능과 15개 작업 Profile로 통합 | docs/features/README.md |
| P-0004 | 2026-07-11 | 모든 구현 기능을 3개 실행 파일·21개 Package·단일 정본 문서 구조로 배치 | docs/architecture/repository-layout.md |
| P-0005 | 2026-07-11 | RouteDecision 계약 정규화, 책임별 문서 migration, D0 최종 설계 결정 기록 | docs/decisions/ADR-0001-최종-설계-기준.md |
| P-0006 | 2026-07-11 | 42개 데이터 계약 Inventory, 설정 병합·Catalog·상태·통신·오류·version 기준 확정 | docs/decisions/ADR-0002-데이터-계약과-설정-정본.md |
| P-0007 | 2026-07-11 | 외부 TOML·EXE Registry·trust·protocol 초기 계약 확정, 동적 MCP 공개 부분은 ADR-0004로 대체 | docs/decisions/ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md |
| P-0008 | 2026-07-11 | MCP·Codex 재시작 없는 고정 generic surface와 Controller live Tool Registry 상세 계약 확정 | docs/decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md |
| P-0009 | 2026-07-11 | MCP fixed wire·Manifest·Win32 runtime·보안·실제 Codex 검증 기준을 구현 계약 v1로 동결 | docs/decisions/ADR-0005-MCP-구현-계약-동결.md |
| P-0010 | 2026-07-12 | 고정 MCP Gateway·authenticated IPC·live Registry·외부 EXE Runtime·CLI를 구현하고 실제 Codex를 포함한 170개 matrix를 모두 통과 | apps/star-mcp/tests/evidence/codex-same-session-v1.json |
