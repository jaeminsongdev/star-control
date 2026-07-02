# StateStore 구현 계약

## 목적

StateStore는 Star-Control의 file-based 실행 상태 저장 계층이다. 모든 job, state, event, route, workspec, report, provider output, tool output은 대상 프로젝트의 `.ai-runs/` 아래에 저장된다. Star-Control repository 내부에는 실행 산출물을 저장하지 않는다.

이 문서는 StateStore의 책임, 경로 계산, atomic write, event append, recovery, error model을 구현자가 그대로 옮길 수 있도록 고정한다.

## 함께 읽을 문서

```text
artifact-layout.md
artifact-naming.md
state-store-recovery.md
data-contracts.md
run-lifecycle.md
schema-validator.md
```

## 책임

StateStore가 담당하는 것:

- job directory 생성
- `job.json` 작성/읽기
- `run-state.json` 작성/읽기
- `events.jsonl` append/read
- route/workspec/report artifact 저장과 읽기
- provider/tool output 경로 계산
- approvals/review-packs/tmp 경로 계산
- job 목록 조회
- atomic write 보장
- path traversal 차단
- 손상된 파일에 대한 명확한 오류 반환

StateStore가 담당하지 않는 것:

- provider 실행
- route 판단
- validation rule 실행
- approval 정책 판단
- 사용자 UI 표시
- credential 저장
- source file 자동 수정

## 경로 규칙

대상 프로젝트 기준 구조는 다음을 따른다.

```text
{project_root}/.ai-runs/
  J-0001/
    job.json
    run-state.json
    events.jsonl
    route.json
    workspecs/
    reports/
    provider-output/
    tool-output/
    approvals/
    review-packs/
    tmp/
```

`project_root`는 사용자가 지정한 대상 프로젝트 root이다. Star-Control repository root가 아니다.

## project root 검증

StateStore는 `project_root` 입력을 받을 때 다음을 확인한다.

1. path가 비어 있지 않다.
2. path가 존재한다.
3. path가 directory다.
4. `.ai-runs/`를 만들 수 있거나 이미 존재한다.
5. Star-Control repository root와 같은 경로로 오인하지 않는다.

초기 구현은 Star-Control repository root 판별이 불가능하면 경고 없이 진행해도 되지만, Star-Control repo 내부 `.ai-runs/` 생성은 repository policy와 사용자 지침에서 금지한다.

## job id 규칙

초기 구현은 다음 형식을 사용한다.

```text
J-0001
J-0002
J-0003
```

규칙:

- `J-` prefix를 사용한다.
- 숫자는 4자리 이상 zero padding을 사용한다.
- 이미 존재하는 job id를 재사용하지 않는다.
- job id 생성은 `.ai-runs/` 아래 기존 job directory를 스캔해서 다음 번호를 선택한다.
- 동시 실행이 도입되기 전에는 단일 process 기준으로 충분하다.

장기적으로 daemon 동시 실행이 생기면 lock 또는 monotonic allocator를 추가한다.

## 주요 파일

### `job.json`

JobSpec 저장 파일이다.

필수 내용:

```text
schema_version
job_id
project_root
request_text
created_at
entrypoint
state
```

작성 시점:

- job 생성 직후 한 번 작성한다.
- 사용자의 원 요청은 보존한다.

### `run-state.json`

RunState 저장 파일이다.

필수 내용:

```text
schema_version
job_id
state
current_stage
workers
artifacts
next_action
```

작성 시점:

- 상태 전이 발생 시
- stage 진입/완료 시
- provider/tool output이 artifact registry에 추가될 때
- approval 대기/완료 시
- terminal state 진입 시

### `events.jsonl`

append-only event ledger이다.

규칙:

- 한 줄에 하나의 CoreEvent JSON object를 기록한다.
- 각 event는 `schema_version`을 포함한다.
- append 실패 시 작업은 실패로 보고한다.
- 기존 event line을 수정하지 않는다.
- 사람이 읽기 쉬운 `message`를 포함한다.

## 권장 API

언어와 package 구조는 후속 구현에서 정하되, 기능 단위는 아래 API를 따른다.

```text
create_job(project_root, request_text, entrypoint, user_constraints) -> JobSpec
allocate_job_id(project_root) -> job_id
load_job(project_root, job_id) -> JobSpec
load_state(project_root, job_id) -> RunState
save_state(project_root, job_id, state) -> None
append_event(project_root, job_id, event) -> None
read_events(project_root, job_id) -> list[CoreEvent]
save_route(project_root, job_id, route) -> None
load_route(project_root, job_id) -> RouteSpec
save_workspec(project_root, job_id, stage, workspec) -> None
load_workspec(project_root, job_id, stage) -> WorkSpec
save_report(project_root, job_id, name, report) -> None
load_report(project_root, job_id, name) -> ReportSpec
list_jobs(project_root) -> list[JobSummary]
resolve_job_path(project_root, job_id, relative_path) -> Path
resolve_provider_output_dir(project_root, job_id, provider_instance_id) -> Path
resolve_tool_output_dir(project_root, job_id, tool_id) -> Path
```

## path normalization

StateStore는 job directory 내부 경로를 받을 때 다음 순서로 처리한다.

```text
1. 입력이 string인지 확인한다.
2. 빈 문자열을 거부한다.
3. NUL byte를 거부한다.
4. absolute path를 거부한다.
5. Windows drive prefix를 거부한다.
6. path segment를 normalize한다.
7. `..` segment를 거부한다.
8. `.git` 내부 직접 쓰기를 거부한다.
9. 최종 resolved path가 job directory 내부인지 확인한다.
```

경로 검증은 문자열 검사와 resolved path containment 검사를 모두 수행한다.

## atomic write

JSON 파일 저장은 atomic write를 사용한다.

권장 절차:

```text
1. tmp file 작성: tmp/{target-name}.tmp-{pid}-{nonce}
2. UTF-8 JSON serialize
3. flush
4. 가능한 경우 fsync
5. target과 같은 filesystem 안에서 atomic rename
6. parent directory fsync가 가능한 환경이면 시도
7. ARTIFACT_WRITTEN event append
```

금지:

- 기존 JSON 파일을 직접 열어서 덮어쓰기
- 실패한 tmp file을 정상 artifact처럼 취급
- partial JSON을 정상 상태로 보고
- atomic rename 전 target을 먼저 제거

## JSON formatting

- UTF-8 사용
- 사람이 읽을 수 있도록 pretty print 권장
- key ordering은 필수는 아니지만 deterministic output을 권장
- trailing comma 금지
- 저장 전 schema validation을 수행한다.

## events append

`events.jsonl`은 append-only이다.

권장 절차:

```text
1. event object schema validation
2. JSON compact serialize
3. newline append
4. flush
5. 가능한 경우 fsync
```

마지막 줄이 손상된 경우:

- 기본 읽기 API는 `CorruptEventLog`를 반환한다.
- `list_jobs`는 해당 job을 숨기지 않고 `corrupt` 표시를 포함한다.
- 자동 수정은 하지 않는다.
- 사용자가 명시적으로 recovery command를 실행할 때만 복구한다.

상세 정책은 `state-store-recovery.md`를 따른다.

## lock 정책

초기 구현:

- 단일 process 실행을 전제로 한다.
- 별도 lock file은 필수 아님.
- 같은 job/stage 재실행은 명시적으로 막아도 된다.

장기 구현:

```text
.ai-runs/J-0001/run.lock
```

lock file에는 process id, created_at, command, hostname 후보를 기록한다.

## error model

StateStore 오류는 최소한 아래 유형을 구분해야 한다.

```text
ProjectRootNotFound
ProjectRootNotDirectory
AiRunsNotWritable
JobNotFound
JobAlreadyExists
ArtifactNotFound
InvalidJson
SchemaValidationFailed
CorruptEventLog
AtomicWriteFailed
PathTraversalBlocked
PathOutsideJobDirectory
TerminalStateBlocked
```

오류는 `specs/schemas/error.schema.json`의 ErrorEnvelope로 표현할 수 있어야 한다.

오류 메시지는 사용자에게 보여줄 수 있어야 한다. secret 또는 sensitive absolute path를 그대로 노출하지 않는다.

## schema validation

StateStore는 저장 전 핵심 artifact를 schema로 검증해야 한다.

최소 검증 대상:

```text
job.json
run-state.json
route.json
workspecs/*.json
reports/*.json
events.jsonl line item
```

Star Sentinel 전용 artifact 검증은 ValidationEngine 또는 Star Sentinel 쪽에서 수행할 수 있다.

## list_jobs

`list_jobs(project_root)`는 `.ai-runs/` 아래 `J-*` directory를 스캔한다.

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

파일이 손상된 job은 목록에서 숨기지 말고 `corrupt` 표시를 포함한다.

## terminal state 처리

terminal state:

```text
DONE
FAILED
BLOCKED
CANCELLED
```

terminal state가 된 job에 대해 새 provider execution을 자동 시작하지 않는다. resume 정책은 별도 문서와 명령에서 처리한다.

## recovery 처리

초기 구현은 자동 복구를 하지 않는다. 다음 상황에서는 명확한 오류를 반환한다.

- `job.json` parse 실패
- `run-state.json` parse 실패
- `route.json` parse 실패
- `events.jsonl` 중간 줄 parse 실패
- schema validation 실패
- job directory 내부가 아닌 path 요청

명시적 recovery command가 생기면 `state-store-recovery.md`의 정책을 따른다.

M9e 기준 StateStore는 `inspect_recovery(job_id)` inspect-only API를 제공한다. 이 API는 `job.json`, `run-state.json`, `events.jsonl`, `tmp/**`를 검사해 `RecoveryInspection`과 `RecoveryIssue` 목록을 반환하지만, 파일을 삭제하거나 수정하거나 승격하지 않는다.

## 테스트 기준

최소 테스트:

1. 새 project root에 `.ai-runs/J-0001` 생성
2. 기존 job이 있을 때 `J-0002` 생성
3. `job.json` roundtrip
4. `run-state.json` roundtrip
5. `events.jsonl` append 순서 보존
6. route/workspec/report 저장과 읽기
7. 잘못된 job id 조회 시 `JobNotFound`
8. invalid JSON 파일 읽기 시 `InvalidJson`
9. path traversal 입력 차단
10. absolute path 입력 차단
11. `.git` 내부 쓰기 차단
12. terminal state job에 대해 상태 보존
13. corrupted job이 `list_jobs`에서 숨겨지지 않음
14. atomic write 실패 시 partial artifact를 정상으로 보지 않음

## Codex 구현 지시

Codex가 StateStore를 구현할 때는 이 문서와 다음 문서를 함께 읽어야 한다.

- `artifact-layout.md`
- `artifact-naming.md`
- `state-store-recovery.md`
- `data-contracts.md`
- `run-lifecycle.md`
- `schema-validator.md`

구현 PR은 StateStore 파일만 수정해야 한다. Router, provider, validation 구현을 같은 PR에 섞지 않는다.
