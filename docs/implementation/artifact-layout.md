# Artifact Layout

## 목적

Star-Control은 실행 산출물을 Star-Control repository 내부에 저장하지 않는다. 모든 run artifact는 대상 프로젝트의 `.ai-runs/` 아래에 저장한다. 이 문서는 file-based StateStore와 provider/tool output이 사용할 디렉터리 구조를 정의한다.

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

append-only ledger다. 한 줄에 하나의 LedgerEvent JSON object를 기록한다.

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

## reports layout

```text
reports/
  implement-report.json
  validate-report.json
  review-report.json
  final-report.json
```

ReportSpec은 stage별 report와 최종 report를 모두 지원한다.

## provider output layout

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

## approvals layout

```text
approvals/
  approval-request.json
  approval-response.json
```

`WAITING_APPROVAL` 상태에서 사용한다. approval response 없이 자동으로 다음 stage로 이동하면 안 된다.

## review packs layout

```text
review-packs/
  review_pack.json
  review_pack.md
```

사람이 확인하는 review 자료를 둔다. Star Sentinel output과 중복될 수 있으나, 최종 user-facing copy는 이 위치에 둘 수 있다.

## tmp layout

```text
tmp/
```

작업 중 임시 파일을 둘 수 있다. terminal state 진입 전 정리하거나, 정리하지 못한 경우 report에 남긴다.

## atomic write 규칙

StateStore는 JSON 파일을 직접 덮어쓰지 않는다. 다음 순서를 권장한다.

```text
1. {file}.tmp 작성
2. fsync 가능한 환경이면 flush
3. atomic rename으로 교체
```

`events.jsonl`은 append-only로 쓴다. 중간에 줄이 깨진 경우 recovery 정책에서 마지막 깨진 줄을 무시할 수 있다.

## path rule

- artifact 내부 path는 가능하면 job directory 기준 상대 경로를 사용한다.
- absolute path는 report에 그대로 노출하지 않는 것을 기본으로 한다.
- secret, token, credential 값은 artifact에 저장하지 않는다.

## cleanup rule

- Star-Control core는 사용자 승인 없이 대상 프로젝트 source file을 삭제하지 않는다.
- `.ai-runs/` 내부 cleanup도 명시 command 또는 retention policy가 생기기 전까지 자동 삭제하지 않는다.
- 실패한 run artifact는 debugging을 위해 보존한다.

## Star-Control repository 금지 경로

Star-Control repository 내부에는 다음을 만들지 않는다.

```text
.ai-runs/
provider-output/
tool-output/
```

이 금지는 repository-policy-check의 장기 확장 대상이다.

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
        fake-default/
          request.json
          response.json
      tool-output/
        star-sentinel/
          diagnostics.json
          approval.json
          review_pack.md
      reports/
        final-report.json
```
