use super::yaml::yaml_list_section;
use crate::constants::REQUIRED_MANIFEST_OUTPUTS;
use std::fs;
use std::path::Path;

pub(super) fn check_manifest_outputs(manifest_path: &Path, diagnostics: &mut Vec<String>) {
    let Ok(content) = fs::read_to_string(manifest_path) else {
        diagnostics.push(format!(
            "missing or unreadable manifest {}",
            manifest_path.display()
        ));
        return;
    };
    let outputs = yaml_list_section(&content, "outputs");
    for required in REQUIRED_MANIFEST_OUTPUTS {
        if !outputs.iter().any(|output| output == required) {
            diagnostics.push(format!("manifest outputs missing {}", required));
        }
    }
}
