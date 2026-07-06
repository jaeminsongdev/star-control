mod io;
mod model;
mod validation;

use crate::constants::RELEASE_READINESS_PATH;
use crate::error::ReleaseReadinessError;
use serde_json::Value;
use star_control_state::{ArtifactKind, StateStore};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ReleaseReadinessWriter {
    schema_root: PathBuf,
}

impl ReleaseReadinessWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn check(
        &self,
        name: impl Into<String>,
        status: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Value {
        model::check(name, status, evidence_paths)
    }

    pub fn reserved(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
    ) -> Value {
        self.readiness(
            release_id,
            target,
            version,
            "reserved",
            model::reserved_checks(),
            model::reserved_blockers(),
        )
    }

    pub fn not_ready(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        checks: Vec<Value>,
        blockers: Vec<String>,
    ) -> Value {
        self.readiness(release_id, target, version, "not_ready", checks, blockers)
    }

    pub fn readiness(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        status: impl Into<String>,
        checks: Vec<Value>,
        blockers: Vec<String>,
    ) -> Value {
        model::readiness(release_id, target, version, status, checks, blockers)
    }

    pub fn write(
        &self,
        store: &StateStore,
        job_id: &str,
        readiness: &Value,
    ) -> Result<Value, ReleaseReadinessError> {
        self.validate_readiness(readiness)?;
        let path = store.resolve_job_path(job_id, RELEASE_READINESS_PATH)?;
        io::write_new_json(&path, readiness)?;
        store
            .artifact_ref(
                job_id,
                RELEASE_READINESS_PATH,
                ArtifactKind::Other,
                "star-control-release",
                Some("specs/schemas/release-readiness.schema.json"),
                Some("release readiness artifact"),
            )
            .map_err(ReleaseReadinessError::from)
    }

    pub fn read(
        &self,
        store: &StateStore,
        job_id: &str,
    ) -> Result<Option<Value>, ReleaseReadinessError> {
        let path = store.resolve_job_path(job_id, RELEASE_READINESS_PATH)?;
        if !path.is_file() {
            return Ok(None);
        }
        let value = io::read_json(&path)?;
        self.validate_readiness(&value)?;
        Ok(Some(value))
    }

    pub fn validate_readiness(&self, readiness: &Value) -> Result<(), ReleaseReadinessError> {
        validation::validate_readiness(&self.schema_root, readiness)
    }
}
