use serde_json::json;
use star_control_state::StateStore;

pub(super) fn write_approval_response(store: &StateStore, job_id: &str, task_id: &str) {
    store
        .write_approval_json(
            job_id,
            "approval-response.json",
            &json!({
                "schema_version": "1.0.0",
                "job_id": job_id,
                "stage": "validate",
                "task_id": task_id,
                "response": "approved",
                "reviewer": "integration-smoke",
                "responded_at": "2026-07-01T00:00:00Z",
                "reason": "approved for v0 fake integration smoke",
                "allowed_next_stage": "report",
                "constraints": []
            }),
        )
        .expect("approval response");
}
