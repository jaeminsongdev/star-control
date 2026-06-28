# Target Architecture

## 목적

Star-Control은 provider-neutral 작업 관제 시스템이다. 여러 cloud API model, cloud CLI agent, local model server, local process runner, fake provider, human handoff를 공통 계약으로 다루고, 작업 생성부터 라우팅, 실행, 검증, 리뷰, 보고까지의 상태를 관리한다.

이 문서는 전체 완성형 구조를 정의한다. 일부 항목은 장기 구현 대상이지만, 구현자는 이 문서를 기준으로 package 경계와 데이터 흐름을 깨지 않도록 작업한다.

## 핵심 원칙

1. Star-Control core는 provider-neutral이어야 한다.
2. provider 제품명은 core package 이름에 들어가지 않는다.
3. provider는 `kind`, `transport`, `adapter`, `capability_profile`, `provider_instance`로 식별한다.
4. Star Sentinel은 Star-Control 기본 탑재 검증 도구지만 core에 직접 결합하지 않는다.
5. 실행 산출물은 Star-Control repository가 아니라 대상 프로젝트의 `.ai-runs/` 아래에 저장한다.
6. 모든 작업은 추적 가능한 job, state, event, artifact로 남긴다.
7. 검증 실패를 통과시키기 위해 test, CI, policy를 약화하지 않는다.

## 상위 구성

```text
Star-Control
  Core
    Job lifecycle
    StateStore
    RouterEngine
    ExecutionEngine
    ValidationEngine
    ReportBuilder
  Provider System
    Provider registry
    Provider manifest
    Provider instance
    Provider adapter
    Capability profile
  Builtin Tools
    Star Sentinel
  User Surface
    CLI
    Daemon
    API
    UI shell
  Data Contracts
    JSON schemas
    Templates
    Examples
    Run artifacts
```

## Core 책임

Star-Control core는 다음만 책임진다.

- job 생성과 상태 전이 관리
- route, workspec, report 계약 관리
- StateStore 읽기/쓰기
- provider adapter 호출
- builtin tool 호출
- validation 결과 수집
- approval gate 반영
- report와 ledger 작성

Core는 다음을 직접 구현하지 않는다.

- 특정 cloud provider의 인증 방식
- 특정 CLI agent의 내부 프롬프트
- 특정 local model server의 모델 관리
- Star Sentinel의 rule engine 내부 세부 구현
- UI 프레임워크 상세

## Provider System 책임

Provider System은 여러 실행 주체를 공통 형태로 다룬다.

Provider 종류 후보:

- `fake`: 테스트와 smoke용 provider
- `human`: 사람이 직접 판단하거나 승인하는 provider
- `local_process`: 로컬 명령 실행 provider
- `local_model`: 로컬 모델 서버 provider
- `cloud_api`: cloud API provider
- `cloud_cli`: cloud CLI agent provider
- `remote_agent`: 원격 agent provider

초기 구현은 `fake`와 file-based artifact flow를 먼저 안정화한다. 이후 local/cloud provider를 붙인다.

## Star Sentinel 책임

Star Sentinel은 builtin tool이다. 책임은 다음과 같다.

- changed lines와 repo map 기반 diff 이해
- task scope 검증
- test deletion / assertion weakening / skip-only-ignore 추가 탐지
- dependency change 승인 요구
- secret exposure 탐지
- validation evidence 확인
- diagnostics 생성
- approval decision 생성
- review pack 생성
- tool ledger 작성

Star Sentinel은 core 내부 class로 직접 박히면 안 된다. Core는 tool manifest와 tool invocation contract를 통해 Star Sentinel을 호출한다.

## User Surface

최종 user surface는 다음 계층으로 확장될 수 있다.

- CLI: 직접 명령 실행과 상태 확인
- Daemon: 장시간 작업 관제와 queue 처리
- API: UI 또는 외부 도구가 사용하는 안정 인터페이스
- UI shell: 작업 생성, 진행 상태, 승인, 리뷰 확인

현재 문서 단계에서는 user surface의 목표와 계약만 정의하고 실제 구현은 별도 단계에서 진행한다.

## 전체 흐름

```text
User Request
  -> JobSpec 생성
  -> RunState REQUESTED
  -> RouterEngine
  -> RouteSpec
  -> WorkSpec 생성
  -> ProviderAdapter 실행
  -> Provider output 저장
  -> ValidationEngine
  -> Star Sentinel check/gate/review-pack
  -> ReportSpec 생성
  -> RunState DONE / FAILED / BLOCKED / WAITING_APPROVAL
```

## 장기 확장 원칙

- 새 provider는 core를 수정하지 않고 manifest와 adapter로 추가한다.
- 새 tool은 builtin 또는 plugin tool 경계로 추가한다.
- 새 UI는 API와 StateStore contract를 통해 붙인다.
- 새 validation rule은 Star Sentinel policy profile로 추가한다.
- schema 변경은 반드시 example과 CI 검증을 함께 수정한다.

## 금지 사항

- core에서 특정 provider 제품명에 의존하는 import, package, module 이름을 만들지 않는다.
- 검증 실패를 해결하기 위해 test, CI, policy를 삭제하거나 약화하지 않는다.
- 실행 산출물을 Star-Control repository에 저장하지 않는다.
- approval required change를 사용자 승인 없이 자동 진행하지 않는다.
- runtime dependency 또는 package manager를 명시 승인 없이 추가하지 않는다.
