pub(super) fn artifact_path_for_stage(stage: &str) -> String {
    format!("workspecs/{}.json", stage)
}
