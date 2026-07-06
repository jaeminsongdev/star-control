use crate::error::ReleaseReadinessError;
use std::fs;
use std::path::Path;

pub(crate) fn read_release_text(path: &Path) -> Result<String, ReleaseReadinessError> {
    fs::read_to_string(path).map_err(|source| ReleaseReadinessError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn declared_version_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.is_empty()
        && !trimmed.contains('\n')
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
    {
        return Some(trimmed.to_string());
    }

    text.lines().filter_map(version_assignment_value).next()
}

fn version_assignment_value(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with('#') || !line.starts_with("version") {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    if key.trim() != "version" {
        return None;
    }
    let value = value.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
