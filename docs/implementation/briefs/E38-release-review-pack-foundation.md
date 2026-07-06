# E38 Release Review Pack Foundation

## 목표

M9m slice는 release readiness를 사람이 검토할 수 있는 Markdown review pack으로 렌더링하는 foundation을 추가한다. 이 slice는 existing ReleaseReadiness value를 검증한 뒤 `.ai-runs/{job_id}/review-packs/release-review-pack.md`를 쓰며, approval record나 release/deploy/publish action을 만들지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
testing-ci-release.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-release/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
새 external dependency
release/deploy/publish automation
external account/repository settings 변경
provider live call
HTTP server 구현
browser UI app 구현
new CLI/API/UI surface
approval record 생성
artifact signing 구현
package registry 설정
repository branch protection/settings 변경
Star Sentinel profile evaluator 변경
```

## 입력

```text
schema-valid ReleaseReadiness value
StateStore job_id
```

## 출력

```text
.ai-runs/{job_id}/review-packs/release-review-pack.md
ArtifactRef kind = review_pack
ArtifactRef producer = star-control-release
```

## 핵심 TASK

```text
ReleaseReviewPackWriter 추가
ReleaseReadinessWriter validation 재사용
release review pack Markdown render
review-packs/release-review-pack.md create_new write
ready status rejection regression
overwrite rejection regression
release action disabled regression
```

## 완료 기준

- `ReleaseReviewPackWriter`가 existing ReleaseReadiness value를 검증한 뒤 Markdown review pack을 생성해야 한다.
- writer는 `.ai-runs/{job_id}/review-packs/release-review-pack.md`를 새 파일로만 써야 하고 overwrite를 거부해야 한다.
- 반환 ArtifactRef는 `kind=review_pack`, `producer=star-control-release`를 사용해야 한다.
- `ready` status는 readiness validation에서 계속 거부되어야 한다.
- review pack은 approval record가 아니며 release/deploy/publish/signing/repository settings action을 실행하거나 활성화하지 않아야 한다.
- schema field, workflow, dependency, CLI/API/UI surface는 변경하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-release --locked -- --nocapture
cargo clippy -p star-control-release --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9n는 recovery command surface로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
