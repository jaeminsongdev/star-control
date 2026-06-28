# Star Sentinel P0 Contracts

## 목적

이 문서는 Star Sentinel P0 구현자가 필요한 rule registry, diff input, fixture outcome 계약을 고정한다. 전체 스펙은 `star-sentinel-full-spec.md`를 따르되, P0 구현 PR은 이 문서를 우선 읽는다.

## machine-readable contracts

```text
builtin-tools/star-sentinel/schemas/sentinel-task.schema.json
builtin-tools/star-sentinel/schemas/changed-lines.schema.json
builtin-tools/star-sentinel/schemas/diagnostic.schema.json
builtin-tools/star-sentinel/schemas/p0-rule-registry.schema.json
builtin-tools/star-sentinel/schemas/fixture-outcome.schema.json
builtin-tools/star-sentinel/policies/p0-rule-registry.json
builtin-tools/star-sentinel/examples/p0/changed-lines.example.json
builtin-tools/star-sentinel/examples/p0/diagnostic-block.example.json
builtin-tools/star-sentinel/examples/p0/fixture-outcome-scope-block.example.json
```

위 schema/example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## P0 입력

P0 evaluator의 최소 입력은 다음이다.

```text
SentinelTask
ChangedLines
P0RuleRegistry
```

선택 입력:

```text
ValidationRun[]
ReportSpec
ApprovalDecision
RepoMap
```

## ChangedLines 계약

`changed_lines.json`은 diff를 line-level로 구조화한 입력이다.

핵심 필드:

```text
schema_version
task_id
files[].path
files[].change_type
files[].old_path
files[].hunks[].old_start
files[].hunks[].old_lines
files[].hunks[].new_start
files[].hunks[].new_lines
files[].hunks[].lines[].kind
files[].hunks[].lines[].old_line
files[].hunks[].lines[].new_line
files[].hunks[].lines[].content
```

line kind 후보:

```text
added
removed
context
```

P0 rule은 source diff 원문을 직접 파싱하지 않고 ChangedLines를 입력으로 받는다. diff parser는 별도 구현 단위다.

## P0 rule registry

`p0-rule-registry.json`은 P0 rule id, severity, input, output, decision effect를 선언한다.

초기 rule:

```text
task.scope.allowed_paths
test.no_deletion
dependency.requires_approval
secret.no_plaintext_secret
validator.no_self_bypass
```

후속 rule 후보:

```text
test.no_skip_only_ignore
claim.validation_evidence_required
report.changed_files_match_diff
validator.policy_change_requires_approval
```

P0 selfcheck는 최소한 다음을 검사한다.

1. rule id 중복 없음
2. rule id가 비어 있지 않음
3. severity가 schema enum에 포함됨
4. decision_effect가 review/gate decision과 모순되지 않음
5. registry가 schema-example-check를 통과함

## rule behavior

### task.scope.allowed_paths

- 입력: SentinelTask, ChangedLines
- 조건: changed file path가 `allowed_paths`에 포함되지 않음
- severity: block
- decision effect: BLOCK

### test.no_deletion

- 입력: ChangedLines
- 조건: test file 또는 test directory가 삭제됨
- severity: block
- decision effect: BLOCK

### dependency.requires_approval

- 입력: ChangedLines, ApprovalDecision 후보
- 조건: dependency manifest 또는 lock file 변경
- severity: warn
- decision effect: HUMAN_REVIEW

### secret.no_plaintext_secret

- 입력: ChangedLines
- 조건: added line에 plaintext secret 후보가 있음
- severity: block
- decision effect: BLOCK

### validator.no_self_bypass

- 입력: ChangedLines
- 조건: validator/policy/schema/CI가 self bypass 방향으로 변경됨
- severity: block
- decision effect: BLOCK

## Fixture outcome

Fixture outcome은 fixture 입력에 대해 기대되는 gate decision과 diagnostics를 선언한다.

핵심 필드:

```text
schema_version
fixture_id
profile
expected_decision
expected_diagnostics
notes
```

fixture outcome은 evaluator 테스트의 expected value다. 구현자는 test에서 실제 diagnostics와 expected_diagnostics를 비교해야 한다.

## P0 output

P0 evaluator의 최소 output:

```text
diagnostics.json
approval.json 후보
ledger.jsonl
```

`check` command는 diagnostics와 validation_runs를 만들 수 있다. `gate` command는 diagnostics와 validation evidence를 기반으로 approval decision을 만든다.

## decision rule

P0 decision은 아래 우선순위를 따른다.

```text
BLOCK > HUMAN_REVIEW > AUTO_PASS
```

- block diagnostic이 하나라도 있으면 BLOCK
- approval required change만 있으면 HUMAN_REVIEW
- block/review 조건이 없으면 AUTO_PASS

## 테스트 기준

1. p0 rule registry schema validation
2. fixture outcome schema validation
3. changed-lines schema validation
4. task.scope.allowed_paths fixture -> BLOCK
5. dependency.requires_approval fixture -> HUMAN_REVIEW
6. secret.no_plaintext_secret fixture -> BLOCK
7. duplicate rule id -> selfcheck error
8. unknown rule id in fixture outcome -> selfcheck error

## Codex 구현 지시

P0 rule evaluator 구현 PR은 다음만 포함한다.

- changed-lines reader
- rule registry loader
- P0 rule evaluator
- diagnostics writer
- fixture outcome tests

Gate writer, review pack writer, full profile, security profile, release profile은 별도 PR로 분리한다.
