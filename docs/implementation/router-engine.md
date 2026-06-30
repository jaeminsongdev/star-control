# RouterEngine 구현 계약

## 목적

RouterEngine은 사용자 요청을 분석해 RouteSpec을 생성한다. RouteSpec은 작업 크기, 위험도, policy profile, stage 목록, provider assignment, approval 필요 여부, decision을 고정한다.

상세 size/risk/profile/approval/decision matrix는 `router-decision-matrix.md`를 따른다.

## 함께 읽을 문서

```text
router-decision-matrix.md
provider-system.md
config-system.md
policy-profiles.md
approval-review-flow.md
data-contracts.md
```

## 책임

RouterEngine이 담당하는 것:

- request_text와 user_constraints 분석
- change_types 후보 산출
- size 판단
- risk 판단
- policy profile 선택
- decision 산출
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
- approval response 처리

## 입력

최소 입력:

```text
JobSpec
available provider instances
capability profiles
role specs
policy specs
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
policy_profile
decision
change_types
routing_reasons
stages
assignments
requires_user_approval
approval_reasons
workspecs
```

`policy_profile`, `decision`, `change_types`, `routing_reasons`는 optional로 시작하지만 RouterEngine 구현 시 채우는 것을 기본으로 한다.

## decision pipeline

RouterEngine은 아래 순서를 따른다.

```text
1. request_text와 user_constraints 분석
2. change_types 후보 산출
3. size 산출
4. risk 산출
5. policy_profile 산출
6. requires_user_approval 산출
7. decision 산출
8. stages 산출
9. provider assignment 산출
10. RouteSpec 생성
```

앞 단계에서 더 높은 위험이 발견되면 뒤 단계는 더 안전한 방향으로 승격한다.

## size 판단

size enum:

```text
SMALL
MEDIUM
LARGE
CRITICAL
```

기준:

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

기준:

- `LOW`: 문서, example, non-runtime schema 보강
- `MEDIUM`: runtime code 추가, test 추가, local-only provider
- `HIGH`: workflow, dependency, schema breaking change, file deletion, public API
- `CRITICAL`: credential, deployment, release, destructive operation, external account, security bypass

모호하면 더 높은 risk를 선택한다.

## change_types

RouterEngine은 request에서 change type 후보를 산출한다.

주요 후보:

```text
docs_only
example_change
schema_change
schema_breaking_change
runtime_code_change
multi_package_change
provider_contract_change
dependency_addition
dependency_version_change
workflow_change
credential_change
secret_exposure
release_change
deploy_change
validator_sensitive_change
validator_self_bypass
file_deletion
bulk_move
external_account_change
unknown_high_risk
```

## policy profile 선택

profile 후보:

```text
quick
near
full
security
release
validator
```

우선순위:

```text
validator > release > security > full > near > quick
```

Special profile 조건은 일반 risk/size보다 우선한다.

```text
validator-sensitive -> validator
release/deploy -> release
security-sensitive -> security
LARGE or CRITICAL -> full
MEDIUM -> near
LOW/SMALL -> quick
```

## decision

Decision 후보:

```text
AUTO_PASS
HUMAN_REVIEW
BLOCK
```

기준:

- `AUTO_PASS`: approval false, risk LOW/MEDIUM, block reason 없음
- `HUMAN_REVIEW`: approval true, 사람이 판단 가능, block reason 없음
- `BLOCK`: secret exposure, out-of-scope, validator self-bypass, destructive action

`BLOCK` 조건이 있으면 `HUMAN_REVIEW`보다 우선한다.

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

schema/validator sensitive 변경 기본 stage:

```text
design
implement
validate
review
report
```

blocked 작업은 자동 구현 stage를 만들지 않고 `route`, `report`만 사용할 수 있다.

## provider assignment

RouterEngine은 provider 이름이 아니라 capability로 선택한다.

초기 assignment 예시:

```json
{
  "implement": {
    "role": "worker-impl",
    "provider": "fake-default",
    "profile": "quick"
  },
  "review": {
    "role": "worker-review",
    "provider": "fake-default",
    "profile": "quick"
  }
}
```

Provider assignment는 다음 순서를 따른다.

```text
1. stage가 요구하는 role 확인
2. role이 요구하는 capability 확인
3. enabled provider instance만 후보로 사용
4. routing_tags와 capability_profile 확인
5. policy_profile을 assignment.profile에 기록
6. 후보가 없으면 NoProviderAvailable
```

초기 구현은 FakeProviderAdapter를 우선 사용한다. 실제 cloud/local provider assignment는 provider system smoke 이후 활성화한다.

## approval 판단

다음 경우 `requires_user_approval`을 true로 설정한다.

```text
dependency_addition
dependency_version_change
workflow_change
release_change
deploy_change
public_api_change
schema_breaking_change
schema_change
file_deletion
bulk_move
risk_path_change
credential_change
secret_exposure
validator_sensitive_change
validator_self_bypass
external_account_change
budget_exceeded
unknown_high_risk
```

approval reason은 구체적으로 기록한다.

예시:

```text
schema_change_requires_approval
validator_profile_requires_review
workflow_change_requires_approval
secret_exposure_blocked
```

## forbidden routing

RouterEngine은 다음을 자동 route로 보내면 안 된다.

- 사용자 승인 없는 dependency install
- 사용자 승인 없는 deploy/release
- 사용자 승인 없는 external account mutation
- test 삭제나 weakening이 목표인 작업
- CI 검사를 삭제해서 통과시키는 작업
- secret 값을 출력하거나 저장하는 작업
- validator self-bypass 요청

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

allowed_scope는 좁게 잡는다. 불명확하면 작업을 `BLOCK` 또는 `WAITING_APPROVAL`로 보내는 것이 안전하다.

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
- provider availability race 기반 임의 선택 금지

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

1. 문서 요청 -> LOW/SMALL/quick/AUTO_PASS route
2. schema 변경 요청 -> HIGH/validator/HUMAN_REVIEW route
3. dependency 추가 요청 -> security profile과 approval reason 포함
4. secret exposure -> BLOCK
5. unknown high risk 요청 -> HIGH 또는 CRITICAL
6. provider capability 부족 -> NoProviderAvailable
7. 같은 입력에 deterministic output
8. stage list가 schema enum만 사용
9. assignments에 role/provider/profile 포함
10. route approval example이 schema-example-check를 통과

## Codex 구현 지시

RouterEngine 구현 PR은 다음만 포함한다.

- RouterEngine module
- route generation tests
- fake provider assignment tests
- 필요한 schema/example 보강

ProviderAdapter 실행, StateStore 구현, CLI 구현을 같은 PR에 섞지 않는다.
