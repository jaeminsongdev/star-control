# Star-Control

Star-Control은 사용자가 CLI 또는 Codex 앱에서 지정한 개발 목표를 실행 단계로 나누고, 각 단계에 알맞은 실행 방식·검사 방법과 필요한 경우 모델·생각 깊이를 배정하는 Windows용 개발 제어 도구다.

결정적 계획·검사·상태·승인 경계는 Codex 없이 동작하는 CLI-only core가 담당한다. Codex 연동은 같은 application command를 사용하는 선택 소비자이며, 이 프로젝트는 Codex를 대체하거나 새로운 AI 실행기를 만들지 않는다.

현재 설계 정본은 [문서 안내](docs/README.md) 아래에만 둔다. 0~11단계 최종 설계는 서로 연결돼 있고 P-0041~P-0052가 첫 bounded 제품 Slice를, P-0054/P-0055가 Recovery Slice와 M1~M11의 내부·비서명 외부 경로를 확장했다. P-0056은 최신 `main`에서 다시 전수 감사해 v2 validation/planning과 portable recovery, M7/M8 effect receipt·current evidence, M9 handoff, M10 precommitted evaluation case/policy·verified cost/budget·actual Finding/Suppression/Radar와 23개 기능·16 Profile final audit를 실제 제품 경로에 연결한다. required core source와 현재 설치된 P-0055 runtime은 모두 owning handler·generated Schema를 가진 17/17 action이 `ready`다. 새 source candidate의 설치·출시 증거, 외부 signer와 ARM64 native 실행은 별도로 검증하며 추측하지 않는다. 현재 판정은 [PLANS.md](PLANS.md)와 [P-0056 최신 기능·복구 감사](docs/testing/p0056-current-functional-recovery-audit-2026-07-24.md), 과거 seal은 [P-0054 감사](docs/testing/p0054-functional-completion-audit-2026-07-23.md)와 [P-0055 감사](docs/testing/p0055-nonsigning-external-seal-2026-07-23.md)에서 확인한다.

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
