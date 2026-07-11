# 최종 구현 로드맵

## 원칙

이 로드맵은 작은 시험판만 만들고 멈추는 계획이 아니다. [기능 범위](../product/scope.md)의 상위 경계, [1인 개발자용 구현 대상 기능](../features/README.md)의 A01~D03과 [최종 Repository·Package·문서 구조](../architecture/repository-layout.md)의 책임 경계를 최종 제품 완료 조건으로 삼는다.

다만 한 번에 전체를 구현하지 않는다. 각 단계는 다음 단계가 믿고 사용할 수 있을 만큼 완성하고 검사한다.

15개 개발 작업 유형은 별도 전문 도구를 각각 만드는 단계가 아니다. 공통 관제·검증 기반 위에 Profile과 adapter로 구현하고, 구체적인 도구와 규칙은 해당 단계 직전에 최신 자료로 다시 조사한다.

## D0. 설계 확정 — 완료

### 결과

- 새 프로젝트 헌장
- 전체 구조
- 단계 분해 기준
- 모델 배정 규칙
- Codex 통합 방식
- 승인과 검사 기준
- 상태와 증거 저장 방식
- 병렬 작업과 병합 기준
- 기능 범위와 제외 사항
- 1인 개발자용 구현 대상 기능과 작업 Profile
- 최종 Repository·Package·문서 구조와 의존 규칙
- RouteDecision의 모델 역할·원시 생각 깊이·단계 성격·실행 방식 분리
- 책임별 문서 폴더 migration과 내부 링크 갱신
- D0 최종 설계 결정 기록
- 공개 배포 기준

### 완료 조건

- 새 문서만으로 설계 전체 이해 가능
- 문서 사이 기준 충돌 없음
- 사용자가 최종 방향을 승인했고 [ADR-0001](../decisions/ADR-0001-최종-설계-기준.md)에 고정함

## P1. 기초 계약과 설정

계약 의미와 설정 병합 설계는 [데이터 계약 지도](../contracts/README.md), [ADR-0002](../decisions/ADR-0002-데이터-계약과-설정-정본.md), [ADR-0004](../decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)와 [ADR-0005](../decisions/ADR-0005-MCP-구현-계약-동결.md)로 확정했다. MCP exact field·hash·Win32 순서·검증 행렬은 문서 동결됐고 Rust type, generated Schema, fixture와 runtime 구현은 아직 시작하지 않았다.

### 첫 수직 Slice

1. [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)의 Rust type·고정 12 tool Schema·JCS hash fixture
2. [Manifest Reference](../contracts/tool-package-manifest-reference.md)의 ToolPackageManifest·ToolDescriptor·TrustRecord·RegistryCache Schema와 generated required `star-control-core.toml`
3. authenticated named pipe, deterministic loader·trust·search index·LKG·watcher+demand scan
4. `rmcp 2.2.0` 기반 fixed `star-mcp.exe`와 fake Controller vertical slice
5. [Windows Tool Runtime](../architecture/windows-tool-runtime.md)의 argv·JSON-STDIO·identity lease·Job Object
6. [MCP 검증 행렬](../testing/mcp-verification-matrix.md)의 실제 Codex same-session C001~C008

실제 `rg`, validator와 debugger 연결은 이 slice가 통과한 뒤 TOML 예시로만 추가한다.

### 구현

- GoalSpec
- StageSpec
- RouteDecision
- `model_role`, `reasoning_effort`, `stage_mode`, `execution_mode`, CapabilitySnapshot의 분리 계약
- PermissionPlan
- ValidationPlan
- EvidenceBundle
- Checkpoint
- MergePlan
- CapabilitySnapshot
- 외부 Tool Registry와 executable trust
- 설정 계층과 프로필
- 상태 전이와 안전한 파일 저장
- foundation Package와 기계 계약 생성 흐름
- Package 의존 방향 검사

### 완료 조건

- 잘못된 입력을 명확히 거부
- 중단 중 파일 손상 없음
- 이전 상태 재개 가능
- safe_default와 personal_auto 동작 구분
- fake EXE·TOML 추가가 Gateway source 변경 없이 같은 MCP session의 search 결과에 나타남
- TOML path 변경과 같은 path의 호환 EXE 교체가 MCP·Controller·Codex 재시작 없이 다음 호출에 반영됨
- 잘못된 candidate는 해당 package last-known-good를 유지하고 다른 package를 막지 않음
- descriptor hash·risk lane·Schema·executable update policy 불일치가 side effect 전에 거부됨
- MCP 검증 행렬 전체 통과, 미실행·flaky·quarantined test 0개
- Windows 11 24H2 x64·ARM64 smoke 통과

## P2. Plugin 진입과 MCP

### 구현

- Star-Control Plugin
- 개발 작업 Skill
- exact 13개 core action을 실제 application command handler에 연결
- installer MCP 설정, Controller startup과 Plugin entry readiness 연결
- 사용자 입력 검사
- 실행 전후 검사
- 설치 상태 확인 명령

### 완료 조건

- Codex 앱 입력에서 Star-Control 목표 시작
- 계획 없는 수정 도구 호출 차단
- 단순 대화는 불필요하게 차단하지 않음
- Plugin, Hook, MCP가 꺼졌을 때 안전하게 실패

## P3. 단계 계획과 자동 배정

### 구현

- 목표 질문
- 단계 분해
- 순서와 병렬 가능성 판단
- 모델·생각 깊이·Max·병렬 실행 배정
- 사용자 계획 수정
- 비용 한도와 승급 규칙

### 완료 조건

- 지나치게 작은 작업 분해를 피함
- 배정 이유를 사람이 이해할 수 있음
- 사용자 선택이 자동 배정보다 우선함
- 지원되지 않는 모델 선택을 안전하게 대체

## P4. Codex 실행과 필요한 자료 묶음

### 구현

- App Server 초기화
- 모델과 기능 조회
- 새 작업 생성, 재개, 분기, 중단
- 단계별 모델·생각 깊이·권한 지정
- 프로젝트 규칙과 관련 파일 탐색
- 앞 단계 결과 전달
- Windows 배경 Controller

### 완료 조건

- OpenAI API 직접 호출 없음
- 앱 종료 뒤에도 상태 재개 가능
- 불필요한 전체 자료 전달을 피함
- App Server 실패 원인과 복구 방법 기록

## P5. 검사·증거·이어하기

### 구현

- 변경 종류별 검사 선택
- 프로젝트 검사 등록
- 범위, 비밀정보, 테스트 약화, 의존 항목 검사
- 실제 diff·완료 주장·증거 검증과 Review Pack
- 테스트 신뢰성, 검증기 보호와 회귀 Corpus
- 계약·구조·설정·보안·실패 재현·문서·성능·release 검증 Profile
- 자동 수정과 제한된 재시도
- 독립 검토
- 증거 묶음과 최종 요약
- 이어하기 기록

### 완료 조건

- 필요한 검사를 빠뜨리지 않음
- 불필요한 전체 검사 남용 없음
- 실패와 미실행 검사를 숨기지 않음
- 자동 완료 조건을 기계적으로 판단

## P6. 병렬 작업과 로컬 병합

### 구현

- 단계별 Git worktree
- 동시 수정 충돌 사전 검사
- 병렬 Codex 실행 한도
- 로컬 검토 정보
- 병합 대기열
- 충돌 처리
- 통합 검사

### 완료 조건

- 겹치는 수정의 잘못된 병렬 실행 방지
- 사용자 기존 변경 보존
- 단계별 결과 추적 가능
- 병합 뒤 전체 목표 검사 통과

## P7. 여러 프로젝트와 원격 저장소

### 구현

- 여러 프로젝트 목표
- 프로젝트 간 순서와 연결 계약
- 로컬 변경 기록
- 원격 업로드
- 검토 요청 생성과 갱신
- 상태 검사와 병합
- 인터넷 조사와 출처 기록

### 완료 조건

- 프로젝트별 변경과 증거 분리
- 제공하는 프로젝트를 먼저 처리
- 원격 대상과 결과 추적
- 출처 없는 최신 정보 사용 방지

## P8. 비용·비교 시험·규칙 개선

### 구현

- 시간, 사용량, 실패, 재작업 수집
- 실제 개발 작업 모음
- 모델과 생각 깊이 비교
- 배정 규칙 변경 기록
- 한도 초과 중단

### 완료 조건

- 거짓 가격이나 사용량을 만들지 않음
- 품질과 안전을 낮춰 비용을 맞추지 않음
- 규칙 변경 전후를 비교할 수 있음

## P9. 공개 배포와 최종 완성

### 구현

- Windows 설치, 업데이트, 제거
- Plugin 패키징
- Hook 신뢰 안내
- 상태와 기록 정리 명령
- 배포 준비 검사
- 보안과 개인정보 검토
- 전체 사용자 문서

### 완료 조건

- 깨끗한 Windows 환경에서 설치 가능
- safe_default 첫 작업 성공
- personal_auto 설정 가능
- 업데이트와 복구 성공
- 포함된 모든 최종 기능 구현
- A01~D03과 15개 작업 Profile의 연결 검증
- Package 소유권·단일 Writer·adapter 경계 검증
- 제외 기능이 다시 들어오지 않음
- 전체 검사와 독립 최종 검토 통과

## 구현 순서 변경

의존관계가 유지된다면 한 단계 안의 세부 순서는 바꿀 수 있다. 다음은 바꾸지 않는다.

- 문서 확정 전에 제품 코드 작성 금지
- 상태와 계약보다 실행 자동화를 먼저 만들지 않음
- 단일 실행이 안정되기 전에 병렬 병합을 기본값으로 만들지 않음
- 검사와 증거 없이 원격 자동화를 완료로 보지 않음
- 공개 안전 기본값 없이 personal_auto만 배포하지 않음
