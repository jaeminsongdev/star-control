# E05 FakeProviderAdapter Brief

## 목표

외부 실행 없이 deterministic provider result를 만드는 FakeProviderAdapter를 구현한다.

## 선행 문서

```text
docs/implementation/provider-system.md
docs/implementation/execution-engine.md
docs/decisions/0003-fake-provider-instance.md
examples/execution-contracts/
```

## 수정 허용 파일

```text
packages/star-control-provider/** 또는 선택된 provider crate
examples/execution-contracts/** 필요 최소 범위
관련 unit tests
```

## 수정 금지 파일

```text
local/cloud provider 구현
network 호출
shell command 실행
RouterEngine 구현 파일
ExecutionEngine orchestration 구현 파일
CLI 구현 파일
```

## 핵심 작업

```text
FakeProviderAdapter interface implementation
ExecutionRequest reader 후보
ProviderRunResult writer 후보
fake success result
fake failed/blocked result simulation
deterministic metrics cost=0
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

`fake-default` provider instance로 deterministic ProviderRunResult를 만들 수 있어야 한다.

## handoff

E07이 호출할 adapter API와 output artifact shape를 PR 보고에 남긴다.

## 중단 조건

대상 프로젝트 source 수정, network, package manager, shell 실행이 필요하면 멈춘다.
