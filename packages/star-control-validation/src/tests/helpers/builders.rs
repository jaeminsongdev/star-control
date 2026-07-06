use crate::constants::SCHEMA_VERSION;
use crate::ValidationContext;
use serde_json::{json, Value};

pub(crate) fn context() -> ValidationContext {
    ValidationContext::new("J-0001", "validate", "p0-task-demo", "2026-07-01T00:00:00Z")
}

pub(crate) fn approval(decision: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": "p0-task-demo",
        "decision": decision,
        "reasons": ["schema_change_requires_approval"],
        "diagnostics": [
            {
                "rule_id": "dependency.requires_approval",
                "severity": if decision == "BLOCK" { "block" } else { "warn" }
            }
        ],
        "required_human_actions": [
            "Review HUMAN_REVIEW diagnostics and record approval before continuing."
        ]
    })
}

pub(crate) fn review_pack(decision: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": "p0-task-demo",
        "decision": decision,
        "summary": "Dependency-related files changed and require explicit review before proceeding.",
        "changed_files": ["Cargo.toml"],
        "risks": ["dependency_addition"],
        "validations": [
            {
                "command": "policy:p0",
                "result": if decision == "BLOCK" { "blocked" } else { "requires_human_review" }
            }
        ],
        "unverified_claims": [],
        "diagnostics": [
            {
                "rule_id": "dependency.requires_approval",
                "severity": if decision == "BLOCK" { "block" } else { "warn" }
            }
        ],
        "source_artifacts": [
            "tool-output/star-sentinel/approval.json"
        ],
        "questions_for_human": [
            "Is this dependency approved?"
        ],
        "review_pack_markdown": "# Review Pack\n\nDependency-related files changed and require explicit review before proceeding."
    })
}

pub(super) fn state(job_id: &str, state: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": state,
        "current_stage": "validate",
        "updated_at": "2026-07-01T00:00:00Z",
        "workers": {},
        "artifacts": {},
        "next_action": "run_validation",
        "history": []
    })
}
