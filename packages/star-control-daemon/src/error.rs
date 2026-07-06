mod display;
mod source;

use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::path::PathBuf;

#[derive(Debug)]
pub enum DaemonError {
    ConfigDirectoryFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    StateReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    StateWriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
    InvalidDaemonState {
        message: String,
    },
    StateStore {
        source: StateStoreError,
    },
    TerminalJobRejected {
        job_id: String,
        state: String,
    },
    ApprovalRequired {
        job_id: String,
        path: PathBuf,
    },
    ApprovalResponseNotApproved {
        job_id: String,
        response: String,
    },
    ApprovalJobMismatch {
        expected: String,
        actual: String,
    },
    DuplicateQueuedJob {
        job_id: String,
        project_root: String,
    },
}

impl From<StateStoreError> for DaemonError {
    fn from(source: StateStoreError) -> Self {
        Self::StateStore { source }
    }
}
