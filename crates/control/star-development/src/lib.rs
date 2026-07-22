//! Deterministic M5-M9 development maintenance engines.

pub mod compatibility;
pub mod coordination;
pub mod maintenance;
pub mod managed_registry;
pub mod migration;

use star_contracts::Sha256Hash;
use star_domain::versioned_fingerprint;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DevelopmentError {
    #[error("input is invalid")]
    Invalid,
    #[error("input is stale, partial, or unverified")]
    Unverified,
    #[error("identity or lifecycle conflicts")]
    Conflict,
    #[error("operation is blocked by policy")]
    Blocked,
    #[error("adapter operation failed")]
    Adapter,
    #[error("fingerprint calculation failed")]
    Fingerprint,
}

pub(crate) fn fingerprint<T: serde::Serialize>(
    domain: &str,
    value: &T,
) -> Result<Sha256Hash, DevelopmentError> {
    versioned_fingerprint(domain, 1, value).map_err(|_| DevelopmentError::Fingerprint)
}

pub(crate) fn placeholder() -> Sha256Hash {
    Sha256Hash::digest(b"unsealed")
}

pub(crate) fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.starts_with('/')
        && !value.contains(':')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

pub(crate) fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
