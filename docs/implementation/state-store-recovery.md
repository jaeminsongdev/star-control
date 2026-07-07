# StateStore Recovery

## 목적

이 문서는 StateStore가 손상되었거나 불완전한 run artifact를 만났을 때의 판단 기준을 정의한다. 초기 구현은 자동 복구보다 명확한 오류 반환과 보존을 우선한다.

## 기본 원칙

- 자동 복구는 하지 않는다.
- 손상된 job을 목록에서 숨기지 않는다.
- 원본 artifact를 조용히 덮어쓰지 않는다.
- 복구가 필요한 경우 별도 recovery command를 통해 명시적으로 처리한다.
- 복구 command가 생기기 전까지는 오류를 반환하고 report에 위험을 남긴다.

## 손상 유형

| 유형 | 예시 | 기본 처리 |
|---|---|---|
| missing required file | `job.json` 없음 | `JobCorrupt` 표시 |
| invalid JSON | `run-state.json` parse 실패 | `InvalidJson` |
| schema mismatch | 필수 필드 누락 | `SchemaValidationFailed` |
| corrupt event log | `events.jsonl` 중간 줄 parse 실패 | `CorruptEventLog` |
| partial tmp file | `tmp/*.tmp-*` 남음 | 정상 artifact로 보지 않음 |
| path violation | job directory 밖 path | `PathTraversalBlocked` |

## 읽기 API 기준

### `load_job`

- `job.json`이 없으면 `JobNotFound` 또는 `JobCorrupt`를 반환한다.
- JSON parse 실패는 `InvalidJson`으로 반환한다.
- schema validation 실패는 `SchemaValidationFailed`로 반환한다.

### `load_state`

- `run-state.json`이 없으면 `ArtifactNotFound`로 반환한다.
- JSON parse 실패는 `InvalidJson`으로 반환한다.
- schema validation 실패는 `SchemaValidationFailed`로 반환한다.

### `read_events`

- 모든 line이 parse 가능하면 정상 event list를 반환한다.
- 마지막 line만 비어 있으면 무시할 수 있다.
- 마지막 line이 partial JSON이면 `CorruptEventLog`를 반환한다.
- 중간 line이 깨져 있으면 `CorruptEventLog`를 반환한다.

### `list_jobs`

`list_jobs`는 손상된 job directory를 숨기지 않는다.

반환 후보:

```text
job_id
state
current_stage
created_at
updated_at
summary
corrupt
corrupt_reason
```

손상된 job은 가능한 경우 directory name에서 `job_id`를 추정한다.

## tmp file 처리

초기 구현:

- tmp file은 정상 artifact로 보지 않는다.
- tmp file은 자동 승격하지 않는다.
- tmp file은 자동 제거하지 않는다.
- terminal state report에 남은 tmp file이 있음을 기록할 수 있다.

M9e 구현:

- `StateStore::inspect_recovery(job_id)`는 남은 `tmp/**` file을 `partial_tmp_file` warning issue로 보고한다.
- inspect 중 tmp file을 삭제하거나 정상 artifact로 승격하지 않는다.
- report는 `destructive_actions_performed=false`를 유지한다.

장기 recovery command 후보:

```text
star-control recover --project <path> --job <job-id> --list
star-control recover --project <path> --job <job-id> --discard-tmp
```

M9n 구현:

- `star-control recover --project <path> --job <job-id> --list --json`은 `StateStore::inspect_recovery(job_id)` 결과를 CLI envelope으로 표시한다.
- output은 `mode=inspect_only`, `recovery_actions_enabled=false`, `destructive_actions_performed=false`를 포함한다.
- tmp file은 warning issue로만 보고하고 삭제하지 않는다.
- `--discard-tmp` 같은 destructive recovery action은 별도 승인 전까지 구현하지 않는다.

E53 구현:

- `StateStore::plan_recovery_action(job_id, action, mode)`은 inspect result를 기반으로 recovery action 계획을 만든다.
- `star-control recover --project <path> --job <job-id> --action <name> --dry-run --json`은 action plan을 CLI envelope으로 표시한다.
- 지원 action name은 `tmp-cleanup`, `recovered-copy`, `event-log-trim`, `artifact-replace`, `retention-cleanup`이다.
- dry-run 없는 action은 실제 mutation을 수행하지 않고 `status=blocked`, `mode=approval_required`, `approval_gate.approval_token`을 반환한다.
- destructive executor는 action-specific 후속 slice로 남기며, `destructive_actions_performed=false`를 유지한다.

E57 구현:

- `StateStore::execute_recovery_action(job_id, action, approval_token)`은 action plan의 approval token을 검증한 뒤 executor를 수행한다.
- `tmp-cleanup`과 `retention-cleanup`은 승인 token이 일치할 때만 `tmp/**` file을 삭제한다.
- `recovered-copy`는 원본 artifact를 덮어쓰지 않는 비파괴 action이므로 approval token 없이 `recovery/*.recovered-copy`를 생성할 수 있다.
- `event-log-trim`은 승인 token이 일치할 때 `recovery/events.trimmed.jsonl`을 쓰고 원본 `events.jsonl`을 parse 가능한 줄만 남긴 copy로 교체한다.
- executor result는 `recovery/{action}-result.json`에 기록한다.

E59 구현:

- `RecoverySourceSelection { artifact_path, source_path }`은 `artifact-replace`의 명시 target/source 선택을 표현한다.
- `StateStore::plan_recovery_action_with_source(job_id, action, mode, selection)`은 current inspection issue와 일치하는 `artifact-replace` planned change에 approved source path를 주입한다.
- `StateStore::execute_recovery_action_with_source(job_id, action, approval_token, selection)`은 approval token이 일치하고 target/source가 모두 job directory 내부 relative path일 때만 target artifact를 source bytes로 atomic replace한다.
- CLI `--recovery-artifact <path> --recovery-source <path>`는 `artifact-replace`에서만 허용되며, current inspection issue와 일치하지 않는 target은 invalid input으로 거부한다.
- source path를 자동 추론하거나 job directory 밖 file을 읽지 않는다.

## event log recovery 후보

초기 구현에서는 자동 repair를 하지 않는다.

M9e 구현은 corrupt `events.jsonl`을 `corrupt_event_log` issue로만 보고한다. event log를 trim하거나 recovered copy를 만들거나 원본을 교체하지 않는다.

장기적으로 recovery command가 생기면 다음 mode를 둘 수 있다.

```text
inspect-only
trim-last-partial-line
copy-to-recovered-file
```

권장 방식:

1. 원본 `events.jsonl`을 그대로 보존한다.
2. 복구본을 새 파일로 쓴다.
3. 복구 report를 생성한다.
4. 사용자가 확인한 뒤 교체한다.

## JSON artifact recovery 후보

`job.json`, `run-state.json`, `route.json`, `workspecs/*.json`, `reports/*.json`은 자동 추론 복구하지 않는다.

허용 가능한 장기 recovery:

- backup artifact가 명시적으로 있을 때 복원
- 사용자가 직접 선택한 파일로 교체
- schema validation 오류를 report로만 출력

금지:

- 깨진 JSON을 임의로 추론해서 저장
- 필수 필드를 가짜 값으로 채워 정상 처리
- source file이나 provider output을 기반으로 job state를 조용히 재구성

## audit / report 기준

recovery 관련 작업은 event와 report에 남긴다.

후보 event type:

```text
ERROR_RECORDED
ARTIFACT_WRITTEN
```

후보 report 항목:

```text
corrupt_artifacts
recovery_action
manual_followup_required
```

## 테스트 기준

1. `job.json` 없음 감지
2. `run-state.json` invalid JSON 감지
3. `events.jsonl` 마지막 partial line 감지
4. `events.jsonl` 중간 corrupt line 감지
5. tmp file을 정상 artifact로 보지 않음
6. corrupt job이 `list_jobs`에서 숨겨지지 않음
7. path traversal recovery 입력 차단
8. inspect-only report가 tmp file을 삭제하지 않음
9. 정상 job은 recovery issue 없이 `ok`로 보고됨
