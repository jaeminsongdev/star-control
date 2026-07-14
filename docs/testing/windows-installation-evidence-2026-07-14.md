# Windows 설치·Codex Plugin 로컬 실증 — 2026-07-14

## 목적과 판정 범위

이 문서는 P-0026 Windows 설치 transport를 현재 x64 PC에서 비파괴적으로 검증한 근거다. x64·ARM64 설치 파일 생성과 정적 architecture 검증, x64 실제 설치·복구, Codex Plugin 실제 설치까지 다룬다. native ARM64 실행·설치, 실제 제거·`/PURGEDATA`, code signing, 공개 배포를 통과했다는 근거로 사용하지 않는다.

## 패키지 산출물

| 항목 | 검증 값 |
|---|---|
| x64 stage | 77 files, `sha256:64b65da4f62e8c810c4d0a6577dc28566aa90130a586c5390aa540f0f863bf9b` |
| ARM64 stage | 77 files, `sha256:189d646185d1998018d59ffde887732da88a7660111a50fa63a0f4a9bed6e049` |
| x64 installer | `fcf092bd7d244a463d2c6295242c81207348a9529a5d35aac632e1982605a88f` |
| ARM64 installer | `ecc042ad0e712726b2e339498f66eb6ee56b425d7f32f264669903262ead78c7` |
| compiler | Inno Setup 6.7.3 |
| signing state | `unsigned_local`; 세 설치 binary와 두 installer 모두 `NotSigned` 확인 |

두 stage는 `star-package-release verify`로 exact file set, manifest hash와 세 PE binary의 machine type을 다시 확인했다. ARM64는 cross-build·stage 검증 근거이며 이 x64 PC에서 native runtime을 실행했다는 뜻이 아니다.

## 실제 x64 설치·복구

- installer를 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR="D:\도구\Star-Control"`로 실행했고 exit code `0`을 확인했다.
- `star installation status`는 `verified=true`, `install_root=D:\도구\Star-Control`, `product_version=0.1.0`, `target_architecture=x64`를 반환했다.
- 설치본은 `release-manifest.json`, `star-control-install.v1.json`, `star.exe`, `star-controller.exe`, `star-mcp.exe`와 Inno Setup 제거기를 포함한다.
- `star controller autostart status`는 `enabled`였고 HKCU Run 값은 `"D:\도구\Star-Control\star-controller.exe" --background`와 정확히 일치했다.
- 유효한 `SessionStart` stdin JSON은 exit code `0`과 `continue=true` 출력을 냈다. 다른 event는 exit code `2`로 거부됐다.
- `/PURGEDATA`는 `ParamCount`·`ParamStr`로 대소문자만 무시한 exact 인자일 때만 활성화되며 부분 문자열은 purge 승인이 아니다.
- 새 local data directory는 이미 존재하는 가장 가까운 상위 directory의 fixed-volume·reparse 안전성을 먼저 확인한 뒤 생성한다. 상대 경로 거부가 directory를 남기지 않는 회귀 test를 통과했다.

## Codex Plugin 실제 설치

- source Plugin과 설치 과정에서 렌더링된 Plugin 모두 공식 validator를 통과했다.
- 로컬 Marketplace의 `source.path`는 Marketplace root 기준 `./plugins/star-control`이고 실제 Plugin directory와 일치한다.
- 렌더링된 `.mcp.json`은 `D:\도구\Star-Control\star-mcp.exe`를, `hooks/hooks.json`은 `"D:\도구\Star-Control\star.exe" hook session-start`를 가리킨다.
- Codex 앱 Plugin 화면에서 `Star Control`을 설치했고 설치 완료 알림, `MCP 서버 1`, 활성화된 `스킬 1`을 확인했다.
- Codex cache `C:\Users\thdqu\.codex\plugins\cache\star-control-local\star-control\0.1.0+codex.f1aa1a021fd8`에 같은 MCP·Hook 경로가 설치된 것을 확인했다.
- Hook은 화면에 `검토 필요`로 표시되며 신뢰하지 않았다. Plugin 설치와 Hook 실행 신뢰를 분리한다는 계약을 지킨 상태다.

Store 앱 Codex CLI를 installer process에서 직접 실행할 수 없어 제품의 `CodexIntegrationRecord.registration_state`는 `manual_action_required`로 남는다. 이는 CLI 자동 등록을 검증하지 못했다는 기록이다. Codex 앱에서 수행한 실제 Plugin 설치 근거와 합쳐 해석하며, record를 임의로 `registered`로 고치지 않는다. 새 Codex 작업에서 MCP를 불러오는 단계는 Plugin 설치 후 session 경계다.

## 검증 명령과 결과 요약

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo run --locked -p star-schema-gen -- --check`
- `cargo run --locked -p star-matrix-check` → `expected=170`, `mapped=170`, `missing=[]`
- `star-package-release verify` x64·ARM64 → 각각 `verified=true`
- Plugin validator source·rendered cache → PASS
- Inno Setup x64·ARM64 compile과 x64 repair installer 실행 → PASS

한 번의 workspace test에서 `unused_stdin_is_closed_so_the_child_receives_eof`가 500 ms process startup 예산을 넘겼다. 테스트 의도는 유지한 채 timeout을 2초로 조정했고, 해당 test 10회 반복과 전체 workspace test를 다시 통과했다.

최종 Markdown link, 변경 범위, `git diff --check`, secret scan과 `legacy/` 무변경 검사는 P-0026 완료 시점의 `PLANS.md`가 최신 판정을 소유한다.
