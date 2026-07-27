# P-0073 Codex Plugin·Skill·Hook 적용 감사

## 판정

Windows x64 설치본의 Codex 통합을 현재 Code Health·Runtime update 기능에 맞게 갱신했다. 단일 `star-control-operations` Skill의 progressive-disclosure route를 확장하고, main session 종료를 기존 bounded `root_stop` lifecycle로 관찰하는 `SessionEnd` Hook을 추가했다. source, complete release, installed fixed Bridge, active Runtime, rendered Marketplace와 Codex cache를 별도 identity로 검증했다.

공개 Authenticode 서명·timestamp와 non-x64 native evidence는 이 Slice가 주장하지 않는다. Codex Hook 신뢰는 제품이 직접 변경하지 않는 사용자 보안 결정이며 `integration status`가 계속 명시한다.

## 설계 경계

- 별도 Code Health Skill을 만들지 않고 기존 `star-control-operations`와 `references/routing-matrix.md`를 확장했다. implicit invocation 중복과 route 경쟁을 만들지 않는다.
- `SessionEnd`만 새 Hook으로 추가해 기존 `root_stop` 관찰에 연결했다. `SessionStart(source=compact)`가 context를 재주입하므로 `PreCompact`·`PostCompact`는 중복 등록하지 않았다.
- exact Star-Control PermissionPlan/Approval binding이 없는 `PermissionRequest`는 등록하지 않았다. Hook은 lifecycle/context guardrail이며 product action 승인기가 아니다.
- Hook event set과 Plugin 7-component inventory는 closed contract로 검증한다. 향후 fixed Bridge/Plugin-only 후보는 새 outer source revision과 기존 nested Runtime source revision을 분리한 `reseal-integration`으로 봉인한다.

## Source와 검증

| 항목 | 결과 |
|---|---|
| 구현 commit | `038b2dbc340d7238866171725b87254a2f41ec37` |
| installer Hook 안내 commit | `37f8ae880786179c5a0f42550f79aa88502afb74` |
| live Hook 안내 commit | `0d25948c8d8c560a067530ada0eb8ef207e60d82` |
| Profile resolution | 8개 Profile, `sha256:23d783bff3df3ac39684ca52bad60cf870d2af606886f33ab8543459548fef43` |
| Plugin validator | pass |
| Skill validator | pass |
| focused packages | `star-adapter-codex` 23/23, `star-cli` 23+1, `star-package-release` 8/8 pass |
| affected Clippy | `-D warnings` pass |
| product inventory | feature 23/23, Schema 217, MCP 170/170, Profile 16/16, Runtime EXE 4/4 |
| implementation FULL | `target/validation/20260727T214843209Z-42900/report.json`, 11/11 complete·stable·pass, 205,055 ms |

STRICT review에서 수정한 주요 결함은 integration-only outer manifest가 새 Bridge byte에도 과거 source revision을 유지하던 provenance 오류, Plugin UI metadata의 closed validator drift, installer/live status가 `SessionEnd` trust를 누락한 안내 불일치다. validator나 test를 약화하지 않고 canonical 상수·계약 테스트·실제 candidate inspect로 닫았다.

## x64 package와 Updater 적용

complete stage는 `dist/stage/p0073-complete-37f8ae88/x64`다.

| identity | 값 |
|---|---|
| source revision | `37f8ae880786179c5a0f42550f79aa88502afb74` |
| file set | 541 files, `sha256:a5f6d2000215b09f317ef41e1ad498469f9ce675bedca58d8d0eb355ddf61d51` |
| bundled Runtime | `rt_4f5e2b2ea6dbe52d` |
| installer | `dist/installers/p0073-37f8ae88/star-control-windows-x64-0.1.0-setup.exe` |
| installer SHA-256 | `sha256:459358eba13ae07172d9b9c077b081e64bb67e44704a0e503e4ddbe585e20e39` |
| signing | `NotSigned` / `unsigned_local` |

설치는 installer 직접 실행이 아니라 설치된 `star update offline-installer-restart`로만 시작했다. 최초 receipt `upd_nw9J1sTjLDyn8KNDfqEpKU7dGV9yWkwg-Sxu_pPn9C4`는 fixed file 적용 뒤 새 Runtime postcheck를 닫지 못해 `partially_applied`를 남겼고 selector를 기존 generation으로 복구했다. 설치 root manifest가 complete 후보와 byte-identical이고 `update inspect`가 `no_change`임을 확인한 뒤, 해당 상태 전용 `update reconcile-installed-runtime`을 실행했다. operation `upd_uXMl65d2zDAvHJFwvW4KiwRtXMirrhItSujFMaE0sE0`이 fallback termination 0건으로 `rt_4f5e2b2ea6dbe52d`를 activation revision 14에 활성화했다.

live Hook trust 안내 수정은 `dist/stage/p0073-status-0d25948c/x64`에 `star.exe` 한 파일만 바뀐 integration-only 후보로 봉인했다.

| identity | 값 |
|---|---|
| outer source revision | `0d25948c8d8c560a067530ada0eb8ef207e60d82` |
| candidate set | `sha256:a03a4ca9c74492701699f449c9aa6c8c7d9b76888638beb4b1dda7ddee17ce2d` |
| candidate manifest | `sha256:cba96694bbba52c469502157d91c230b7757da1d187f5c531d2639ca65edb807` |
| approval scope | `sha256:1c58618271849fe51cd6333d554714c5499afbbd26c7e9cf6b8b6a31cc1494f2` |
| classification | `codex_integration_update`, restart/rollback true |
| terminal receipt | `upd_NgVLVjteHvdyttyn8RM3m_2iD5VaH-v7VhHg9t2zW7k`, `exited`, 9 Codex instances |

적용 뒤 같은 candidate를 다시 inspect한 결과는 `no_change`다. outer install manifest는 `0d25948c…`를 기록하고, unchanged nested Runtime release manifest는 `37f8ae88…`와 content set `sha256:4f5e2b2ea6dbe52d11995055f2dd9ca7095e8a63cfddecb9afe8b83f1c0614f6`를 유지한다.

## 설치 후 기능 증거

- installation: verified, x64, source `0d25948c…`, outer set `sha256:a03a4ca9…`.
- Runtime: active `rt_4f5e2b2ea6dbe52d`, activation revision 14.
- integration: verified/registered, Plugin `0.1.0+codex.bf4f8319e732`, render `sha256:47d4ff6838680e6695d89c1e9c57d61ff60d4232ae9abfa71e6d9d0c78d39a91`.
- rendered Plugin과 Codex cache의 `.mcp.json`, `plugin.json`, `hooks.json`, `SKILL.md`, `agents/openai.yaml`, `routing-matrix.md` 6개는 모두 byte-identical이다. Marketplace source까지 포함한 rendered inventory는 정확히 7개다.
- Hook set은 `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `Stop`, `PreToolUse`, `PostToolUse`, `SubagentStart`, `SubagentStop` 정확히 8개다. rendered `SessionEnd.commandWindows`는 설치된 `D:\도구\Star-Control\star.exe` 절대 경로다.
- paired Hook smoke는 `SessionStart` context/continue와 `SessionEnd` empty output이 각각 exit 0이었다.
- live `integration status`는 SessionStart와 SessionEnd를 포함한 Hook trust 검토를 함께 안내한다.
- Doctor는 4/4 pass다.
- 설치 후 TARGET은 `target/validation/20260727T215721702Z-33952/report.json`, 8/8 complete·stable·pass, 161,816 ms다. 첫 호출의 `IPC_AUTH_FAILED`는 성공으로 처리하지 않았고 search→describe를 다시 수행한 재시도에서 terminal pass를 확인했다.

## 남은 외부·사용자 경계

- `hook_trust_required=true`, `requires_new_task=true`는 의도된 상태다. 사용자는 Codex `/hooks`에서 exact Star-Control Hook을 검토·신뢰해야 하며 제품은 trust DB/cache를 직접 수정하지 않는다.
- public Stable은 Authenticode certificate/private key와 trusted timestamp가 없어 `blocked_external`이다.
- ARM64를 포함한 non-x64 build·native evidence는 이 Slice 범위 밖이다.
- 원격 push, PR, publish는 수행하지 않는다.
