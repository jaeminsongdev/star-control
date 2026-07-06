use super::super::{create_job, open_store, temp_project};
use std::fs;

#[test]
fn resolves_provider_and_tool_output_dirs_inside_job() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    assert!(store
        .resolve_provider_output_dir("J-0001", "fake-default")
        .expect("provider output dir")
        .ends_with("provider-output/fake-default"));
    assert!(store
        .resolve_tool_output_dir("J-0001", "star-sentinel")
        .expect("tool output dir")
        .ends_with("tool-output/star-sentinel"));

    fs::remove_dir_all(project).ok();
}
