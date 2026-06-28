# RouterEngine 구현 계약

## 목적

RouterEngine은 사용자 요청을 분석해 RouteSpec을 생성한다. RouteSpec은 작업 크기, 위험도, stage 목록, provider assignment, approval 필요 여부를 고정한다.

## 책임

RouterEngine이 담당하는 것:

- request_text와 user_constraints 분석
- size 판단
- risk 판단
- stage sequence 결정
- provider capability 기반 assignment
- approval 필요 여부 판단
- approval reason 기록
- WorkSpec 생성에 필요한 route metadata 제공

RouterEngine이 담당하지 않는 것:

- provider 실행
- source file 직접 수정
- validation rule 실행
- Star Sentinel 내부 rule 판정
- cost 실제 과금 처리

## 입력

최소 입력:

```text
JobSpec
available provider instances
capability profiles
project metadata 후보
user constraints
```

## 출력

RouteSpec을 생성한다.

핵심 필드:

```text
schema_version
job_id
summary
size
risk
stages
assignments
requires_user_approval
approval_reasons
workspecs
```

## size 판단

size enum:

```text
SMALL
MEDIUM
LARGE
CRITICAL
```

초기 휴리스틱:

- `SMALL`: 문서 수정, schema/example 추가, 단일 파일 수준 변경
- `MEDIUM`: 여러 파일의 제한된 구현, 단일 package 작업
- `LARGE`: package 여러 개, provider/CLI/state 연동 포함
- `CRITICAL`: 보안, workflow, dependency, public API, 대규모 삭제/이동, release 관련

모호하면 더 높은 size를 선택한다.

## risk 판단

risk enum:

```text
LOW
MEDIUM
HIGH
CRITICAL
```

초기 휴리스틱:

- `LOW`: 문서, example, non-runtime schema 보강
- `MEDIUM`: runtime code 추가, test 추가, local-only provider
- `HIGH`: workflow, dependency, schema breaking change, file deletion, public API
- `CRITICAL`: credential, deployment, release, destructive operation, external account, security bypass

모호하면 더 높은 risk를 선택한다.

## stage 결정

사용 가능한 stage:

```text
route
plan
design
implement
validate
review
polish
report
```

문서/계약 작업 기본 stage:

```text
implement
validate
review
report
```

코드 구현 작업 기본 stage:

```text
design
implement
validate
review
polish
report
```

고위험 작업에는 approval stage를 명시적으로 반영한다.

## provider assignment

RouterEngine은 provider 이름이 아니라 capability로 선택한다.

초기 assignment 예시:

```json
{
  "implement": {
    "role": "worker-impl",
    "provider": "fake",
    "profile": "quick"
  },
  "review": {
    "role": "worker-review",
    "provider": "fake",
    "profile": "quick"
  }
}
```

초기 구현은 FakeProviderAdapter를 우선 사용한다. 실제 cloud/local provider assignment는 provider system smoke 이후 활성화한다.

## approval 판단

다음 경우 `requires_user_approval`을 true로 설정한다.

- dependency 추가 또는 version 변경
- package manager 도입
- workflow 변경
- release/deploy 관련 작업
- public API breaking change
- schema breaking change
- file deletion 또는 대량 이동
- security-sensitive path 변경
- credential 또는 secret 관련 작업
- validation policy 변경
- Star Sentinel self-bypass 가능성이 있는 변경

approval reason은 구체적으로 기록한다.

예시:

```text
dependency_addition_requires_approval
workflow_change_requires_approval
schema_breaking_change_requires_approval
validator_policy_change_requires_approval
```

## forbidden routing

RouterEngine은 다음을 자동 route로 보내면 안 된다.

- 사용자 승인 없는 dependency install
- 사용자 승인 없는 deploy/release
- 사용자 승인 없는 external account mutation
- test 삭제나 weakening이 목표인 작업
- CI 검사를 삭제해서 통과시키는 작업
- secret 값을 출력하거나 저장하는 작업

## WorkSpec 생성 기준

RouterEngine은 WorkSpec 생성에 필요한 정보를 제공한다.

각 WorkSpec에는 다음이 포함되어야 한다.

```text
stage
role
provider
project_root
goal
allowed_scope
forbidden_actions
required_outputs
validation_requirements
```

allowed_scope는 좁게 잡는다. 불명확하면 작업을 BLOCK 또는 WAITING_APPROVAL로 보내는 것이 안전하다.

## policy guard

RouterEngine은 Star Sentinel policy와 중복되더라도 고위험 change type을 route 단계에서 먼저 표시한다.

후보 change type:

```text
public_api_change
schema_change
dependency_addition
dependency_version_change
validator_config_change
risk_path_change
file_deletion
```

## budget guard

RouterEngine은 provider assignment 전에 budget 후보를 확인한다.

초기 구현:

- budget enforcement는 optional
- budget metadata는 RouteSpec 또는 RunState에 남길 수 있음
- budget 초과 가능성이 있으면 approval required

## deterministic behavior

초기 RouterEngine은 deterministic해야 한다.

- 같은 JobSpec과 같은 provider registry면 같은 RouteSpec을 생성한다.
- randomness 사용 금지
- 현재 시간 기반 route decision 금지

## error model

Router 오류 후보:

```text
NoProviderAvailable
CapabilityMissing
ApprovalRequired
InvalidJobSpec
UnsupportedRequest
RouteGenerationFailed
```

## 테스트 기준

최소 테스트:

1. 문서 요청 -> LOW/SMALL route
2. schema 변경 요청 -> approval reason 포함
3. dependency 추가 요청 -> requires_user_approval true
4. unknown high risk 요청 -> HIGH 또는 CRITICAL
5. provider capability 부족 -> NoProviderAvailable
6. 같은 입력에 deterministic output
7. stage list가 schema enum만 사용
8. assignments에 role/provider 포함

## Codex 구현 지시

RouterEngine 구현 PR은 다음만 포함한다.

- RouterEngine module
- route generation tests
- fake provider assignment tests
- 필요한 schema/example 보강

ProviderAdapter 실행, StateStore 구현, CLI 구현을 같은 PR에 섞지 않는다.
