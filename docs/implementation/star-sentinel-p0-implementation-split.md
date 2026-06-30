# Star Sentinel P0 Implementation Split

## 목적

이 문서는 Codex가 E09 Star Sentinel P0를 구현할 때 한 PR에 과도한 rule engine, gate, review pack, selfcheck를 몰아넣지 않도록 분할 기준을 고정한다.

범위 결정은 `docs/decisions/0004-star-sentinel-p0-scope.md`를 따른다.

## v0 P0 rule set

v0 P0는 다음 5개 rule만 구현한다.

```text
task.scope.allowed_paths
test.no_deletion
dependency.requires_approval
secret.no_plaintext_secret
validator.no_self_bypass
```

P1 이후 후보:

```text
test.no_skip_only_ignore
claim.validation_evidence_required
report.changed_files_match_diff
validator.policy_change_requires_approval
```

## PR 분할

### E09a P0 evaluator

수정 허용 후보:

```text
packages/star-sentinel/**
builtin-tools/star-sentinel/policies/**
builtin-tools/star-sentinel/examples/p0/**
관련 unit tests
필요한 최소 docs 업데이트
```

포함:

```text
task input reader
changed-lines reader
p0 rule registry loader
5개 v0 P0 rule evaluator
fixture outcome tests
```

제외:

```text
gate writer
review-pack writer
ledger writer
selfcheck
ValidationEngine integration
```

완료 기준:

```text
P0 fixtures가 expected diagnostics 또는 expected decision 후보를 생성한다.
```

### E09b diagnostics + gate writer

포함:

```text
diagnostics.json writer
approval.json writer
BLOCK > HUMAN_REVIEW > AUTO_PASS priority
invalid input handling
schema validation before write
```

제외:

```text
review-pack writer
ValidationEngine integration
full/security/release profile
```

완료 기준:

```text
P0 diagnostics에서 approval decision artifact를 deterministic하게 생성한다.
```

### E09c review-pack writer

포함:

```text
review_pack.json writer
review_pack.md writer
changed files / risks / validations summary
human questions section
```

제외:

```text
new rule evaluator
ValidationEngine integration
UI rendering
```

완료 기준:

```text
HUMAN_REVIEW 또는 BLOCK decision에 대해 사람이 읽을 review pack을 생성한다.
```

### E09d ledger + selfcheck

포함:

```text
ledger.jsonl writer
manifest output name check
policy parse check
schema parse check
fixture parse check
rule id duplicate check
legacy alias location check
```

제외:

```text
release profile automation
external scanner integration
```

완료 기준:

```text
selfcheck가 P0 policy/schema/example/fixture 정합성을 확인한다.
```

## E10 handoff

E10 ValidationEngine은 E09b 이후 `check`와 `gate` command 계약을 사용할 수 있다. 다만 review pack handoff까지 포함하려면 E09c 완료를 기다린다.

최소 handoff artifact:

```text
tool-output/star-sentinel/diagnostics.json
tool-output/star-sentinel/approval.json
tool-output/star-sentinel/validation_runs.json 후보
```

review handoff artifact:

```text
tool-output/star-sentinel/review_pack.json
tool-output/star-sentinel/review_pack.md
```

## 금지 사항

- Star-Control core에 Star Sentinel rule을 직접 구현하지 않는다.
- P1 rule을 E09a에 섞지 않는다.
- 실패한 validation을 AUTO_PASS로 낮추지 않는다.
- policy/schema/fixture/CI validation 변경을 approval 없이 자동 통과시키지 않는다.
