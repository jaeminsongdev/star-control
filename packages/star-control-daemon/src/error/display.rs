use super::DaemonError;
use std::fmt;

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigDirectoryFailed { path, source } => {
                write!(
                    formatter,
                    "failed to create daemon config directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::StateReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read daemon state {}: {}",
                    path.display(),
                    source
                )
            }
            Self::StateWriteFailed { path, source } => {
                write!(
                    formatter,
                    "failed to write daemon state {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
            Self::InvalidDaemonState { message } => {
                write!(formatter, "invalid daemon state: {}", message)
            }
            Self::StateStore { source } => write!(formatter, "state store error: {}", source),
            Self::TerminalJobRejected { job_id, state } => {
                write!(
                    formatter,
                    "job {} is terminal and cannot be queued: {}",
                    job_id, state
                )
            }
            Self::ApprovalRequired { job_id, path } => {
                write!(
                    formatter,
                    "job {} requires approval response at {}",
                    job_id,
                    path.display()
                )
            }
            Self::ApprovalResponseNotApproved { job_id, response } => {
                write!(
                    formatter,
                    "job {} approval response is not approved: {}",
                    job_id, response
                )
            }
            Self::ApprovalJobMismatch { expected, actual } => {
                write!(
                    formatter,
                    "approval response job_id mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::DuplicateQueuedJob {
                job_id,
                project_root,
            } => {
                write!(
                    formatter,
                    "job {} is already queued for project {}",
                    job_id, project_root
                )
            }
        }
    }
}
