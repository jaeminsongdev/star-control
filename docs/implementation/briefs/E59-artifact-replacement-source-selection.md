# E59 Artifact Replacement Source Selection

## 목적

E57에서 source path 선택 부재로 skipped/reserved 상태였던 `artifact-replace` recovery action을 명시 source 기반 approval-gated executor로 확장한다. 이 slice는 자동 복구 추론을 하지 않고, 사용자가 지정한 target/source가 current recovery inspection issue와 일치할 때만 교체한다.

## 구현 범위

- `RecoverySourceSelection { artifact_path, source_path }`을 제공한다.
- `StateStore::plan_recovery_action_with_source`가 matching `artifact-replace` planned change에 source path를 주입한다.
- `StateStore::execute_recovery_action_with_source`가 approved token과 source selection을 함께 검증한 뒤 target artifact를 source bytes로 atomic replace한다.
- CLI `recover --action artifact-replace --recovery-artifact <target> --recovery-source <source> --approve-recovery-action <token> --json`을 제공한다.
- source selection option은 `artifact-replace`에서만 허용한다.

## 제외 범위

- source path 자동 추론
- job directory 밖 source/target 접근
- provider/tool output 임의 추론 복구
- approval 없는 destructive mutation
- release/deploy/publish 실행

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

- source 없는 approved `artifact-replace`는 invalid input으로 거부하고 target을 수정하지 않는다.
- current inspection issue와 일치하지 않는 target은 invalid input으로 거부한다.
- approved target/source selection은 target artifact를 source bytes로 교체하고 result artifact를 기록한다.
- path traversal과 absolute path는 StateStore path guard로 거부된다.
