# E07 ExecutionEngine Brief

## 목표

WorkSpec, provider registry, FakeProviderAdapter, StateStore를 연결해 provider execution artifact를 생성한다.

## 선행 문서

```text
docs/implementation/execution-engine.md
docs/implementation/state-store.md
docs/implementation/artifact-layout.md
docs/implementation/provider-system.md
```

## 수정 허용 파일

```text
packages/star-control-execution/** 또는 선택된 execution crate
관련 unit tests
필요한 최소 docs/example 업데이트
```

## 수정 금지 파일

```text
RouterEngine 판단 로직
Star Sentinel rule 구현
ValidationEngine 구현 파일
CLI 구현 파일
cloud/local provider 실제 연결
```

## 핵심 작업

```text
WorkSpec loading
provider assignment lookup
output directory preparation
ExecutionRequest writing
FakeProviderAdapter connection
ProviderRunResult validation
RunState update 후보
event append 후보
idempotency guard
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

FakeProvider WorkSpec 실행으로 request/response artifact를 만들고 기존 output을 조용히 덮어쓰지 않아야 한다.

## handoff

E08 CLI가 호출할 execution entrypoint, required state/artifact precondition을 PR 보고에 남긴다.

## 중단 조건

local/cloud provider, shell command 직접 실행, retry policy 확장이 필요하면 멈춘다.
