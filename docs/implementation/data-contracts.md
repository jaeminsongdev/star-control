# Data Contracts

## 목적

이 문서는 Star-Control과 Star Sentinel 구현자가 공유해야 하는 데이터 계약을 정리한다. JSON schema가 존재하는 항목은 schema를 우선하고, 이 문서는 필드의 의미와 사용 위치를 설명한다.

## 계약 분류

```text
Core contracts
  JobSpec
  RunState
  RouteSpec
  WorkSpec
  ReportSpec

Star Sentinel contracts
  SentinelTask
  Diagnostic
  ApprovalDecision
  ReviewPack
  ValidationRun
  LedgerEvent
  RepoMap
  ChangedLines

Provider contracts
  ProviderManifest
  ProviderInstance
  CapabilityProfile
  ProviderOutput
```

## 공통 규칙

- 모든 새 JSON artifact는 `schema_version`을 가져야 한다.
- 현재 기본 schema version은 `1.0.0`이다.
- job id는 사람이 추적 가능한 문자열이어야 한다.
- artifact는 가능하면 상대 경로를 사용한다.
- schema 변경은 example과 CI 검증 case를 함께 수정한다.

## JobSpec

위치 후보:

```text
specs/schemas/job.schema.json
.ai-runs/J-0001/job.json
```

역할:

- 사용자 요청의 원본 계약
- project root, request text, entrypoint, initial state 저장
- run lifecycle의 시작점

핵심 필드:

```text
schema_version
job_id
project_root
request_text
created_at
entrypoint
state
user_constraints
```

## RunState

위치 후보:

```text
specs/schemas/run-state.schema.json
.ai-runs/J-0001/run-state.json
```

역할:

- 현재 job의 진행 상태
- 현재 stage
- worker/provider 상태
- 다음 action
- budget, artifact, history 요약

`state`는 canonical job state enum을 사용한다. `current_stage`는 canonical stage enum을 사용한다.

## RouteSpec

위치 후보:

```text
specs/schemas/route.schema.json
.ai-runs/J-0001/route.json
```

역할:

- 요청을 stage와 provider assignment로 나눈 결과
- size/risk/approval 필요 여부 판단 결과

핵심 필드:

```text
schema_version
job_id
summary
size
risk
stages
assignments
requires_user_approval
approval_reasons
workspecs
```

## WorkSpec

위치 후보:

```text
specs/schemas/workspec.schema.json
.ai-runs/J-0001/workspecs/{stage}.json
```

역할:

- provider 또는 worker에게 전달되는 단일 작업 단위
- 허용 scope, 금지 action, required output을 명확히 제한

핵심 필드:

```text
schema_version
job_id
stage
role
provider
project_root
goal
allowed_scope
forbidden_actions
context_pack
required_outputs
validation_requirements
```

## ReportSpec

위치 후보:

```text
specs/schemas/report.schema.json
.ai-runs/J-0001/reports/{stage}-report.json
```

역할:

- stage 또는 job 완료 후 사람이 읽을 수 있는 보고 계약
- changed files, validation, risks, next step을 정리

핵심 필드:

```text
schema_version
job_id
stage
status
changed_files
commands_run
validation
risks
blocked_reason
next_step
artifacts
```

## SentinelTask

위치 후보:

```text
builtin-tools/star-sentinel/schemas/sentinel-task.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/task.json
```

역할:

- Star Sentinel이 검증할 작업 범위를 고정
- allowed paths, forbidden paths, forbidden change types, required validation 명시

## Diagnostic

위치 후보:

```text
builtin-tools/star-sentinel/schemas/diagnostic.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json
```

역할:

- rule 위반 또는 위험 신호를 구조화
- severity는 `info`, `warn`, `block` 중 하나

## ApprovalDecision

위치 후보:

```text
builtin-tools/star-sentinel/schemas/approval.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/approval.json
```

역할:

- gate 결과를 구조화
- decision은 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK` 중 하나

## ReviewPack

위치 후보:

```text
builtin-tools/star-sentinel/schemas/review-pack.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/review_pack.json
.ai-runs/J-0001/tool-output/star-sentinel/review_pack.md
```

역할:

- 사람이 검토할 changed files, risks, validations, questions를 정리
- JSON은 구조화된 중간 산출물
- Markdown은 사람이 읽는 최종 산출물

## ValidationRun

위치 후보:

```text
builtin-tools/star-sentinel/schemas/validation-run.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/validation_runs.json
```

역할:

- 실행한 검증 명령과 결과를 기록
- evidence와 diagnostics를 연결

## LedgerEvent

위치 후보:

```text
builtin-tools/star-sentinel/schemas/ledger-event.schema.json
.ai-runs/J-0001/events.jsonl
.ai-runs/J-0001/tool-output/star-sentinel/ledger.jsonl
```

역할:

- append-only event 기록
- 사람이 추적 가능한 run history 제공
- failure debugging과 audit에 사용

## RepoMap

위치 후보:

```text
builtin-tools/star-sentinel/schemas/repo-map.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/repo_map.json
```

역할:

- 검증 시점의 파일 목록, 파일 kind, language, symbols 요약
- diff와 scope 판단의 보조 자료

## ChangedLines

위치 후보:

```text
builtin-tools/star-sentinel/schemas/changed-lines.schema.json
.ai-runs/J-0001/tool-output/star-sentinel/changed_lines.json
```

역할:

- diff hunk와 line-level 변경 내용을 구조화
- test weakening, secret exposure, assertion change 탐지의 입력

## ProviderManifest

위치 후보:

```text
builtin-providers/{provider-id}/provider.yaml
specs/schemas/provider-manifest.schema.json
```

역할:

- provider kind, transport, adapter, capability를 선언
- core가 provider를 제품명이 아니라 capability로 다루도록 보장

## ProviderInstance

위치 후보:

```text
configs/providers/{instance-id}.json
examples/fake/provider-instance.json
```

역할:

- 특정 project/run에서 사용할 provider 설정
- credential 값은 직접 저장하지 않는다

## ProviderOutput

위치 후보:

```text
.ai-runs/J-0001/provider-output/{provider-instance-id}/
```

역할:

- provider stdout/stderr/log/result artifact 저장
- core는 provider output을 해석 가능한 최소 계약으로만 읽는다

## contract evolution

- 새 필드 추가는 optional부터 시작한다.
- 필수 필드 추가는 schema version을 검토한다.
- enum 추가는 기존 구현과 CI 영향도를 확인한다.
- schema 변경 PR은 반드시 example과 `schema-example-check`를 함께 갱신한다.
