# Star-Control

Star-Control은 사용자가 CLI 또는 Codex 앱에서 지정한 개발 목표를 실행 단계로 나누고, 각 단계에 알맞은 실행 방식·검사 방법과 필요한 경우 모델·생각 깊이를 배정하는 Windows용 개발 제어 도구다.

결정적 계획·검사·상태·승인 경계는 Codex 없이 동작하는 CLI-only core가 담당한다. Codex 연동은 같은 application command를 사용하는 선택 소비자이며, 이 프로젝트는 Codex를 대체하거나 새로운 AI 실행기를 만들지 않는다.

현재 설계 정본은 [문서 안내](docs/README.md) 아래에만 둔다. 0~11단계 최종 설계와 P-0054~P-0056의 historical seal 위에서 P-0057은 실행 중 설치본, live source와 과거 artifact를 revision·byte별로 다시 분리해 점검했다. 현재 source에서는 empty-v1 migration·active-set reseal, Registry verbatim path 중복 집계와 destructive core action의 EffectiveConfig approval 누락을 수정했다. 실행 중 source `b20d234` Runtime은 설치·Codex 연동·17 action readiness가 verified이지만 이 수정 전 byte이므로 update는 별도 승인 전까지 held다. x64 unsigned candidate와 ARM64 cross-build는 격리 검증했고, 공개 Stable은 Authenticode·trusted timestamp와 signed-byte lifecycle/publish가 없어 계속 fail-closed다. 현재 판정은 [PLANS.md](PLANS.md), [P-0057 현재 시스템 전수 점검](docs/testing/p0057-current-system-audit-2026-07-25.md), 과거 exact seal은 [P-0056 감사](docs/testing/p0056-current-functional-recovery-audit-2026-07-24.md)에서 확인한다.

## 현재 원칙

- AI 연동은 Codex만 지원하지만 CLI-only core는 AI 없이 동작한다.
- 로컬 AI와 다른 AI 제공자는 지원하지 않는다.
- OpenAI API를 직접 호출하지 않는다.
- Windows만 지원한다.
- 공개 version은 `v0.1.0`, destination은 GitHub Releases다. x64는 signed Stable, ARM64는 cross-build·simulation 기반 `native_unverified` Preview다.
- current Rust baseline은 `1.96`이고 설치 Runtime은 `star.exe`, `star-controller.exe`, `star-mcp.exe`, `star-updater.exe` 네 개다.
- 브라우저 화면은 만들지 않고 Codex 앱과 터미널을 사용한다.
- compiler, scanner, debugger, profiler, package manager, CI·installer·signing·deploy 서비스를 다시 구현하지 않는다.
- 레거시는 로컬 참고자료일 뿐 현재 설계 기준이 아니다.
- 코드 구현은 명시된 P-ID와 수직 Slice 경계 안에서만 진행한다.
