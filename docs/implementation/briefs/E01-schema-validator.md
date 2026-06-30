# E01 Schema / Runtime Validator Brief

## 목표

JSON schema와 canonical example을 runtime에서 검증할 최소 validator package를 구현한다.

## 선행 문서

```text
docs/decisions/0002-runtime-stack.md
docs/implementation/schema-validator.md
docs/implementation/data-contracts.md
docs/implementation/ci-contract-validation.md
```

## 수정 허용 파일

```text
packages/star-control-schema/** 또는 선택된 schema crate
관련 unit tests
필요한 최소 docs/example 업데이트
Cargo workspace baseline 파일
```

## 수정 금지 파일

```text
StateStore 구현 파일
ProviderAdapter 구현 파일
RouterEngine 구현 파일
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
Star Sentinel rule 구현 파일
```

## 핵심 작업

```text
Cargo workspace 최소 scaffold
schema file loader
JSON parse errors
schema_version const validation
required/properties/items validation
pattern/minLength validation 후보
validation error model
canonical example validation tests
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

canonical examples를 runtime validator로 검증할 수 있어야 한다.

## handoff

E02가 사용할 validator API 이름, error model, test fixture 위치를 PR 보고에 남긴다.

## 중단 조건

새 production dependency가 필요하거나 schema breaking change가 필요하면 작업을 멈추고 확인한다.
