use super::repo_root;
use serde_json::json;
use star_control_state::StateStore;
use std::path::Path;

pub(crate) fn write_waiting_approval_job(project: &Path, include_request: bool) {
    let store = StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
    store
        .create_job("needs approval", "codex", vec![])
        .expect("create job");
    store
        .save_state(
            "J-0001",
            &json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "state": "WAITING_APPROVAL",
                "current_stage": "validate",
                "updated_at": "test:waiting-approval",
                "threads": {},
                "workers": {},
                "artifacts": {},
                "latest_event_id": "",
                "active_provider": null,
                "next_action": "await_approval",
                "budget": {},
                "history": []
            }),
        )
        .expect("save waiting approval state");
    if include_request {
        store
            .write_approval_json(
                "J-0001",
                "approval-request.json",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": "J-0001",
                    "stage": "validate",
                    "task_id": "p0-human-review",
                    "decision": "HUMAN_REVIEW",
                    "reasons": ["dependency_change_requires_approval"],
                    "changed_files": ["Cargo.toml"],
                    "risks": ["dependency update"],
                    "diagnostics": [
                        {
                            "rule_id": "dependency.requires_approval",
                            "severity": "human_review",
                            "message": "Review dependency change before continuing."
                        }
                    ],
                    "review_pack_path": "review-packs/review_pack.md",
                    "requested_at": "2026-07-01T00:00:00Z",
                    "requested_by": "star-sentinel"
                }),
            )
            .expect("write approval request");
    }
}
