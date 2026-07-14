# 기능 범위와 레거시 판정

## 목적

기존 Star-Control의 기능 이름을 그대로 끌고 오지 않고, 새 Codex 전용 목적에 필요한 책임만 선별한다.

legacy/는 로컬 읽기 전용 참고자료다. 새 문서는 legacy/를 읽지 않아도 이해할 수 있어야 하며 새 제품은 legacy/ 파일에 의존하지 않는다.

레거시 자료에 어떤 기능이 어떻게 설명되어 있었는지와 그 역사적 근거는 [레거시 기능 카탈로그](../history/legacy-feature-catalogue.md)에서 확인한다. 카탈로그는 과거 자료의 설명을 기록하고, 이 문서는 그 기능을 새 Star-Control의 범위에서 어떻게 다룰지 상위 경계를 정한다. 카탈로그에 실렸다는 사실 자체는 새 범위 반영이나 실제 동작을 뜻하지 않는다.

레거시 기능과 개발 도구·검증 플랫폼 자료를 1인 개발자 효용성 기준으로 다시 선별한 상세 구현 범위는 [1인 개발자용 구현 대상 기능](../features/README.md)이 소유한다. 정리하면 문서 14는 과거 기능의 사실 자료, 이 문서는 상위 범위 판정, 문서 15는 실제 구현할 개념 기능 목록이다.

## 새 설계에서 유지할 개념

다음 개념은 새 목적에 직접 필요하므로 유지하되 새 계약으로 다시 작성한다.

| 개념 | 새 역할 |
|---|---|
| 목표 기록 | 사용자의 최종 개발 목표와 완료 조건 저장 |
| 단계 계획 | 모델이나 실행 방식이 달라지는 단위로 분해 |
| 순서와 의존관계 | 단계 실행 순서와 병렬 가능 여부 관리 |
| 모델 배정 | Sol, Terra, Luna 역할과 실제 모델 연결 |
| 필요한 자료 묶음 | 현재 단계에 필요한 정보만 전달 |
| 실행 상태 | 중단, 재개, 실패, 완료 상태 저장 |
| 권한 정책 | 행동별 자동, 질문, 금지 설정 |
| 실행 제어 | Codex App Server로 단계 작업 생성과 제어 |
| 검사 계획 | 변경에 실제로 필요한 검사만 선택 |
| 완료 증거 | 변경, 검사, 비용, 위험 기록 |
| 이어하기 기록 | 다음 작업이 반복 조사하지 않도록 요약 |
| 비용과 사용량 | 실패와 재작업을 포함한 사용량 기록 |
| 비밀정보 가림 | 로그와 외부 보고서 보호 |
| 복구 | 중단된 상태와 손상된 실행 기록 확인 |
| 배포 준비 확인 | 공개 산출물, 문서, 설치 상태 검사 |

## 새 목적에 맞게 다시 만들 기능

### Provider와 Router

여러 AI 제공자 중 하나를 고르는 구조는 제거한다. 새 배정 기능은 Codex 내부의 모델, 생각 깊이, Max, 병렬 실행, 검토 방식을 고른다.

### 실행 엔진

AI 제공자별 실행기를 제거한다. Codex App Server만 사용해 작업을 생성하고 제어한다.

### 사용자 화면

기존 브라우저 화면과 HTTP 제어 화면을 제거한다. Codex 앱과 star 터미널 명령만 사용한다.

### 배경 실행

기존의 범용 실행 서버 대신 목표, 단계, App Server 작업, 병렬 실행 상태를 관리하는 Windows 로컬 Controller를 만든다.

### 설정

여러 AI 제공자와 연결 정보를 위한 설정을 제거한다. 모델 역할, 승인, 비용, 검사, 병렬 한도, 보관 기간을 위한 설정으로 단순화한다.

### 작업 도구 연결

범용 도구 연결 계층을 크게 만들지 않는다. Codex가 이미 제공하는 파일, 터미널, 브라우저, 외부 연결 기능을 사용하고 Star-Control은 계획과 허가만 관리한다.

## Star Sentinel 판정

Star Sentinel을 별도 제품과 별도 이름으로 유지하지 않는다.

효용이 있는 다음 기능은 Star-Control의 범위 검사와 검사 엔진에 흡수한다.

- 허용 범위 밖 변경 감지
- 비밀정보 감지
- 테스트 삭제와 약화 감지
- 예상하지 않은 의존 항목 변경 감지
- 변경량과 위험 신호
- 검사 결과 정규화
- 독립 검토용 요약 생성

사용자가 직접 실행할 필요가 있는 기능은 star check와 star review 명령으로 제공한다.

## CLI-only 본체와 Codex 선택 연동

Star-Control의 개발 관리·검증·release·평가 본체는 `star` CLI와 Controller application service만으로 실행할 수 있어야 한다. Project Catalog·Code Index, 영향 분석, Patch/Gate, ChangeBundle, release readiness와 Rule·Profile 평가는 Codex·AI 호출을 선행조건으로 삼지 않는다. 결정적 도구로 확정할 수 없는 의미 판단은 CLI-only에서 성공으로 바꾸지 않고 `HUMAN_REVIEW`로 남긴다.

Codex Plugin·MCP·App Server는 자연어 목표, Codex 작업 실행·병렬화와 선택적 독립 검토를 같은 application command에 연결하는 소비자다. 여기서 **Codex 전용**은 지원하는 AI 연동이 Codex 하나라는 뜻이지, 모든 `star` 명령이 Codex를 호출한다는 뜻이 아니다. CLI-only core와 Codex 연동 효용은 [10단계 정본](../contracts/ci-release-evaluation-and-product-completion.md#cli-only와-codex-연동-효용-분리)에서 별도 cohort로 측정한다.

## 최종 범위에 포함

- Codex 앱 자연어 목표 입력
- 질문과 계획 승인
- 단계 분해와 자동 배정
- 사용자 배정 수정
- Codex 작업 자동 생성
- 자동 재시도와 승급
- 터미널 제어
- Windows 배경 Controller
- Plugin, MCP, Hook
- 여러 프로젝트 작업
- 인터넷 조사
- 병렬 Codex 작업
- Git worktree 생성과 정리
- 로컬 병합 대기열과 충돌 처리
- 로컬 변경 기록
- 원격 업로드, 검토 요청, 병합
- 위험 기반 검사
- 증거, 비용, 이어하기 기록
- 비교 시험과 배정 규칙 보정
- 공개 설치와 업데이트
- 배포 준비 확인
- local quick·target·full·release 검사 계층과 build-once artifact 승격
- `ready`·`approved`·`published`를 분리한 공개 release·deploy 상태
- install·safe_default 첫 실행·update·rollback·uninstall과 사용자 자료 보존
- Rule·Check·Profile·Recipe의 평가·trial·deprecation·migration

## 최종 범위에서 제외

| 제외 기능 | 이유 |
|---|---|
| 로컬 AI | Codex 전용 구조를 흐리고 관리 비용이 큼 |
| 다른 AI 제공자 | 제공자 비교와 공통 연결 계층이 핵심 목적이 아님 |
| OpenAI API 직접 호출 | Codex의 인증과 실행 기능을 그대로 사용하기 위함 |
| OpenAI 호환 서버 | 로컬 AI와 범용 제공자 연결이 다시 생김 |
| 제공자 등록소 | 선택 대상이 Codex 하나뿐임 |
| 제공자별 연결 방식 | App Server 하나로 통일 |
| GPU 관리 | AI 실행 환경을 직접 운영하지 않음 |
| 여러 AI 사이 장애 대체 | Codex 전용 원칙과 충돌 |
| 브라우저 Star-Control UI | Codex 앱과 터미널로 충분함 |
| 자체 예약 실행 | Codex 기본 예약 기능과 중복 |
| 자체 CI runner·build farm·artifact registry | 기존 CI·build·registry를 adapter로 연결하고 상태·증거·승격만 관리함 |
| 자체 installer·signing·PKI·deploy service | 외부 전문 도구의 typed invocation·receipt·검증만 관리함 |
| compiler·scanner·debugger·profiler·package manager 재구현 | 프로젝트 전문 도구를 등록된 adapter로 호출함 |
| 범용 운영 플랫폼 | 개인 개발 제어와 검사 범위를 벗어남 |

## 보류하지 않는 원칙

최종 범위에 포함된 기능은 작은 시험판 이후로 무기한 미루지 않는다. 구현 순서는 나누되 모든 포함 기능을 최종 완료 조건에 둔다.
