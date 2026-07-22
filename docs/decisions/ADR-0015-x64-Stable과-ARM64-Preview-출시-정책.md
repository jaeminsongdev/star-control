# ADR-0015: x64 Stable과 ARM64 Preview 출시 정책

## 상태

- 상태: 채택
- 결정일: 2026-07-20
- 적용 단계: P-0040, P-0051, P-0053

## 맥락

Star-Control의 공개 Windows target은 x64와 ARM64지만 현재 검증 환경에는
native ARM64 장치가 없다. cross-build와 architecture 검사는 ARM64 byte의
구조를 증명할 수 있어도 native process, IPC, Controller, CLI, MCP와 설치
수명주기를 통과했다는 근거가 될 수 없다. 반대로 ARM64 실기가 없다는 이유로
검증된 x64 release를 영구히 막거나 ARM64 결과를 native 성공으로 승격해서도
안 된다.

현재 repository의 제품 version은 root `Cargo.toml`의 `0.1.0`, 최소 Rust
version은 `1.96`이다. 설치 Runtime은 `star.exe`, `star-controller.exe`,
`star-mcp.exe`, `star-updater.exe` 네 개이며 required core manifest는 17개
action을 선언한다. 2026-07-12의 13-action 감사는 당시 snapshot으로 보존하되
현재 inventory로 재해석하지 않는다.

## 결정

1. 최초 공개 version은 `v0.1.0`이고 publication destination은
   `jaeminsongdev/star-control`의 GitHub Releases다. 별도 server deploy는
   release 범위가 아니다.
2. `x86_64-pc-windows-msvc` artifact는 **Stable**이다. native x64 build,
   release 검증, clean install, first run, update, failure rollback, repair,
   uninstall과 사용자 자료 보존 evidence가 모두 required Gate다.
3. `aarch64-pc-windows-msvc` artifact는 **Preview**다. cross-build provenance,
   PE architecture, package file manifest, final Authenticode signature,
   installer model과 fake lifecycle을 required evidence로 삼고 지원 상태를
   정확히 `native_unverified`로 기록한다.
4. ARM64 Preview evidence는 native ARM64 process·IPC·Controller·CLI·MCP 또는
   실제 install lifecycle 성공으로 승격되지 않는다. 이 limitation은 x64
   Stable Gate를 채우지 않으며, native ARM64 Stable 지원은 별도 versioned
   정책 변경과 실기 evidence 뒤에만 선언한다.
5. 두 architecture의 public Runtime EXE와 installer는 Authenticode 검증을
   통과해야 한다. certificate 또는 timestamp provider가 없으면 release를
   `blocked_external`로 유지하며 unsigned artifact를 Stable로 낮춰 공개하지
   않는다.
6. 최종 candidate 생성 순서는 Runtime EXE 서명, installer 생성, installer
   서명, 최종 digest와 file manifest 재계산이다. 서명으로 byte가 바뀌면 새
   candidate이며 unsigned 검증 결과를 상속하지 않는다.
7. `ready`, exact publication approval, `published`를 분리한다. tag, GitHub
   draft, asset upload와 publish는 각각 exact action 승인을 요구한다.
   timeout 뒤에는 같은 upload를 자동 재시도하지 않고 read-only reconcile로
   exact remote asset digest를 확인한다.
8. required core의 current inventory는 17개다. owning handler와 generated
   input·output Schema가 모두 연결된 action만 `ready`이며 P-0053에서 17/17을
   current search·describe·invoke 및 Codex·Inspector evidence로 다시 증명한다.

## 결과

- x64 Stable과 ARM64 Preview의 evidence 요구가 서로 다른 support tier로
  표현되며 partial·unverified 결과가 pass로 승격되지 않는다.
- ARM64 실기 부재는 Preview의 명시적 limitation으로 남고 x64 Stable의 native
  결과를 오염시키지 않는다.
- 서명 credential 부재는 비용·외부 환경 blocker로 드러나며 unsigned Stable
  우회가 없다.
- 공개 원격 효과는 GitHub Release asset publication 하나로 제한된다.

## 대체·유지 관계

- ADR-0010의 build-once, byte 변경 시 새 candidate, ready·approved·published
  분리 원칙은 유지한다.
- 기존 문서에서 native ARM64 evidence를 x64 공개 release 전체의 필수 Gate로
  묶은 부분은 이 결정의 architecture별 support tier 정책으로 대체한다.
- ADR-0014의 4 Runtime EXE와 updater lifecycle 결정은 유지한다.

## 관련 정본

- [10단계 CI·Release·평가 계약](../contracts/ci-release-evaluation-and-product-completion.md)
- [Windows 설치와 Codex 연동 계약](../contracts/windows-installation-and-codex-integration.md)
- [설치와 공개 배포](../operations/installation.md)
- [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)
