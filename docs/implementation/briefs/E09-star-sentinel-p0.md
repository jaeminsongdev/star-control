# E09 Star Sentinel P0 Brief

## 목표

Star Sentinel v0 P0를 E09a~E09d로 나누어 구현한다. E09a는 5개 핵심 rule evaluator만 포함한다.

## 선행 문서

```text
docs/decisions/0004-star-sentinel-p0-scope.md
docs/implementation/star-sentinel-p0-contracts.md
docs/implementation/star-sentinel-p0-implementation-split.md
docs/implementation/star-sentinel-full-spec.md
```

## 수정 허용 파일

```text
packages/star-sentinel/** 또는 선택된 Star Sentinel crate
builtin-tools/star-sentinel/policies/**
builtin-tools/star-sentinel/examples/p0/**
관련 unit tests
필요한 최소 docs/example 업데이트
```

## 수정 금지 파일

```text
Star-Control core에 rule 직접 구현
cloud/local provider 구현
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
release profile automation
```

## v0 P0 rule

```text
task.scope.allowed_paths
test.no_deletion
dependency.requires_approval
secret.no_plaintext_secret
validator.no_self_bypass
```

## PR 분할

```text
E09a input reader + registry loader + evaluator
E09b diagnostics + gate writer
E09c review-pack writer
E09d ledger + selfcheck
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

P0 fixtures가 expected diagnostics 또는 expected decision 후보를 생성해야 한다.

## handoff

E10 ValidationEngine이 호출할 command, required input artifact, output artifact, decision mapping을 PR 보고에 남긴다.

## 중단 조건

P1 rule, full/security/release profile, ValidationEngine integration을 E09a에 섞어야 할 것 같으면 멈춘다.
