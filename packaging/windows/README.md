# Windows 설치 패키지

이 폴더는 [Windows 설치와 Codex 연동 계약](../../docs/contracts/windows-installation-and-codex-integration.md)에 따른 current-user Inno Setup 6 설치 파일을 만든다. 공개 배포 상태기계와 signer 자체는 이 폴더의 책임이 아니지만, stage의 file set·PE architecture와 실제 Authenticode 결과를 검증해 `unsigned_local|signed`를 거짓 없이 봉인하는 경계는 `star-package-release`가 소유한다.

## 산출물 만들기

```powershell
.\packaging\windows\build-installer.ps1 -Architecture x64
.\packaging\windows\build-installer.ps1 -Architecture arm64
```

같은 version의 기존 `dist/stage/<version>/<architecture>`가 비어 있지 않으면 덮어쓰지 않는다. 검증된 stage를 의도적으로 다시 만들 때만 `-ReplaceStage`를 사용한다. 이 switch는 `dist/stage` 아래에서 확인된 정확한 architecture 폴더에만 적용된다.

개발·복구용 ZIP이 별도로 필요할 때만 `-PortableZip`을 추가한다. 이 경로는 verified stage를 ZIP으로 봉인할 뿐이므로 Inno Setup 설치가 없어도 실행할 수 있으며 installer 생성·설치 E2E를 대체하지 않는다. 최초 local stage는 항상 `unsigned_local`이다.

허용된 비서명 package-side 변환 뒤에는 설치 root가 아닌 `dist/stage` 하위의 동일 release stage만 `reseal`로 다시 봉인할 수 있다. 이 명령은 상태를 `unsigned_local`로 유지한다.

```powershell
cargo run --locked -p star-package-release -- reseal `
  --architecture x64 --stage .\dist\stage\0.1.0\x64 `
  --source-revision <source-revision>
```

`reseal`은 architecture, version, unsigned-local identity와 declared file set을 다시 검증하며, 설치본·Codex cache·사용자 설정은 변경하지 않는다.

공개 후보는 먼저 stage 안의 root·Runtime Generation `.exe`를 모두 외부 signer로 서명한 뒤 `seal-signed`를 실행한다. 하나라도 offline Authenticode `Valid`가 아니거나 pre-sign manifest의 file path inventory가 달라졌으면 manifest를 쓰기 전에 실패한다. 이 명령은 nested Runtime Generation manifest와 top-level file manifest의 source revision·digest를 함께 다시 계산하므로 서명 전 검증 결과를 상속하지 않는다.

```powershell
cargo run --locked -p star-package-release -- seal-signed `
  --architecture x64 --stage .\dist\stage\0.1.0\x64 `
  --source-revision <40-or-64-hex-source-revision>
```

그 다음 installer를 만들고 installer 자체를 서명한 뒤 최종 installer digest와 release Gate를 새로 계산한다. `seal-signed`는 signer나 timestamp provider를 호출하지 않고 이미 서명된 byte의 Windows trust만 검증한다. approved certificate identity·timestamp receipt와 final installer signature는 별도 `ReleaseManifest` signature evidence에 결합해야 한다.

표준 `dist/stage/<version>/<architecture>`가 `signed`로 봉인되면 다음 명령은 Runtime을 다시 빌드하지 않고 그 exact stage만 검증해 installer를 만든다. 생성 직후 installer 상태는 아직 `unsigned_local`이며 외부 signer로 installer를 서명한 다음 최종 digest를 다시 계산해야 한다.

```powershell
.\packaging\windows\build-installer.ps1 -Architecture x64 `
  -SourceRevision <40-or-64-hex-source-revision> -UseExistingSignedStage
```

## 설치와 확인

설치 마법사의 기본 경로는 `%LOCALAPPDATA%\Programs\Star-Control`이고 사용자가 바꿀 수 있다. 조용한 설치 예시는 다음과 같다.

```powershell
& .\dist\star-control-windows-x64-0.1.0-setup.exe `
  /VERYSILENT /SUPPRESSMSGBOXES /NORESTART `
  /DIR="D:\도구\Star-Control"

& 'D:\도구\Star-Control\star.exe' installation status
& 'D:\도구\Star-Control\star.exe' integration status
```

Codex CLI 등록이 실행 환경에서 허용되지 않으면 설치 자체는 유지하고 integration 상태를 `manual_action_required`로 기록한다. 결과의 `manual_commands`를 공식 CLI에서 실행하거나 Codex Plugin 화면에서 설치한 뒤 새 작업을 열고 Hook을 검토한다. Star-Control은 Codex `config.toml`, Plugin cache와 Hook trust 저장소를 직접 수정하지 않는다.

## update·repair·제거

- update·repair는 같은 installer를 다시 실행하고 이전 선택 경로를 재사용한다.
- installer EXE를 직접 실행하는 update·repair·제거 전에는 Codex 앱을 완전히 종료하고 Codex 밖의 별도 PowerShell을 사용한다. 실행 중 host에서 full/mixed payload를 교체해야 할 때는 검증된 `star update offline-installer-restart`만 사용한다.
- 설치 payload는 이미 verified이고 Runtime selector만 root manifest 소유 generation보다 stale이면 installer를 다시 실행하지 않는다. `star update reconcile-installed-runtime --install-root <absolute-path> --json`은 Codex/MCP를 유지한 채 prior Controller exact image만 drain하고 installed trusted CLI로 release declared/ready exact set을 검증한다. fixed EXE·Plugin·Hook byte가 다르면 이 경로를 사용하지 않는다.
- Installer는 실행 중인 Codex나 Star-Control process를 강제로 닫지 않는다. 설치 전 WMI preflight가 `ChatGPT.exe`, `Codex.exe`, `star-controller.exe`, `star-mcp.exe`, `star-updater.exe`를 확인하며, 실행 중이거나 확인할 수 없으면 파일을 변경하기 전에 중단한다. integration install·repair·uninstall도 Codex가 실행 중이면 쓰기 전에 실패한다.
- Controller autostart는 설치본에서 항상 비활성화한다. Hook/MCP가 필요할 때 시작된 Controller는 모든 관측 작업세션 종료 뒤 30초 lease로 종료한다.
- 기본 제거는 program payload, installation record와 exact-owned 자동 시작 entry를 제거한다. 사용자 설정·runtime state·Project 자료는 보존한다.
- `/PURGEDATA` 제거는 `%APPDATA%\Star-Control`과 `%LOCALAPPDATA%\Star-Control`을 추가로 제거하는 명시적 파괴 동작이다. Project의 `.star-control`과 `.ai-runs`는 대상이 아니다.

실제 사용자 설치를 제거하는 검증은 파일 삭제 승인을 별도로 받은 경우에만 수행한다. 자동 검증은 unit test, stage hash·PE machine 검사, installer compile과 비파괴 install·repair smoke를 우선한다.
