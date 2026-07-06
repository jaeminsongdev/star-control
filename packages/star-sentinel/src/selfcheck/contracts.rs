use crate::readers::read_p0_rule_registry;
use star_control_schema::load_schema;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(super) fn check_p0_registry(
    registry_path: &Path,
    schema_root: &Path,
    diagnostics: &mut Vec<String>,
) {
    match read_p0_rule_registry(registry_path, schema_root) {
        Ok(registry) => {
            let mut seen = HashSet::new();
            for rule in registry.rules {
                if !seen.insert(rule.rule_id.clone()) {
                    diagnostics.push(format!("duplicate rule id {}", rule.rule_id));
                }
            }
        }
        Err(error) => diagnostics.push(format!("p0 registry check failed: {}", error)),
    }
}

pub(super) fn check_schema_files(schema_root: &Path, diagnostics: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(schema_root) else {
        diagnostics.push(format!(
            "schema directory unreadable {}",
            schema_root.display()
        ));
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            if let Err(error) = load_schema(&path) {
                diagnostics.push(format!("schema parse failed {}: {}", path.display(), error));
            }
        }
    }
}

pub(super) fn check_fixture_files(fixtures_root: &Path, diagnostics: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(fixtures_root) else {
        diagnostics.push(format!(
            "fixture directory unreadable {}",
            fixtures_root.display()
        ));
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            diagnostics.push(format!("fixture unreadable {}", path.display()));
            continue;
        };
        for required in ["case_id:", "rule_id:", "input:", "expected:", "decision:"] {
            if !content.contains(required) {
                diagnostics.push(format!("fixture {} missing {}", path.display(), required));
            }
        }
    }
}
