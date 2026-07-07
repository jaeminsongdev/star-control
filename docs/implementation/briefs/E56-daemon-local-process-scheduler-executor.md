# E56 Daemon Local Process Scheduler Executor

## 목적

`apps/star-daemon serve --max-ticks`가 `fake-default`만 실행하던 상태에서 allowlisted local-process provider까지 실행할 수 있게 확장한다. 이 slice는 provider instance file path를 queue entry에 보존하고, scheduler가 해당 path로 registry를 재구성해 local-process provider를 실행한다. Local/Cloud AI live connector execution은 계속 disabled로 유지한다.

## 구현 범위

- `DaemonQueue`가 scheduler용 `provider_instance_paths`를 queue entry에 보존할 수 있다.
- scheduler는 `provider_instance_paths`가 있는 queue entry에서 builtin provider registry와 provider instance file을 로드한다.
- scheduler는 `fake-default`와 `local_process_model` + `process` manifest만 실행한다.
- local-process execution은 기존 `ExecutionEngine`과 `LocalProcessProviderAdapter`를 사용해 request/stdout/stderr/response artifact를 대상 project `.ai-runs/` 아래에 쓴다.
- provider instance path가 없는 non-fake provider와 cloud/local-server live connector 계열은 `DISABLED` scheduler result로 남긴다.

## 제외 범위

- Local AI live connector execution
- Cloud AI live connector execution
- cloud CLI/API live scheduler execution
- local model server HTTP connector execution
- long-running background daemon worker
- socket/remote exposure/auth/session
- release/deploy/publish 실행
- destructive recovery mutation
- credential raw value 접근/출력

## 검증

```text
cargo test -p star-control-daemon -p star-daemon --all-targets --locked
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

- queue entry가 `provider_instance_paths`를 보존한다.
- queued local-process job이 `star-daemon serve --max-ticks 1`에서 실행된다.
- local-process provider output artifact가 `.ai-runs/{job_id}/provider-output/{provider_instance}/` 아래에 생성된다.
- 실행된 queue entry는 제거되고 run state가 `IMPLEMENTED`로 갱신된다.
- Local/Cloud AI live connector는 disabled 상태로만 보고된다.
