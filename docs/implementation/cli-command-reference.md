# CLI Command Reference

## 목적

이 문서는 Star-Control CLI의 초기 command, exit code, `--json` 출력 envelope, error envelope를 고정한다. Daemon/API/UI는 별도 문서의 RESERVED 영역이며, 초기 구현은 CLI file-based flow를 우선한다.

## machine-readable contracts

```text
specs/schemas/cli-output.schema.json
specs/schemas/cli-error.schema.json
examples/cli-contracts/run-output.example.json
examples/cli-contracts/status-output.example.json
examples/cli-contracts/approve-output.example.json
examples/cli-contracts/error-output.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 공통 stdout/stderr 규칙

- 기본 stdout은 사람이 읽는 요약이다.
- `--json`을 사용하면 stdout은 `cli-output.schema.json` 또는 `cli-error.schema.json`을 따른다.
- stderr는 warning/error 요약만 출력한다.
- secret raw value는 stdout/stderr/artifact에 출력하지 않는다.
- artifact path는 가능하면 project-relative path로 출력한다.

## exit code

```text
0: success
1: validation failed or policy blocked
2: invalid user input
3: missing artifact or state error
4: provider execution error
5: internal error
```

## JSON success envelope

필수 필드:

```text
schema_version
command
status
exit_code
data
```

선택 필드:

```text
warnings
artifacts
```

status 후보:

```text
success
failed
blocked
waiting_approval
```

## JSON error envelope

필수 필드:

```text
schema_version
command
status
exit_code
error
```

error 필드:

```text
code
message
recoverable
category
artifact_paths
```

CLI error envelope는 core `ErrorEnvelope`와 호환되는 정보를 담되, command와 exit_code를 추가한다.

## star-control run

목적:

- JobSpec 생성
- RouteSpec 생성
- WorkSpec 생성
- provider 실행 또는 dry-run
- validation/report 후보 생성

필수 옵션 초기 후보:

```text
--project <path>
--request <text>
```

선택 옵션:

```text
--entrypoint <path-or-command>
--provider <instance-id>
--provider-instance <path>
--profile <profile>
--dry-run
--json
```

`--provider` 기본값은 `fake-default`다. `fake-default`가 아닌 provider instance를 실행하려면 해당 instance contract 파일을 `--provider-instance <path>`로 명시해야 한다. 초기 M5 local process flow는 shell을 호출하지 않는 `local_process_model` + `process` instance만 대상으로 한다.

`--json` example:

```text
examples/cli-contracts/run-output.example.json
```

## star-control status

목적:

- RunState를 읽어 현재 job 상태를 표시한다.

옵션:

```text
--project <path>
--job <job-id>
--json
```

`--json` example:

```text
examples/cli-contracts/status-output.example.json
```

## star-control report

목적:

- stage 또는 final report를 표시한다.

옵션:

```text
--project <path>
--job <job-id>
--stage <stage>
--release-readiness
--json
--markdown
```

초기 구현은 existing report artifact를 읽는 read-only command로 시작한다.

`--release-readiness`를 사용하면 stage report 대신 `.ai-runs/{job_id}/release/release-readiness.json`을 read-only로 읽는다. 이 옵션은 `--stage`와 함께 사용할 수 없다. CLI는 readiness artifact를 표시할 뿐이며 signing, publish, deploy, release action을 실행하지 않는다.

## star-control approve

목적:

- `WAITING_APPROVAL` job에 대해 approval response를 작성한다.

옵션:

```text
--project <path>
--job <job-id>
--response approved|rejected|needs_changes|cancelled
--reason <text>
--constraint <text>
--json
```

규칙:

- `approvals/approval-request.json`이 없으면 실패한다.
- response는 `approvals/approval-response.json`에 저장한다.
- 승인 이후 즉시 실행하지 않고 `resume`에서 이어가는 것을 기본으로 한다.
- M7a 기준 구현은 `approved` response를 쓰면 RunState는 `WAITING_APPROVAL`을 유지하고 `next_action=resume`을 기록한다.
- `rejected` 또는 `needs_changes`는 `BLOCKED`, `cancelled`는 `CANCELLED`로 기록한다.

`--json` example:

```text
examples/cli-contracts/approve-output.example.json
```

## star-control cancel

목적:

- running, waiting, approval 대기 job을 취소한다.

규칙:

- terminal state job은 cancel하지 않는다.
- provider cancel 지원 여부를 확인한다.
- 취소 event를 events.jsonl에 기록한다.
- M7a 기준 구현은 non-terminal RunState를 `CANCELLED`로 전이하고 `next_action=stop`을 기록한다.

## star-control resume

목적:

- 중단되거나 approval 이후 대기 중인 job을 이어서 진행한다.

진입 조건:

- RunState가 terminal state가 아니다.
- 필요한 artifact가 존재한다.
- approval required 상태라면 approval-response.json이 존재한다.
- M7a 기준 구현은 `WAITING_APPROVAL` job의 approval response가 request와 일치하고 `approved`일 때 `VALIDATED`, `next_action=report`로 전이한다. daemon/API orchestration은 다음 M7 slice에서 다룬다.

## star-control recover

목적:

- 손상되었거나 불완전할 수 있는 job artifact를 inspect-only로 점검한다.

옵션:

```text
--project <path>
--job <job-id>
--list
--json
```

M9n 기준 구현은 `--list`만 지원한다. CLI는 `StateStore::inspect_recovery(job_id)` 결과를 `cli-output.schema.json` envelope으로 표시하고, `mode=inspect_only`, `recovery_actions_enabled=false`를 반환한다.

금지:

- tmp file 삭제
- event log trim
- recovered copy 생성
- artifact 교체
- retention cleanup
- provider/tool output 수정

`--list` 없는 `recover`와 non-recovery option 조합은 invalid input으로 거부한다.

## star-control providers

초기 하위 명령:

```text
list
show
healthcheck
```

초기 구현은 `list`와 `show`를 read-only로 시작한다. `healthcheck`는 provider adapter smoke가 준비된 뒤 구현한다.

M9s 기준 구현:

```text
star-control providers list --json
star-control providers show <provider-id> --json
star-control providers show --provider <provider-id> --json
```

`list`와 `show`는 `configs/registries/builtin-provider-registry.yaml`과 builtin provider manifest/capability profile만 읽는다. output은 schema-valid CLI envelope이며 `healthcheck_enabled=false`, `actions_enabled=false`를 포함한다. 이 command group은 `.ai-runs/`, provider output, daemon state, release artifact를 생성하거나 수정하지 않는다.

`providers healthcheck`는 provider smoke가 준비되기 전까지 reserved invalid input으로 남긴다.

## star-control sentinel

Star Sentinel builtin tool을 CLI에서 직접 실행하는 command group이다.

초기 하위 명령:

```text
check
gate
review-pack
selfcheck
```

CLI는 Star Sentinel rule을 직접 구현하지 않고 tool command를 호출한다.

M9t 기준 구현:

```text
star-control sentinel selfcheck --json
star-control sentinel check --project <path> --job <job-id> --json
star-control sentinel gate --project <path> --job <job-id> --json
star-control sentinel review-pack --project <path> --job <job-id> --json
```

`check`, `gate`, `review-pack`은 `.ai-runs/{job_id}/tool-output/star-sentinel/task.json`과 `changed_lines.json`을 existing input으로 읽는다. `check`는 diagnostics artifact를 쓰고, `gate`는 diagnostics와 approval artifact를 쓰며, `review-pack`은 tool output과 canonical `review-packs/review_pack.md`를 쓴다. output은 schema-valid CLI envelope이며 `actions_enabled=false`를 포함한다.

Star Sentinel의 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`은 command execution error가 아니라 policy decision data다. Artifact 생성이 성공하면 CLI는 success envelope을 반환하고, policy decision은 `status`와 `data.decision`으로 표시한다. missing input, invalid option, schema read failure 같은 command execution failure만 error envelope을 반환한다.

`sentinel` command group은 provider execution, provider live call, release/deploy/publish, destructive recovery action을 실행하지 않는다. provider, stage, release, recovery 같은 unrelated options는 invalid input으로 거부한다.

## 오류 처리 기준

대표 error example:

```text
examples/cli-contracts/error-output.example.json
```

규칙:

- invalid input은 exit code 2
- missing state/artifact는 exit code 3
- provider execution error는 exit code 4
- internal invariant break는 exit code 5
- policy blocked 또는 validation failed는 exit code 1

## 테스트 기준

1. `run --json` output이 schema를 만족
2. `status --json` output이 schema를 만족
3. `approve --json` output이 schema를 만족
4. error output이 schema를 만족
5. stderr에 secret raw value가 나오지 않음
6. missing job은 non-zero exit
7. approval request 없이는 approve 실패
8. terminal job은 cancel 실패
9. `providers list/show --json` output이 schema를 만족하고 provider healthcheck/action을 실행하지 않음
10. `sentinel selfcheck/check/gate/review-pack --json` output이 schema를 만족하고 Star Sentinel input/output artifact boundary를 지킴

## Codex 구현 지시

CLI 구현 PR은 다음 순서로 쪼갠다.

1. status read-only
2. report read-only
3. run dry-run
4. run with FakeProvider
5. approve/cancel/resume
6. providers list/show
7. sentinel command group

Daemon, API, UI 구현을 CLI PR에 섞지 않는다.
