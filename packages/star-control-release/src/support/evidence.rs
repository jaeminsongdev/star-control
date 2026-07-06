use crate::error::ReleaseReadinessError;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn evidence_paths(path: String) -> Vec<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}

pub(crate) fn display_or_empty(value: &str) -> &str {
    if value.is_empty() {
        "<empty>"
    } else {
        value
    }
}

pub(crate) fn resolve_project_file(
    project_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ReleaseReadinessError> {
    let normalized = normalized_evidence_path(relative_path)?;
    let root =
        fs::canonicalize(project_root).map_err(|source| ReleaseReadinessError::ReadFailed {
            path: project_root.to_path_buf(),
            source,
        })?;
    let path = root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR));
    let canonical =
        fs::canonicalize(&path).map_err(|source| ReleaseReadinessError::ReadFailed {
            path: path.clone(),
            source,
        })?;
    if !canonical.starts_with(&root) {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!(
                "release evidence path escapes project root: {}",
                relative_path
            ),
        });
    }
    if !canonical.is_file() {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!("release evidence path is not a file: {}", relative_path),
        });
    }
    Ok(canonical)
}

pub(crate) fn normalized_evidence_path(path: &str) -> Result<String, ReleaseReadinessError> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(':')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!("unsafe release evidence path: {}", display_or_empty(&path)),
        });
    }
    Ok(path)
}

pub(crate) fn normalize_evidence_paths(
    paths: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    paths
        .into_iter()
        .map(|path| normalized_evidence_path(&path))
        .collect()
}
