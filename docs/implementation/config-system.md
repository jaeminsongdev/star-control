# Config System 구현 계약

## 목적

Config System은 Star-Control의 provider instance, policy, hook, role, renderer, skill 설정을 일관된 방식으로 읽고 병합하는 계층이다. 초기 구현은 repository 내부 example과 schema 계약만 고정하고, 실제 사용자 config discovery는 별도 구현 PR에서 진행한다.

## 계약 파일

Config 관련 machine-readable schema는 다음을 기준으로 한다.

```text
specs/schemas/config.schema.json
specs/schemas/policy.schema.json
specs/schemas/hook.schema.json
specs/schemas/role.schema.json
specs/schemas/renderer.schema.json
specs/schemas/skill.schema.json
```

Canonical example은 다음에 둔다.

```text
examples/config-contracts/config.example.json
examples/config-contracts/policy.example.json
examples/config-contracts/hook.example.json
examples/config-contracts/role.example.json
examples/config-contracts/renderer.example.json
examples/config-contracts/skill.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## config 종류

| 종류 | 역할 |
|---|---|
| ConfigSpec | 어떤 provider, policy, hook, role, renderer, skill을 사용할지 묶는 상위 설정 |
| PolicySpec | 변경 위험과 approval 필요 조건을 선언 |
| HookSpec | lifecycle event 전후에 실행할 내부 step 선언 |
| RoleSpec | stage별 worker/reviewer 역할과 필요한 capability 선언 |
| RendererSpec | JSON artifact를 Markdown 또는 text로 변환하는 template 선언 |
| SkillSpec | 재사용 가능한 내부 기능 단위와 안전 조건 선언 |

## 병합 계층

초기 병합 순서는 다음을 권장한다.

```text
repository
project
user
run
```

우선순위는 뒤쪽 계층이 앞쪽 계층을 덮어쓴다.

```text
repository < project < user < run
```

초기 구현은 `deep_override`만 지원해도 된다. `replace`는 장기 옵션이다.

## ConfigSpec

필수 필드:

```text
schema_version
config_id
scope
merge
providers
policies
hooks
roles
renderers
skills
```

`scope` 후보:

```text
repository
project
user
run
```

각 배열 값은 설정 파일 path 또는 logical id를 담을 수 있다. 초기 구현에서는 repository-relative path를 우선한다.

## PolicySpec

PolicySpec은 RouterEngine, ValidationEngine, Star Sentinel 또는 CLI가 approval 후보를 판단할 때 참고할 수 있는 rule 묶음이다.

필수 필드:

```text
schema_version
policy_id
description
rules
```

rule severity 후보:

```text
info
warn
block
approval_required
```

초기 구현에서는 policy rule을 직접 평가하지 않아도 된다. 대신 schema와 example을 고정해 후속 evaluator가 같은 계약을 사용하도록 한다.

## HookSpec

HookSpec은 lifecycle event 전후에 실행할 내부 step을 선언한다.

지원 event 후보:

```text
before_route
after_route
before_provider_run
after_provider_run
before_validation
after_validation
before_report
after_report
```

지원 step kind 후보:

```text
validate_schema
render_template
append_event
write_report
call_tool
```

Hook step은 shell command가 아니다. 외부 command hook은 별도 approval과 security policy가 정리된 뒤 도입한다.

## RoleSpec

RoleSpec은 RouterEngine이 stage assignment를 만들 때 사용할 역할 정의다.

필수 필드:

```text
schema_version
role_id
description
allowed_stages
required_capabilities
default_policy_profiles
```

역할 예시:

```text
worker-impl
worker-review
worker-validate
worker-report
```

## RendererSpec

RendererSpec은 ReportSpec, ReviewPack, ApprovalRequest 같은 구조화 artifact를 사람이 읽을 수 있는 문서로 변환하는 기준이다.

초기 template engine 후보:

```text
plain
handlebars_like
reserved
```

초기 구현에서는 `plain` template만 지원해도 된다. 복잡한 template engine dependency는 별도 승인 전까지 추가하지 않는다.

## SkillSpec

SkillSpec은 내부 재사용 기능 단위다. 예를 들어 repo summary, diff summary, report rendering, schema validation 같은 기능을 skill로 선언할 수 있다.

필수 필드:

```text
schema_version
skill_id
description
inputs
outputs
safety
```

`safety.requires_approval`이 true인 skill은 자동 실행하지 않는다.

## path와 secret 규칙

- config path는 repository-relative 또는 project-relative path를 사용한다.
- absolute path를 user-facing artifact에 그대로 노출하지 않는다.
- credential raw value를 config에 저장하지 않는다.
- credential은 provider instance의 `credential_ref` 같은 reference로만 표현한다.
- config merge 결과도 secret raw value를 포함하면 안 된다.

## 구현 순서

ConfigSystem 구현은 다음 순서로 진행한다.

1. JSON/YAML parse helper
2. ConfigSpec schema validation
3. policy/hook/role/renderer/skill schema validation
4. repository-relative path resolver
5. config merge
6. merged config report
7. router/provider/validation integration

## 테스트 기준

1. config example schema validation
2. policy example schema validation
3. hook example schema validation
4. role example schema validation
5. renderer example schema validation
6. skill example schema validation
7. merge order가 뒤쪽 계층 우선임을 확인
8. unknown config path 오류 처리
9. absolute path 또는 traversal path 차단
10. credential raw value가 report에 노출되지 않음

## 후속 작업

- `check_config_contracts.py`에서 config path cross-reference를 검증한다.
- YAML config example을 schema validation에 연결한다.
- project/user/run config discovery 위치를 OS별로 정리한다.
- CLI에서 `star-control config inspect` 후보를 정의한다.
