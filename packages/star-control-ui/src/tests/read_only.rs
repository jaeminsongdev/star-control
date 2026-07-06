mod approval;
mod detail;
mod errors;
mod list;
mod redaction;
mod release;

#[test]
fn job_list_builds_schema_valid_views_from_api() {
    list::job_list_builds_schema_valid_views_from_api();
}

#[test]
fn job_detail_includes_timeline_report_and_artifacts_without_writes() {
    detail::job_detail_includes_timeline_report_and_artifacts_without_writes();
}

#[test]
fn release_readiness_viewer_reads_api_artifact_without_mutation() {
    release::release_readiness_viewer_reads_api_artifact_without_mutation();
}

#[test]
fn waiting_approval_view_exposes_approval_path_without_mutation() {
    approval::waiting_approval_view_exposes_approval_path_without_mutation();
}

#[test]
fn ui_view_model_redacts_secret_like_values() {
    redaction::ui_view_model_redacts_secret_like_values();
}

#[test]
fn missing_api_artifact_surfaces_read_only_report_error() {
    errors::missing_api_artifact_surfaces_read_only_report_error();
}
