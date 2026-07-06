use super::repo_root;
use serde_json::json;
use star_control_state::StateStore;
use std::fs;
use std::path::Path;

pub(crate) fn write_release_readiness_job(project: &Path, include_readiness: bool) {
    let store = StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
    store
        .create_job("release readiness", "codex", vec![])
        .expect("create job");
    if include_readiness {
        let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
        fs::create_dir_all(path.parent().expect("release dir")).expect("create release dir");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "1.0.0",
                "release_id": "release-0008",
                "target": "star-control",
                "version": "1.2.3",
                "status": "reserved",
                "checks": [
                    {
                        "name": "release-profile-passed",
                        "status": "pass",
                        "evidence_paths": ["review-packs/release-profile.json"]
                    }
                ],
                "blockers": [
                    "release approval/signing/publish/deploy automation remains reserved"
                ],
                "approvals": [],
                "generated_at": "unix:8"
            }))
            .expect("release readiness JSON"),
        )
        .expect("write release readiness");
    }
}
