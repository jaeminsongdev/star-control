use crate::tests::{create_job, fs, open_store, save_report, temp_project, ui_with_store};

pub(super) fn ui_view_model_redacts_secret_like_values() {
    let project = temp_project("redact");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    save_report(
        &store,
        "validate",
        vec!["Authorization: Bearer sk-test-secret"],
    );
    let ui = ui_with_store(store);

    let view = ui.job_detail("local", "J-0001").expect("job detail");
    let text = serde_json::to_string(&view).expect("view text");
    assert!(!text.contains("sk-test-secret"));
    assert!(text.contains("[REDACTED]"));

    fs::remove_dir_all(project).ok();
}
