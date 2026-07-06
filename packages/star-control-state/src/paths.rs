use crate::StateStoreError;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) const AI_RUNS_DIR: &str = ".ai-runs";

const CANONICAL_STAGES: &[&str] = &[
    "route",
    "plan",
    "design",
    "implement",
    "validate",
    "review",
    "polish",
    "report",
];

pub(crate) fn ensure_standard_dirs(job_dir: &Path) -> Result<(), StateStoreError> {
    for name in [
        "workspecs",
        "reports",
        "provider-output",
        "tool-output",
        "approvals",
        "review-packs",
        "validation",
        "tmp",
    ] {
        let path = job_dir.join(name);
        fs::create_dir_all(&path)
            .map_err(|source| StateStoreError::AiRunsNotWritable { path, source })?;
    }
    Ok(())
}

pub(crate) fn resolve_inside_job(
    job_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, StateStoreError> {
    let (normalized, _) = normalize_relative_path(relative_path)?;
    let resolved = job_dir.join(normalized);
    if !resolved.starts_with(job_dir) {
        return Err(StateStoreError::PathOutsideJobDirectory { path: resolved });
    }
    Ok(resolved)
}

pub(crate) fn normalized_relative_path(relative_path: &str) -> Result<String, StateStoreError> {
    let (_, normalized) = normalize_relative_path(relative_path)?;
    Ok(normalized)
}

pub(crate) fn validate_job_id(job_id: &str) -> Result<(), StateStoreError> {
    if parse_job_number(job_id).is_some() {
        Ok(())
    } else {
        Err(StateStoreError::InvalidJobId {
            job_id: job_id.to_string(),
        })
    }
}

pub(crate) fn parse_job_number(job_id: &str) -> Option<u64> {
    let suffix = job_id.strip_prefix("J-")?;
    if suffix.len() < 4 || !suffix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

pub(crate) fn validate_stage(stage: &str) -> Result<(), StateStoreError> {
    if CANONICAL_STAGES.contains(&stage) {
        Ok(())
    } else {
        Err(StateStoreError::InvalidStage {
            stage: stage.to_string(),
        })
    }
}

pub(crate) fn validate_safe_name(name: &str) -> Result<(), StateStoreError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains(':')
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name == ".git"
    {
        return Err(StateStoreError::PathTraversalBlocked {
            path: name.to_string(),
        });
    }
    Ok(())
}

fn normalize_relative_path(relative_path: &str) -> Result<(PathBuf, String), StateStoreError> {
    if relative_path.is_empty()
        || relative_path.contains('\0')
        || relative_path.contains(':')
        || Path::new(relative_path).is_absolute()
    {
        return Err(StateStoreError::PathTraversalBlocked {
            path: relative_path.to_string(),
        });
    }

    let mut normalized = PathBuf::new();
    let mut normalized_segments = Vec::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(segment) if segment == ".git" => {
                return Err(StateStoreError::PathTraversalBlocked {
                    path: relative_path.to_string(),
                });
            }
            Component::Normal(segment) => {
                normalized.push(segment);
                normalized_segments.push(segment.to_string_lossy().to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(StateStoreError::PathTraversalBlocked {
                    path: relative_path.to_string(),
                });
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(StateStoreError::PathTraversalBlocked {
            path: relative_path.to_string(),
        });
    }

    Ok((normalized, normalized_segments.join("/")))
}
