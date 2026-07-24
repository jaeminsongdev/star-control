# Windows 설치와 Codex 연동 계약

## 목적과 범위

이 문서는 Windows 설치 파일이 최종 binary를 배치하고, 실제 설치 경로를 Codex Plugin의 MCP·Hook 설정에 연결하고, update·repair·제거 때 소유 자료를 안전하게 처리하는 계약을 정의한다. P-0026은 이 설치 transport 수직 Slice를 구현한다. M10의 전체 `ReleaseManifest` lifecycle·CI·공개 배포 engine이 구현됐다는 뜻은 아니다.

P-0039의 전용 updater와 lifecycle 관측 및 installed-tree restart E2E는 구현됐다. 이후 공개 candidate의 clean 설치·update·rollback·repair·uninstall 검증과 사용자 설치 상태 변경은 각각 해당 Slice와 승인 경계를 따른다.

기준 결정은 [ADR-0012](../decisions/ADR-0012-선택형-Windows-설치와-Codex-Plugin-연동.md), P-0039의 [ADR-0014](../decisions/ADR-0014-전용-Star-Updater와-Codex-생명주기.md)와 [ADR-0015](../decisions/ADR-0015-x64-Stable과-ARM64-Preview-출시-정책.md)다. ADR-0012의 offline installer 경계는 유지하되, 설치 후 통합 변경의 restart transaction은 전용 updater가 소유한다.

## 소유권 경계

| 자료 | 정본 위치 | Writer | 제거 기본값 |
|---|---|---|---|
| 설치된 program file | 사용자가 고른 install root | Inno Setup | 제거 |
| release-file manifest | `<install-root>/release-manifest.json` | package build tool | program과 함께 제거 |
| Controller bootstrap manifest | `<install-root>/star-control-install.v1.json` | package build tool | program과 함께 제거 |
| installation record | `%LOCALAPPDATA%/Star-Control/installation/installation-record.v1.json` | `star installation finalize` | 제거 |
| Codex Marketplace source | `%LOCALAPPDATA%/Star-Control/integrations/codex/<version>/marketplace-root` | `star integration install|repair` | 제거 |
| Codex marketplace 등록·Plugin cache | Codex 소유 위치 | Codex 공식 command/app | Star-Control이 직접 삭제하지 않음 |
| Hook 신뢰 | Codex 소유 위치 | 사용자와 Codex | 보존, 자동 승인 금지 |
| 사용자 설정 | `%APPDATA%/Star-Control` | 사용자·Controller | 기본 보존 |
| runtime state·log·cache | `%LOCALAPPDATA%/Star-Control`의 installation/integrations 밖 | Controller | 기본 보존 |
| Project 자료 | `<project>/.star-control`, `<project>/.ai-runs` | Project workflow | 항상 보존 |

Codex cache·`config.toml`·Hook trust file을 Star-Control이 직접 쓰는 것은 금지한다. installation record와 integration record는 Controller persisted projection이 아니라 installer-owned local fact이므로 local CLI가 atomic write할 수 있다.

## 설치 파일과 경로

### 배포 파일

- `star-control-windows-x64-<version>-setup.exe` — signed Stable
- `star-control-windows-arm64-<version>-setup.exe` — signed `native_unverified` Preview

두 파일은 같은 source revision과 제품 version을 사용하지만 architecture별 binary와 hash가 다르다. public GitHub Release에서는 각각 Authenticode 검증을 통과해야 한다. P-0026 local build는 unsigned를 허용하되 `release-manifest.json`에 signed인 것처럼 기록하지 않으며 unsigned 결과를 Stable로 공개하지 않는다. certificate 또는 timestamp provider가 없으면 release는 `blocked_external`이다. ARM64 Preview는 cross-build·PE architecture·file manifest·signature·installer model·fake lifecycle evidence만 주장하고 native process·IPC·설치 성공으로 승격하지 않는다.

### 경로 규칙

- 공개 기본값: `%LOCALAPPDATA%\Programs\Star-Control`
- 이 PC에서 사용자가 선택할 목표값: `D:\도구\Star-Control`
- Installer UI에서 변경 가능
- update·repair: 같은 AppId와 `UsePreviousAppDir=yes`로 이전 선택값 재사용
- runtime: `current_exe()`의 parent를 실제 install root로 사용
- 정적 문서·template·source의 `D:\도구\Star-Control`은 예시일 뿐 package 기본값이나 runtime 분기 조건이 아니다.

install root는 local fixed volume의 absolute path여야 한다. UNC·device path와 symlink/reparse 우회는 기존 Controller bootstrap 검증에서 거부한다.

## 설치 root 구조

```text
<install-root>/
├─ star.exe
├─ star-controller.exe
├─ star-mcp.exe
├─ star-updater.exe
├─ star-control-install.v1.json
├─ release-manifest.json
├─ catalog/
│  ├─ profiles/
│  ├─ policies/
│  ├─ tool-packages/
│  └─ fake/
├─ schemas/v1/
├─ examples/tool-packages/
├─ migrations/                       # 실제 migration source가 있을 때
├─ integrations/codex-plugin-template/
├─ legal/
│  ├─ LICENSE.txt
│  └─ THIRD-PARTY-NOTICES.txt        # policy가 요구하고 실제 생성됐을 때
├─ sbom.spdx.json                    # policy가 요구하고 실제 생성됐을 때
└─ provenance.json                   # policy가 요구하고 실제 생성됐을 때
```

`sbom.spdx.json`, `provenance.json`과 third-party notice는 package policy가 요구하는 실제 자료만 넣는다. 빈 파일로 성공을 흉내 내지 않는다. 네 runtime EXE 외 build helper를 install root에 두지 않는다. Inno Setup이 소유하는 `unins*.exe`·`unins*.dat`는 제품 runtime EXE가 아니라 제거 metadata이므로 설치 뒤 추가될 수 있다.

## release-file manifest v1

파일명은 `release-manifest.json`, `schema_id`는 `star.release-file-manifest`, `schema_version`은 `1`이다.

| 필드 | 형식 | 규칙 |
|---|---|---|
| `product_version` | SemVer string | root workspace version과 같음 |
| `target_architecture` | `x64\|arm64` | installer architecture와 같음 |
| `created_at` | RFC 3339 UTC | package 생성 시각 |
| `source_revision` | string | Git commit 또는 명시적 `dirty:<hash>` 식별자, 빈 값 금지 |
| `files[]` | entry array | 상대 경로 사전식 정렬, 중복 금지 |
| `files[].path` | slash 상대 경로 | absolute·`..`·빈 segment 금지 |
| `files[].size` | non-negative integer | byte 길이 |
| `files[].sha256` | `sha256:<64 lowercase hex>` | 설치 byte hash |
| `generated_files` | string array | 설치 경로가 정해진 뒤 만드는 파일. v1은 `star-control-install.v1.json`만 허용 |
| `set_sha256` | SHA-256 | `files[]` canonical JSON의 hash |
| `signing` | `unsigned_local\|signed` | 실제 상태만 기록 |

manifest 자기 자신과 installation-bound generated file은 `files[]`에 넣지 않는다. `star installation finalize`는 모든 entry의 존재·regular file·size·hash, 예상 architecture, 네 runtime EXE의 포함과 `generated_files` allowlist를 검사한 뒤 bootstrap manifest를 만든다. 누락·추가 program file 정책은 package build 검증이 담당한다.

이 문서는 M10의 승인·published state를 가진 `star.release-manifest` v2와 다른 technical package manifest다.

## installation record v1

파일명은 `installation-record.v1.json`, `schema_id`는 `star.installation-record`, `schema_version`은 `1`이다.

| 필드 | 형식 | 규칙 |
|---|---|---|
| `installation_id` | ULID prefixed `ins_` | 최초 finalize 때 생성, 같은 root repair 때 유지 |
| `product_version` | SemVer | 검증한 release-file manifest와 같음 |
| `target_architecture` | `x64\|arm64` | 검증한 manifest와 같음 |
| `install_root` | absolute Windows path | canonical actual root |
| `release_manifest_sha256` | SHA-256 | 설치한 manifest byte hash |
| `installed_at` | RFC 3339 UTC | 최초 성공 시각 |
| `updated_at` | RFC 3339 UTC | update·repair 성공 시각 |
| `codex_integration` | summary or null | integration record 경로·상태, secret 금지 |

record는 최대 64 KiB, duplicate key·unknown field 거부, atomic temp-write/flush/rename으로 갱신한다. 기존 record의 install root가 다르면 `--replace-existing` 없이는 덮어쓰지 않는다.

## Controller bootstrap manifest

`star-control-install.v1.json`의 역할은 설치 위치에서 실행되는 gateway와 Controller의 version·hash 결합이다. `star installation finalize`는 release-file manifest가 검증한 최종 byte에서 다음을 생성한다.

- `product_version`
- `gateway_sha256`: `star-mcp.exe` hash
- `controller_path`: 실제 설치 전에는 staging absolute path가 아니라 설치 시 사용할 `<install-root>/star-controller.exe`로 finalize에서 다시 렌더링
- `controller_sha256`

따라서 bootstrap manifest는 immutable package file set에 포함되지 않는 명시적 installation-bound generated file이다. Installer가 `star installation finalize`를 호출해 실제 absolute path의 manifest를 atomic write한다. CLI와 MCP는 기존 엄격한 bootstrap 검증을 그대로 사용한다.

## Codex Plugin template와 렌더링

### source

```text
integrations/codex-plugin-template/
└─ marketplace-root/
   └─ .agents/plugins/
      ├─ marketplace.json
      └─ plugins/star-control/
         ├─ .codex-plugin/plugin.json
         ├─ .mcp.json
         ├─ hooks/hooks.json
         └─ skills/star-control-operations/SKILL.md
```

source template은 Plugin validator를 통과하는 중립 authoring 값(`0.0.0+codex.template`, `star-mcp`, `star hook session-start`)을 가진다. 렌더러는 JSON을 strict parse한 뒤 허용된 typed field인 Plugin version, MCP `command`, Hook `commandWindows`만 바꾸고 다시 serialize한다. 문자열 token 치환이나 shell 문자열 연결은 사용하지 않는다. source tree의 나머지 파일과 unknown component를 임의로 복사하지 않고 allowlist file set만 렌더링한다.

현재 Skill identifier는 `star-control-operations` 하나로 고정한다. 이 source 변경은 이미 설치된 rendered Marketplace나 Codex Plugin cache를 직접 바꾸지 않으며 실제 설치 상태 전환은 후속 package·repair Gate가 담당한다.

### rendered Marketplace

```text
%LOCALAPPDATA%/Star-Control/integrations/codex/<version>/
├─ integration-record.v1.json
└─ marketplace-root/
   └─ .agents/plugins/
      ├─ marketplace.json
      └─ plugins/star-control/
         ├─ .codex-plugin/plugin.json
         ├─ .mcp.json
         ├─ hooks/hooks.json
         └─ skills/star-control-operations/SKILL.md
```

Marketplace name은 `star-control-local`, Plugin name은 `star-control`로 고정한다. Plugin version은 제품 SemVer에 `+codex.<render-hash-prefix>` cachebuster를 붙인다. `plugin.json`에는 실제 `.mcp.json`이 있을 때만 `mcpServers`를 둔다. `hooks` manifest field는 쓰지 않고 기본 `hooks/hooks.json` discovery를 사용한다.

`.mcp.json`은 stdio server `star-control`의 `command`를 실제 `<install-root>/star-mcp.exe` absolute path로 렌더링한다. lifecycle Hook의 `commandWindows`는 실제 `<install-root>/star.exe hook <event>`를 가리킨다.

## Hook 계약

P-0039의 Hook은 `SessionStart`, `UserPromptSubmit`, `Stop`, `PreToolUse`, `PostToolUse`, `SubagentStart`, `SubagentStop`을 사용한다. 모든 입력은 event 이름과 `session_id`, 최대 1 MiB, 중복 key 없음만 수용한다. Hook은 Controller에 관측을 best-effort로 남기되 Controller 부재만으로 Codex task를 실패시키지 않는다. `SessionStart`만 stdout에 다음 의미의 JSON을 낸다.

```json
{
  "continue": true,
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "`star-control-operations` 지침을 따른다. Star-Control action을 사용할 때는 `star_tool_search`로 현재 registry를 검색하고 action readiness가 `ready`인 결과만 `star_tool_describe`로 다시 확인한다. describe에서 현재 Schema, 위험 lane, `descriptor_hash`, `required_call_tool`을 받은 뒤 그 tool에 `tool_id`, `descriptor_hash`, `arguments`를 전달한다. package나 manifest의 ready 상태는 action readiness가 아니다. 검색 결과가 없거나 action이 non-ready이거나 MCP 연결이 실패하면 일반 Codex 개발 작업을 막지 말고 프로젝트 native 도구를 사용하며 fallback 사실과 이유를 결과에 기록한다. `star_tool_registry_status`는 진단용이며 필수 선행 Gate가 아니다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다. `approval_required`, `question_required`와 Operation ID 반환은 완료가 아니다."
  }
}
```

Hook은 작업을 직접 실행하거나 결제를 승인하지 않는다. ready action이 없거나 MCP 연결이 실패해도 일반 개발 작업을 차단하지 않고 native fallback과 이유를 안내한다. 잘못된 입력에는 non-zero로 종료하고 stdout에 성공 JSON을 쓰지 않는다. Plugin 설치 뒤 사용자는 Codex `/hooks`에서 exact Hook을 검토·신뢰한다.

## CLI 계약

다음 command는 Controller를 먼저 시작하지 않는 local installer command다.

```text
star installation finalize --architecture x64|arm64 [--replace-existing] [--json]
star installation status [--json]
star integration install [--codex <exe>] [--skip-register] [--json]
star integration repair [--codex <exe>] [--skip-register] [--json]
star integration status [--json]
star integration uninstall [--codex <exe>] [--json]
star update offline-installer-restart --install-root <absolute-path> --installer <absolute-exe> --codex-desktop <absolute-exe> [--json]
star hook session-start
```

공통 exit code:

| code | 의미 |
|---:|---|
| 0 | local 변경·검증 성공. Codex 후속 조치가 있으면 구조화 결과에 표시 |
| 2 | 인자·template·manifest 오류 |
| 3 | 명시적 사용자 조치 필요 |
| 4 | local I/O·process 실패 |
| 6 | version·architecture 불일치 |
| 7 | Codex desktop 실행 중인 offline-only integration 변경 시도 |

Codex CLI 실행 실패는 program 설치와 local Marketplace 렌더링을 되돌리지 않는다. 결과는 `registration_state=manual_action_required`, 실패 단계와 secret 없는 `manual_commands[]`를 반환한다.

## 설치·update·repair·제거 순서

### 새 설치

1. 사용자가 경로와 optional task(Codex 연동, current-user 자동 시작)를 선택한다.
2. Installer가 architecture file set을 배치한다.
3. `star installation finalize`가 hash를 검증하고 bootstrap manifest·installation record를 쓴다.
4. 선택 시 `star integration install`이 Marketplace를 렌더링하고 공식 Codex command를 시도한다.
5. P-0039 설치본은 Controller autostart를 기본으로 만들지 않는다. Controller는 Hook/MCP가 필요할 때 verified bootstrap으로 시작하고, 관측된 모든 작업세션이 종료된 뒤 30초 유휴 lease를 거쳐 종료한다.
6. Installer는 Plugin 설치·새 작업·Hook 신뢰가 필요한지 완료 화면에 표시한다.

update·repair에서 task를 해제하면 자동 시작은 exact owned value를 제거하고, Codex 연동은 공식 deregistration을 best-effort로 시도한다. Codex 등록 해제가 확인되지 않으면 설치 update 자체를 되돌리지 않고 Marketplace source와 수동 제거 명령을 보존한다.

### update와 repair

- 같은 AppId가 이전 경로를 재사용한다.
- Installer EXE를 직접 실행하는 update·repair는 사용자가 Codex 앱을 완전히 종료한 뒤 Codex 밖의 별도 PowerShell에서 실행한다. Installer 자체는 실행 중인 Codex나 Star-Control process를 강제로 닫지 않는다. 실행 중 host에서 승인된 전환은 `star update offline-installer-restart`만 사용하며, detached Updater가 exact Codex census·10초 countdown·bounded 종료와 같은 Desktop 재실행을 소유한다.
- 설치 후 `codex_integration_update`는 Installer가 아니라 `star-updater.exe`만 처리한다. updater가 verified Codex process census와 Controller update lease를 확보한 뒤 `restart_armed`에서 정확히 10초를 세고, 새 mutation admission을 막고, 대상 Codex를 정상 종료 요청 후 exact identity fallback으로만 종료한다. MCP EOF와 owner-death가 확인된 뒤에만 Plugin·Hook·`.mcp.json`을 교체하고, postcheck 뒤 같은 Codex executable을 다시 시작한다. chat 주입·thread 재개·새 turn·중단 Tool replay는 없다.
- full/mixed replacement installer가 기존 v2 설치를 갱신하면 setup의 bridge initialize는 prior selector를 보존한다. detached Updater는 setup 성공 뒤 새 root manifest가 소유한 정확히 한 Runtime Generation을 선택·활성화하고, manifest-declared release ToolId 전체와 live declared/ready 집합의 exact equality를 검증한다. 실패하면 prior selector를 복원·기동하되 replacement fixed files가 남으므로 `partially_applied`를 기록하고, selector 복구 실패는 `rollback_failed`로 기록한다. setup exit 0과 file 교체만으로 update 완료나 full rollback을 표시하지 않는다.
- 새 version의 finalize가 전부 성공한 뒤 record를 바꾼다.
- Codex template hash 또는 install path가 바뀌면 `integration repair`가 새 version source를 렌더링하고 공식 add command를 다시 실행한다.
- 실패한 setup은 Inno Setup의 transaction rollback을 사용한다. 성공 뒤 이전 version으로 돌아가려면 보관한 이전 installer를 명시적으로 다시 실행한다.

### 제거

1. `star integration uninstall`이 공식 marketplace remove를 best-effort로 요청하고 Star-Control 소유 source를 제거한다.
2. `star controller autostart disable`은 exact owned value만 제거한다.
3. Inno Setup이 자신이 설치한 program file과 installation record를 제거한다.
4. 사용자·runtime 자료는 보존한다.
5. `/PURGEDATA`를 명시한 제거만 `%APPDATA%/Star-Control`과 installation/integrations를 포함한 `%LOCALAPPDATA%/Star-Control`을 제거한다. Project 자료는 어떤 경우에도 대상이 아니다.

## 검증

- template source에 PC별 absolute path가 없는지 검사
- x64·ARM64 stage의 file set, version, architecture, hash 검사
- 경로에 공백·한글·작은따옴표가 있어도 JSON과 Inno command가 보존되는지 검사
- install → status → repair → update simulation → uninstall → preserve/purge simulation
- fake Codex CLI로 marketplace add·plugin add·marketplace remove argv를 정확히 검사
- 실제 Codex CLI를 실행할 수 없는 환경에서 `manual_action_required`가 되는지 검사
- rendered Plugin validator와 JSON parse
- SessionStart exact snapshot, 고정 MCP tool reference와 ready action 0건의 native fallback 검사
- `cargo fmt --check`, target crate tests, workspace tests, package script check, `git diff --check`
- `legacy/` 무변경과 사용자 변경 보존 확인

## 공식 근거

- [Codex Plugin 만들기](https://learn.chatgpt.com/docs/build-plugins)
- [Codex Hooks](https://learn.chatgpt.com/docs/hooks)
- [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp)
- [Inno Setup DefaultDirName](https://jrsoftware.org/ishelp/topic_setup_defaultdirname.htm)
- [Inno Setup PrivilegesRequired](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)
- [Inno Setup ArchitecturesAllowed](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm)
