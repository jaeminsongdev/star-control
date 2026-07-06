use super::ValidationCase;

pub(super) const CASES: &[ValidationCase] = &[
    ValidationCase {
        schema_path: "specs/schemas/cli-output.schema.json",
        document_path: "examples/cli-contracts/run-output.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/cli-output.schema.json",
        document_path: "examples/cli-contracts/status-output.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/cli-output.schema.json",
        document_path: "examples/cli-contracts/approve-output.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/cli-error.schema.json",
        document_path: "examples/cli-contracts/error-output.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/daemon-state.schema.json",
        document_path: "examples/surface-contracts/daemon-state.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/api-response.schema.json",
        document_path: "examples/surface-contracts/api-job-response.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/ui-job-view.schema.json",
        document_path: "examples/surface-contracts/ui-job-view.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/redaction-report.schema.json",
        document_path: "examples/security-contracts/redaction-report.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/audit-event.schema.json",
        document_path: "examples/security-contracts/audit-event.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/cost-metric.schema.json",
        document_path: "examples/security-contracts/cost-metric.fake.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/privacy-handoff.schema.json",
        document_path: "examples/security-contracts/privacy-handoff.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/release-readiness.schema.json",
        document_path: "examples/release-contracts/release-readiness.example.json",
    },
];
