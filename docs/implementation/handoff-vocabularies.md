# Handoff Vocabularies

## 목적

이 문서는 RouterEngine, WorkSpec, ExecutionEngine, ValidationEngine 사이에서 공유하는 handoff vocabulary의 기준 위치를 고정한다. 구현자는 같은 변경 유형과 금지 action을 서로 다른 문자열로 표현하지 않는다.

## machine-readable source of truth

Canonical vocabulary는 아래 schema가 우선한다.

```text
specs/schemas/route.schema.json
specs/schemas/router-decision.schema.json
specs/schemas/workspec.schema.json
```

`RouteSpec.change_types`와 `RouterDecision.change_types`는 같은 canonical enum을 사용한다. 같은 의미의 새 alias를 추가하지 않는다.

## canonical change_types 기준

민감정보 노출 위험은 다음 change type으로만 표현한다.

```text
sensitive_data_exposure
```

다음 표현은 canonical vocabulary로 사용하지 않는다.

```text
secret_exposure
secret_leak
plain_secret
```

## canonical forbidden_actions 기준

민감정보 출력 금지는 다음 forbidden action으로만 표현한다.

```text
sensitive_data_output
```

다음 표현은 canonical vocabulary로 사용하지 않는다.

```text
secret_print
secret_output
plaintext_secret_output
```

## RouteSpec handoff 필수 필드

RouterEngine이 생성하는 RouteSpec은 다음 handoff 필드를 반드시 채워야 한다.

```text
policy_profile
decision
change_types
routing_reasons
workspecs
```

이 필드는 구현 handoff에서 optional처럼 취급하지 않는다.

## WorkSpec handoff 필수 필드

WorkSpec은 provider 실행과 artifact layout을 연결하기 위해 다음 필드를 반드시 채워야 한다.

```text
provider
provider_instance
allowed_scope
forbidden_actions
required_outputs
```

`provider_instance`는 provider output directory 계산의 기준이다. ExecutionEngine은 `provider_instance`가 없는 WorkSpec을 실행하지 않는다.

## 변경 규칙

- 새 vocabulary가 필요하면 schema, canonical example, 이 문서를 같은 PR에서 수정한다.
- 기존 vocabulary를 rename하거나 제거하는 변경은 schema/example 영향과 compatibility를 함께 검토한다.
- RouterEngine은 schema에 정의된 `change_types`만 기록한다.
- ExecutionEngine은 schema에 정의된 `forbidden_actions`만 비교한다.
- ValidationEngine과 Star Sentinel은 같은 의미의 위험을 새 문자열로 만들지 않는다.
