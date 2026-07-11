# 설치와 공개 배포

## 목표

개인 사용자는 유료 동작 외에는 자동으로 진행할 수 있고, 공개 사용자는 안전한 기본값으로 시작할 수 있어야 한다. 설정 계층과 Catalog의 상세 계약은 [설정과 Catalog 계약](../contracts/config-and-catalog.md)에서 확인한다.

MCP·외부 Tool Runtime의 지원 기준은 Windows 11 24H2 build 26100 이상, x64·ARM64다.

## 공개 배포 묶음

하나의 Star-Control release는 다음 두 산출물을 같은 version으로 제공한다.

### Codex Plugin

- Plugin 설명 파일
- 반복 작업 Skill
- MCP server 설정
- Hook 정의
- 기본 설정과 안내 자산
- 권한과 개인정보 설명

### Windows Runtime

- `star`, `star-controller`, `star-mcp` 실행 파일
- required `star-control-core.toml`, ToolPackageManifest Schema와 fake example
- Windows installer와 uninstall 정보
- 상태·설정 migration
- license와 제3자 고지

Installer는 runtime과 Plugin의 호환 version을 확인한다. Plugin은 설치 뒤 활성화 상태, Hook 신뢰 상태, MCP 준비 상태를 확인해야 한다.

## 설치 경험

1. 사용자가 Windows runtime과 Codex Plugin을 같은 release에서 설치한다.
2. 포함된 기능과 권한을 확인한다.
3. Hook 정의를 검토하고 신뢰한다.
4. Star-Control MCP와 Controller를 활성화한다.
5. `star doctor`로 binary·Plugin·MCP·Hook의 version과 상태를 확인한다.
6. safe_default로 첫 작업을 실행한다.
7. 원하는 사용자는 personal_auto를 선택한다.

Installer는 current-user Controller startup entry를 만들기 전에 이를 알리고, `star controller autostart enable|disable|status`와 제거 방법을 함께 제공한다. entry는 `star-controller.exe --background`만 시작하며 Goal이나 개발 작업을 예약·실행하지 않는다.

## 업데이트

- 상태 파일 형식이 바뀌면 이전 버전을 읽을 방법을 제공한다.
- 설정의 알 수 없는 항목을 조용히 삭제하지 않는다.
- 모델 이름은 실행 시 조회하므로 제품 업데이트 없이도 새 모델을 선택할 수 있어야 한다.
- 외부 개발 도구 EXE는 [ToolPackageManifest Reference](../contracts/tool-package-manifest-reference.md)에 맞는 TOML로 추가한다. 저장 뒤 `star tools status`로 새 revision을 확인하며 Star-Control binary update, MCP 재등록·재시작과 Codex 재시작을 요구하지 않는다.
- installer가 만드는 Codex MCP 설정은 [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md#codex-mcp-설정-정본)의 fixed server·approval 설정과 비교한다.
- Plugin Hook 내용이 바뀌면 사용자가 다시 검토해야 할 수 있음을 안내한다.
- 실패한 업데이트에서 이전 버전으로 돌아갈 수 있어야 한다.

## 개인정보와 기록

- 기본 실행 기록은 로컬에 저장한다.
- 외부 업로드는 사용자가 활성화한 기능에서만 일어난다.
- 공개 보고서에 로컬 절대 경로, 사용자 이름, 인증 정보를 넣지 않는다.
- 사용자가 기록을 확인하고 정리할 수 있는 명령을 제공한다.

## 공개 프로젝트

- Windows 지원 범위를 명확히 적는다.
- 안전 기본값과 자동화 프로필의 차이를 설명한다.
- Codex 기능 변화에 따른 호환 범위를 공개한다.
- 새 프로젝트도 MIT License로 배포한다.
- 최종 배포 전 설치, 제거, 업데이트, 복구 흐름을 실제 Windows 환경에서 검증한다.
