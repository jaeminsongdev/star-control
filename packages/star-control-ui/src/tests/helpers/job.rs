use crate::constants::SCHEMA_VERSION;
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn create_job(store: &StateStore, state: &str, stage: &str, next_action: &str) {
    let mut job = store
        .create_job("Core schema contract update", ".", Vec::new())
        .expect("create job");
    job["state"] = json!(state);
    store.save_job("J-0001", &job).expect("save job");

    store
        .save_state(
            "J-0001",
            &json!({
                "schema_version": SCHEMA_VERSION,
                "job_id": "J-0001",
                "state": state,
                "current_stage": stage,
                "updated_at": "unix:2",
                "workers": {},
                "artifacts": {
                    "provider_output": {
                        "path": "provider-output/fake-default/output.json",
                        "kind": "provider_output"
                    },
                    "validation": {
                        "path": "validation/validation-decision.json",
                        "kind": "other"
                    },
                    "approval_request": {
                        "path": "approvals/approval-request.json",
                        "kind": "approval"
                    },
                    "review_pack": {
                        "path": "review-packs/review_pack.md",
                        "kind": "review_pack"
                    }
                },
                "latest_event_id": "J-0001-0002",
                "active_provider": "fake-default",
                "next_action": next_action
            }),
        )
        .expect("save state");
    store
        .append_event(
            "J-0001",
            &json!({
                "schema_version": SCHEMA_VERSION,
                "event_id": "J-0001-0002",
                "job_id": "J-0001",
                "type": "APPROVAL_REQUESTED",
                "created_at": "unix:2",
                "stage": stage,
                "state": state,
                "message": "Approval requested",
                "artifact_paths": ["approvals/approval-request.json"],
                "details": {}
            }),
        )
        .expect("append event");
}

pub(crate) fn save_report(store: &StateStore, stage: &str, risks: Vec<&str>) {
    store
        .save_report(
            "J-0001",
            &format!("{}-report", stage),
            &json!({
                "schema_version": SCHEMA_VERSION,
                "job_id": "J-0001",
                "stage": stage,
                "status": "NEEDS_APPROVAL",
                "changed_files": ["src/lib.rs"],
                "commands_run": [],
                "validation": [],
                "risks": risks,
                "blocked_reason": Value::Null,
                "next_step": "approve",
                "artifacts": ["approvals/approval-request.json"]
            }),
        )
        .expect("save report");
}

pub(crate) fn write_approval_request(store: &StateStore, stage: &str) {
    store
        .write_approval_json(
            "J-0001",
            "approval-request.json",
            &json!({
                "schema_version": SCHEMA_VERSION,
                "job_id": "J-0001",
                "stage": stage,
                "task_id": format!("{}-approval", stage),
                "decision": "HUMAN_REVIEW",
                "reasons": ["API control mutation requires human approval"],
                "changed_files": ["src/lib.rs"],
                "risks": [],
                "diagnostics": [],
                "review_pack_path": "review-packs/review_pack.md",
                "requested_at": "unix:2",
                "requested_by": "star-control-ui-test"
            }),
        )
        .expect("write approval request");
}

pub(crate) fn write_release_readiness(project: &Path) -> PathBuf {
    let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
    fs::create_dir_all(path.parent().expect("release dir")).expect("create release dir");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "release_id": "release-0007",
            "target": "star-control",
            "version": "1.2.3",
            "status": "reserved",
            "checks": [
                {
                    "name": "release-profile-passed",
                    "status": "pass",
                    "evidence_paths": ["review-packs/release-profile.json"]
                },
                {
                    "name": "version-consistent",
                    "status": "pass",
                    "evidence_paths": ["VERSION"]
                }
            ],
            "blockers": [
                "release approval/signing/publish/deploy automation remains reserved"
            ],
            "approvals": [],
            "generated_at": "unix:7"
        }))
        .expect("release readiness JSON"),
    )
    .expect("write release readiness");
    path
}
