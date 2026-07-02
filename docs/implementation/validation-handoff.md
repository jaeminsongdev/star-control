# Validation Handoff

## 목적

ValidationEngine이 tool output을 core artifact로 연결하는 최소 계약이다.

## contracts

```text
specs/schemas/validation-decision.schema.json
specs/schemas/approval-request.schema.json
specs/schemas/approval-response.schema.json
specs/schemas/review-pack-handoff.schema.json
examples/validation-contracts/
```

## mapping

```text
AUTO_PASS -> VALIDATED
HUMAN_REVIEW -> WAITING_APPROVAL
BLOCK -> BLOCKED
invalid output -> FAILED
```

## paths

```text
tool-output/star-sentinel/approval.json
tool-output/star-sentinel/review_pack.json
tool-output/star-sentinel/review_pack.md
tool-output/star-sentinel/validation_runs.json
validation/validation-decision.json
approvals/approval-request.json
approvals/approval-response.json
review-packs/review_pack.json
review-packs/review_pack.md
review-packs/handoff.json
```

## rules

- tool output is original evidence.
- `review-packs/` is the canonical user-facing copy.
- approval response is required before leaving `WAITING_APPROVAL`.
- decision can be upgraded to more risk, but not downgraded silently.
