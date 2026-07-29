# Codex 통합과 진입 통제

## 사용자가 원하는 동작

사용자는 Codex 앱에 개발 목표를 입력한다. Star-Control이 설치된 환경에서는 실제 파일 수정이나 명령 실행이 Star-Control 계획 없이 시작되지 않아야 한다.

이를 MCP 하나로 해결하지 않는다. Plugin, 작업 규칙, MCP, 실행 전후 검사, App Server 연결을 함께 사용한다.

## Plugin 구성

공개 배포 Plugin은 다음을 포함한다.

- MCP·Profile·lifecycle 작업을 Star-Control 흐름으로 안내하는 `star-control-operations` Skill
- Sol Max 설계·전체 리뷰와 목표추진 Terra High 작업자를 묶는 `orchestrate-parallel-implementation` Skill
- Star-Control MCP 설정
- 사용자 입력 시 실행되는 검사
- 파일 수정과 명령 실행 전에 실행되는 검사
- 실행 결과를 수집하는 검사
- 설정 예시와 기본 프로필
- Plugin 설명·개인정보·권한·설치 정보

`star`, `star-controller`, `star-mcp` Windows 실행 파일은 같은 release의 runtime installer가 설치한다. Plugin source와 runtime binary를 한 폴더에 뒤섞지 않으며 installer가 호환 version을 확인한다.

Plugin 설치만으로 검사 코드를 자동 신뢰하지는 않는다. 사용자가 현재 Plugin 검사 정의를 검토하고 신뢰해야 한다.

## 시작 흐름

1. `SessionStart` Hook이 fixed MCP/CLI 경계와 `star-control-operations` 사용 지침을 추가하고, 구현 요청에는 사용자가 single-agent를 명시하지 않은 한 `orchestrate-parallel-implementation`도 적용하도록 안내하며 session lifecycle을 관찰한다.
2. operations Skill이 요청을 MCP-first, Catalog-declared CLI-only, installed local lifecycle 또는 명시적 native fallback으로 분류한다. Code Health는 `project.register` → `scan.run` → `index.status|search` → `finding.list|diagnostic.list`의 live action을 우선한다.
3. 구현 Skill은 중앙 작업을 goal로 만들지 않고 Sol Max가 설계·DAG·재계획을 소유하게 한다. 각 응집된 Bundle은 Terra High가 별도 Goal Pursuit로 구현·교정하며 Goal은 Sol의 해당 Terra 전체 diff 승인까지 active로 유지한다. 승인 뒤 Goal complete와 통합을 확인하고, 최종 결합 전체 diff도 Sol Max가 승인해야 한다.
4. 실제 product action은 live Registry의 readiness·Schema·risk lane·descriptor hash를 확인한 뒤 Controller의 같은 application command로 실행한다.
5. Codex 권한과 Star-Control 승인·PermissionPlan은 서로를 대신하지 않는다. 각 경계에서 요구한 승인을 모두 보존한다.
6. Hook lifecycle evidence, worker Goal terminal state, Sol 전체 리뷰, operation terminal result와 실제 ChangeSet·Gate를 결합해 완료를 판정한다.

Hook은 일부 local tool path가 opt-out할 수 있는 보조 guardrail이다. 현재 Plugin Hook은 lifecycle과 context를 관찰하며 product action을 독자적으로 허용·거부하지 않는다. 실제 실행 통제는 fixed MCP lane, Controller admission, Codex permission과 exact approval가 소유한다.

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

## Hook lifecycle과 통제 경계

| Hook | 현재 역할 | decision output |
|---|---|---|
| `SessionStart` | `startup|resume|clear|compact`에서 Skill/MCP route context 주입, session 시작 관찰 | `continue=true`, `additionalContext` |
| `UserPromptSubmit` | turn 시작과 updater activity lease 관찰 | 없음 |
| `PreToolUse` / `PostToolUse` | local tool 실행 depth 시작·종료 관찰 | 없음 |
| `SubagentStart` / `SubagentStop` | subagent depth 시작·종료 관찰 | 없음 |
| `Stop` | root turn stop과 bounded drain lease 관찰 | 없음 |
| `SessionEnd` | main session 종료를 기존 bounded `root_stop` lifecycle로 관찰 | 없음 |

- `SessionStart(source=compact)`가 compaction 뒤 context를 다시 주입하므로 `PreCompact`와 `PostCompact`를 중복 등록하지 않는다.
- `PermissionRequest`는 tool name과 permission request를 Star-Control의 exact PermissionPlan·Approval scope에 결합하는 별도 계약이 생기기 전에는 등록하지 않는다. observation-only Hook을 권한 강제로 표현하지 않는다.
- Codex의 `SessionEnd` timeout 상한은 3초다. Plugin은 정확히 3초를 선언하고 `star.exe`의 Controller lifecycle 관찰에는 2초 내부 deadline을 적용해 process 시작·입출력·정리 여유를 남긴다. 다른 lifecycle Hook의 기존 10초 선언은 유지한다.
- Hook definition이 바뀌면 Codex의 신뢰 검토가 다시 필요하다. Plugin/Bridge 변경은 candidate review가 `requires_codex_restart=true`를 반환한 경우에만 전용 Updater restart transaction으로 적용한다.
- lifecycle observation 중 Controller가 unavailable이면 Hook은 Codex 작업을 실패시키지 않고 evidence 누락을 남긴다. 누락 evidence를 idle·pass·approval로 승격하지 않는다.

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
