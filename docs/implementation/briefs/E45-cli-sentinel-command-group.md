# E45 CLI Sentinel Command Group

## 목표

M9t slice는 public CLI surface에 남아 있던 `star-control sentinel` command group을 Star Sentinel tool-wrapper로 구현한다. 이 slice는 Star Sentinel rule engine을 CLI에 재구현하지 않고 `packages/star-sentinel` API를 호출해 existing job artifact를 읽거나 tool/review artifact를 쓴다.

## 선행 문서

```text
cli-command-reference.md
star-sentinel-full-spec.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-cli/**
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
provider healthcheck 실행
provider live call
provider execution
credential raw value 출력
release/deploy/publish automation
repository settings 변경
destructive recovery action
HTTP server 구현
browser UI app 구현
Star Sentinel rule engine 중복 구현
```

## 입력

```text
.ai-runs/{job_id}/tool-output/star-sentinel/task.json
.ai-runs/{job_id}/tool-output/star-sentinel/changed_lines.json
builtin-tools/star-sentinel/policies/p0-rule-registry.json
```

## 출력

```text
star-control sentinel selfcheck --json
star-control sentinel check --project <path> --job <job-id> --json
star-control sentinel gate --project <path> --job <job-id> --json
star-control sentinel review-pack --project <path> --job <job-id> --json
schema-valid CLI output envelope
actions_enabled = false
```

## 핵심 TASK

```text
CLI sentinel selfcheck/check/gate/review-pack subcommand 추가
Star Sentinel task/changed_lines schema validation 연결
diagnostics, approval, review-pack artifact writer 연결
missing input artifact error path 고정
reserved/mutating/provider/release options reject
schema-valid CLI envelope regression test 추가
```

## 완료 기준

- `sentinel selfcheck --json`은 Star Sentinel selfcheck 결과를 schema-valid CLI output envelope으로 반환해야 한다.
- `sentinel check --project <path> --job <job-id> --json`은 existing `task.json`과 `changed_lines.json`을 읽고 diagnostics artifact를 써야 한다.
- `sentinel gate`는 같은 평가 결과로 diagnostics와 approval artifact를 써야 한다.
- `sentinel review-pack`은 같은 평가 결과로 tool output review pack과 canonical `review-packs/review_pack.md`를 써야 한다.
- missing `task.json` 또는 `changed_lines.json`은 schema-valid CLI error envelope과 project-relative artifact path로 반환해야 한다.
- `sentinel` command는 provider execution, provider live call, release/deploy/publish, destructive recovery action, schema field, workflow를 변경하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-cli --locked sentinel -- --nocapture
cargo run --quiet -p star-control-cli -- sentinel selfcheck --json
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9u는 explicit approval을 받은 뒤 stacked PR ready/merge coordination으로 이어가거나, 별도 승인된 destructive recovery/release action surface를 작은 slice로 다룬다. Provider healthcheck, live call, release/deploy/publish, destructive recovery action, main 병합은 별도 승인 전까지 RESERVED다.
