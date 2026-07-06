use serde_json::{json, Value};

pub(crate) fn artifact_sections(artifacts: &Value) -> Vec<Value> {
    let mut sections = Vec::new();
    if let Some(object) = artifacts.as_object() {
        for (section, value) in object {
            let paths = artifact_paths(value);
            if !paths.is_empty() {
                sections.push(json!({
                    "section": normalize_section(section, &paths),
                    "source_key": section,
                    "paths": paths
                }));
            }
        }
    }
    sections
}

fn normalize_section(section: &str, paths: &[String]) -> String {
    if section.contains("provider") || paths.iter().any(|path| path.contains("provider-output/")) {
        "provider_output".to_string()
    } else if section.contains("validation")
        || paths.iter().any(|path| path.contains("validation/"))
    {
        "validation".to_string()
    } else if section.contains("approval") || paths.iter().any(|path| path.contains("approvals/")) {
        "approval_request".to_string()
    } else if section.contains("review") || paths.iter().any(|path| path.contains("review-packs/"))
    {
        "review_pack".to_string()
    } else {
        section.to_string()
    }
}

pub(super) fn artifact_paths(artifacts: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_artifact_paths(artifacts, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn collect_artifact_paths(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                paths.push(path.to_string());
            }
            for value in object.values() {
                collect_artifact_paths(value, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_artifact_paths(item, paths);
            }
        }
        _ => {}
    }
}

pub(crate) fn paths_for_section(sections: &[Value], section_name: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for section in sections {
        if section.get("section").and_then(Value::as_str) == Some(section_name) {
            if let Some(section_paths) = section.get("paths").and_then(Value::as_array) {
                paths.extend(
                    section_paths
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            }
        }
    }
    paths
}
