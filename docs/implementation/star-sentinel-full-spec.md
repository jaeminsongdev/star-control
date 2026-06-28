# Star Sentinel 전체 구현 스펙

## 목적

Star Sentinel은 Star-Control 기본 탑재 검증 도구다. AI 또는 provider가 만든 변경사항을 diff, policy, evidence, validation 기준으로 검증하고, diagnostics, approval gate, review pack, ledger를 생성한다.

이 문서는 Star Sentinel의 전체 목표 기능을 정의한다. P0는 현재 기준선이며, P1/P2는 장기 확장 profile이다.

## 경계

Star Sentinel은 builtin tool이다.

- 구현 코드는 `packages/star-sentinel/`에 둔다.
- manifest, policy, schema, fixture, example은 `builtin-tools/star-sentinel/`에 둔다.
- Star-Control core는 Star Sentinel rule을 직접 구현하지 않는다.
- Star-Control core는 Star Sentinel command와 artifact 계약을 통해 호출한다.

## command

Star Sentinel command:

```text
check
review-pack
gate
selfcheck
```

### `check`

입력 artifact를 읽고 diagnostics와 validation_runs를 생성한다.

입력 후보:

```text
task.json
repo_map.json
changed_lines.json
provider output
validation evidence
policy profile
```

출력:

```text
diagnostics.json
validation_runs.json
ledger.jsonl
```

### `gate`

diagnostics, validation evidence, policy profile을 기준으로 approval decision을 생성한다.

출력:

```text
approval.json
ledger.jsonl
```

Decision:

```text
AUTO_PASS
HUMAN_REVIEW
BLOCK
```

### `review-pack`

사람이 읽을 review 자료를 생성한다.

출력:

```text
review_pack.json
review_pack.md
ledger.jsonl
```

### `selfcheck`

Star Sentinel 자신과 policy/schema/fixture/example의 정합성을 검사한다.

초기 selfcheck 후보:

- policy file parse
- schema file parse
- example schema validation
- fixture parse
- rule id 중복 확인
- output name manifest 일치 확인

## output artifacts

Star Sentinel output:

```text
repo_map.json
changed_lines.json
diagnostics.json
validation_runs.json
review_pack.md
approval.json
ledger.jsonl
```

추가 구조화 output:

```text
review_pack.json
```

## SentinelTask

`task.json`은 Star Sentinel 검증 범위의 입력 계약이다.

핵심 필드:

```text
schema_version
task_id
goal
allowed_paths
forbidden_paths
forbidden_change_types
required_validation
approval_required_changes
notes
```

Star Sentinel은 `allowed_paths` 밖 변경을 P0에서 block으로 다룬다.

## RepoMap

`repo_map.json`은 검증 시점의 파일 목록과 파일 kind를 요약한다.

용도:

- source/test/docs/config/schema 구분
- changed lines 해석 보조
- risky path 판단
- review pack 문맥 제공

## ChangedLines

`changed_lines.json`은 diff hunk를 구조화한다.

용도:

- out-of-scope change 감지
- test deletion 감지
- assertion weakening 감지
- secret exposure 감지
- dependency file change 감지

## Diagnostics

Diagnostic은 rule 결과를 구조화한다.

핵심 필드:

```text
schema_version
diagnostic_id
rule_id
severity
message
locations
evidence
recommendation
```

severity:

```text
info
warn
block
```

## ValidationRun

ValidationRun은 수행한 검증과 evidence를 기록한다.

status:

```text
passed
failed
skipped
blocked
error
```

ValidationRun은 command output을 그대로 길게 저장하지 말고 핵심 evidence와 artifact path를 남긴다.

## ApprovalDecision

ApprovalDecision은 gate 결과다.

```text
AUTO_PASS
HUMAN_REVIEW
BLOCK
```

- `AUTO_PASS`: block diagnostic이 없고 required validation이 충족됨
- `HUMAN_REVIEW`: 자동 차단은 아니지만 사람 확인 필요
- `BLOCK`: 자동 진행 금지

## ReviewPack

ReviewPack은 사람이 검토하기 위한 구조화 문서다.

포함 항목:

```text
task_id
decision
summary
changed_files
risks
validations
unverified_claims
questions_for_human
generated_artifacts
review_pack_markdown
```

Markdown은 사람이 읽는 최종 산출물이다. JSON은 도구 간 전달과 테스트에 사용한다.

## Ledger

`ledger.jsonl`은 Star Sentinel 내부 event 기록이다.

event type 후보:

```text
TASK_CREATED
POLICY_CHECKED
VALIDATION_RECORDED
GATE_DECIDED
REVIEW_PACK_CREATED
ARTIFACT_WRITTEN
ERROR_RECORDED
```

## P0 rule set

P0 최소 rule:

```text
task.scope.allowed_paths
test.no_deletion
test.no_skip_only_ignore
dependency.requires_approval
secret.no_plaintext_secret
claim.validation_evidence_required
report.changed_files_match_diff
validator.no_self_bypass
validator.policy_change_requires_approval
```

## rule behavior

### `task.scope.allowed_paths`

allowed_paths 밖 변경이 있으면 block diagnostic을 만든다.

### `test.no_deletion`

test file 삭제 또는 test directory 대량 삭제를 block 후보로 본다.

### `test.no_skip_only_ignore`

테스트를 통과시키기 위한 skip/only/ignore 추가를 block 또는 human review 후보로 본다.

### `dependency.requires_approval`

dependency file 변경은 explicit approval required로 처리한다.

### `secret.no_plaintext_secret`

plaintext secret 후보가 changed lines에 있으면 block 처리한다.

### `claim.validation_evidence_required`

report가 검증했다고 주장하지만 validation_runs evidence가 없으면 human review 또는 block 후보로 본다.

### `report.changed_files_match_diff`

report의 changed_files와 changed_lines가 맞지 않으면 human review 후보로 본다.

### `validator.no_self_bypass`

검증기 자체를 우회하는 변경은 block 또는 approval required로 본다.

### `validator.policy_change_requires_approval`

policy/schema/fixture/CI validation 변경은 approval required 후보로 본다.

## profile 확장

Profile:

```text
quick
near
full
security
release
validator
```

- `quick`: P0 핵심 rule
- `near`: quick + evidence/report 일치성
- `full`: 더 넓은 파일/semantic 검사
- `security`: secret, risky command, permission 중심
- `release`: 배포 전 gate
- `validator`: Star Sentinel 자기검증

## exit behavior

Star Sentinel command는 명확한 exit code를 사용해야 한다.

후보:

```text
0: success / AUTO_PASS
1: diagnostics found requiring review or block
2: invalid input
3: tool execution error
4: internal error
```

초기 구현에서는 decision artifact를 source of truth로 삼고 exit code는 보조 정보로 둔다.

## schema validation

Star Sentinel은 산출물을 쓰기 전에 schema validation을 수행해야 한다.

최소 대상:

```text
diagnostics.json
validation_runs.json
approval.json
review_pack.json
ledger.jsonl line item
repo_map.json
changed_lines.json
```

## selfcheck 기준

selfcheck는 다음을 검사한다.

1. manifest 필수 command/output 존재
2. policy profile parse 가능
3. schema parse 가능
4. example이 schema를 만족
5. fixture parse 가능
6. rule id 중복 없음
7. legacy alias 위치 제한 준수
8. Star Sentinel 명칭 정책 준수

## 금지 사항

Star Sentinel은 다음을 하면 안 된다.

- core 내부 구현 세부사항에 의존
- provider 제품명에 의존
- validation failure를 숨김
- policy 위반을 warn으로 낮춰 자동 통과
- approval required change를 AUTO_PASS 처리
- secret 후보를 artifact에 그대로 확대 저장

## 테스트 기준

최소 테스트:

1. scope violation fixture -> BLOCK
2. dependency change fixture -> HUMAN_REVIEW 또는 BLOCK
3. diagnostic schema validation
4. approval schema validation
5. review pack schema validation
6. ledger event schema validation
7. selfcheck success case
8. invalid input -> non-zero result
9. missing required artifact -> error
10. policy profile parse failure -> error

## Codex 구현 지시

Star Sentinel 구현은 다음 순서로 진행한다.

1. schema model과 loader
2. task input reader
3. changed lines reader
4. P0 rule evaluator
5. diagnostics writer
6. gate decision writer
7. review pack writer
8. ledger writer
9. selfcheck
10. integration smoke with ValidationEngine

각 단계는 별도 PR로 진행한다. 전체 rule engine을 한 PR에 몰아넣지 않는다.
