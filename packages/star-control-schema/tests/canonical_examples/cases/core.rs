use super::ValidationCase;

pub(super) const CASES: &[ValidationCase] = &[
    ValidationCase {
        schema_path: "specs/schemas/job.schema.json",
        document_path: "examples/runs/J-0001/job.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/run-state.schema.json",
        document_path: "examples/runs/J-0001/run-state.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/route.schema.json",
        document_path: "configs/templates/route-template.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/route.schema.json",
        document_path: "examples/fake/route-done.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/route.schema.json",
        document_path: "examples/runs/J-0001/route.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/route.schema.json",
        document_path: "examples/router-contracts/route-approval-required.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/router-decision.schema.json",
        document_path: "examples/router-contracts/router-decision.schema-change.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/workspec.schema.json",
        document_path: "examples/runs/J-0001/workspecs/implement.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/report.schema.json",
        document_path: "configs/templates/report-template.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/report.schema.json",
        document_path: "examples/fake/impl-report-done.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/event.schema.json",
        document_path: "examples/core/event.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/artifact-ref.schema.json",
        document_path: "examples/core/artifact-ref.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/error.schema.json",
        document_path: "examples/core/error.example.json",
    },
];
