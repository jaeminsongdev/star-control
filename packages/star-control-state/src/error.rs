mod display;
mod source;

use star_control_schema::ValidationError;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StateStoreError {
    ProjectRootNotFound {
        path: PathBuf,
    },
    ProjectRootNotDirectory {
        path: PathBuf,
    },
    AiRunsNotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
    JobNotFound {
        job_id: String,
    },
    JobAlreadyExists {
        job_id: String,
    },
    ArtifactNotFound {
        path: PathBuf,
    },
    ArtifactAlreadyExists {
        path: PathBuf,
    },
    InvalidArtifactShape {
        message: String,
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
    CorruptEventLog {
        path: PathBuf,
        line: usize,
        message: String,
    },
    AtomicWriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    PathTraversalBlocked {
        path: String,
    },
    PathOutsideJobDirectory {
        path: PathBuf,
    },
    TerminalStateBlocked {
        job_id: String,
        state: String,
    },
    InvalidJobId {
        job_id: String,
    },
    InvalidStage {
        stage: String,
    },
    JobIdMismatch {
        expected: String,
        actual: String,
    },
}
