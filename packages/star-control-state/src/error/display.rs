use super::StateStoreError;
use std::fmt;

impl fmt::Display for StateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRootNotFound { path } => {
                write!(formatter, "project root does not exist: {}", path.display())
            }
            Self::ProjectRootNotDirectory { path } => {
                write!(
                    formatter,
                    "project root is not a directory: {}",
                    path.display()
                )
            }
            Self::AiRunsNotWritable { path, source } => {
                write!(
                    formatter,
                    ".ai-runs directory is not writable at {}: {}",
                    path.display(),
                    source
                )
            }
            Self::JobNotFound { job_id } => write!(formatter, "job not found: {}", job_id),
            Self::JobAlreadyExists { job_id } => {
                write!(formatter, "job already exists: {}", job_id)
            }
            Self::ArtifactNotFound { path } => {
                write!(formatter, "artifact not found: {}", path.display())
            }
            Self::ArtifactAlreadyExists { path } => {
                write!(formatter, "artifact already exists: {}", path.display())
            }
            Self::InvalidArtifactShape { message } => {
                write!(formatter, "invalid artifact shape: {}", message)
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
            Self::CorruptEventLog {
                path,
                line,
                message,
            } => write!(
                formatter,
                "corrupt event log {} at line {}: {}",
                path.display(),
                line,
                message
            ),
            Self::AtomicWriteFailed { path, source } => {
                write!(
                    formatter,
                    "atomic write failed for {}: {}",
                    path.display(),
                    source
                )
            }
            Self::PathTraversalBlocked { path } => {
                write!(formatter, "path traversal blocked: {}", path)
            }
            Self::PathOutsideJobDirectory { path } => {
                write!(
                    formatter,
                    "path is outside job directory: {}",
                    path.display()
                )
            }
            Self::TerminalStateBlocked { job_id, state } => {
                write!(formatter, "job {} is in terminal state {}", job_id, state)
            }
            Self::InvalidJobId { job_id } => write!(formatter, "invalid job id: {}", job_id),
            Self::InvalidStage { stage } => write!(formatter, "invalid stage: {}", stage),
            Self::JobIdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "artifact job_id mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}
