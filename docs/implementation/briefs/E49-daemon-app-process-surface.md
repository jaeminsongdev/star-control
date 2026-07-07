# E49 Daemon App Process Surface

## 목표

Productization daemon slice는 `apps/star-daemon`을 실제 Cargo binary로 등록하고, queue state를 process surface에서 열 수 있게 한다. 이 slice는 HTTP server, provider scheduling, Local/Cloud AI live connector를 구현하지 않고 disabled 상태로 명시한다.

## 선행 문서

```text
daemon-contract.md
cli-daemon-api-ui.md
complete-implementation-roadmap.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
apps/star-daemon/**
packages/star-control-daemon/**
Cargo.toml
Cargo.lock
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
HTTP server 구현
provider execution
provider live call
network probe
credential raw value 접근/출력
release/deploy/publish automation
repository settings 변경
destructive recovery action
browser UI app 구현
```

## 입력

```text
specs/schemas/daemon-state.schema.json
packages/star-control-daemon/**
```

## 출력

```text
apps/star-daemon Cargo binary
star-daemon status --json
star-daemon serve --max-ticks 1 --json
daemon state opened under explicit config root
```

## 핵심 TASK

```text
apps/star-daemon workspace package 추가
explicit config-root/schema-root option 처리
status command로 daemon state를 JSON 출력
serve --max-ticks smoke 추가
HTTP server/provider scheduling/live connector disabled 상태 명시
```

## 완료 기준

- `star-daemon status --json`은 explicit config root 아래 daemon state를 열고 state path와 process capability를 반환해야 한다.
- `star-daemon serve --max-ticks 1 --json`은 테스트 가능한 process tick summary를 반환해야 한다.
- output은 HTTP server, provider scheduling, Local/Cloud AI live connector, live call을 disabled/false로 표시해야 한다.
- command는 Star-Control repo에 `.ai-runs/`를 만들지 않아야 한다.
- external dependency, provider live call, release/deploy/publish, destructive recovery action은 수행하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-daemon --offline -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --offline
cargo test --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 HTTP API server, browser UI app, daemon queue loop/provider scheduling integration, observability/security 자동 통합, recovery/retention action, release automation surface 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
