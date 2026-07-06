# E32 Release Readiness API Read

## 목표

M9g slice는 M9f에서 생성한 ReleaseReadiness artifact를 API read-only control-plane surface에 연결한다. 이 단계는 HTTP server 없이 `ApiReadOnlyService` path dispatch만 확장하며, readiness artifact를 읽고 schema-valid API envelope으로 반환한다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
api-contract.md
testing-ci-release.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
Cargo.lock
packages/star-control-api/**
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
CLI command 추가
artifact signing 구현
package registry 설정
repository branch protection/settings 변경
```

## 입력 artifact

```text
.ai-runs/{job_id}/release/release-readiness.json
specs/schemas/release-readiness.schema.json
specs/schemas/api-response.schema.json
```

## 출력 surface

```text
GET /projects/{project_id}/jobs/{job_id}/release-readiness
ApiReadOnlyService response envelope
```

## 핵심 TASK

```text
star-control-api -> star-control-release local dependency 추가
ApiReadOnlyService release-readiness GET path 추가
ReleaseReadinessWriter::read 기반 readback
missing readiness structured error 추가
read-only no mutation regression test
API response schema validation
release/deploy/publish automation 미구현 유지
```

## 완료 기준

- `GET /projects/{project_id}/jobs/{job_id}/release-readiness`가 existing readiness artifact를 반환해야 한다.
- 반환 envelope은 `api-response.schema.json`을 만족해야 한다.
- missing readiness artifact는 `release_readiness_not_found` structured error로 반환해야 한다.
- endpoint는 StateStore artifact를 수정하지 않아야 한다.
- HTTP server, CLI command, browser UI app, release/deploy/publish, repository settings, workflow, schema field 변경은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-api --locked -- --nocapture
cargo clippy -p star-control-api --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9h는 release readiness CLI/UI read surface, release profile/changelog/version checker, 또는 recovery command surface 중 하나로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
