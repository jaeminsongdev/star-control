# E06 RouterEngine Brief

## 목표

JobSpec과 provider registry를 기반으로 deterministic RouteSpec과 WorkSpec 후보 metadata를 생성한다.

## 선행 문서

```text
docs/implementation/router-decision-matrix.md
docs/implementation/router-engine.md
docs/implementation/handoff-vocabularies.md
docs/implementation/provider-system.md
docs/implementation/policy-profiles.md
```

## 수정 허용 파일

```text
packages/star-control-router/** 또는 선택된 router crate
examples/router-contracts/** 필요 최소 범위
관련 unit tests
```

## 수정 금지 파일

```text
ProviderAdapter 실행 로직
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
cloud/local provider 실제 연결
```

## 핵심 작업

```text
size/risk decision
change_types canonical enum 사용
policy_profile selection
decision selection
stage list generation
fake-default assignment
approval reason generation
deterministic route output
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

같은 JobSpec과 같은 registry에서 같은 RouteSpec을 생성해야 한다.

## handoff

E07이 읽을 RouteSpec/WorkSpec path와 assignment structure를 PR 보고에 남긴다.

## 중단 조건

schema breaking change, 새 vocabulary, provider activation 정책 변경이 필요하면 멈춘다.
