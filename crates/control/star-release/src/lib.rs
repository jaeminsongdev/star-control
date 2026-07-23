//! Build-once release and EvaluationRun v2 engines.

pub mod candidate;
pub mod evaluation;
pub mod lifecycle;
pub mod publisher;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReleaseError {
    #[error("release input is invalid")]
    Invalid,
    #[error("release identity conflicts with immutable state")]
    Conflict,
    #[error("release operation is blocked by policy or evidence")]
    Blocked,
    #[error("adapter returned incomplete or mismatched evidence")]
    Adapter,
    #[error("fingerprint calculation failed")]
    Fingerprint,
}
