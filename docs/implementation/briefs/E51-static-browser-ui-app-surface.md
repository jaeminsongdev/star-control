# E51 Static Browser UI App Surface

## 목표

Productization UI slice는 `apps/star-control-ui`를 실제 정적 browser app으로 구현한다. 이 app은 `star-daemon api` loopback endpoint를 소비하되 provider process, Star Sentinel rule, StateStore file mutation, Local/Cloud AI live connector를 직접 실행하지 않는다.

## 선행 문서

```text
ui-shell-contract.md
api-contract.md
cli-daemon-api-ui.md
complete-implementation-roadmap.md
```

## 허용 파일

```text
apps/star-control-ui/**
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
Star Sentinel rule 직접 구현
StateStore file 직접 mutation
credential raw value 접근/출력
release/deploy/publish automation
repository settings 변경
destructive recovery action
remote API exposure
```

## 입력

```text
star-daemon api loopback endpoint
ApiControlService GET/POST response envelope
packages/star-control-ui library model contract
```

## 출력

```text
apps/star-control-ui/index.html
apps/star-control-ui/styles.css
apps/star-control-ui/app.js
node --test apps/star-control-ui/tests/app.test.mjs
```

## 핵심 TASK

```text
정적 browser app shell 구현
API base/project id connection form 구현
daemon state/job list/job detail/timeline/release readiness rendering
approve/cancel/resume action panel 구현
Local/Cloud AI connector disabled state 표시
package manager 없는 helper regression test 추가
```

## 완료 기준

- `apps/star-control-ui/index.html`은 browser에서 직접 열 수 있는 정적 app이어야 한다.
- app은 `star-daemon api`의 loopback endpoint를 소비해 daemon state, project jobs, job detail, event timeline, release readiness, approve/cancel/resume action result를 표시해야 한다.
- UI는 provider process, Star Sentinel rule, StateStore file mutation, Local/Cloud AI live connector를 직접 실행하지 않아야 한다.
- 새 package manager, external dependency, remote exposure, release/deploy/publish, destructive recovery action은 수행하지 않는다.

## 검증

```text
node --test apps/star-control-ui/tests/app.test.mjs
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

다음 productization slice는 observability/security 자동 통합, recovery/retention action, release automation surface, productization E2E smoke, final readiness 정리 중 하나를 작은 단위로 구현한다. Local AI connector live execution과 Cloud AI connector live execution은 최종 blocker로 남긴다.
