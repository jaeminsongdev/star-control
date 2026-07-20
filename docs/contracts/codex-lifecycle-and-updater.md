# Codex 생명주기와 Star Updater 계약

## 소유자와 범위

이 문서는 P-0039의 Codex task census, Controller idle lifecycle,
`star-mcp.exe` ownership, `star-updater.exe` update operation과 restart
integration을 소유한다.

- `star-controller.exe`는 lifecycle projection과 update operation의 persisted
  current state single writer다.
- `star-updater.exe`는 Controller가 quiesce한 update lease 동안 activation,
  rollback, Codex stop/relaunch를 수행하는 일시 writer다.
- `star.exe`와 `star-mcp.exe`는 Controller 또는 Updater가 소유한 state를 직접
  쓰지 않는 stable client/bridge다.
- Codex Plugin Hook은 관측 입력만 제공한다. Hook이 작업을 실행하거나 trust를
  승인하거나 update를 직접 적용하지 않는다.

이 문서는 최초 설치·제거, release transport, Codex Plugin rendering의 세부
형식을 소유하지 않는다. 각각 Windows 설치 계약과 release package 계약이
소유한다.

## 용어와 identity

```text
CodexInstance(instance_id, executable_identity, process_id, started_at)
CodexTask(task_id, instance_id, session_id, cwd, observed_at)
WorkSession(work_session_id, task_id, turn_id?, state, lease_expires_at)
McpConnection(connection_id, task_id?, mcp_pid, owner_identity, state)
GoalBinding(task_id, goal_id?)
```

- `GoalBinding`은 nullable 부가 정보이며 lifecycle 활성 여부를 결정하지 않는다.
- `session_id`는 Hook이 제공하는 Codex session identifier다. transcript path는
  편의 정보일 뿐 stable protocol key가 아니다.
- `turn_id`를 제공하지 않는 event는 task-level heartbeat만 갱신하며 다른 turn을
  종료시키지 않는다.
- `executable_identity`는 canonical path, file identity/hash, PID, parent/owner
  identity와 install generation을 포함한다. PID만으로 process를 종료하거나
  orphan으로 판정하지 않는다.
- Hook event를 받지 못한 Codex instance/task는 `unknown`이며 global idle이나
  restart shutdown에서 안전하게 활성으로 취급한다.

## 관측 입력

Plugin Hook은 공식적으로 지원되는 event만 사용한다.

| 입력 | projection 효과 |
|---|---|
| `SessionStart` | task·instance heartbeat 등록 또는 갱신 |
| `UserPromptSubmit` | 새 WorkSession 시작 |
| `PreToolUse`/`PostToolUse` | Tool activity counter 증감 |
| `SubagentStart`/`SubagentStop` | child activity counter 증감 |
| `Stop` | root turn 종료 후보 기록 |
| MCP initialize | MCP connection 등록·owner 검증 |
| MCP stdio EOF | connection terminal 및 gateway exit |
| durable Operation state | mutation/child activity counter 반영 |
| Codex process exit | instance terminal, 소유 task/MCP 종료 후보 |

설치된 Hook/MCP process가 같은 ToolHelp snapshot에서 `ChatGPT.exe` Desktop
ancestor를 확인한 경우에만 `owner_pid`와 parent/image identity를 보고한다. Controller는
매초 fresh snapshot에서 이 identity가 사라진 사실을 확인할 때만 해당 instance의 모든
task를 terminal로 만든다. Hook input에 owner를 얻지 못한 경우에는 기존 session identity를
유지하며, 이를 Codex 종료로 추정하지 않는다.

`Stop`은 Tool, Subagent, durable Operation counter가 모두 0이고 해당
WorkSession이 terminal일 때만 WorkSession 종료를 확정한다. 이벤트 유실은
bounded heartbeat lease로 감지하되, unknown 상태를 idle로 축소하지 않는다.

## Controller lifecycle

Controller lifecycle은 MCP process count나 Star Goal count가 아니라 모든
관측 가능한 WorkSession의 aggregate로 결정한다.

| aggregate | Controller 동작 |
|---|---|
| active WorkSession ≥ 1 | 유지, idle lease 없음 |
| active WorkSession = 0, known instance 존재 | 30초 `controller_idle` lease 시작 |
| lease 중 새 WorkSession | lease 취소, 유지 |
| 30초 만료 | 정상 shutdown |
| Codex instance 모두 terminal | 즉시 shutdown 또는 pending update handoff |
| unknown instance/task 존재 | shutdown 금지, 상태 진단 노출 |

idle lease는 monotonic clock 기준이며 busy polling을 사용하지 않는다. Controller
shutdown은 새 mutation admission을 닫고, bounded drain 뒤 terminal outcome을
기록한 다음 실행한다.

## MCP lifecycle

MCP Gateway는 connection-scoped stdio process다.

1. stdin EOF 또는 owner process terminal은 Gateway의 정상 terminal 이유다.
   owner terminal watchdog는 PID 재사용을 막기 위해 PID, parent PID, exact image path를
   매 fresh snapshot에서 함께 대조한다.
2. Gateway가 연결된 채 task가 유휴여도 Controller idle lease를 연장하지 않는다.
3. owner task/instance terminal 뒤 grace를 넘긴 MCP만 orphan 후보가 된다.
4. orphan cleanup은 canonical executable path, file identity/hash, target install
   root, parent/owner relation, update scope를 모두 검증해야 한다.
5. 열린 Codex task의 MCP를 time-based idle로 죽이는 정책은 Codex가 다음 Tool
   호출에 gateway를 재생성한다는 installed E2E evidence가 생기기 전까지 금지한다.
   따라서 Controller의 30초 idle lease는 Controller를 정리하는 정책이지, 연결된
   MCP를 추측으로 kill하는 정책이 아니다.

## Update class와 admission

| class | apply owner | Codex restart |
|---|---|---|
| `tool_hot_reload` | Controller Registry watcher | 없음 |
| `runtime_update` | Updater under update lease | 없음 |
| `codex_integration_update` | Updater under update lease | 필요 |
| `updater_update` | offline installer | 필요 경계 밖 |

모든 candidate는 exact artifact hash, install root, architecture, source revision,
descriptor/approval scope, rollback reference에 바인딩된다. `approved` 또는
`restart_armed`는 `committed`가 아니다.

`star update inspect <absolute-release-stage>`는 full `release-manifest.json`의
모든 file identity를 읽기 전용으로 확인해 `codex_integration_update`,
`runtime_update`, `updater_update`, `mixed_update`, `no_change`로 자동 분류한다.
`star-updater.exe`가 바뀐 후보와 runtime/integration 혼합 후보는 restart transaction의
apply 대상이 아니며 offline installer 또는 분리된 candidate가 필요하다.

`codex_integration_update`만 다음의 명시적 CLI 경로를 가진다.

```text
star update inspect <absolute-release-stage>
star update apply <absolute-release-stage> --codex-desktop <absolute-path> --approve <inspect approval_scope_sha256>
```

두 번째 명령은 후보를 다시 inspect해 class·hash·architecture를 재검증한 뒤에만
detached `star-updater.exe integration-apply-restart`를 시작한다. updater는 변경된
`star.exe`, `star-mcp.exe`, Codex Plugin template 파일과 기존 release manifest를
current-user backup에 먼저 기록하고, 개별 payload 원자 교체 뒤 release manifest를
commit marker로 마지막에 교체한다. Controller/install record는 새 manifest로 재생성한다.
Plugin registration 또는 offline verification이 실패하면 backup 파일·old manifest를
복원하고 이전 template로 repair한 뒤 같은 Desktop executable을 재실행한다. 이 경우
command는 성공으로 끝나지 않고 `rolled_back` receipt를 남긴다.
backup은 `prepared` → `applied` → `committed|rolled_back` 상태를 가진다. updater가
`committed` 전에 중단되면 다음 updater transaction이 같은 install root의 pending backup을
먼저 원복한다. audit backup은 삭제하지 않으며 committed/rolled_back record는 재적용하지 않는다.

Updater는 current-user named mutex `Local\\Star-Control.Updater.<sid-hash>.v1`을
countdown 시작 전 획득하고 receipt finalization까지 보유한다. 이미 보유 중이면 두 번째
request는 새 countdown이나 Codex 종료를 시작하지 않고 명시적 busy 오류로 실패한다.

## Restart integration operation

### 사전 조건

`apply_restart`는 다음이 모두 참일 때만 `restart_armed`를 반환한다.

1. candidate가 staged·verified이고 integration class다.
2. target install root와 architecture가 현재 activation과 일치한다.
3. 이전 last-known-good artifact가 읽기 가능하고 rollback이 가능하다.
4. 정확한 Desktop executable path 기준으로 모든 실행 instance/process tree가 census됐다.
   현재 공개 Codex 표면에는 Desktop의 모든 UI task ID와 mutation safe point를 읽는
   공식 API가 없으므로 receipt의 task count는 unknown이며 Star 등록 task count로
   축소하지 않는다. updater는 그래서 특정 task만 닫지 않고 대상 executable의 모든
   instance를 종료한다.
5. global update mutex와 exact Controller update lease가 확보됐다.
6. Updater는 Codex process tree 밖에서 실행될 준비가 됐다.

`restart_armed`를 반환하기 전에 `star.exe`는 updater image의 hash가 일치하는
current-user staging copy를 만든다. 이 copy는 local WMI `Win32_Process.Create`
broker로 시작되어 Codex의 Job 종료와 분리되며, 설치 root의 `star-updater.exe`를 직접 실행해 self-update
file lock을 만들지 않는다. stage copy와 그 자식은 fallback Desktop termination의
보호 집합이다. 이 보호는 updater 외의 Codex descendant를 종료할 권한을 넓히지
않는다.

### 성공 상태

```text
planned → staged → candidate_verified → restart_armed → countdown
→ draining → codex_stopped → applying → offline_verified
→ relaunching → online_postcheck → committed → exited
```

`restart_armed` 응답이 flush된 뒤 정확히 10초 countdown을 시작한다. countdown
동안 Controller는 새 mutation을 `UPDATE_RESTART_PENDING`으로 거절하고, 동일
request는 같은 operation으로 idempotent하게 반환한다.

### 종료·적용·재실행

1. countdown 종료 뒤 Updater는 target Codex instance에 graceful stop을 요청한다.
2. bounded grace 뒤에도 남은 process는 exact identity가 일치할 때만 종료한다.
   updater의 own PID와 descendant는 이 fallback에서 제외한다.
3. 소유 MCP가 EOF로 종료되고 Controller가 update lease를 handoff한 것을 확인한다.
4. Updater는 current-user backup을 만든 뒤 integration-only file set을 교체하고
   release manifest를 마지막 commit marker로 원자 교체하며 official Plugin registration
   path를 사용한다. runtime generation activation은 이 경로의 대상이 아니다.
5. offline hash/manifest/Plugin render postcheck 실패 시 backup의 prior file set과
   derived install/controller record를 복구한 뒤 이전 template를 repair한다.
6. 성공하면 기록한 same Codex executable을 재실행한다.
7. bounded online postcheck는 new MCP initialize, expected installation hash,
   Plugin/Hook version을 확인한다.
8. online signal이 없으면 `applied_validation_pending` receipt만 남기고
   Updater는 종료한다. 다음 실제 `SessionStart`가 후속 검증을 수행한다.

offline installer는 process-local `TEMP`/`TMP`를 fixed local
`Star-Control/installer-temp`로만 바꾼다. 이는 reparse-point 기반 사용자 TEMP가
Inno Setup extraction을 거부하는 경우를 위한 것이며 시스템 환경변수는 변경하지
않는다. installer, offline verification 또는 integration verification이 실패하면
`rollback_required` receipt를 유지하면서도 Codex relaunch를 best-effort로 시도해
사용자가 수동으로 앱을 다시 열 필요가 없게 한다.

### 실패 상태

| 상태 | 의미와 다음 동작 |
|---|---|
| `waiting_for_safe_point` | unsafe mutation이 terminal이 될 때까지 restart countdown 미시작 |
| `aborted` | apply 전 중단, active installation 무변경 |
| `rollback_required`/`rolling_back` | apply/postcheck 실패 뒤 prior 복구 진행 |
| `rolled_back` | prior integration 복구 뒤 Codex relaunch 시도 |
| `rollback_failed` | 이전 설치본 복구도 실패; 성공 표시 금지 |
| `outcome_unknown` | process/activation 결과를 증명할 수 없음 |
| `relaunch_failed` | 설치 결과와 Codex launch 결과를 구분해 기록 |
| `applied_validation_pending` | offline apply는 성공했으나 online signal 대기 |
| `hook_review_required` | 변경 Hook hash의 Codex trust review가 필요 |

Codex가 이미 종료된 뒤 Controller handoff가 사라진 것은 apply를 건너뛰고 Desktop을
꺼진 채로 남길 이유가 아니다. Updater는 verified exact-root process drain을 계속
시도한다. 그 뒤 apply 전 abort가 필요하거나 candidate repair/rollback이 실패하면,
receipt를 먼저 기록한 뒤에도 기록해 둔 exact Desktop executable의 relaunch를
best-effort로 시도한다. 따라서 `aborted`, `rollback_required`, `rolled_back`,
`rollback_failed`는 각각 installation 결과를 말할 뿐 Codex relaunch를 생략한다는
뜻이 아니다. Desktop relaunch 자체 실패만 `relaunch_failed`로 별도 기록한다.
receipt 기록 자체의 실패는 원래 오류를 성공으로 바꾸지 않으며 updater command도
실패로 종료한다.

중단된 Tool mutation, 외부 효과, 채팅 turn은 replay하지 않는다.

## Runtime update

`runtime_update`는 P-0038의 generation selector와 same-task contract를 유지한다.
Updater가 Controller drain, selector atomic write, new Controller start, IPC
postcheck, rollback을 수행하지만 Codex와 fixed MCP PID는 유지돼야 한다.

## API 표면

고정 MCP method를 늘리지 않는다. live Tool Registry 또는 local CLI가 다음
operation을 노출한다.

- `star.update.inspect`
- `star.update.stage`
- `star.update.apply`
- `star.update.apply_restart`
- `star.update.status`
- `star.update.cancel`
- `star.update.recover`

모든 mutation request는 descriptor hash와 approval scope를 재검증한다.

`star update status`는 activation record와 가장 최근 integration restart receipt를
함께 반환한다. receipt가 손상되었거나 읽을 수 없으면 상태를 정상으로 축소하지 않고
명시적으로 실패한다.

## 금지와 회복 경계

- Codex cache, `config.toml`, Hook trust, Desktop DB 직접 수정 금지
- 외부 프로그램의 기존 task 채팅 주입, `thread/resume`, 새 task 생성 금지
- Updater 상주, automatic network download, 자체 scheduler 금지
- Updater self-replacement 금지
- 실패 또는 timeout을 `committed`로 승격 금지

실제 Codex restart E2E 직전에는 operation receipt에 P-ID, 단계, candidate hash,
install root, rollback ref, 재개 검증 명령, Context Pack을 기록한다. Codex 앱의
재실행은 새 turn을 만들지 않으며 사용자의 다음 메시지 또는 Codex의 자연스러운
goal continuation만 후속 작업을 시작할 수 있다.

## 검증 요구

- state transition/invalid transition, mutex, idempotency
- heartbeat/unknown/false-idle, 30초 lease start·cancel·expire
- stdio EOF, owner death, wrong-owner 보호, orphan cleanup
- detached Updater survival, 10초 countdown, shutdown failure no-mutation
- apply/rollback/relaunch/online-timeout/recover
- runtime update의 Codex·fixed MCP PID 유지
- integration update의 모든 대상 MCP 교체
- fake process tree fixture 후 actual installed-tree E2E

실제 설치 root mutation과 현재 Codex 자동 종료·재실행은 명시적 사용자 승인
없이는 실행하지 않는다.
