use super::DaemonError;
use std::error::Error;

impl Error for DaemonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfigDirectoryFailed { source, .. }
            | Self::StateReadFailed { source, .. }
            | Self::StateWriteFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::StateStore { source } => Some(source),
            Self::SchemaLoadFailed { .. }
            | Self::SchemaValidationFailed { .. }
            | Self::InvalidDaemonState { .. }
            | Self::TerminalJobRejected { .. }
            | Self::ApprovalRequired { .. }
            | Self::ApprovalResponseNotApproved { .. }
            | Self::ApprovalJobMismatch { .. }
            | Self::DuplicateQueuedJob { .. } => None,
        }
    }
}
