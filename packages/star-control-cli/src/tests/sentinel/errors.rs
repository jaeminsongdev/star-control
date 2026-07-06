use crate::test_support::{repo_root, temp_project};
use crate::{run_cli, CliConfig};
use serde_json::Value;
use star_control_state::StateStore;
use std::fs;

pub(super) fn sentinel_rejects_missing_inputs_and_reserved_options() {
    let config = CliConfig::new(repo_root());
    let project = temp_project();
    let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("open store");
    store
        .create_job("missing sentinel inputs", "codex", vec![])
        .expect("create job");

    let missing = run_cli(
        [
            "sentinel",
            "check",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        &config,
    );
    assert_eq!(missing.exit_code, 3);
    let missing_json: Value = serde_json::from_str(&missing.stdout).expect("missing json");
    assert_eq!(missing_json["error"]["code"], "MissingArtifact");
    assert_eq!(
        missing_json["error"]["artifact_paths"][0],
        ".ai-runs/J-0001/tool-output/star-sentinel/task.json"
    );

    let invalid_selfcheck = run_cli(
        [
            "sentinel",
            "selfcheck",
            "--project",
            project.to_str().expect("project path"),
            "--json",
        ],
        &config,
    );
    assert_eq!(invalid_selfcheck.exit_code, 2);

    let invalid_option = run_cli(
        [
            "sentinel",
            "gate",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--provider",
            "fake-default",
            "--json",
        ],
        &config,
    );
    assert_eq!(invalid_option.exit_code, 2);

    fs::remove_dir_all(project).ok();
}
