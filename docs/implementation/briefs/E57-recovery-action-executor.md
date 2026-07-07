# E57 Recovery Action Executor

## 목적

E53에서 계획과 approval gate까지만 제공하던 recovery action을 실제 executor로 확장한다. destructive action은 approval token이 일치할 때만 실행하고, recovered-copy 같은 비파괴 action은 원본을 덮어쓰지 않는 별도 artifact로 생성한다.

## 구현 범위

- `StateStore::execute_recovery_action(job_id, action, approval_token)`을 제공한다.
- `tmp-cleanup`은 approved token이 있을 때 `tmp/**` artifact를 삭제한다.
- `retention-cleanup`은 현재 tmp artifact cleanup 범위에서 approved token이 있을 때 삭제한다.
- `recovered-copy`는 approval 없이 `recovery/*.recovered-copy`를 생성한다.
- `event-log-trim`은 approved token이 있을 때 `recovery/events.trimmed.jsonl`을 생성하고 원본 `events.jsonl`을 parse 가능한 줄만 남긴 copy로 교체한다.
- executor 결과는 `recovery/{action}-result.json`에 기록한다.
- CLI `recover --action <name> --approve-recovery-action <token> --json`은 executor 결과를 envelope으로 반환한다.

## 제외 범위

- source path 선택이 필요한 `artifact-replace` 실제 교체
- 승인 없는 destructive mutation
- provider/tool output 임의 추론 복구
- release/deploy/publish 실행
- remote API exposure
- credential raw value 접근/출력

## 검증

```text
cargo test -p star-control-cli recover --locked
cargo test -p star-control-state recovery --locked
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 완료 기준

- destructive action은 approval token이 없거나 틀리면 mutation 없이 blocked 상태를 반환한다.
- approved `tmp-cleanup`은 temp artifact를 삭제하고 result artifact를 쓴다.
- `recovered-copy`는 원본을 덮어쓰지 않고 copy artifact를 생성한다.
- approved `event-log-trim`은 corrupt event line을 제거한 log로 교체하고 preview/result artifact를 남긴다.
