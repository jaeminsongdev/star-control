# E37 Release Readiness CLI Read

## 목표

M9l slice는 existing ReleaseReadiness artifact를 CLI에서 read-only로 조회하는 surface를 추가한다. 새 top-level command를 늘리지 않고 기존 `star-control report`에 `--release-readiness` option을 추가해 `.ai-runs/{job_id}/release/release-readiness.json`을 읽는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
cli-command-reference.md
testing-ci-release.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-cli/**
Cargo.lock
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
new top-level CLI command
artifact signing 구현
package registry 설정
repository branch protection/settings 변경
StateStore mutation
Star Sentinel profile evaluator 변경
```

## 입력

```text
star-control report --project <path> --job <job-id> --release-readiness
```

## 출력

```text
schema-valid CLI output envelope
report_kind = release_readiness
release_readiness_path
release_actions_enabled = false
readiness
```

## 핵심 TASK

```text
report --release-readiness option 추가
ReleaseReadinessWriter readback 재사용
missing readiness artifact error
--stage 조합 거부
release action disabled regression
no mutation regression
```

## 완료 기준

- `star-control report --release-readiness --json`이 existing ReleaseReadiness artifact를 schema-valid CLI output envelope로 반환해야 한다.
- missing artifact는 schema-valid CLI error envelope과 `.ai-runs/{job_id}/release/release-readiness.json` artifact path를 반환해야 한다.
- `--stage`와 `--release-readiness`를 함께 쓰면 invalid input으로 거부해야 한다.
- CLI가 readiness artifact, StateStore, release/deploy/publish state를 수정하지 않아야 한다.
- 새 top-level CLI command, browser app, HTTP server, schema field, workflow, release/deploy/publish, repository settings 변경은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-cli --locked -- --nocapture
cargo clippy -p star-control-cli --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9m는 release review pack foundation으로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
