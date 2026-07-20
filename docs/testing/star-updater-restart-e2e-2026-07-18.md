# Star Updater restart E2E — 2026-07-18

## 범위

Windows x64의 기존 3-EXE 설치본을 4-EXE offline installer package로 교체하면서
실제 Codex Desktop의 10초 종료와 자동 재실행을 검증했다. 최종 설치본은 updater를
current-user staging copy로 만든 뒤 local WMI `Win32_Process.Create` broker에서
시작한다. 기존 task에 채팅을 주입하거나 Codex cache·config·trust state를 직접
수정하지 않았다.

## 성공 증거

- latest installed-root self-update receipt
  `upd_pYIT2_noIHBkipYbFgdecEI4h6GPkH7AtU6XwrDhSNI`의 final state는 `exited`다.
- 10초 뒤 생성된 새 Desktop root PID `2372`의 parent PID는 staged updater
  `28512`였다. 즉 Codex를 닫은 process가 Desktop 재실행까지 생존했으며, 사용자의
  수동 재실행은 없었다.
- 자동 재시작 E2E 당시 설치 root ReleaseManifest set hash는 당시 x64 stage
  `sha256:a192f5659fba47d5d2ce9da3d009ab0043058e4e7cec5cceccdf162a72f66f2e`와
  일치했다. 후속 `clippy`-only source 정리 뒤 최종 x64 package stage는
  `sha256:f2afe5a351f98253b8f96c732883e978eaf16d100c4f48d162b7bfbf67f0eb02`로
  다시 생성·verify했으며 restart state-machine의 동작 변경은 없다.
- 실제 root에는 `star.exe`, `star-controller.exe`, `star-mcp.exe`,
  `star-updater.exe`가 모두 있고, 네 EXE SHA-256은 final x64 release stage와
  각각 일치했다. `star installation status --json`은 `verified`,
  `star integration status --json`은 `verified/registered`를 반환했다.
- ARM64 package도 local Rust 1.96.0 toolchain으로 최신 source에서 다시 생성했고,
  stage verifier는 187 files·4 EXE·set
  `sha256:d856d923d7bb9ebd5f81e3692c638801cfc0e758845fc195c291a160d5ea958a`를
  `verified`로 확인했다.
- installer extraction log는 Star-owned fixed temp directory에 기록됐다.

## 실패를 통해 고정한 경계

1. legacy 3-EXE controller handoff 실패가 installer 진입을 막지 않는다.
2. desktop tree fallback은 updater own subtree를 종료하지 않는다.
3. updater는 direct Codex child나 Shell 분리에 의존하지 않고 staged local WMI
   broker launch를 사용해야 한다.
4. reparse-point TempLink는 installer TEMP로 쓰지 않는다.

## 실패 recovery 증거

- installer 실행 자체가 실패하는 fixture(`where.exe`, updater가 주는 Inno 인수를
  거부해 exit 2)를 사용했다. 실제 transaction은 10초 뒤 Desktop을 닫고,
  `upd_5N1pIE9JHDRnYMtdGFywmX7Q-FIX64pY7D_-wJ_rZVU` receipt에
  `rollback_required`를 기록한 다음 자동 재실행했다.
- 새 Desktop root PID `26224`의 parent PID는 해당 staged updater `15188`이다.
  따라서 실패 recovery에서도 사용자의 수동 Codex 재실행은 없었다.
- 이 fixture는 installer가 file mutation 전에 실패하므로 4 EXE hash가 final stage와
  모두 동일하게 유지됨을 확인했다.

## 실제 integration apply/rollback 증거

- 동일 installation root의 manifest-declared file만 복제한 후보 stage에서 Plugin
  description을 의도적으로 invalid 값으로 바꾸고, package-stage 전용 `reseal`로
  manifest hash를 다시 봉인했다. 이 후보는 `codex_integration_update`로 분류됐고
  hash-bound approval scope로만 실행됐다.
- 첫 실행은 Codex 종료 뒤 사라진 Controller handoff를 strict failure로 취급해
  `aborted` receipt `upd_edKf-rFzgAO0uBIfrIj0YO9oar_vkdpO9Gin0f2UtRE`를 남기고
  relaunch를 놓쳤다. 이 회귀를 수정해, handoff 부재에서는 exact-root drain을
  계속하고 abort path도 relaunch를 시도하게 했다.
- 수정 뒤 실제 후보 실행 receipt
  `upd_iK7hJmgMjo4ZFv6eRk07ryB1TnSt8He0xC8jw9qt-mc`는 `rolled_back`이다. 새
  Desktop root PID `1852`의 parent PID는 staged updater `15180`이므로 사용자
  수동 실행 없이 rollback 뒤 자동 재실행됐다.
- `integration-backups/upd_iK7hJmgMjo4ZFv6eRk07ryB1TnSt8He0xC8jw9qt-mc`가
  남았고, `star installation status --json`은 `verified`, Plugin description은
  원래 값으로 복구됐음을 확인했다. 따라서 실제 Desktop을 종료한 integration
  file-set apply → postcheck failure → rollback → relaunch 경로까지 검증됐다.

## 남은 외부 경계

- Plugin의 새 task/SessionStart Hook trust는 Codex 공식 UI에서 사용자 검토가
  필요하며 자동 승인하지 않는다.
- native ARM64 실행·설치, code signing과 공개 배포는 이 E2E 범위 밖이다.
