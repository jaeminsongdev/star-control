# E04 Provider Registry Brief

## 목표

builtin provider registry, provider manifest, capability profile, provider instance loader를 구현한다.

## 선행 문서

```text
docs/implementation/provider-system.md
docs/implementation/config-system.md
docs/decisions/0003-fake-provider-instance.md
configs/registries/builtin-provider-registry.yaml
```

## 수정 허용 파일

```text
packages/star-control-provider/** 또는 선택된 provider crate
관련 unit tests
필요한 최소 docs/example 업데이트
```

## 수정 금지 파일

```text
FakeProviderAdapter 실행 로직
local/cloud provider 실제 연결
network 호출
provider session 관리
ExecutionEngine 구현 파일
RouterEngine 구현 파일
```

## 핵심 작업

```text
ProviderManifest loader
ProviderInstance loader
ProviderRegistry loader
CapabilityProfile loader
provider id/path cross-reference check
fake-default instance fixture
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

`provider.fake` manifest와 `fake-default` instance를 로딩하고 capability를 조회할 수 있어야 한다.

## handoff

E05/E06/E07이 사용할 ProviderRegistry API와 fake-default fixture 위치를 PR 보고에 남긴다.

## 중단 조건

credential raw value 저장, network check, cloud/local provider 활성화가 필요하면 멈춘다.
