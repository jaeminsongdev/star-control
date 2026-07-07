# E55 Daemon Queue Scheduler Tick

## 목적

`apps/star-daemon serve --max-ticks`를 단순 process smoke에서 실제 queue scheduler tick으로 확장한다. 이 slice는 queued `fake-default` job만 실행하고, provider-specific scheduler executor와 Local/Cloud AI live connector execution은 disabled 상태로 유지한다.

## 구현 범위

- `serve --max-ticks`에서 daemon queue state를 읽고 queued entry를 tick 단위로 처리한다.
- queued entry는 실행 전 `RUNNING`과 `active_jobs`에 반영한다.
- `fake-default` workspec은 `ExecutionEngine`으로 실행하고 성공 시 queue에서 제거한다.
- non-fake provider workspec은 provider output을 만들지 않고 `DISABLED` scheduler result로 남긴다.
- 모든 scheduler output은 `live_calls_performed=false`, `local_ai_live_connector=disabled`, `cloud_ai_live_connector=disabled`를 명시한다.

## 제외 범위

- Local AI live connector execution
- Cloud AI live connector execution
- provider-specific scheduler executor
- long-running background daemon worker
- socket/remote exposure/auth/session
- release/deploy/publish 실행
- destructive recovery mutation
- credential raw value 접근/출력

## 검증

```text
cargo test -p star-daemon --all-targets --locked
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

- idle queue에서 `serve --max-ticks 1`이 scheduler tick result를 반환한다.
- queued `fake-default` job이 target project `.ai-runs/` 아래 provider output artifact를 생성하고 run state를 `IMPLEMENTED`로 갱신한다.
- executed queue entry는 daemon state queue에서 제거된다.
- non-fake provider job은 `DISABLED`로 남고 provider-specific output artifact를 생성하지 않는다.
- Local/Cloud AI live connector는 disabled 상태로만 보고된다.
