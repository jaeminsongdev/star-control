# P-0074 SessionEnd Hook 3초 계약 교정·설치 감사

## 판정

Codex가 `SessionEnd` Hook의 `timeout: 10`을 3초로 clamp하던 원인을 source 계약에서 교정하고 Windows x64 설치본까지 Updater로 적용했다. source template은 `SessionEnd`만 정확히 3초를 선언하고, 설치된 `star.exe`는 Controller lifecycle 관찰을 2초 내부 deadline으로 제한한다. 다른 7개 Hook의 기존 10초 계약은 유지한다.

Codex cache나 Runtime DB를 직접 수정하지 않았다. complete release candidate를 inspect한 결과 `requires_codex_restart=true`였으므로 전용 Updater가 Codex를 재시작했고, terminal restart receipt와 새 rendered Marketplace·Plugin cache identity를 확인했다.

## 원인과 계약

- OpenAI Codex Hook 계약상 `SessionEnd` timeout은 기본 1초, 상한 3초다. 기존 Plugin source/rendered/cache는 10초를 요청해 host가 매번 clamp 경고를 냈다.
- 기존 일반 Controller IPC는 connect·response budget이 host 상한보다 길 수 있어, 선언만 3초로 낮추면 종료 시점이 보장되지 않았다.
- `SessionEnd`는 3초를 exact contract로 검증하고 Controller lifecycle report만 2초로 제한한다. timeout·Controller 부재는 기존 advisory 관찰 실패로 기록하되 Codex task를 실패시키지 않고 exit 0을 유지한다.
- `SessionStart`, `UserPromptSubmit`, `Stop`, `PreToolUse`, `PostToolUse`, `SubagentStart`, `SubagentStop`은 계속 10초다.

공식 계약: <https://developers.openai.com/codex/hooks/>

## Source 변경과 회귀 검증

구현 commit은 `d825516b86823793d53b607d6b2a0b6852d459fb` (`fix(codex): bound SessionEnd hook timeout`)이다.

| 영역 | 변경 |
|---|---|
| `integrations/codex-plugin-template/.../hooks/hooks.json` | `SessionEnd.timeout=3`, 다른 Hook은 10 유지 |
| `star-adapter-codex` | event별 exact timeout validator와 rendered drift 회귀 테스트 |
| `star-cli` | `SessionEnd` 전용 2초 lifecycle report deadline과 stalled report 취소 테스트 |
| validation·문서 | contract test와 architecture/install 계약 동기화 |

- 회귀 테스트를 구현보다 먼저 실행했을 때 `SessionEnd`가 `Some(10)`이라 `Some(3)` 계약에 실패했다.
- 교정 뒤 `star-adapter-codex` 25/25, `star-cli` unit 25개와 integration 1개가 통과했다.
- stalled lifecycle fixture는 약 2초에 취소됐고 Hook exit 0 계약을 유지했다.
- focused Clippy `-D warnings`와 `scripts/validation/contract-tests.ps1`가 exit 0이었다.
- source evidence가 stale했던 중간 FULL 10/11은 성공으로 사용하지 않았다. 최종 source byte에서 evidence를 재생성한 뒤 `target/validation/20260728T082414543Z-36512/report.json`이 FULL 11/11 complete·stable·pass, 146,876ms로 통과했다.

## Candidate와 Updater 적용

첫 candidate는 raw installed root의 uninstaller 등 manifest 밖 파일을 포함해 `reseal cannot add or remove staged files`로 거부됐다. 설치본은 변경되지 않았고, 삭제 승인이 없으므로 무시되는 `dist/stage/p0074-session-end-d825516b/x64`을 임의 삭제하지 않았다.

manifest-exact 이전 release stage에서 만든 최종 candidate는 다음과 같다.

| 항목 | 값 |
|---|---|
| stage | `dist/stage/p0074-session-end-d825516b-v2/x64` |
| source revision | `d825516b86823793d53b607d6b2a0b6852d459fb` |
| release manifest | `sha256:e8ca01784134db94002618005df8255e6d1eeae18c26ffbc8347a8f26cd85e64` |
| content set | `sha256:a0fe98c5b33b6eabb4f1b68cf7b0ef0b9a6101daad1881f694645a17acb04e34` |
| changed files | `star.exe`, Plugin `hooks/hooks.json` 정확히 2개 |
| inspect | `codex_integration_update`, restart 필요, rollback 가능 |
| approval scope | `sha256:e791f6cb2ef8b048c43ba9d0a0b9ee265f217b625a662b71e5942ed8566094d0` |

설치된 Updater가 operation `upd_PiQI_gFjk8cBNmcQ_ALzoEK5Az-Q8T9PXXtaBu6bkOc`를 적용했다. 최종 `update status`는 `state=exited`, affected instance 9를 기록하며, 같은 candidate 재검사는 `candidate_class=no_change`, changed file 0, restart 불필요다.

## 설치 identity와 기능 증거

| 경계 | 현재 identity |
|---|---|
| fixed install | `D:\도구\Star-Control`, verified x64, source `d825516b…`, outer manifest `sha256:e8ca0178…`, set `sha256:a0fe98c5…` |
| active Runtime | `rt_4f5e2b2ea6dbe52d`, activation revision 14, nested source `37f8ae88…`, nested manifest `sha256:0c899d98…`, set `sha256:4f5e2b2e…` |
| Codex integration | verified/registered, Plugin `0.1.0+codex.8499a0d181aa`, render `sha256:d5c894af…` |
| management | normal/read_write, `recovery_required=false` |

- source, rendered Marketplace와 Plugin cache 모두 `SessionEnd.timeout=3`이며 다른 Hook은 10이다.
- rendered/cache의 `.mcp.json`, `plugin.json`, `hooks.json`, `SKILL.md`, `agents/openai.yaml`, `routing-matrix.md` 6개는 모두 byte-identical이다. rendered/cache `hooks.json` SHA-256은 `sha256:f20b80cee24c8ae9aab8ad5cb7f56ac3bf50cb8c39c28453d4272a78b26013b7`다.
- paired installed Hook smoke `p0074-smoke-db832d378da84e6498f7b54b1bffd161`은 `SessionStart` 72ms/exit 0, `SessionEnd` 62ms/exit 0/empty stdout였다.
- 첫 smoke harness는 PowerShell JSON serialization overload 오류로 빈 stdin을 보내 exit 2가 났다. 이는 제품 실패로 사용하지 않았고 수정한 harness의 위 결과만 기능 증거로 사용했다.
- Doctor는 4/4 pass다.
- post-install TARGET operation `opn_01KYKY7XAFJ5RN9NFTQ3ZB906F`은 `target/validation/20260728T084244340Z-39820/report.json`, 8/8 complete·stable·pass, 128,961ms, evidence `sha256:052123a1bb2dd4282b631a57e21f4acf3036f2042ef97e93de302198929ec991`로 terminal `succeeded`였다.
- 설치 직후 첫 MCP 호출의 `IPC_AUTH_FAILED`와 기본 auto-wait operation `opn_01KYKY0Z3QHBKMR4WW8QA9A2QD`의 `VALIDATION_CANCELLED`는 pass로 처리하지 않았다. live search→describe를 다시 수행하고 `wait_mode=accepted`로 분리한 위 terminal operation만 수용했다.
- closure FULL의 MCP operations `opn_01KYKYJJDWKQRB3883GZEZ4HG7`, `opn_01KYKYQZVP9J89N3VE19VJCXDZ`는 Controller idle shutdown으로 `TOOL_CANCELLED`돼 Gate 증거로 사용하지 않았다. AGENTS.md가 정한 동일 canonical entrypoint를 native fallback으로 실행한 `target/validation/20260728T085348769Z-40484/report.json`은 FULL 11/11 complete·stable·pass, 179,872ms였다. 이 fallback에는 Star-Control terminal FULL evidence가 없으므로 closure acceptance에는 같은 canonical entrypoint의 final-source terminal report가 별도로 필요하다.

## 남은 외부·사용자 경계

- `hook_trust_required=true`, `requires_new_task=true`는 의도된 사용자 보안 경계다. 제품은 Codex trust DB/cache를 직접 수정하지 않는다.
- public Stable은 Authenticode certificate/private key와 trusted timestamp가 없어 `blocked_external`이다.
- ARM64를 포함한 non-x64 build·native evidence와 remote push·PR·publish는 이 Slice 범위 밖이다.
