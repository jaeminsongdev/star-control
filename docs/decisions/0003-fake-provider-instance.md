# 0003 Fake Provider Instance Decision

## 상태

Accepted.

## 결정

Star-Control v0 fake flow에서 사용하는 canonical provider 식별자를 다음으로 고정한다.

```text
provider manifest id: provider.fake
provider instance id: fake-default
provider output dir: provider-output/fake-default/
```

## 범위

이 결정은 v0 fake provider 기반 구현 흐름에 적용한다.

- RouterEngine의 초기 fake assignment
- WorkSpec의 `provider`와 `provider_instance`
- ExecutionRequest의 `provider_instance_id`
- ProviderRunResult의 `provider_instance_id`
- provider output artifact path
- integration smoke fixture

## 사용 기준

초기 v0에서는 provider manifest id와 provider instance id를 구분한다.

```text
provider.fake   # builtin provider manifest id
fake-default    # default provider instance id
```

RouteSpec assignment와 WorkSpec은 실행 가능한 instance를 가리키기 위해 `fake-default`를 사용한다.

예시:

```json
{
  "role": "worker-impl",
  "provider": "fake-default",
  "profile": "near"
}
```

WorkSpec 예시:

```json
{
  "provider": "fake-default",
  "provider_instance": "fake-default"
}
```

## 금지되는 임시 이름

다음 이름은 v0 canonical example에서 사용하지 않는다.

```text
my-fake-provider
fake-provider-instance
sample-fake-provider
```

새 fake provider instance가 필요하면 `fake-default`를 대체하지 말고 별도 instance id를 추가하고, route/workspec/example/schema 영향 범위를 함께 갱신한다.

## 이유

기존 example 일부는 `my-fake-provider`를 사용했고, provider contract와 execution contract는 `fake-default`를 사용했다. v0 구현 전 canonical instance id를 하나로 고정해 RouterEngine, ProviderRegistry, FakeProviderAdapter, ExecutionEngine, CLI smoke가 같은 artifact path를 사용하게 한다.

## 비결정 사항

아래 항목은 이 결정에서 확정하지 않는다.

- local/cloud provider instance naming
- provider registry loader의 내부 자료구조
- user/project config discovery 위치
- fake provider 외 provider activation 시점
