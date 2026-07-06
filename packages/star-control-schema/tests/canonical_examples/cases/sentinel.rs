use super::ValidationCase;

pub(super) const CASES: &[ValidationCase] = &[
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/approval.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/approval-block.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/sentinel-task.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/sentinel-task.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/diagnostic.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/diagnostic-block.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/ledger-event.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/ledger-event.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/validation-run.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/validation-run.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/review-pack.schema.json",
        document_path:
            "builtin-tools/star-sentinel/examples/p0/review-pack-human-review.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/repo-map.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/repo-map.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/changed-lines.schema.json",
        document_path: "builtin-tools/star-sentinel/examples/p0/changed-lines.example.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/p0-rule-registry.schema.json",
        document_path: "builtin-tools/star-sentinel/policies/p0-rule-registry.json",
    },
    ValidationCase {
        schema_path: "builtin-tools/star-sentinel/schemas/fixture-outcome.schema.json",
        document_path:
            "builtin-tools/star-sentinel/examples/p0/fixture-outcome-scope-block.example.json",
    },
];
