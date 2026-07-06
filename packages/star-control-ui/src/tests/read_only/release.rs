use crate::tests::{
    create_job, fs, open_store, save_report, temp_project, ui_with_store, write_release_readiness,
};

pub(super) fn release_readiness_viewer_reads_api_artifact_without_mutation() {
    let project = temp_project("release-readiness");
    let store = open_store(&project);
    create_job(&store, "DONE", "report", "none");
    save_report(&store, "report", Vec::new());
    let readiness_path = write_release_readiness(&project);
    let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");
    let ui = ui_with_store(store);

    let readiness = ui
        .release_readiness("local", "J-0001")
        .expect("release readiness view");
    assert_eq!(readiness["available"], true);
    assert_eq!(readiness["read_only"], true);
    assert_eq!(readiness["mutations_enabled"], false);
    assert_eq!(readiness["release_actions_enabled"], false);
    assert_eq!(
        readiness["readiness_path"],
        ".ai-runs/J-0001/release/release-readiness.json"
    );
    assert_eq!(readiness["status"], "reserved");
    assert_eq!(readiness["checks"][0]["name"], "release-profile-passed");
    assert_eq!(
        readiness["blockers"][0],
        "release approval/signing/publish/deploy automation remains reserved"
    );

    let detail = ui.job_detail("local", "J-0001").expect("job detail");
    assert_eq!(detail["release_readiness_viewer"], readiness);
    let after_readiness = fs::read_to_string(&readiness_path).expect("read readiness after");
    assert_eq!(after_readiness, before_readiness);
    assert!(!project
        .join(".ai-runs/J-0001/release/release-action.json")
        .exists());

    fs::remove_dir_all(project).ok();
}
