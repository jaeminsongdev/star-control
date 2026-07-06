use crate::tests::{create_job, fs, open_store, temp_project, ui_with_store};

pub(super) fn missing_api_artifact_surfaces_read_only_report_error() {
    let project = temp_project("missing-report");
    let store = open_store(&project);
    create_job(&store, "IMPLEMENTED", "implement", "report");
    let ui = ui_with_store(store);

    let view = ui.job_detail("local", "J-0001").expect("job detail");
    assert_eq!(view["report_summary"]["available"], false);
    assert_eq!(
        view["report_summary"]["error"]["code"],
        "report_read_failed"
    );
    assert_eq!(view["read_only"], true);

    fs::remove_dir_all(project).ok();
}
