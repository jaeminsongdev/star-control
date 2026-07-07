# E48 Provider Offline Readiness Healthcheck

## 목표

Productization readiness slice는 `star-control providers healthcheck`를 실제 provider live call 없이 실행 가능한 offline readiness surface로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 의도적으로 disabled/reserved 상태로 남긴다.

## 선행 문서

```text
provider-system.md
cloud-provider-policy.md
local-process-provider-policy.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-cli/**
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
provider execution
provider live call
network/process probe
credential raw value 접근/출력
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
star-control providers healthcheck --json
star-control providers healthcheck <provider-id> --json
schema-valid CLI output envelope
healthcheck_mode = offline_readiness
live_calls_performed = false
```

## 핵심 TASK

```text
providers healthcheck subcommand 추가
provider kind별 connector scope 분류
manifest/capability profile presence check
Local/Cloud AI connector disabled 상태 명시
live call/probe 금지 regression test 추가
```

## 완료 기준

- `providers healthcheck --json`은 builtin provider registry를 읽고 provider별 offline readiness를 반환해야 한다.
- `providers healthcheck <provider-id> --json`과 `providers healthcheck --provider <provider-id> --json`은 단일 provider readiness를 반환해야 한다.
- fake provider는 `ready`, human handoff는 `manual`, Local AI/Cloud AI connector 계열은 `disabled`/`reserved`로 표시해야 한다.
- output은 `live_calls_performed=false`, `actions_enabled=false`, `healthcheck_mode=offline_readiness`를 포함해야 한다.
- credential raw value, network/process probe, provider live call은 수행하지 않는다.
- `.ai-runs/` artifact, provider output, daemon state, release artifact를 생성하거나 수정하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-cli --locked providers -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 daemon process, HTTP API server, browser UI app, observability/security 자동 통합, recovery/retention action, release automation surface 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
