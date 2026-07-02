# E44 CLI Providers Read-only Surface

## 목표

M9s slice는 public CLI surface에 남아 있던 `star-control providers` command group 중 read-only `list`와 `show`를 구현한다. 이 slice는 provider healthcheck, provider execution, live call, external account access를 수행하지 않는다.

## 선행 문서

```text
cli-command-reference.md
provider-system.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-cli/**
packages/star-control-provider/**
docs/implementation/**
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
```

## 입력

```text
configs/registries/builtin-provider-registry.yaml
builtin-providers/**/provider.yaml
builtin-providers/**/capabilities.yaml
```

## 출력

```text
star-control providers list --json
star-control providers show <provider-id> --json
schema-valid CLI output envelope
healthcheck_enabled = false
actions_enabled = false
```

## 핵심 TASK

```text
ProviderRegistry read-only provider listing accessor 추가
CLI providers list/show subcommand 추가
providers healthcheck reserved error 고정
mutating/run-specific options reject
schema-valid CLI envelope regression test 추가
```

## 완료 기준

- `providers list --json`은 builtin provider registry를 읽고 provider summary 목록을 반환해야 한다.
- `providers show <provider-id> --json`은 manifest와 capability profile을 schema-valid CLI output envelope으로 반환해야 한다.
- output은 repo-relative manifest/capability path를 사용하고 credential raw value를 출력하지 않아야 한다.
- `providers healthcheck`는 provider smoke가 준비되기 전까지 reserved invalid input으로 남아야 한다.
- `providers` command는 `.ai-runs/` artifact, provider output, daemon state, release artifact를 생성하거나 수정하지 않아야 한다.
- schema field, workflow, dependency, provider live call, release/deploy/publish, destructive recovery action은 변경하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-cli --locked providers -- --nocapture
cargo test -p star-control-provider --locked loads_builtin_yaml_registry_and_fake_provider_contracts -- --nocapture
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9t는 public CLI surface의 남은 `sentinel` command group을 별도 read-only/tool-wrapper slice로 구현하거나, explicit approval을 받은 뒤 stacked PR ready/merge coordination으로 이어간다. Provider healthcheck, live call, release/deploy/publish, destructive recovery action은 별도 승인 전까지 RESERVED다.
