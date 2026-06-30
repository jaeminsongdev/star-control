# 0004 Star Sentinel P0 Scope Decision

## 상태

Accepted.

## 결정

Star Sentinel v0 P0 범위는 5개 핵심 rule로 고정한다.

```text
task.scope.allowed_paths
test.no_deletion
dependency.requires_approval
secret.no_plaintext_secret
validator.no_self_bypass
```

아래 rule은 v0 P0가 아니라 P1 이후 확장 후보로 둔다.

```text
test.no_skip_only_ignore
claim.validation_evidence_required
report.changed_files_match_diff
validator.policy_change_requires_approval
```

## E09 분할 기준

E09는 한 PR에 몰아넣지 않고 다음 단위로 나눈다.

```text
E09a P0 input reader + rule registry loader + evaluator
E09b diagnostics writer + gate decision writer
E09c review-pack writer
E09d ledger writer + selfcheck
```

## 적용 범위

이 결정은 다음 문서와 구현 작업에 적용한다.

- `docs/implementation/star-sentinel-p0-contracts.md`
- `docs/implementation/star-sentinel-full-spec.md`
- `docs/implementation/star-sentinel-p0-implementation-split.md`
- `docs/implementation/codex-work-queue-current.md`의 E09 해석
- Codex가 수행할 Star Sentinel P0 구현 PR

## 이유

전체 Star Sentinel 목표 기능은 넓지만, v0 fake flow와 ValidationEngine 연동 전에는 최소 gate가 먼저 안정화되어야 한다. P0를 9개 rule로 시작하면 evaluator, evidence/report consistency, policy self-review, review-pack rendering이 한 PR에 섞일 수 있다.

따라서 v0 P0는 scope, test deletion, dependency approval, sensitive literal, validator self-bypass에 집중하고, evidence/report consistency와 policy-change review는 P1로 미룬다.

## 비결정 사항

아래 항목은 이 결정에서 확정하지 않는다.

- P1/P2 rule 구현 순서
- semantic diff engine
- full/security/release profile 세부 정책
- external scanner 연동
- UI review 화면
