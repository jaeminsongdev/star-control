# ADR-0012: 선택형 Windows 설치와 Codex Plugin 연동

## 상태

승인됨 — P-0026 구현 기준

## 배경

Star-Control은 Windows current-user 제품이며 사용자가 설치 위치를 바꿀 수 있어야 한다. 이 PC에서는 `D:\도구\Star-Control`을 사용하지만, 배포본이 특정 PC의 드라이브나 폴더를 정답으로 가정하면 안 된다. 또한 Codex가 `star-mcp.exe`를 실행하려면 설치 뒤 확정된 절대 경로가 필요하다. 정적 Plugin source나 MCP 실행 파일에 그 경로를 하드코딩하면 설치 위치 변경 때마다 다시 빌드하고 등록해야 한다.

Codex Plugin은 MCP 설정과 Hook을 함께 배포할 수 있지만, 설치된 Plugin source는 Codex가 관리하는 cache로 복사된다. Star-Control은 그 cache를 직접 쓰거나 Hook 신뢰를 우회하면 안 된다.

## 결정

1. Windows 설치본은 Inno Setup으로 만드는 architecture별 current-user `.exe` 설치 파일을 기본 배포 형식으로 사용한다.
2. 공개 기본 설치 경로는 `{localappdata}\Programs\Star-Control`이다. 설치 마법사에서 사용자가 경로를 바꿀 수 있고, 같은 AppId의 update·repair는 이전 경로를 기억한다.
3. x64와 ARM64 설치 파일을 따로 만든다. 한 설치 파일에 서로 다른 architecture binary를 섞지 않는다.
4. 설치 root에는 사용자 실행 파일을 `star.exe`, `star-controller.exe`, `star-mcp.exe` 세 개만 둔다. installer helper와 updater를 별도 상주 EXE로 추가하지 않는다.
5. package의 `release-manifest.json`은 상대 경로와 SHA-256을 가진 immutable release-file manifest다. `%LOCALAPPDATA%\Star-Control\installation\installation-record.v1.json`은 실제 절대 설치 경로와 설치 instance를 가진 machine-local record다. 두 문서는 역할을 합치지 않는다.
6. 기존 `star-control-install.v1.json`은 같은 폴더의 Controller와 gateway hash를 묶는 bootstrap 보안 manifest로 유지한다. release-file manifest나 machine-local record로 대체하지 않는다.
7. Installer는 설치된 `star.exe integration install`을 호출한다. 이 command가 Plugin template을 실제 경로로 렌더링하고, `%LOCALAPPDATA%\Star-Control\integrations\codex\<version>\marketplace-root`에 Star-Control 소유 로컬 Marketplace를 만든다.
8. 정적 source에는 PC별 절대 경로를 넣지 않는다. 렌더링된 `.mcp.json`과 `hooks/hooks.json`만 실제 `star-mcp.exe`와 `star.exe` 절대 경로를 가진다.
9. Codex cache, `config.toml`, Hook trust 저장소를 직접 수정하지 않는다. `codex plugin marketplace add <root>`와 `codex plugin add star-control@star-control-local`을 best-effort로 실행하고, 실행할 수 없으면 같은 명령과 Codex 앱의 후속 조치를 상태 결과로 반환한다.
10. Plugin을 설치하거나 갱신한 뒤에는 새 Codex 작업이 필요하다. Plugin Hook은 사용자가 Codex에서 검토·신뢰해야 한다. Installer나 Star-Control은 이 신뢰를 자동 승인하지 않는다.
11. Controller 자동 시작은 HKCU Run을 사용하고 설치 task로 켜고 끌 수 있다. Windows service, 관리자 권한 설치와 machine-wide 설정은 사용하지 않는다.
12. 제거는 program file, Star-Control 소유 Marketplace source, installation record와 자동 시작 entry만 제거한다. `%APPDATA%\Star-Control`, `%LOCALAPPDATA%\Star-Control`의 사용자·runtime 자료는 기본 보존하며 명시적 purge에서만 제거한다. Project의 `.star-control`, `.ai-runs`는 purge에도 포함하지 않는다.
13. 별도 network updater는 만들지 않는다. update는 새 installer를 승인해 실행하고, rollback은 보관한 이전 installer를 다시 실행하는 방식으로 한다. 공개 배포 전 code signing은 release Gate지만 P-0026 로컬 구현에서 유료 서명이나 공개는 수행하지 않는다.
14. portable ZIP은 개발·복구용 보조 산출물이다. 설치·update·제거 수명주기의 정본은 installer다.

## 결과

- 설치 위치를 바꿔도 binary 재빌드 없이 Plugin 설정을 다시 렌더링할 수 있다.
- 외부 개발 도구는 기존 ToolPackage TOML을 바꾸는 것으로 추가되며 MCP 재등록과 무관하다.
- Installer, Plugin source, Codex cache와 사용자 데이터의 소유권이 분리된다.
- Codex CLI가 없거나 Store 설치본 실행이 제한된 환경에서는 제품 설치는 성공할 수 있지만 Codex Plugin 상태는 `manual_action_required`가 된다.
- 경로 이동은 폴더 복사가 아니라 새 위치에 installer를 실행한 뒤 `star integration repair`로 연동을 다시 만드는 절차다.

## 제외

- local AI와 다른 AI provider
- OpenAI API 직접 호출
- browser UI와 HTTP control UI
- Windows service와 machine-wide 설치
- Codex cache·Hook trust 내부 파일 직접 변경
- 자동 network update와 백그라운드 예약 실행
- 공개·서명 credential 사용

## 연결

- [Windows 설치와 Codex 연동 계약](../contracts/windows-installation-and-codex-integration.md)
- [설치·업데이트·제거](../operations/installation.md)
- [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)
- [Repository·Package 구조](../architecture/repository-layout.md)
