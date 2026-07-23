# Star-Control

Star-Control은 사용자가 CLI 또는 Codex 앱에서 지정한 개발 목표를 실행 단계로 나누고, 각 단계에 알맞은 실행 방식·검사 방법과 필요한 경우 모델·생각 깊이를 배정하는 Windows용 개발 제어 도구다.

결정적 계획·검사·상태·승인 경계는 Codex 없이 동작하는 CLI-only core가 담당한다. Codex 연동은 같은 application command를 사용하는 선택 소비자이며, 이 프로젝트는 Codex를 대체하거나 새로운 AI 실행기를 만들지 않는다.

현재 설계 정본은 [문서 안내](docs/README.md) 아래에만 둔다. 0~11단계 최종 설계는 서로 연결돼 있고 P-0041~P-0052가 첫 bounded 제품 Slice를, P-0054가 최신 `main`에서 Recovery Slice와 M1~M11의 내부 contract→engine→repository→Controller→CLI 경로 및 최종 16 Profile Catalog를 확장 구현했다. required core current source inventory 17개는 owning handler와 generated input/output Schema를 모두 가지지만 현재 Codex에 설치된 과거 runtime은 6개 action만 `ready`다. source candidate의 installed 17/17 실행 성공이나 외부 signer·publisher·authenticated remote·ARM64 native 실행을 추측하지 않는다. 현재 구현·검증·외부 Gate는 [PLANS.md](PLANS.md)와 [P-0054 실제 기능 완성 감사](docs/testing/p0054-functional-completion-audit-2026-07-23.md), 과거 출시 근거는 [P-0053 최종 감사](docs/testing/p53-final-release-audit-2026-07-20.md)에서 확인한다.

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
