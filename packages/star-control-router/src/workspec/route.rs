use super::path::artifact_path_for_stage;
use super::role::role_for_stage;
use crate::analysis::PolicyProfile;
use serde_json::{json, Map, Value};

pub(crate) fn assignments_for_stages(
    stages: &[&str],
    provider_instance_id: &str,
    profile: PolicyProfile,
) -> Value {
    let mut assignments = Map::new();
    for stage in stages.iter().filter(|stage| **stage != "route") {
        assignments.insert(
            (*stage).to_string(),
            json!({
                "role": role_for_stage(stage),
                "provider": provider_instance_id,
                "profile": profile.as_str()
            }),
        );
    }
    Value::Object(assignments)
}

pub(crate) fn workspec_paths_for_stages(stages: &[&str]) -> Value {
    let mut paths = Map::new();
    for stage in stages.iter().filter(|stage| **stage != "route") {
        paths.insert(
            (*stage).to_string(),
            Value::String(artifact_path_for_stage(stage)),
        );
    }
    Value::Object(paths)
}

pub(crate) fn decision_id(job_id: &str) -> String {
    format!("{}-route", job_id.to_lowercase())
}

pub(crate) fn summary(request_text: &str) -> String {
    let trimmed = request_text.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let mut output: String = trimmed.chars().take(93).collect();
    output.push_str("...");
    output
}
