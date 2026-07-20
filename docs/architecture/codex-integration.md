# Codex 통합과 진입 통제

## 사용자가 원하는 동작

사용자는 Codex 앱에 개발 목표를 입력한다. Star-Control이 설치된 환경에서는 실제 파일 수정이나 명령 실행이 Star-Control 계획 없이 시작되지 않아야 한다.

이를 MCP 하나로 해결하지 않는다. Plugin, 작업 규칙, MCP, 실행 전후 검사, App Server 연결을 함께 사용한다.

## Plugin 구성

공개 배포 Plugin은 다음을 포함한다.

- 개발 작업을 Star-Control 흐름으로 안내하는 Skill
- Star-Control MCP 설정
- 사용자 입력 시 실행되는 검사
- 파일 수정과 명령 실행 전에 실행되는 검사
- 실행 결과를 수집하는 검사
- 설정 예시와 기본 프로필
- Plugin 설명·개인정보·권한·설치 정보

`star`, `star-controller`, `star-mcp` Windows 실행 파일은 같은 release의 runtime installer가 설치한다. Plugin source와 runtime binary를 한 폴더에 뒤섞지 않으며 installer가 호환 version을 확인한다.

Plugin 설치만으로 검사 코드를 자동 신뢰하지는 않는다. 사용자가 현재 Plugin 검사 정의를 검토하고 신뢰해야 한다.

## 시작 흐름

1. 사용자 입력 검사가 요청을 확인한다.
2. 단순 대화인지 실제 개발 동작인지 구분한다.
3. 개발 동작이면 Star-Control 목표 기록을 만들거나 기존 목표를 찾는다.
4. Codex에 Star-Control MCP를 사용하라는 작업 지침을 추가한다.
5. 계획이 승인되기 전에는 읽기와 설계만 허용한다.
6. 파일 수정이나 명령 실행 직전에 활성 단계와 실행 허가를 확인한다.
7. 허가가 없으면 동작을 거부하고 계획 흐름으로 돌아간다.

요청 분류가 틀릴 수 있으므로 단순 대화를 막지 않는 것을 우선한다. 대신 실제 변경 도구를 사용하려는 시점에 반드시 실행 허가를 확인한다.

## MCP가 제공할 기능

정확한 tool 이름, input, output과 승인 경계는 [Star-Control MCP 도구 계약](../contracts/mcp-tools.md)이 소유한다. 책임은 다음과 같다.

- 목표 시작과 질문 기록
- 단계 계획 생성과 수정
- 모델·생각 깊이·실행 방식 배정 조회
- 질문 답변과 승인 요청 해소
- 단계 실행 시작
- 상태 확인
- 일시 중단, 재개, 취소
- 검사 계획과 결과 조회
- 증거와 이어하기 기록 조회
- 병렬 작업과 병합 상태 조회
- 목표 종료

MCP adapter는 이 책임을 직접 구현하지 않고 [Windows Local IPC](../contracts/local-ipc.md)를 통해 Controller의 같은 application command를 호출한다.

`star-mcp.exe`에는 search·describe·Registry status·Operation·승인과 여섯 risk lane으로 된 고정 tool 목록만 둔다. Star-Control 기본 action과 외부 EXE action은 Controller의 [live Tool Registry](../contracts/external-tool-registry.md)에 등록한다. Codex는 검색→설명→지정 lane 호출 순서를 사용하며 설명에서 받은 `descriptor_hash`를 실행에 돌려준다.

고정 12개 tool, MCP protocol·capability·approval 설정은 [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)을 따른다. MCP Tasks는 사용하지 않고 장기 실행은 Operation 도구로만 조회·취소한다.

Controller는 watcher와 호출 직전 demand scan으로 TOML·Schema·EXE 변경을 반영한다. 새 EXE 추가, path 수정과 같은 path의 EXE 교체는 MCP rebuild·재등록·process 재시작과 Codex 재시작 없이 다음 호출부터 적용한다.

`star-mcp.exe`와 Hook의 `star.exe`는 설치 루트에 남는 Bootstrap Bridge다. Bridge는 `%LOCALAPPDATA%\\Star-Control\\installation\\active-runtime.v1.json`이 가리키는 Runtime Generation의 Controller만 선택한다. Runtime Generation 교체는 Controller를 drain하고 다시 연결할 수 있지만 Codex·MCP stdio process의 재시작이나 Plugin/MCP 설정 변경을 요구하지 않는다. Bridge/Plugin 자체를 바꾸는 통합 변경은 [ADR-0014](../decisions/ADR-0014-전용-Star-Updater와-Codex-생명주기.md)의 전용 Updater restart transaction·Hook 검토 경계다. persisted shape와 후보 검토는 [Runtime update와 activation 계약](../contracts/runtime-update-and-activation.md)을 따른다.

## 실행 전후 검사

### 사용자 입력 시

- 개발 작업 후보를 감지한다.
- 프로젝트와 기존 목표를 찾는다.
- 필요한 Star-Control 안내를 Codex에 전달한다.

### 도구 실행 전

- 활성 목표와 단계가 있는지 확인한다.
- 현재 도구와 대상이 단계에서 허용되는지 확인한다.
- 유료 동작인지 확인한다.
- 거부, 허용, 경고 중 정책에 맞는 결과를 반환한다.

### 권한 요청 시

- Star-Control 승인 설정과 Codex 권한 요청을 함께 판단한다.
- Star-Control은 Codex나 관리자가 요구한 승인을 없애지 않는다.

### 도구 실행 후

- 명령 결과와 변경 파일을 실행 기록에 연결한다.
- 실패와 범위 변화를 기록한다.

### 작업 종료 시

- 완료 조건과 필요한 검사를 확인한다.
- 미완료면 이어서 해야 할 일을 Codex에 전달한다.
- 완료면 증거와 이어하기 기록을 닫는다.

## App Server 사용

Controller는 Codex App Server를 통해 다음을 수행한다.

- model/list로 사용 가능한 모델과 생각 깊이 조회
- thread/start로 새 단계 작업 생성
- thread/resume으로 중단된 작업 재개
- thread/fork로 기존 작업에서 분기
- turn/start로 모델, 생각 깊이, 작업 폴더, 권한을 지정해 실행
- turn/interrupt로 중단
- review/start로 독립 검토
- thread/goal 기능으로 긴 목표 상태 연결

이 결과는 외부 응답 그대로 core에 전달하지 않고 [라우팅 계약](../contracts/routing.md)의 CapabilitySnapshot으로 정규화한다.

App Server의 실험 기능은 기본 경로로 사용하지 않는다. 꼭 필요하면 지원 여부를 확인하고 대체 경로와 함께 사용한다.

## 필수 연결 확인

개발 작업을 시작하기 전에 다음을 확인한다.

- Star-Control Plugin 활성화
- Plugin 검사 신뢰 상태
- Star-Control MCP 활성화
- Controller 실행 상태
- Codex App Server 연결 가능
- 대상 프로젝트 접근 가능
- 설정과 비용 정책 해석 가능

Star-Control MCP를 필수 연결로 설정할 수 있는 환경에서는 초기화 실패 시 Codex 작업도 시작하지 않게 한다.

## 보장 범위

Star-Control은 설치·활성화·신뢰된 환경에서만 진입 통제를 보장한다.

사용자가 Plugin, Hook, MCP를 끄거나 Star-Control이 없는 Codex 환경에서 작업하면 통제할 수 없다. 공개 문서와 상태 명령은 현재 보호 상태를 명확히 보여줘야 한다.

    star doctor

이 명령은 설치, 연결, 검사 신뢰, App Server, 프로젝트 설정 상태를 한 번에 확인하는 역할을 가진다.

## 터미널과 배경 실행

- Codex 앱은 목표 입력과 대화 화면이다.
- star 명령은 상태 확인과 직접 제어 수단이다.
- Controller는 긴 작업과 여러 작업을 앱 화면과 독립적으로 추적한다.
- 브라우저 UI와 별도 HTTP 화면은 만들지 않는다.

## 공식 근거

- [Customization](https://developers.openai.com/codex/concepts/customization/)
- [MCP 지원 기능](https://learn.chatgpt.com/docs/extend/mcp#supported-mcp-features)
- [Hooks](https://developers.openai.com/codex/hooks/)
- [Plugins](https://developers.openai.com/codex/build-plugins/)
- [App Server API 개요](https://learn.chatgpt.com/docs/app-server#api-overview)
