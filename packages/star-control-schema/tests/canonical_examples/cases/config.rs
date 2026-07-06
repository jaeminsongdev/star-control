use super::ValidationCase;

pub(super) const CASES: &[ValidationCase] = &[
    ValidationCase {
        schema_path: "specs/schemas/config.schema.json",
        document_path: "examples/config-contracts/config.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/policy.schema.json",
        document_path: "examples/config-contracts/policy.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/hook.schema.json",
        document_path: "examples/config-contracts/hook.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/role.schema.json",
        document_path: "examples/config-contracts/role.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/renderer.schema.json",
        document_path: "examples/config-contracts/renderer.example.json",
    },
    ValidationCase {
        schema_path: "specs/schemas/skill.schema.json",
        document_path: "examples/config-contracts/skill.example.json",
    },
];
