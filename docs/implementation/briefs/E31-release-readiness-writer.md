# E31 Release Readiness Writer

## 목표

M9f slice는 release/deploy/publish 자동화 전에 필요한 ReleaseReadiness artifact writer를 구현한다. 이 단계는 readiness JSON을 생성하고 검증할 뿐이며, GitHub release, package publish, cloud deploy, repository settings 변경은 수행하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
testing-ci-release.md
artifact-layout.md
state-store.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
Cargo.toml
Cargo.lock
packages/star-control-release/**
packages/star-control-validation/src/lib.rs
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
artifact signing 구현
package registry 설정
repository branch protection/settings 변경
```

## 입력 artifact

```text
specs/schemas/release-readiness.schema.json
examples/release-contracts/release-readiness.example.json
StateStore job directory
```

## 출력 artifact

```text
release/release-readiness.json
ArtifactRef(kind=other, producer=star-control-release)
ReleaseReadinessWriter tests
```

## 핵심 TASK

```text
star-control-release crate 추가
ReleaseReadinessWriter 추가
reserved readiness builder 추가
not_ready readiness builder 추가
release-readiness.schema.json validation
ready status reserved rejection
reserved status blocker explanation check
release/release-readiness.json write/readback helper
overwrite rejection test
path traversal job id rejection test
validation fixture temp path counter stabilization if workspace test exposes a Windows temp collision
```

## 완료 기준

- `ReleaseReadinessWriter`가 schema-valid ReleaseReadiness JSON을 `.ai-runs/{job_id}/release/release-readiness.json`에 저장해야 한다.
- writer가 반환하는 ArtifactRef는 `kind=other`, `producer=star-control-release`, `schema_path=specs/schemas/release-readiness.schema.json`을 사용해야 한다.
- 현재 slice에서는 `ready` status를 거부하고, `reserved` status는 blocker explanation을 요구해야 한다.
- 기존 readiness artifact를 조용히 덮어쓰지 않아야 한다.
- release/deploy/publish, external account/repository settings 변경, workflow 변경, schema field 변경은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-release --locked -- --nocapture
cargo clippy -p star-control-release --all-targets --locked -- -D warnings
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9g는 release readiness를 control-plane report/API/CLI read surface에 연결하거나 recovery command surface를 구현하는 slice로 이어간다. 실제 signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
