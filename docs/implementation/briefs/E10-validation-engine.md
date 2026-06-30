# E10 ValidationEngine Brief

## 목표

ProviderRunResult와 Star Sentinel output을 ValidationDecision, approval request, review-pack handoff, RunState 전이로 연결한다.

## 선행 문서

```text
docs/implementation/validation-engine.md
docs/implementation/validation-handoff.md
docs/implementation/star-sentinel-p0-contracts.md
docs/implementation/approval-review-flow.md
```

## 수정 허용 파일

```text
packages/star-control-validation/** 또는 선택된 validation crate
examples/validation-contracts/** 필요 최소 범위
관련 unit tests
```

## 수정 금지 파일

```text
Star Sentinel 전체 rule engine 구현
cloud provider 구현
daemon 구현
UI 구현
package manager 추가 도입
```

## 핵심 작업

```text
validation requirement collection
provider output check
Star Sentinel task artifact generation
check/gate output loading
ValidationDecision generation
ApprovalRequest generation
ReviewPackHandoff generation
RunState transition
report validation section 후보
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

AUTO_PASS, HUMAN_REVIEW, BLOCK, invalid output이 각각 정확한 next_state로 mapping되어야 한다.

## handoff

E11 integration smoke가 사용할 fake run validation path와 required artifact list를 PR 보고에 남긴다.

## 중단 조건

Star Sentinel rule을 core에 직접 구현하거나 approval response 없이 다음 stage로 진행해야 할 것 같으면 멈춘다.
