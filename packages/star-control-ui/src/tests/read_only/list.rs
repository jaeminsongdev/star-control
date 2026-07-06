use crate::tests::{create_job, fs, open_store, temp_project, ui_with_store};

pub(super) fn job_list_builds_schema_valid_views_from_api() {
    let project = temp_project("list");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    let ui = ui_with_store(store);

    let view = ui.job_list("local").expect("job list");
    assert_eq!(view["view"], "job_list");
    assert_eq!(view["read_only"], true);
    assert_eq!(view["mutations_enabled"], false);
    let job = &view["jobs"][0];
    assert_eq!(job["job_id"], "J-0001");
    assert_eq!(job["approval_required"], true);
    assert_eq!(job["next_action"], "approve");
    ui.validate_job_view(job).expect("schema-valid job view");

    fs::remove_dir_all(project).ok();
}
