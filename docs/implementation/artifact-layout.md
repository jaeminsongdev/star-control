# Artifact Layout

## 목적

Star-Control은 실행 산출물을 Star-Control repository 내부에 저장하지 않는다. 모든 run artifact는 대상 프로젝트의 `.ai-runs/` 아래에 저장한다. 이 문서는 file-based StateStore와 provider/tool output이 사용할 디렉터리 구조를 정의한다.

파일명, attempt, tmp naming의 세부 규칙은 `artifact-naming.md`를 함께 따른다. 손상된 artifact 처리와 복구 기준은 `state-store-recovery.md`를 따른다.

## 최상위 구조

```text
{target-project}/.ai-runs/
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

## job directory

각 job은 하나의 job directory를 가진다.

```text
.ai-runs/J-0001/
```

Job directory 이름은 JobSpec의 `job_id`와 일치해야 한다.

## required files

### `job.json`

JobSpec 저장 파일이다. 사용자 요청 원문, project root, created_at, entrypoint, initial state를 포함한다.

### `run-state.json`

RunState 저장 파일이다. 현재 state, current_stage, workers, artifacts, next_action을 포함한다.

### `events.jsonl`

append-only ledger다. 한 줄에 하나의 CoreEvent JSON object를 기록한다.

### `route.json`

RouterEngine이 생성한 RouteSpec이다.

## WorkSpec layout

```text
workspecs/
  route.json
  plan.json
  design.json
  implement.json
  validate.json
  review.json
  polish.json
  report.json
```

실제로 존재하는 파일은 RouteSpec의 `stages`에 따라 달라진다.

WorkSpec path는 RouteSpec의 `workspecs` object에서 stage별 상대 경로로 참조한다.

## reports layout

```text
reports/
  implement-report.json
  validate-report.json
  review-report.json
  polish-report.json
  final-report.json
```

ReportSpec은 stage별 report와 최종 report를 모두 지원한다.

## provider output layout

초기 기본 layout:

```text
provider-output/
  {provider-instance-id}/
    request.json
    response.json
    stdout.txt
    stderr.txt
    logs/
    artifacts/
```

장기 attempt layout 후보:

```text
provider-output/
  {provider-instance-id}/
    attempt-0001/
      request.json
      response.json
      stdout.txt
      stderr.txt
      logs/
      artifacts/
```

초기 구현에서는 attempt directory를 만들지 않아도 된다. 같은 stage/provider를 재실행해야 하는 경우 기존 output을 덮어쓰지 말고 명확한 오류를 반환하거나 attempt layout을 도입하는 별도 PR을 만든다.

Provider output은 provider adapter가 쓴다. Core는 provider-specific 내부 파일을 직접 해석하지 말고 `response.json`과 ReportSpec 계약을 우선한다.

## Star Sentinel tool output layout

```text
tool-output/
  star-sentinel/
    task.json
    repo_map.json
    changed_lines.json
    diagnostics.json
    validation_runs.json
    approval.json
    review_pack.json
    review_pack.md
    ledger.jsonl
```

Star Sentinel manifest의 outputs와 일치해야 한다.

`tool-output/star-sentinel/`은 Star Sentinel command의 원본 output 위치다.

## approvals layout

```text
approvals/
  approval-request.json
  approval-response.json
```

`WAITING_APPROVAL` 상태에서 사용한다. approval response 없이 자동으로 다음 stage로 이동하면 안 된다.

`approval-request.json`은 ValidationEngine 또는 control plane이 만든 human-facing request다. `tool-output/star-sentinel/approval.json`은 Star Sentinel gate decision이다. 두 파일은 같은 의미가 아니다.

## review packs layout

```text
review-packs/
  review_pack.json
  review_pack.md
```

사람이 확인하는 review 자료를 둔다. `tool-output/star-sentinel/review_pack.*`와 중복될 수 있다.

초기 canonical copy 기준:

1. Star Sentinel이 생성한 원본은 `tool-output/star-sentinel/review_pack.*`에 둔다.
2. 사용자가 보는 최종 copy는 `review-packs/review_pack.*`에 둘 수 있다.
3. 두 copy가 모두 존재하면 ReportSpec은 `review-packs/` 경로를 우선 참조한다.
4. 두 copy의 내용이 다르면 report에 차이를 숨기지 않는다.

## tmp layout

```text
tmp/
  {artifact-name}.tmp-{pid}-{nonce}
```

작업 중 임시 파일을 둘 수 있다.

규칙:

- tmp file은 job directory 내부 `tmp/` 아래에 둔다.
- tmp file은 정상 artifact로 registry에 등록하지 않는다.
- terminal state 진입 전 정리할 수 있다.
- 정리하지 못한 경우 report에 남긴다.
- recovery command가 생기기 전까지 tmp file을 자동으로 정상 artifact로 승격하지 않는다.

## atomic write 규칙

StateStore는 JSON 파일을 직접 덮어쓰지 않는다. 다음 순서를 권장한다.

```text
1. job directory 내부 tmp file 작성
2. UTF-8 JSON serialize
3. flush
4. 가능한 경우 fsync
5. target path로 atomic rename
6. 가능한 경우 parent directory fsync
7. ARTIFACT_WRITTEN event append
```

`events.jsonl`은 append-only로 쓴다. 중간에 줄이 깨진 경우 기본 읽기 API는 corruption error를 반환하고, 명시적 recovery command가 있을 때만 복구한다.

## path rule

- artifact 내부 path는 job directory 기준 상대 경로를 사용한다.
- absolute path는 artifact reference로 저장하지 않는 것을 기본으로 한다.
- `..`, drive prefix, NUL byte, `.git` 내부 쓰기는 차단한다.
- secret, token, credential raw value는 artifact에 저장하지 않는다.
- provider/tool output path는 반드시 해당 output directory 내부여야 한다.

## cleanup rule

- Star-Control core는 사용자 승인 없이 대상 프로젝트 source file을 제거하지 않는다.
- `.ai-runs/` 내부 cleanup도 명시 command 또는 retention policy가 생기기 전까지 자동 정리하지 않는다.
- 실패한 run artifact는 debugging을 위해 보존한다.
- tmp file cleanup은 정상 artifact cleanup과 분리한다.

## Star-Control repository 금지 경로

Star-Control repository 내부에는 실제 run artifact 경로를 만들지 않는다.

```text
.ai-runs/
provider-output/
tool-output/
approvals/
review-packs/
```

단, `examples/` 아래 sample artifact는 문서와 schema 검증용 예시로 허용된다.

## artifact registry 기준

RunState의 `artifacts` object와 ReportSpec의 `artifacts` array는 job directory 기준 상대 경로를 기록한다.

권장 kind:

```text
job
state
event_log
route
workspec
report
provider_output
tool_output
approval
review_pack
log
other
```

정교한 ArtifactRef 구조가 필요한 경우 `specs/schemas/artifact-ref.schema.json`을 사용한다.

## 예시

```text
my-project/
  src/
  tests/
  .ai-runs/
    J-0001/
      job.json
      run-state.json
      events.jsonl
      route.json
      workspecs/
        implement.json
        validate.json
      provider-output/
        my-fake-provider/
          request.json
          response.json
      tool-output/
        star-sentinel/
          diagnostics.json
          approval.json
          review_pack.md
      review-packs/
        review_pack.md
      reports/
        final-report.json
```

## 후속 구현 기준

- StateStore 구현자는 이 layout 밖에 artifact를 쓰지 않는다.
- ProviderAdapter는 자신의 provider output directory 밖에 쓰지 않는다.
- Star Sentinel은 `tool-output/star-sentinel/` 밖에 원본 tool output을 쓰지 않는다.
- CLI/UI/API는 artifact를 직접 조작하지 말고 StateStore 또는 정해진 API를 통해 읽는다.
