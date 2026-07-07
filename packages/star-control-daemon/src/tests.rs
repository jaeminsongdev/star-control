mod approval;
mod enqueue;
mod helpers;
mod state;

#[test]
fn opens_default_state_under_config_root_not_project_root() {
    state::opens_default_state_under_config_root_not_project_root();
}

#[test]
fn enqueue_nonterminal_job_records_project_reference_without_copying_artifacts() {
    enqueue::enqueue_nonterminal_job_records_project_reference_without_copying_artifacts();
}

#[test]
fn enqueue_job_can_preserve_provider_instance_paths_for_scheduler() {
    enqueue::enqueue_job_can_preserve_provider_instance_paths_for_scheduler();
}

#[test]
fn terminal_job_is_not_queued() {
    enqueue::terminal_job_is_not_queued();
}

#[test]
fn waiting_approval_requires_approved_response() {
    approval::waiting_approval_requires_approved_response();
}

#[test]
fn non_approved_response_is_not_queued() {
    approval::non_approved_response_is_not_queued();
}

#[test]
fn duplicate_queue_entry_is_rejected() {
    enqueue::duplicate_queue_entry_is_rejected();
}
