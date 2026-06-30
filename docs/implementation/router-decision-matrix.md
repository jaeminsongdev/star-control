# Router Decision Matrix

## 목적

이 문서는 RouterEngine이 request를 RouteSpec으로 바꿀 때 사용하는 size, risk, policy profile, approval, decision 기준을 한곳에 고정한다. 구현자는 임의 휴리스틱을 추가하기 전에 이 matrix와 schema/example을 먼저 갱신한다.

## machine-readable contracts

```text
specs/schemas/route.schema.json
specs/schemas/router-decision.schema.json
examples/router-contracts/route-approval-required.example.json
examples/router-contracts/router-decision.schema-change.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 결정 순서

RouterEngine은 아래 순서로 판단한다.

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

## size matrix

| 조건 | size |
|---|---|
| 문서, example, non-runtime schema만 수정 | `SMALL` |
| 단일 package 또는 제한된 runtime code 변경 | `MEDIUM` |
| 여러 package 연동, provider/router/execution/validation 연결 | `LARGE` |
| release, deploy, workflow permission, credential, destructive operation | `CRITICAL` |

동시에 여러 조건이 있으면 가장 큰 size를 선택한다.

## risk matrix

| 조건 | risk |
|---|---|
| 문서, example, non-runtime schema 보강 | `LOW` |
| local-only runtime code, tests, fake provider 구현 | `MEDIUM` |
| workflow, dependency, schema breaking, public API, file deletion | `HIGH` |
| credential, deploy, release, destructive operation, external mutation, security bypass | `CRITICAL` |

모호하면 더 높은 risk를 선택한다.

## change type matrix

| change_type | 기본 risk | 기본 profile | approval | decision 후보 |
|---|---|---|---|---|
| `docs_only` | `LOW` | `quick` | false | `AUTO_PASS` |
| `example_change` | `LOW` | `quick` | false | `AUTO_PASS` |
| `schema_change` | `HIGH` | `validator` | true | `HUMAN_REVIEW` |
| `schema_breaking_change` | `HIGH` | `validator` | true | `HUMAN_REVIEW` |
| `runtime_code_change` | `MEDIUM` | `near` | false | `AUTO_PASS` or `HUMAN_REVIEW` |
| `multi_package_change` | `HIGH` | `full` | true | `HUMAN_REVIEW` |
| `provider_contract_change` | `HIGH` | `full` | true | `HUMAN_REVIEW` |
| `dependency_addition` | `HIGH` | `security` | true | `HUMAN_REVIEW` |
| `dependency_version_change` | `HIGH` | `security` | true | `HUMAN_REVIEW` |
| `workflow_change` | `HIGH` | `security` | true | `HUMAN_REVIEW` |
| `credential_change` | `CRITICAL` | `security` | true | `HUMAN_REVIEW` or `BLOCK` |
| `secret_exposure` | `CRITICAL` | `security` | true | `BLOCK` |
| `release_change` | `CRITICAL` | `release` | true | `HUMAN_REVIEW` |
| `deploy_change` | `CRITICAL` | `release` | true | `HUMAN_REVIEW` |
| `validator_sensitive_change` | `HIGH` | `validator` | true | `HUMAN_REVIEW` |
| `validator_self_bypass` | `CRITICAL` | `validator` | true | `BLOCK` |
| `file_deletion` | `HIGH` | `full` | true | `HUMAN_REVIEW` |
| `bulk_move` | `HIGH` | `full` | true | `HUMAN_REVIEW` |
| `external_account_change` | `CRITICAL` | `security` | true | `HUMAN_REVIEW` or `BLOCK` |

## profile precedence

Special profile 조건은 일반 risk/size보다 우선한다.

```text
validator-sensitive -> validator
release/deploy -> release
security-sensitive -> security
LARGE or CRITICAL -> full
MEDIUM -> near
LOW/SMALL -> quick
```

여러 profile 후보가 있으면 아래 우선순위를 사용한다.

```text
validator > release > security > full > near > quick
```

단, `validator`, `release`, `security`는 목적별 special profile이므로 단순 강도 비교로 낮추면 안 된다.

## approval matrix

`requires_user_approval`은 다음 중 하나라도 해당하면 true다.

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

Approval reason은 기계적으로 추적 가능한 snake_case 문자열로 기록한다.

예시:

```text
schema_change_requires_approval
validator_profile_requires_review
workflow_change_requires_approval
secret_exposure_blocked
```

## decision matrix

| 조건 | decision |
|---|---|
| approval false, risk LOW/MEDIUM, block reason 없음 | `AUTO_PASS` |
| approval true, 사람이 판단 가능, block reason 없음 | `HUMAN_REVIEW` |
| secret exposure, out-of-scope, validator self-bypass, destructive action | `BLOCK` |

`BLOCK` 조건이 있으면 `HUMAN_REVIEW`보다 우선한다.

## stage matrix

| 작업 유형 | stages |
|---|---|
| 문서/계약 변경 | `implement`, `validate`, `review`, `report` |
| runtime code 변경 | `design`, `implement`, `validate`, `review`, `polish`, `report` |
| schema/validator sensitive 변경 | `design`, `implement`, `validate`, `review`, `report` |
| release/deploy 변경 | `design`, `validate`, `review`, `report` |
| blocked 작업 | `route`, `report` |

## provider assignment 기준

초기 구현에서는 fake provider를 우선 사용한다.

Provider assignment는 다음 순서를 따른다.

```text
1. stage가 요구하는 role 확인
2. role이 요구하는 capability 확인
3. enabled provider instance만 후보로 사용
4. routing_tags와 capability_profile 확인
5. policy_profile을 assignment.profile에 기록
6. 후보가 없으면 NoProviderAvailable
```

RouterEngine은 provider 제품명을 직접 기준으로 선택하지 않는다.

## forbidden route

RouterEngine은 다음 요청을 자동 진행시키지 않는다.

- test 삭제 또는 weakening 자체가 목표인 요청
- CI 검사를 제거해서 통과시키는 요청
- secret raw value 출력 또는 저장 요청
- 사용자 승인 없는 release/deploy 요청
- 사용자 승인 없는 외부 계정 변경 요청
- validator self-bypass 요청

## deterministic rule

같은 입력과 같은 provider registry에서 같은 RouteSpec을 생성한다.

금지:

```text
randomness
current time based routing
provider availability race 기반 임의 선택
```

## 테스트 기준

1. docs only -> `SMALL`, `LOW`, `quick`, `AUTO_PASS`
2. schema change -> `HIGH`, `validator`, `HUMAN_REVIEW`
3. dependency addition -> `HIGH`, `security`, `HUMAN_REVIEW`
4. secret exposure -> `CRITICAL`, `security`, `BLOCK`
5. release request -> `CRITICAL`, `release`, `HUMAN_REVIEW`
6. validator self-bypass -> `CRITICAL`, `validator`, `BLOCK`
7. unknown high risk -> approval required
8. same input -> same route
