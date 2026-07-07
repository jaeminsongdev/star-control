# E50 Local HTTP API Server Surface

## 목표

Productization API slice는 기존 `ApiControlService` GET/POST surface를 `star-daemon api`의 loopback-only HTTP request/response로 연결한다. 이 slice는 remote exposure, provider scheduling, Local/Cloud AI live connector를 구현하지 않고 disabled 상태로 명시한다.

## 선행 문서

```text
api-contract.md
daemon-contract.md
cli-daemon-api-ui.md
complete-implementation-roadmap.md
```

## 허용 파일

```text
apps/star-daemon/**
packages/star-control-api/**
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
remote API exposure
provider execution
provider live call
credential raw value 접근/출력
release/deploy/publish automation
repository settings 변경
destructive recovery action
browser UI app 구현
```

## 입력

```text
packages/star-control-api/**
packages/star-control-daemon/**
packages/star-control-state/**
```

## 출력

```text
star-daemon api --bind 127.0.0.1:0 --max-requests 0 --json
loopback-only HTTP server bridge
GET /daemon/state
GET /projects
POST control mutation routing
```

## 핵심 TASK

```text
ApiControlService를 stdlib TCP HTTP request/response로 연결
loopback-only bind policy 추가
project registration option 추가
GET/POST routing smoke 추가
remote exposure/provider scheduling/live connector disabled 상태 명시
```

## 완료 기준

- `star-daemon api --bind 127.0.0.1:0 --max-requests 0 --json`은 local HTTP server plan을 반환해야 한다.
- server bridge는 `GET /daemon/state`, `GET /projects`, POST control path를 `ApiControlService`에 전달해야 한다.
- bind는 loopback-only여야 하며 remote exposure는 명시 승인 전까지 disabled여야 한다.
- provider execution, provider live call, credential raw value 접근, release/deploy/publish, destructive recovery action은 수행하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-daemon --locked -- --nocapture
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 browser UI app, daemon queue loop/provider scheduling integration, observability/security 자동 통합, recovery/retention action, release automation surface 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
