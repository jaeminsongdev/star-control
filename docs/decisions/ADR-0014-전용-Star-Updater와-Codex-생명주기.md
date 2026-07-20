# ADR-0014: 전용 Star Updater와 Codex 전체 생명주기

## 상태

확정 — P-0039 구현 기준.

## 배경

Bootstrap v2는 고정 `star.exe`·`star-mcp.exe`와 교체 가능한 Runtime
Generation을 분리해 routine Runtime update를 같은 Codex task에서 완료할 수
있게 했다. 그러나 Bridge, MCP Gateway, Plugin, `.mcp.json`, Hook, Skill 계약을
바꾸는 통합 업데이트는 기존 Codex process와 stdio MCP 연결이 살아 있는 동안
안전하게 적용할 수 없다.

또한 Controller와 MCP process의 존재만으로 Codex가 실제 작업 중인지 판단하면
유휴 process가 남고, 반대로 Subagent·Tool·durable Operation이 남은 상태를
유휴로 오판할 위험이 있다. Star Goal 연결 여부는 이 판단의 충분한 근거가
아니다.

## 결정

1. 설치 root의 Runtime executable은 `star.exe`, `star-controller.exe`,
   `star-mcp.exe`, `star-updater.exe` 네 개다. 최초 설치·제거와
   `star-updater.exe` 자체의 교체는 offline installer가 소유한다.
2. `star-updater.exe`는 update operation에만 존재하는 one-shot background
   process다. network downloader, scheduler, package manager, 장기 service가
   아니다.
3. Codex 생명주기의 관측 단위는 다음과 같다.

   ```text
   CodexInstance
   └─ CodexTask
      ├─ WorkSession
      ├─ McpConnection
      └─ GoalBinding (optional)
   ```

   Controller는 Star에 등록된 task만이 아니라 Plugin Hook과 MCP connection으로
   관측 가능한 모든 Codex task·instance를 census한다. 관측 불가능한 instance는
   유휴로 추정하지 않고 unknown/active 안전 상태로 남긴다.
4. WorkSession이 하나라도 활성인 동안 Controller는 유지한다. 모두 종료되면
   30초 cancellable idle lease를 시작하며, 그 안에 새 WorkSession이 시작되면
   lease를 취소한다. Codex instance가 종료되면 Controller는 30초를 기다리지
   않고 종료 절차로 들어간다.
5. `star-mcp.exe`의 연결 생명주기는 WorkSession과 다르다. stdio EOF 또는
   검증된 owner instance 종료 시 즉시 종료하고, 열린 task의 연결된 MCP를
   임의 idle timeout으로 죽이지 않는다. 연결된 MCP는 Controller idle lease를
   연장하지 않는다.
6. update는 다음 세 class로 분리한다.

   | class | 대상 | Codex restart |
   |---|---|---|
   | `tool_hot_reload` | 외부 Tool Registry package·manifest | 없음 |
   | `runtime_update` | Controller, CLI Runtime, handler, catalog, schema | 없음 |
   | `codex_integration_update` | Bridge, `star-mcp.exe`, Plugin, MCP config, Hook, Skill | 필요 |

   `runtime_update`는 P-0038의 generation selector·drain·rollback 불변식을
   유지하며 Codex와 고정 MCP를 재시작하지 않는다.
7. `codex_integration_update`는 AI가 `inspect → stage → apply_restart`를
   호출해 시작한다. candidate verification, install-root/architecture binding,
   last-known-good rollback 준비, global update mutex, 영향을 받는 task/instance
   census가 모두 성공해야 `restart_armed`가 된다.
8. `restart_armed`가 MCP/CLI 응답을 반환한 뒤 정확히 10초 countdown을 시작한다.
   countdown 동안 새 mutation을 받지 않으며, 기존 unsafe mutation은 countdown
   이전에 safe point 또는 terminal state에 도달해야 한다.
9. countdown 종료 후 Updater는 검증된 대상 Codex instance에 정상 종료를 먼저
   요청하고, bounded grace 뒤 exact executable path·PID·parent/owner identity·
   install generation이 일치하는 process만 후속 종료한다. 모든 소유 MCP EOF와
   Controller lease handoff는 우선 시도하되, Codex 종료 뒤 handoff 대상이 먼저
   사라진 경우에는 verified exact-root drain으로 진행한다. apply 전 abort 또는
   이후 rollback도 기록한 Desktop executable relaunch를 best-effort로 시도한다.
   updater는 current-user staging copy를 local WMI `Win32_Process.Create` broker로
   시작하며, own PID·descendant는 이 fallback 종료 집합에서 제외한다.
10. Updater는 side-by-side stage, atomic activation, official Plugin install path,
   offline postcheck, rollback을 수행한다. 성공하면 기록한 동일 Codex executable을
   재실행하고 bounded online postcheck를 실행한 뒤 종료한다. online signal이
    제한 시간 안에 없으면 `applied_validation_pending` receipt를 남기고 종료하며
    다음 실제 `SessionStart`가 검증을 완료한다.
    offline installer의 extraction에는 process-local fixed temp를 사용하며, installer
    또는 offline verification 실패 뒤에도 Codex relaunch를 best-effort로 시도한다.
11. Updater와 Controller는 update lease 동안 동시에 activation writer가 될 수 없다.
    `committed`는 offline/online postcheck evidence 뒤에만 가능하다.
12. 외부 프로그램의 기존 Codex task 채팅 주입, `thread/resume` 기반 Desktop
    메시지 생성, 새 task 생성, 자동 새 turn, 중단 mutation replay는 제공하지
    않는다. Codex 재실행은 앱 실행만 보장하고 대화 continuation을 보장하지
    않는다.
13. Codex cache, `config.toml`, Hook trust 저장소와 Desktop 내부 DB는 직접
    수정하지 않는다. Plugin/Hook 변경으로 trust 재검토가 필요하면
    `hook_review_required`로 남기며 자동 승인하지 않는다.

## 상태 경계

성공 경로는 다음과 같다.

```text
planned → staged → candidate_verified → restart_armed → countdown
→ draining → codex_stopped → applying → offline_verified
→ relaunching → online_postcheck → committed → exited
```

`waiting_for_safe_point`, `aborted`, `rollback_required`, `rolling_back`,
`rolled_back`, `rollback_failed`, `outcome_unknown`, `relaunch_failed`,
`applied_validation_pending`, `hook_review_required`는 성공으로 승격되지 않는
별도 상태다.

## 결과

- 사용자는 MCP·Plugin·Hook 재설치 때 파일 복사, Codex 종료, MCP process 정리,
  Codex 재실행을 직접 수행하지 않는다.
- 10초 countdown 뒤 현재 Codex task가 종료될 수 있으므로 Updater는 적용 직전의
  durable Context Pack과 update receipt를 보존한다. 다음 turn은 Codex가
  자연스럽게 시작하거나 사용자가 보내야 하며 Updater가 만들지 않는다.
- 이 결정은 ADR-0012의 "세 Runtime EXE", "별도 updater 없음", "사용자가
  Codex를 종료한 뒤 외부 installer 실행" 항목을 supersede한다. ADR-0012의
  설치 위치 선택, 소유권 분리, Codex cache/trust 비직접수정 원칙은 유지한다.
- 이 결정은 ADR-0013의 Runtime Generation 분리를 유지하되, update mutation
  owner를 stable CLI supervisor에서 dedicated Updater로 이전한다.

## 구현 전제와 검증

- Plugin Hook event의 실제 지원 범위와 Hook trust는 최신 공식 Codex 문서와
  설치본에서 검증한다.
- fake Codex process tree fixture로 multiple instance, delayed child exit,
  orphan MCP, file lock, restart race, updater crash/recover, rollback을 먼저
  검증한다.
- 실제 설치 root mutation과 현재 Codex 자동 종료·재실행 E2E는 별도 사용자
  승인 뒤에만 실행한다.

## 관련 문서

- [선택형 Windows 설치와 Codex Plugin 연동](ADR-0012-선택형-Windows-설치와-Codex-Plugin-연동.md)
- [고정 Bootstrap Bridge와 Runtime Generation](ADR-0013-고정-Bootstrap-Bridge와-Runtime-Generation.md)
- [Codex 생명주기와 Updater 계약](../contracts/codex-lifecycle-and-updater.md)
- [Runtime update와 activation 계약](../contracts/runtime-update-and-activation.md)
