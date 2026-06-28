# Provider Registry

`configs/registries/builtin-provider-registry.yaml`은 builtin provider의 색인이다.

구체 provider 정의는 `builtin-providers/{group}/{provider}/provider.yaml`에 두고, capability profile은 같은 디렉터리의 `capabilities.yaml`에 둔다.

## 계약 기준

Provider registry 구현자는 아래 계약을 함께 확인한다.

```text
specs/schemas/provider-registry.schema.json
specs/schemas/provider-manifest.schema.json
specs/schemas/capability-profile.schema.json
docs/implementation/provider-system.md
docs/providers/provider-reference-snapshots.md
```

## 정합성 기준

후속 provider contract validator는 다음을 검사해야 한다.

1. registry entry의 `id`가 manifest의 `id`와 일치한다.
2. registry entry의 `capabilities` path가 존재한다.
3. capability profile의 `provider` 값이 registry entry의 `id`와 일치한다.
4. manifest와 capability file이 YAML로 parse 가능하다.
5. manifest의 provider kind가 `provider-kind.schema.json`의 enum에 포함된다.

## 외부 자료 snapshot

`docs/providers/provider-reference-snapshots.md`는 provider별 공식 자료 확인 상태를 기록한다. Registry에 provider를 추가하거나 manifest capability를 바꿀 때는 해당 snapshot도 함께 검토한다.
