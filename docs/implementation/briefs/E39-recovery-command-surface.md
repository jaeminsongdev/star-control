# E39 Recovery Command Surface

## 목표

M9n slice는 `StateStore::inspect_recovery` 결과를 CLI에서 명시적으로 확인하는 inspect-only recovery command surface를 추가한다. 이 slice는 `star-control recover --project <path> --job <job-id> --list --json`만 다루며, tmp 삭제나 event log trim 같은 destructive recovery action을 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
state-store.md
state-store-recovery.md
cli-command-reference.md
testing-ci-release.md
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
tmp file 삭제
event log trim
recovered copy 생성
artifact 교체
retention cleanup
release/deploy/publish automation
external account/repository settings 변경
provider live call
HTTP server 구현
browser UI app 구현
package registry 설정
repository branch protection/settings 변경
```

## 입력

```text
star-control recover --project <path> --job <job-id> --list --json
```

## 출력

```text
schema-valid CLI output envelope
command = recover
mode = inspect_only
recovery_actions_enabled = false
recovery = StateStore::inspect_recovery(job_id)
```

## 핵심 TASK

```text
recover --list command 추가
StateStore::inspect_recovery 재사용
CLI output envelope validation
tmp file no-delete regression
run-state/events no-mutation regression
unsupported recovery mode rejection
non-recovery option 조합 거부
```

## 완료 기준

- `star-control recover --project <path> --job <job-id> --list --json`이 recovery inspection을 schema-valid CLI output envelope로 반환해야 한다.
- output은 `mode=inspect_only`, `recovery_actions_enabled=false`, `destructive_actions_performed=false`를 포함해야 한다.
- `tmp/**` file은 warning issue로 표시하되 삭제하지 않아야 한다.
- command는 `run-state.json`, `events.jsonl`, provider/tool output, release/deploy/publish state를 수정하지 않아야 한다.
- `--list` 없는 recover와 non-recovery option 조합은 invalid input으로 거부해야 한다.
- schema field, workflow, dependency, HTTP server, browser UI app, destructive recovery action은 변경하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-cli --locked -- --nocapture
cargo clippy -p star-control-cli --all-targets --locked -- -D warnings
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9o는 final M9 conformance/readiness audit 또는 승인된 recovery action surface로 이어간다. destructive recovery, signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
