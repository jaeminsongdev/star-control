use crate::tests::{create_job, fs, open_store, save_report, temp_project, ui_with_store};

pub(super) fn job_detail_includes_timeline_report_and_artifacts_without_writes() {
    let project = temp_project("detail");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    save_report(&store, "validate", Vec::new());
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    let before_state = fs::read_to_string(&state_path).expect("read state before");
    let ui = ui_with_store(store);

    let view = ui.job_detail("local", "J-0001").expect("job detail");
    assert_eq!(view["view"], "job_detail");
    assert_eq!(view["read_only"], true);
    assert_eq!(view["job"]["latest_event"], "J-0001-0002");
    assert!(view["timeline"]["events"].as_array().expect("events").len() >= 2);
    assert_eq!(view["report_summary"]["available"], true);
    assert_eq!(view["release_readiness_viewer"]["available"], false);
    assert_eq!(
        view["release_readiness_viewer"]["error"]["code"],
        "release_readiness_not_found"
    );
    assert_eq!(
        view["provider_output_viewer"]["paths"][0],
        "provider-output/fake-default/output.json"
    );
    assert_eq!(
        view["validation_result_viewer"]["paths"][0],
        "validation/validation-decision.json"
    );
    assert_eq!(
        view["review_pack_viewer"]["paths"][0],
        "review-packs/review_pack.md"
    );

    let after_state = fs::read_to_string(&state_path).expect("read state after");
    assert_eq!(after_state, before_state);
    assert!(!project.join(".ai-runs/J-0001/ui-view.json").exists());

    fs::remove_dir_all(project).ok();
}
