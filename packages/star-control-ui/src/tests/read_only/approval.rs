use crate::tests::{create_job, fs, open_store, save_report, temp_project, ui_with_store};

pub(super) fn waiting_approval_view_exposes_approval_path_without_mutation() {
    let project = temp_project("approval");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    save_report(&store, "validate", Vec::new());
    let ui = ui_with_store(store);

    let view = ui.job_detail("local", "J-0001").expect("job detail");
    let approval = &view["approval_request_viewer"];
    assert_eq!(approval["required"], true);
    assert_eq!(approval["mutations_enabled"], false);
    assert_eq!(approval["mutation_surface"], "api_or_cli");
    assert_eq!(
        approval["response_contract"],
        "approval-response.schema.json"
    );
    assert_eq!(approval["paths"][0], "approvals/approval-request.json");
    assert!(!project
        .join(".ai-runs/J-0001/approvals/approval-response.json")
        .exists());

    fs::remove_dir_all(project).ok();
}
