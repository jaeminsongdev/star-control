use super::yaml::yaml_list_section;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn check_legacy_alias_locations(
    repo_root: &Path,
    manifest_path: &Path,
    diagnostics: &mut Vec<String>,
) {
    let Ok(content) = fs::read_to_string(manifest_path) else {
        return;
    };
    let aliases = yaml_list_section(&content, "legacy_aliases");
    if aliases.is_empty() {
        diagnostics.push("manifest legacy_aliases is empty".to_string());
        return;
    }

    for alias in aliases {
        let mut matches = Vec::new();
        collect_alias_matches(repo_root, &alias, &mut matches);
        for path in matches {
            if path != manifest_path {
                diagnostics.push(format!(
                    "legacy alias {} appears outside manifest at {}",
                    alias,
                    path.display()
                ));
            }
        }
    }
}

fn collect_alias_matches(root: &Path, alias: &str, matches: &mut Vec<PathBuf>) {
    if should_skip_selfcheck_path(root) {
        return;
    }
    let Ok(metadata) = fs::metadata(root) else {
        return;
    };
    if metadata.is_dir() {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            collect_alias_matches(&entry.path(), alias, matches);
        }
        return;
    }

    if !is_text_like_path(root) {
        return;
    }
    if let Ok(content) = fs::read_to_string(root) {
        if content.contains(alias) {
            matches.push(root.to_path_buf());
        }
    }
}

fn should_skip_selfcheck_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | ".ai-runs"))
}

fn is_text_like_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("md" | "txt" | "json" | "jsonl" | "yaml" | "yml" | "toml" | "rs" | "py" | "ps1")
    )
}
