# Windows 설치 패키지

이 폴더는 [Windows 설치와 Codex 연동 계약](../../docs/contracts/windows-installation-and-codex-integration.md)에 따른 current-user Inno Setup 6 설치 파일을 만든다. 공개 배포 상태기계와 서명은 이 폴더의 책임이 아니다.

## 산출물 만들기

```powershell
.\packaging\windows\build-installer.ps1 -Architecture x64
.\packaging\windows\build-installer.ps1 -Architecture arm64
```

같은 version의 기존 `dist/stage/<version>/<architecture>`가 비어 있지 않으면 덮어쓰지 않는다. 검증된 stage를 의도적으로 다시 만들 때만 `-ReplaceStage`를 사용한다. 이 switch는 `dist/stage` 아래에서 확인된 정확한 architecture 폴더에만 적용된다.

개발·복구용 ZIP이 별도로 필요할 때만 `-PortableZip`을 추가한다. 로컬 package는 항상 `unsigned_local`이며 실제 signer와 서명 검증이 구현되기 전에는 signed 상태를 선택할 수 없다.

## 설치와 확인

설치 마법사의 기본 경로는 `%LOCALAPPDATA%\Programs\Star-Control`이고 사용자가 바꿀 수 있다. 조용한 설치 예시는 다음과 같다.

```powershell
& .\dist\star-control-windows-x64-0.1.0-setup.exe `
  /VERYSILENT /SUPPRESSMSGBOXES /NORESTART `
  /DIR="D:\도구\Star-Control"

& 'D:\도구\Star-Control\star.exe' installation status
& 'D:\도구\Star-Control\star.exe' integration status
& 'D:\도구\Star-Control\star.exe' controller autostart status
```

Codex CLI 등록이 실행 환경에서 허용되지 않으면 설치 자체는 유지하고 integration 상태를 `manual_action_required`로 기록한다. 결과의 `manual_commands`를 공식 CLI에서 실행하거나 Codex Plugin 화면에서 설치한 뒤 새 작업을 열고 Hook을 검토한다. Star-Control은 Codex `config.toml`, Plugin cache와 Hook trust 저장소를 직접 수정하지 않는다.

## update·repair·제거

- update·repair는 같은 installer를 다시 실행하고 이전 선택 경로를 재사용한다.
- update·repair·제거 전에 Codex 앱을 완전히 종료하고, Codex 밖의 별도 PowerShell에서 installer를 실행한다. 실행 중인 Codex 작업 안에서 installer를 호출하지 않는다.
- Installer는 실행 중인 Codex나 Star-Control process를 강제로 닫지 않는다. 설치 전 WMI preflight가 `ChatGPT.exe`, `star-controller.exe`, `star-mcp.exe`를 확인하며, 실행 중이거나 확인할 수 없으면 파일을 변경하기 전에 중단한다. integration install·repair·uninstall도 `ChatGPT.exe`가 실행 중이면 쓰기 전에 실패한다.
- 기본 제거는 program payload, installation record와 exact-owned 자동 시작 entry를 제거한다. 사용자 설정·runtime state·Project 자료는 보존한다.
- `/PURGEDATA` 제거는 `%APPDATA%\Star-Control`과 `%LOCALAPPDATA%\Star-Control`을 추가로 제거하는 명시적 파괴 동작이다. Project의 `.star-control`과 `.ai-runs`는 대상이 아니다.

실제 사용자 설치를 제거하는 검증은 파일 삭제 승인을 별도로 받은 경우에만 수행한다. 자동 검증은 unit test, stage hash·PE machine 검사, installer compile과 비파괴 install·repair smoke를 우선한다.
