use super::ValidationCase;

pub(super) const CASES: &[ValidationCase] = &[
    ValidationCase {
        schema_path: "specs/schemas/execution-request.schema.json",
        document_path: "examples/execution-contracts/execution-request.fake.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/execution-attempt.schema.json",
        document_path: "examples/execution-contracts/execution-attempt.success.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/validation-decision.schema.json",
        document_path:
            "examples/validation-contracts/validation-decision.human-review.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/approval-request.schema.json",
        document_path: "examples/validation-contracts/approval-request.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/approval-response.schema.json",
        document_path: "examples/validation-contracts/approval-response.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/review-pack-handoff.schema.json",
        document_path: "examples/validation-contracts/review-pack-handoff.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/provider-manifest.schema.json",
        document_path: "examples/provider-contracts/provider-manifest.fake.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/provider-instance.schema.json",
        document_path: "examples/provider-contracts/provider-instance.fake.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/capability-profile.schema.json",
        document_path: "examples/provider-contracts/capability-profile.fake.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/provider-registry.schema.json",
        document_path: "examples/provider-contracts/provider-registry.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/provider-run-result.schema.json",
        document_path: "examples/provider-contracts/provider-run-result.success.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/provider-run-result.schema.json",
        document_path: "examples/execution-contracts/fake-provider-response.success.example.json",
    },
];
