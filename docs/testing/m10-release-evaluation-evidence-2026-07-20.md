# P-0051 M10 Release·Evaluation 제품 Slice 증거

## 구현

- `ReleaseManifest` v2, `EvaluationRun` v2와 `EvaluationCatalogItem` lifecycle 공개 계약
- sorted final artifact entry의 canonical `artifact_set_digest`와 immutable `BuildOnceStore`
- signed byte가 달라지면 새 `ReleaseManifestId` candidate를 만들고 unsigned verification·Gate·approval을 상속하지 않는 전이
- `local_quick`, `target`, `full`, `release` 네 계층의 complete·exact digest evidence만 `ready` 후보로 인정
- x64 Stable `native_verified`와 ARM64 Preview `native_unverified`를 분리한 compatibility 판정
- fake CI/signer/publisher adapter와 exact GitHub destination approval boundary
- Evaluation comparator와 `active → deprecated → retired`, trial `→ rejected` lifecycle

## fault corpus

- partial CI success: `blocked`
- CI artifact digest mismatch: `blocked`
- signed-byte digest 변화: 새 candidate, 이전 verification 0건 상속
- publish timeout: publish 1회, read-only reconcile 1회, 미확정이면 `publish_outcome_unknown`
- partial publish 또는 remote digest mismatch: `rollback_required`
- validator guard·Corpus·Profile 약화: metric 개선과 무관하게 `reject`
- non-comparable baseline/candidate: `needs_review`
- baseline의 기존 false negative는 그대로라는 이유만으로 reject하지 않되 candidate가 false negative·false positive·failure/reject/revert·rollback을 악화하면 속도 개선과 무관하게 `reject`
- candidate outcome이 `unknown`이면 자동 accept하지 않고 `needs_review`
- duplicate comparability dimension, 다른 corpus binding과 빈 evidence ref는 평가 전에 invalid로 거부

## 검증

- `cargo test --locked -p star-release -p star-contracts`
  - 결과: PASS
  - `star-release`: 16/16
- generated Schema·minimal/full/invalid/future fixture:
  - `release-manifest-v2`
  - `evaluation-run-v2`
  - `evaluation-catalog-item`
- 실제 signer, 인증서, timestamp provider와 GitHub remote write는 사용하지 않았다.
