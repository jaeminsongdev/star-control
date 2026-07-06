use crate::constants::{COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS, M9_REQUIRED_READINESS_CHECKS};
use crate::error::ReleaseReadinessError;

pub(crate) fn normalized_profile_name(
    profile_name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let profile_name = profile_name.into();
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "release profile name is required".to_string(),
        })
    } else {
        Ok(profile_name.to_string())
    }
}

pub(crate) fn normalized_m9_readiness_check_name(
    name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let name = name.into();
    let name = name.trim();
    if name.is_empty() {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "M9 readiness check name is required".to_string(),
        });
    }
    if !M9_REQUIRED_READINESS_CHECKS.contains(&name) {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: format!("unknown M9 readiness check: {}", name),
        });
    }
    Ok(name.to_string())
}

pub(crate) fn normalize_m9_readiness_blockers(
    blockers: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    normalize_blockers(blockers, "M9 readiness blocker must not be empty")
}

pub(crate) fn normalized_complete_implementation_check_name(
    name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let name = name.into();
    let name = name.trim();
    if name.is_empty() {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "complete implementation check name is required".to_string(),
        });
    }
    if !COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS.contains(&name) {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: format!("unknown complete implementation check: {}", name),
        });
    }
    Ok(name.to_string())
}

pub(crate) fn normalize_complete_implementation_blockers(
    blockers: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    normalize_blockers(
        blockers,
        "complete implementation blocker must not be empty",
    )
}

pub(crate) fn normalize_profile_blockers(
    blockers: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    normalize_blockers(blockers, "release profile blocker must not be empty")
}

fn normalize_blockers(
    blockers: Vec<String>,
    empty_message: &str,
) -> Result<Vec<String>, ReleaseReadinessError> {
    let mut normalized = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        let blocker = blocker.trim();
        if blocker.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: empty_message.to_string(),
            });
        }
        normalized.push(blocker.to_string());
    }
    Ok(normalized)
}
