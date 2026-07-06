use super::repo_root;
use serde_json::json;
use star_control_state::StateStore;
use std::fs;
use std::path::Path;

pub(crate) fn write_recovery_inspection_job(project: &Path) {
    let store = StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
    store
        .create_job("recovery inspection", "codex", vec![])
        .expect("create job");
    store
        .save_state(
            "J-0001",
            &json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "state": "DONE",
                "current_stage": "report",
                "updated_at": "test:recovery",
                "threads": {},
                "workers": {},
                "artifacts": {},
                "latest_event_id": "J-0001-0001",
                "active_provider": null,
                "next_action": "none",
                "budget": {},
                "history": []
            }),
        )
        .expect("save recovery state");
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");
    fs::write(&tmp_path, b"{\"partial\":true").expect("write tmp file");
}
