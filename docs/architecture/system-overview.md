# 전체 구조

## 구조 요약

Star-Control은 Codex 위에 놓이는 실행 계획·배정·검사 계층이다.

    사용자
      ↓ Codex 앱에 목표 입력
    Star-Control Plugin
      ├─ 반복 작업 규칙
      ├─ MCP 연결
      └─ 실행 전후 검사
      ↓
    Star-Control Controller
      ├─ 목표 명확화
      ├─ 단계 분해
      ├─ 모델·생각 깊이·실행 방식 배정
      ├─ 권한과 비용 판단
      ├─ 상태와 기록 관리
      └─ 병렬 작업과 병합 조정
      ↓
    Codex App Server
      ├─ 사용할 수 있는 모델과 기능 조회
      ├─ 새 Codex 작업 생성
      ├─ 단계별 모델과 생각 깊이 설정
      ├─ 작업 중단·재개·분기
      └─ 결과와 진행 이벤트 전달
      ↓
    대상 프로젝트
      ├─ 파일 변경
      ├─ 필요한 검사
      └─ .ai-runs/ 실행 기록

## 구성 요소

### 1. Plugin

Codex 통합과 설치의 단위다. 반복 작업 규칙, MCP 설정, 실행 전후 검사와 필요한 자산을 한 묶음으로 제공한다. Windows 실행 파일은 같은 Star-Control release의 runtime installer가 설치한다.

### 2. MCP

Codex가 Star-Control의 기능을 호출하는 통로다. 목표 생성, 계획, 승인, 실행, 상태 확인, 중단, 이어하기, 증거 조회 기능을 제공한다.

`star-mcp.exe`는 실제 tool 이름과 개발 도구 EXE를 하드코딩하거나 TOML을 읽지 않는다. 검색·설명과 위험 종류별 호출로 된 고정 MCP surface만 제공하고 모든 요청을 Controller에 전달한다. Controller의 [live Tool Registry](../contracts/external-tool-registry.md)가 TOML·Schema·EXE 변경을 무재시작으로 반영한다.

MCP는 기능을 제공할 뿐 모든 작업이 자동으로 그 기능을 사용하도록 강제하지는 않는다. 강제 규칙은 Plugin의 작업 규칙과 실행 전 검사가 함께 담당한다.

### 3. 실행 전후 검사

실제 파일 수정이나 명령 실행 전에 활성 Star-Control 단계가 있는지 확인한다. 허용되지 않은 동작은 막거나 경고하고, 실행 뒤에는 결과를 기록한다.

### 4. Controller

Star-Control의 판단, 상태와 live Tool Registry를 소유하는 로컬 프로그램이다. 하나의 배경 프로세스로 동작하며 MCP와 터미널 명령이 같은 상태와 최신 tool descriptor를 사용한다.

### 5. Codex App Server 연결

Codex 작업을 생성하고 제어하는 공식 통로다. 실행 시점에 사용 가능한 모델과 생각 깊이를 조회하고, 새 작업에 모델·생각 깊이·작업 폴더·권한을 지정한다.

Star-Control은 OpenAI API를 직접 호출하지 않는다.

### 6. 단계 계획기

목표를 지나치게 작은 조각이 아니라 실행 성격이 같은 단계로 나눈다. 단계 사이의 순서와 병렬 가능 여부를 정한다.

### 7. 배정 판단기

각 단계에 모델, 생각 깊이, Max 또는 병렬 실행 여부, 검사 강도, 비용 한도를 정한다.

### 8. 필요한 자료 묶음 생성기

현재 단계에 필요한 파일, 프로젝트 규칙, 최근 변경, 이전 결과만 모아 다음 Codex 작업에 전달한다.

### 9. 범위와 안전 검사기

계획과 다른 변경, 비밀정보, 유료 동작, 사용자가 금지한 경로를 확인한다. 범위가 늘었다는 이유만으로 무조건 멈추지는 않고 설정된 정책에 따라 기록, 경고, 일시 중단을 선택한다.

### 10. 검사기

작업 종류와 위험에 맞는 검사만 선택한다. 모든 프로젝트에서 같은 무거운 검사를 실행하지 않는다.

### 11. 증거와 이어하기 기록기

무엇을 바꾸고 어떤 검사를 했는지, 무엇이 실패했고 무엇이 남았는지 저장한다.

### 12. 병렬 작업과 병합 관리자

겹치지 않는 단계를 별도 작업 복사본에서 실행하고, 충돌과 검사 결과를 확인한 뒤 하나로 합친다.

## 프로세스 구성

최종 제품은 다음 실행 파일 책임으로 나눈다.

- star-controller: 배경 Controller와 상태 소유자
- star-mcp: Codex가 호출하는 MCP 진입점
- star: 사용자가 터미널에서 사용하는 명령

구현 과정에서 하나의 실행 파일이 여러 역할을 맡을 수 있지만, 책임과 통신 경계는 위와 같이 유지한다.

실행 파일 내부 Package, 의존 방향, Catalog·Schema·Corpus·test와 최종 문서 배치는 [최종 Repository·Package·문서 구조](repository-layout.md)가 소유한다.

## 중요한 제한

- Plugin이 설치되고 활성화되어야 한다.
- Plugin의 실행 전 검사 정의를 사용자가 신뢰해야 한다.
- Star-Control MCP가 필수 연결로 준비되어야 한다.
- 사용자가 Plugin이나 검사를 끄면 Star-Control 밖에서 작업할 수 있으므로 운영 보장은 사라진다.
- Star-Control은 Codex와 관리자가 정한 더 강한 권한 제한을 약화하지 못한다.

## 공식 기능에 대한 설계 근거

OpenAI 공식 문서는 AGENTS.md, Skills, MCP, Subagents를 서로 보완하는 계층으로 설명한다. MCP 서버는 도구, 읽을 자료, 반복 프롬프트를 제공할 수 있다.

Hook은 사용자 입력, 도구 실행 전후, 권한 요청, 작업 종료 시점에 동작할 수 있다. 실행 전 Hook은 지원되는 도구 호출을 거부할 수 있다.

Codex App Server는 새 작업 생성, 재개, 분기, 모델 목록 조회, 모델과 생각 깊이를 지정한 실행, 검토, 중단을 제공한다.

- https://developers.openai.com/codex/concepts/customization
- https://developers.openai.com/codex/hooks
- https://developers.openai.com/codex/build-plugins
- https://developers.openai.com/codex/app-server
