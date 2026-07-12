# Windows Tool Runtime 구현 계약

## 상태와 책임

이 문서는 Controller가 Tool Registry를 감시하고 검증된 외부 EXE를 실행하는 Windows 구현 정본이다. Gateway는 이 문서의 Windows API를 사용하지 않는다.

지원 기준:

- Windows 11 24H2, build 26100 이상
- x86-64와 ARM64 native build
- local fixed volume
- user-mode per-user Controller
- 관리자 자동 elevation 없음
- UNC·SMB·WebDAV executable과 device path 실행 없음

Controller와 child EXE architecture는 일치해야 한다. ARM64 Windows의 x64 emulation과 32-bit x86 child는 v1 지원 범위가 아니며 `incompatible`로 표시한다.

## Dependency baseline

| 목적 | crate·version 기준 |
|---|---|
| Win32 API | Microsoft `windows = 0.62.2`, 필요한 feature만 |
| async runtime | Tokio current 1.x, exact Cargo.lock |
| MCP | `rmcp = 2.2.0`, Gateway에만 |
| JSON·Schema | serde 1.x, serde_json 1.x, schemars 1.x |
| TOML | toml current compatible release, exact Cargo.lock |
| hash | sha2 0.10.x |
| IPC 인증 | hmac 0.12.x + sha2, constant-time verify |
| secret memory | zeroize 1.x |
| JCS | serde_json_canonicalizer 0.3.2 |
| SemVer·probe | semver 1.x, regex 1.x with size limit |

`notify`, shell wrapper와 PowerShell watcher를 제품 runtime에 사용하지 않는다. Windows 변화 감지와 process 제어는 Win32 API adapter가 직접 소유한다.

## Process topology

```text
Codex
  -> star-mcp.exe (STDIO MCP, 상태 없음)
       -> current-user named pipe
            -> star-controller.exe (Registry·Operation 단일 writer)
                 -> 검증된 child EXE + Job Object
```

- `star-mcp.exe`는 Controller를 필요할 때 한 번 시작할 수 있지만 상태·TOML·EXE를 직접 읽지 않는다.
- Controller 한 instance가 current user의 Registry, trust, cache와 Operation을 쓴다.
- Controller는 HTTP port, public named pipe와 Windows service를 만들지 않는다.
- autostart된 Controller의 process cwd는 project identity가 아니다. 각 인증된 CLI·Gateway 요청의 final fixed-local `project_root`가 그 요청의 project `tools.d`, ProjectPathRef와 process cwd anchor를 선택하며 absolute 원문은 public evidence에 남기지 않는다.

기본 설치는 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`의 `Star-Control` 값에 quoted absolute `star-controller.exe --background` command를 등록한다. 이는 개발 작업 예약 기능이 아니라 current-user logon 때 local control plane을 시작하는 수명주기다. uninstaller는 값의 owner·설치 path가 맞을 때만 제거하고 `star controller autostart enable|disable|status`가 같은 값을 관리한다. Task Scheduler와 Windows service는 사용하지 않는다.

Gateway auto-start 순서:

1. pipe connect를 250 ms 시도한다.
2. 없으면 installer-owned install manifest의 absolute Controller path·SHA-256·version과 호출한 Gateway SHA-256을 fixed local volume의 final file handle에서 확인하고 write·delete share 없는 lease를 잡는다. v1 install manifest에는 독립 detached-signature 형식·key ID·trust anchor가 정의돼 있지 않으므로 runtime이 존재하지 않는 manifest signature를 검증했다고 주장하지 않는다. 배포 package 서명 검증은 installer 경계이고, runtime 정본은 strict manifest 형식과 설치 image의 full hash lease다.
3. explicit application path로 `CreateProcessW`를 suspended·hidden/no-window 상태로 호출하고 실제 image path를 확인한 뒤 resume한다. Gateway가 outer Job 안이면 Controller에 한해서 `CREATE_BREAKAWAY_FROM_JOB`을 요청한다. image 생성 뒤 lease를 놓는다.
4. 최대 `ipc.connect_timeout_ms=5000` 동안 authenticated pipe readiness를 기다린다.
5. 동시에 여러 Gateway가 시작해도 Controller mutex를 얻은 process 하나만 남고 나머지는 정상 종료한다.
6. readiness 실패를 일반 tool 실행으로 우회하지 않는다.

outer Job이 breakaway를 허용하지 않으면 Gateway는 Controller를 같은 kill-on-close Job 안에 조용히 시작하지 않고 `IPC_CONTROLLER_UNAVAILABLE`과 `star controller start --background` 안내를 반환한다. 외부 tool child에는 breakaway를 절대 사용하지 않는다.

## Controller single instance와 IPC 인증

### 이름

- mutex: `Local\Star-Control.Controller.<sid-hash>.v1`
- pipe: `\\.\pipe\star-control-<sid-hash>-v1`
- `sid-hash`: current user SID UTF-8의 SHA-256 앞 16 lowercase hex

### Named pipe

`CreateNamedPipeW` 설정:

- `PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED`
- `PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS`
- instance 최대 16
- input·output buffer 64 KiB
- DACL: current user SID와 LocalSystem만 full control

frame은 `u32 little-endian length + UTF-8 JSON`이며 최대 8 MiB다. frame length 0, 초과, invalid UTF-8·JSON은 연결을 닫는다.

### Challenge-response

pipe DACL만으로 다른 current-user process를 구별할 수 없으므로 per-user IPC key를 사용한다.

1. 첫 Controller가 `BCryptGenRandom`으로 32-byte key를 만든다.
2. DPAPI current-user scope로 암호화해 `%LOCALAPPDATA%\Star-Control\secrets\ipc-key.v1`에 저장한다.
3. 파일 DACL은 current user와 LocalSystem만 허용한다.
4. pipe 연결 직후 client는 `GetNamedPipeServerProcessId`로 installed image를 먼저 확인한다.
5. Controller는 fresh 32-byte server nonce, instance ID, server PID, protocol major와 발급 시각이 있는 `IpcChallenge`를 보낸다.
6. client는 fresh 32-byte client nonce와 `HMAC-SHA256(key, UTF8("client-v1\n") || JCS(IpcChallenge) || JCS(IpcHello에서 auth_tag 제외))`를 보낸다.
7. Controller는 challenge가 같은 connection에서 발급된 5초 이내 single-use 값인지, client PID·image와 HMAC이 맞는지 확인한다.
8. Controller welcome 또는 protocol mismatch handshake-error는 `HMAC-SHA256(key, UTF8("server-v1\n") || client_nonce_raw || JCS(message에서 auth_tag 제외))`를 보낸다.
9. 비교는 constant-time으로 한다.

nonce는 base64url-no-padding으로 wire에 싣는다. JCS object의 protocol version array는 입력 순서를 유지한다. challenge·hello·welcome Schema 위반과 중복 version은 bounded parse에서 거부한다. 실패한 connection의 challenge는 재사용하지 않는다.

이 인증은 accidental pipe squatting과 잘못된 binary 연결을 막지만 이미 current user로 실행되는 malware를 보안 경계 밖으로 밀어내지는 못한다.

Controller는 key file이 삭제되면 memory의 같은 key를 DPAPI로 다시 atomic write한다. 시작 시 blob이 corrupt하면 기존 Controller mutex·pipe가 없음을 확인하고 파일을 quarantine한 뒤 새 key를 만들며 `ipc.key_rotated` audit event를 남긴다. live Controller가 있는데 client가 key를 읽지 못하면 자동으로 다른 key를 만들지 않는다.

`star-mcp.exe`는 `GetNamedPipeServerProcessId`로 server PID를 얻고 `QueryFullProcessImageNameW` 결과가 설치된 Controller path인지 확인한다. Controller도 client PID·image path를 audit에 남긴다.

## Registry source watcher

### 감시 대상

- release `catalog\tool-packages`
- `%APPDATA%\Star-Control\tools.d`
- trusted project `.star-control\tools.d`
- active manifest가 참조하는 local Schema parent
- active ExecutableDescriptor와 integrity file의 parent directory

동일 final directory는 watcher 하나로 합친다. root 최대 128개다.

### Win32 방식

- directory를 `CreateFileW(FILE_LIST_DIRECTORY, FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OVERLAPPED)`로 연다.
- `ReadDirectoryChangesW`를 64 KiB DWORD-aligned buffer와 overlapped I/O로 사용한다.
- filter는 file name, directory name, size, last write, creation과 security다.
- subtree는 source root에서는 true, 단일 executable parent에서는 false다.
- rename old/new pair를 하나의 change set으로 coalesce한다.

buffer bytes 0, `ERROR_NOTIFY_ENUM_DIR`, handle invalidation과 overflow에서는 event를 추측하지 않고 해당 root 전체를 demand scan한다.

### Debounce·stable file

1. 동일 package event를 250 ms debounce한다.
2. file identity, size와 last-write를 읽는다.
3. 250 ms 뒤 같은 세 값을 다시 읽는다.
4. 같으면 candidate byte를 한 handle에서 끝까지 읽는다.
5. 최대 5초 동안 안정되지 않으면 `stabilizing`을 유지하고 last-known-good를 사용한다.

manifest delete는 500 ms 동안 같은 path의 rename/create가 없는 경우에만 실제 삭제로 처리한다. editor의 temporary rename을 package 삭제로 오인하지 않는다.

### Demand scan

`tool.search`, `tool.describe`, `tool.invoke`, `tool.registry.status` 직전에 다음 fingerprint를 비교한다.

- source directory entry set
- manifest·Schema: final path, file ID, size, last-write
- executable: final path, file ID, size, last-write, cached hash key
- trust store revision

watcher 정상 여부와 무관하게 invoke demand scan은 생략할 수 없다.

search·describe는 unchanged fingerprint의 cached SHA-256을 사용할 수 있다. invoke는 timestamp·size cache를 신뢰하지 않고 identity lease로 연 동일 handle에서 EXE와 required integrity file hash를 매번 다시 계산한다.

## Path와 file identity

### 금지 입력

- UNC·network path
- `\\.\`, `\\?\GLOBALROOT`, named device
- alternate data stream `file.exe:stream`
- relative executable absolute 해석
- PATH·PATHEXT search
- executable file reparse point
- final allowed root 밖으로 나가는 junction·mount point

### Identity 획득

1. locator를 absolute candidate path로 해석한다.
2. `CreateFileW`로 `GENERIC_READ | FILE_READ_ATTRIBUTES`, share는 `FILE_SHARE_READ`만, `OPEN_EXISTING`으로 연다.
3. regular `.exe` file이고 reparse point가 아닌지 확인한다.
4. `GetFinalPathNameByHandleW(FILE_NAME_NORMALIZED | VOLUME_NAME_GUID)`로 final path를 얻는다.
5. `GetFileInformationByHandleEx(FileIdInfo)`로 volume serial과 128-bit file ID를 얻는다.
6. 같은 handle에서 size, last-write, SHA-256을 계산한다.
7. PE header로 architecture와 minimum subsystem을 읽는다.
8. Authenticode 정책을 적용한다.

`ExecutableIdentity`는 `{volume_serial,file_id,size,last_write,sha256,product_version,interface_version,architecture,signature_status}`다.

file ID를 제공하지 않는 filesystem에서는 `follow_path`와 `version_compatible`을 거부하고 `pinned_hash`만 허용한다. executable은 NTFS 또는 ReFS local fixed volume을 기본 지원한다.

## Authenticode와 integrity file

- `WinVerifyTrust`를 `WTD_UI_NONE`, `WTD_REVOKE_WHOLECHAIN`, `WTD_CACHE_ONLY_URL_RETRIEVAL`, `WTD_CHOICE_FILE`로 호출해 검증 자체가 network를 열지 않게 한다.
- offline revocation 자료 부족, invalid chain과 unsigned를 구분한다.
- `require_valid`·`require_subject` 실패는 unavailable이다.
- signature 검사 cache key는 executable SHA-256, policy와 normalized subject이며 TTL은 5분이다. Controller restart와 trust 결정 변경에서 비운다.
- integrity file도 final handle·path·hash를 같은 방식으로 확인한다.

`record`는 unsigned·offline_indeterminate를 readiness metadata로 남기되 code trust가 별도로 있어야 실행한다. `require_valid`와 `require_subject`는 `WinVerifyTrust` success가 아니면 fail-closed다.

integrity file 검사는 launch 시점의 byte만 보장한다. process가 나중에 동적으로 다른 DLL을 읽는 것까지 보장하지 않는다. 강한 package 무결성이 필요한 tool은 versioned read-only install root 또는 `appcontainer_adapter`를 사용한다.

## Identity lease와 TOCTOU

검증 handle은 write·delete share를 허용하지 않은 상태로 process image가 생성될 때까지 유지한다.

1. descriptor와 executable handle을 lease한다.
2. hash·signature·probe를 확인한다.
3. `CreateProcessW`를 suspended로 호출한다.
4. 실제 process image path를 다시 조회한다.
5. Job Object에 할당한다.
6. pipe reader를 준비하고 primary thread를 resume한다.
7. process image가 만들어진 뒤 executable lease를 해제한다.

3번 전 path 교체는 share violation으로 실패하고 다시 demand scan한다. 3번 뒤 path가 교체돼도 이미 생성된 process image와 Operation identity는 바뀌지 않는다.

## Argument encoding

manifest binding 결과는 `Vec<OsString>`으로 유지한다. shell string을 만들지 않는다.

- `lpApplicationName`에는 final absolute EXE path를 항상 별도로 전달한다.
- `lpCommandLine`에는 argv[0]으로 같은 EXE path와 이후 argument를 넣는다.
- 지원 parser는 Microsoft C runtime compatible quoting 하나다.
- 공백·tab·quote가 있는 argument는 double quote로 감싼다.
- quote 앞의 연속 backslash는 두 배로 만들고 quote를 escape한다.
- closing quote 앞 trailing backslash도 두 배로 만든다.
- NUL은 거부한다.
- 완성 UTF-16 command line은 terminator 포함 32767 code unit 미만이어야 한다.

custom command-line parser를 쓰는 EXE는 `argv_v1`으로 연결하지 않고 `star_json_stdio_v1` adapter를 둔다.

## Environment block

Controller process의 전체 environment를 상속하지 않는다. case-insensitive map을 다음 순서로 만든다.

1. `SystemRoot`와 `WINDIR`는 Windows system directory에서 얻은 같은 값으로 넣는다.
2. `TEMP`와 `TMP`는 Operation별 Controller temp directory로 넣는다.
3. `USERNAME`, `USERDOMAIN`은 current token에서 얻는다.
4. manifest `environment_allow` 이름만 Controller environment에서 복사한다. `PATH`, `PATHEXT`, `COMSPEC`, `PSModulePath`, `PROMPT`는 복사할 수 없다.
5. `environment_values`의 고정값과 SecretRef를 해석한다.
6. `state_directories.environment_name`에 Controller가 만든 final directory를 넣는다.

중복 이름, `=`로 시작하는 hidden drive variable, NUL과 32767 UTF-16 code unit을 넘는 final block을 거부한다. key를 invariant uppercase로 정렬하고 double-NUL로 끝낸다. SecretRef byte는 child environment block 생성 뒤 zeroize하며 debug dump·event에 넣지 않는다. `argv_v1`과 `star_json_stdio_v1` 모두 같은 규칙을 사용한다.

## Child process 생성

직접 Win32 process adapter를 사용한다.

1. stdin·stdout·stderr anonymous pipe를 만든다.
2. parent end는 non-inheritable로 설정한다.
3. `STARTUPINFOEXW`의 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`에 child end만 넣는다.
4. environment allowlist와 `CREATE_UNICODE_ENVIRONMENT` block을 만든다.
5. `CreateProcessW`를 explicit application path, mutable command line, `CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT`로 호출한다.
6. `bInheritHandles=TRUE`지만 attribute list 밖 handle은 상속되지 않게 검증한다.
7. Job Object 할당·reader 준비 뒤 `ResumeThread`한다.

working directory는 descriptor가 선택한 final project·worktree·artifact path다. 경로는 존재하는 directory여야 하고 ProjectPathRef scope를 벗어나면 안 된다.

## Job Object

Operation마다 Job Object 하나를 만든다.

필수 limit:

- `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`
- manifest의 active process·memory·process time limit
- breakaway와 silent breakaway는 허용하지 않음

process를 suspended로 만든 뒤 Job에 넣으므로 primary process가 child를 먼저 만들 수 없다. Controller crash로 Job handle이 닫히면 child tree가 종료된다. 종료 결과가 durable state에 commit되기 전 crash면 Operation은 복구 시 `outcome_unknown`이다.

Controller가 Codex나 installer의 outer Job Object 안에 있으면 operation Job을 nested job으로 할당한다. outer job이 nesting·limit 조합을 거부하면 breakaway로 우회하거나 unjobbed로 실행하지 않고 `TOOL_ISOLATION_UNAVAILABLE`로 실패한다.

## stdout·stderr·stdin

- stdout·stderr는 별도 async reader가 동시에 drain한다.
- byte count를 읽는 즉시 적용해 child가 pipe full로 멈추지 않게 한다.
- stdout이 action inline limit만 넘고 executable `max_stdout_bytes` 이하면 `overflow=artifact`에서 전체를 artifact로 저장하고 `overflow=error`에서는 실패한다. stdout·stderr가 executable hard limit을 넘으면 pipe는 계속 drain하되 cap 이후 byte는 버리고 captured prefix를 quarantine한 뒤 `TOOL_OUTPUT_LIMIT`로 실패한다.
- stderr는 성공 data가 아니며 redacted log artifact다.
- stdin을 사용하지 않으면 process resume 직후 child stdin을 닫는다.
- secret byte는 stdin write가 끝나면 memory buffer를 zeroize하고 log하지 않는다.

stdout은 `actions.output.encoding`, stderr는 `stderr_encoding`으로 별도 decode한다. `oem`은 process 시작 시 `GetOEMCP` 값을 evidence에 고정하고, `utf16le`은 짝수 byte와 valid surrogate pair를 요구한다. 선언과 맞지 않는 byte는 replacement character로 성공 처리하지 않고 `TOOL_PROTOCOL_INVALID` 또는 binary quarantine으로 끝낸다.

## `argv_v1`

- manifest binding만 argv·stdin을 만든다.
- raw shell, `.cmd`, `.bat`, `.ps1`, script host는 실행하지 않는다.
- exit code는 manifest의 disjoint set으로 정규화한다.
- text·JSON·JSONL·binary output만 지원한다.
- argv tool의 generic graceful cancel은 제공하지 않는다. cancel_mode가 terminate_job이면 Job Object를 종료한다.

## `star_json_stdio_v1`

### framing

- stdin·stdout UTF-8 JSONL
- line당 최대 8 MiB, request arguments canonical byte는 4 MiB, 전체 output은 descriptor 상한
- stdout에는 protocol frame만
- stderr는 log만
- request ID 하나당 final frame 정확히 하나
- process 하나에는 `request` 또는 `probe` 하나만 보내며 multiplex하지 않음
- 모든 frame root는 object이고 unknown field를 거부함
- valid final frame 뒤 adapter는 stdout을 닫고 5초 안에 exit code 0으로 종료함. error는 result status로 표현함

### 실행 request

Controller가 stdin 첫 줄로 보낸다.

```json
{
  "frame": "request",
  "protocol_version": 1,
  "schema_id": "star.external-tool-request",
  "schema_version": 1,
  "request_id": "req_01J00000000000000000000000",
  "tool_id": "user.example.verify",
  "descriptor_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "arguments": {},
  "context": {
    "operation_id": "opn_01J00000000000000000000000",
    "project_id": "prj_01J00000000000000000000000",
    "goal_id": "gol_01J00000000000000000000000",
    "run_id": "run_01J00000000000000000000000",
    "stage_id": "stg_01J00000000000000000000000",
    "deadline_at": "2026-07-11T12:00:00.000Z",
    "artifact_directory": "D:\\work\\.ai-runs\\star-control\\operations\\opn_01J00000000000000000000000\\artifacts",
    "temp_directory": "D:\\work\\.ai-runs\\star-control\\operations\\opn_01J00000000000000000000000\\temp"
  }
}
```

request field 정본:

| 필드 | 형식 | 규칙 |
|---|---|---|
| `frame` | string | 정확히 `request` |
| `protocol_version` | integer | 정확히 1 |
| `schema_id`, `schema_version` | string, integer | `star.external-tool-request`, 1 |
| `request_id` | RequestId | process lifetime에서 하나 |
| `tool_id` | ToolId | lease한 action |
| `descriptor_hash` | SHA-256 | lease한 descriptor |
| `arguments` | object | default 적용·검증이 끝난 값 |
| `context.operation_id` | OperationId | 항상 존재 |
| `context.project_id`, `goal_id`, `run_id`, `stage_id` | ID 또는 null | 해당 scope가 없으면 null |
| `context.deadline_at` | UTC timestamp | process deadline |
| `context.artifact_directory` | string | trusted desktop final path 또는 AppContainer broker path |
| `context.temp_directory` | string | Operation 전용 path |

artifact·temp directory는 child 전용 request에는 전달하지만 MCP·일반 log에는 LocalPathRef로 redaction한다. adapter가 반환하는 artifact path는 artifact directory 기준 relative path만 허용한다.

### progress

```json
{
  "frame": "progress",
  "protocol_version": 1,
  "request_id": "req_01J00000000000000000000000",
  "sequence": 1,
  "progress": 30,
  "total": 100,
  "message": "검사 실행 중"
}
```

sequence와 progress는 증가해야 한다. total은 생략할 수 있지만 한 번 제공한 뒤 감소할 수 없다. Controller는 초당 4회로 coalesce한다.

`sequence`는 1 이상의 safe integer다. `progress`는 0 이상의 safe integer, `total`은 1 이상의 safe integer이고 존재하면 `progress <= total`이다. `message`는 optional 0~500자이며 secret·absolute path를 포함하지 않는다.

### cancel

`cancel_mode=stdin_frame`이면 Controller가 같은 stdin에 다음 한 줄을 쓴다.

```json
{
  "frame": "cancel",
  "protocol_version": 1,
  "request_id": "req_01J00000000000000000000000",
  "reason": "user_requested"
}
```

adapter는 `cancel_ack`를 선택적으로 보낼 수 있으나 final result가 반드시 뒤따른다. grace timeout 뒤에는 Controller가 Job을 종료한다.

cancel reason은 `user_requested`, `deadline`, `controller_shutdown`, `policy_revoked` 중 하나다. `cancel_ack`는 `{frame:"cancel_ack",protocol_version:1,request_id,accepted:boolean}`과 정확히 일치하며 final frame이 아니다.

### result

```json
{
  "frame": "result",
  "protocol_version": 1,
  "schema_id": "star.external-tool-response",
  "schema_version": 1,
  "request_id": "req_01J00000000000000000000000",
  "status": "ok",
  "summary": "검사를 완료했습니다.",
  "data": {},
  "diagnostics": [],
  "artifacts": [],
  "error": null
}
```

status는 `ok`, `cancelled`, `error`다. output Schema·artifact path·Diagnostic을 Controller가 다시 검증한다.

- `summary`는 1~1000자다.
- `data`는 output Schema를 통과하는 object이며 status=ok에서 필수, 나머지에서는 null이다.
- `diagnostics`는 외부 Diagnostic candidate 최대 256개다. Controller가 허용 field만 새 Diagnostic ID로 정규화하며 외부 severity를 그대로 신뢰하지 않는다.
- `artifacts`는 최대 256개의 `{path,media_type,role,sha256}`다. path는 artifact directory 기준 relative path, `role`은 `result`, `log`, `evidence`, `debug`다. Controller가 handle로 다시 열어 final path·size·hash를 검사한다.
- `error`는 status=error에서 필수인 `{code,message,retryable,details}`다. 외부 code는 `external.<package_id>.<code>` Diagnostic detail로 보존하고 Controller ErrorEnvelope namespace를 직접 주장하지 못한다.
- status=cancelled는 data·error가 null이다.

### probe

Controller가 실행 request 대신 다음 frame을 보낸다.

```json
{
  "frame": "probe",
  "protocol_version": 1,
  "request_id": "req_01J00000000000000000000000"
}
```

adapter는 다음 하나의 final frame을 반환한다.

```json
{
  "frame": "probe_result",
  "protocol_version": 1,
  "request_id": "req_01J00000000000000000000000",
  "product_version": "1.4.0",
  "interface_version": "1.0.0",
  "capabilities": ["progress", "stdin_cancel"]
}
```

probe request·result도 unknown field를 거부한다. result의 두 version은 SemVer, capabilities는 중복 없는 local ID 최대 32개다. 인식하는 값은 `progress`, `stdin_cancel`, `artifact_output`이며 unknown 값은 evidence에만 보존한다. probe process도 final frame 뒤 5초 안에 exit code 0으로 종료해야 한다.

## Isolation profile 결정

### `trusted_desktop`

- arbitrary CLI의 기본 호환 profile
- current user token
- Job Object와 handle allowlist
- manifest·EXE code trust 필요
- filesystem·network를 sandbox하지 않음
- Codex sandbox가 이 child process에 자동 적용된다고 주장하지 않음

### `appcontainer_adapter`

- Star-Control protocol을 아는 self-contained adapter만
- operation별 broker input·output directory만 ACL grant
- project source 전체를 직접 mount하지 않음
- network capability를 부여하지 않음
- input은 materialized artifact, output은 broker directory relative path
- AppContainer profile SID와 `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` 사용

profile 이름은 `StarControl.Tool.` + `SHA-256(package_id)` 앞 32 lowercase hex다. installer·Controller가 `CreateAppContainerProfile` 또는 기존 profile SID를 확인하고, uninstall 때 Star-Control 소유 profile만 제거한다. capability SID 목록은 비어 있으며 `internetClient`, `privateNetworkClientServer`, broadFileSystemAccess를 부여하지 않는다. Operation directory ACL은 해당 profile SID에 필요한 read·write만 주고 끝나면 retention 정책에 따라 회수한다. AppContainer process도 같은 per-Operation Job Object와 output limit을 적용한다.

launch 전 `NetworkIsolationGetAppContainerConfig`로 loopback exemption 목록을 읽고 해당 profile SID가 있으면 `TOOL_ISOLATION_UNAVAILABLE`로 닫힌 상태에서 실패한다. 사용자의 전역 exemption을 자동 제거하거나 firewall 설정을 바꾸지 않는다.

network나 project 전체 접근이 필요한 일반 개발 tool은 `trusted_desktop`과 명시적 code trust를 사용한다. `restricted_token`은 별도 profile로 제공하지 않는다.

## Timeout·cancel·shutdown

- queue timeout, process timeout, MCP request timeout을 별도 clock으로 관리한다.
- MCP disconnect는 accepted Operation을 취소하지 않는다.
- sync MCP cancellation은 cancel intent를 보낸다.
- process timeout은 cancel grace 뒤 Job Object 종료다.
- Controller shutdown은 새 invoke를 막고 10초 drain 후 configured policy에 따라 Operation을 계속할 helper가 없으므로 child Job을 종료하고 outcome을 기록한다.
- 강제 종료 뒤 파일 변경은 ChangeSet에서 partial·unverified로 표시한다.

## Recovery

- invocation intent는 process 생성 전에 durable event로 commit한다.
- process PID, creation time, executable identity와 Job ID는 생성 직후 commit한다.
- Controller가 재시작했을 때 이전 Job handle은 닫혀 child가 종료된 것으로 기대하지만 PID 재사용 때문에 PID만 믿지 않는다.
- final event가 없으면 process creation time·artifact·ChangeSet을 대조하고 `outcome_unknown`으로 복구한다.
- non-idempotent action을 자동 재실행하지 않는다.

## Last-known-good와 trust 저장

| 자료 | 위치 | 보호 |
|---|---|---|
| IPC key | `%LOCALAPPDATA%\Star-Control\secrets\ipc-key.v1` | DPAPI + user DACL |
| trust store | `%LOCALAPPDATA%\Star-Control\trust\tool-trust.v1.json` | atomic replace + user DACL |
| Registry cache | `%LOCALAPPDATA%\Star-Control\registry-cache\v1\` | hash 검증 + user DACL |
| Operation state | `%LOCALAPPDATA%\Star-Control\state\` | event + snapshot transaction |

TrustRecord Schema ID는 `star.tool-trust-record`, RegistryCache Schema ID는 `star.tool-registry-cache`다.

### ToolTrustRecord v1

| 필드 | 형식 | 규칙 |
|---|---|---|
| `schema_id`, `schema_version` | string, integer | `star.tool-trust-record`, `1` |
| `trust_id` | ToolTrustId | `trt_` + ULID, immutable ID |
| `package_id`, `package_version` | ID, SemVer | trusted package |
| `source_kind`, `source_id_hash` | enum, SHA-256 | release·user·project와 redacted source |
| `manifest_hash` | SHA-256 | normalized manifest |
| `schema_hashes` | sorted map | relative Schema ID→hash |
| `trust_mode` | enum | `exact`, `compatible`, `managed_path` |
| `executables` | array | executable ID, locator hash, update policy, exact hash 또는 publisher·version constraint |
| `permission_actions` | sorted ActionId set | trust 당시 최대 권한 |
| `isolation_profiles` | sorted enum set | 허용 execution profile |
| `granted_by`, `granted_at` | ActorRef, timestamp | provenance |
| `expires_at` | timestamp 또는 null | 기본 null |
| `revoked_at`, `revoke_reason` | optional | revoke 뒤 재사용 금지 |

`exact`는 manifest·Schema·EXE hash가 모두 같아야 한다. `compatible`은 manifest·Schema·locator·permission이 같고 version·publisher probe가 trust constraint 안이어야 한다. `managed_path`는 personal_auto user source의 follow_path만 사용하며 path·manifest·permission은 고정하고 EXE identity는 호출마다 기록한다.

permission, backend, protocol, locator, Schema, isolation 또는 paid 상태가 넓어지면 기존 trust를 사용할 수 없다. description만 바뀌어 manifest hash가 달라도 실행 의미 hash가 같다면 safe_default에 변경 요약을 보여주고 trust 갱신을 요구하며 personal_auto는 provenance를 남기고 갱신할 수 있다.

### ToolRegistryCache v1

| 필드 | 형식 | 규칙 |
|---|---|---|
| `schema_id`, `schema_version` | string, integer | `star.tool-registry-cache`, `1` |
| `cache_id` | ToolCacheId | `trc_` + ULID, immutable cache entry |
| `package_id`, `package_version` | ID, SemVer | 대상 package |
| `source_kind`, `source_id_hash` | enum, SHA-256 | 원 source |
| `source_file_identity` | redacted object | volume·file ID·size·last-write |
| `source_content_hash` | SHA-256 | TOML raw byte hash |
| `manifest_hash` | SHA-256 | normalized manifest hash |
| `package_snapshot` | object | normalized descriptor·Schema·search metadata |
| `trust_id` | TrustId 또는 null | active trust reference |
| `mcp_contract_version` | integer | 정확히 1 |
| `product_version` | SemVer | writer version |
| `validated_at` | timestamp | 마지막 full validation |

cache outer document도 RFC 8785 JCS hash를 별도 sidecar manifest에 기록한다. outer `ToolRegistryCache` entry와 sidecar에는 secret value·EXE byte·raw absolute path를 저장하지 않는다. 다만 이 표의 hash-only `PackageSnapshot`만으로는 invalid source 뒤 argv·Schema·resolved locator를 복원할 수 없으므로, last-known-good 실행 복구에 필요한 operational package snapshot은 current-user DPAPI로 봉인한 payload에만 저장하고 MCP·CLI·log에는 공개하지 않는다. source 삭제, trust revoke, contract incompatibility에서는 cache를 active로 만들지 않는다.

## 명시적으로 지원하지 않는 것

- interactive console·TTY·GUI prompt
- 자동 UAC·runas
- password prompt scraping
- shell script와 command string
- remote executable·UNC
- process attach·DLL injection
- arbitrary service·daemon lifecycle 관리
- Controller child가 Codex sandbox 안에 있다고 가정
- manifest permission 선언만으로 malicious EXE를 제한한다고 주장

## 공식 근거

- [ReadDirectoryChangesW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)
- [CreateNamedPipeW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-createnamedpipew)
- [Run and RunOnce Registry Keys](https://learn.microsoft.com/windows/win32/setupapi/run-and-runonce-registry-keys)
- [GetNamedPipeServerProcessId](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid)
- [CryptProtectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [BCryptGenRandom](https://learn.microsoft.com/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom)
- [CreateProcessW](https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
- [AssignProcessToJobObject](https://learn.microsoft.com/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject)
- [GetFinalPathNameByHandleW](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew)
- [WinVerifyTrust](https://learn.microsoft.com/windows/win32/api/wintrust/nf-wintrust-winverifytrust)
- [CertGetNameStringW](https://learn.microsoft.com/windows/win32/api/wincrypt/nf-wincrypt-certgetnamestringw)
- [AppContainer for legacy applications](https://learn.microsoft.com/windows/win32/secauthz/appcontainer-for-legacy-applications-)
- [CreateAppContainerProfile](https://learn.microsoft.com/windows/win32/api/userenv/nf-userenv-createappcontainerprofile)
- [NetworkIsolationGetAppContainerConfig](https://learn.microsoft.com/windows/win32/api/netfw/nf-netfw-networkisolationgetappcontainerconfig)
- [Microsoft windows-rs](https://github.com/microsoft/windows-rs)
