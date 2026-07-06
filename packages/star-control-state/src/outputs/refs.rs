use crate::artifacts::CoreSchema;
use crate::constants::SCHEMA_VERSION;
use crate::paths::{normalized_relative_path, validate_safe_name};
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::{json, Value};
use std::path::PathBuf;

impl StateStore {
    pub fn artifact_ref(
        &self,
        job_id: &str,
        relative_path: &str,
        kind: ArtifactKind,
        producer: &str,
        schema_path: Option<&str>,
        description: Option<&str>,
    ) -> Result<Value, StateStoreError> {
        let normalized_path = normalized_relative_path(relative_path)?;
        self.resolve_job_path(job_id, &normalized_path)?;
        let artifact_ref = json!({
            "schema_version": SCHEMA_VERSION,
            "path": normalized_path,
            "kind": kind.as_str(),
            "producer": producer,
            "schema_path": schema_path,
            "description": description.unwrap_or("")
        });
        self.validate_artifact(
            CoreSchema::ArtifactRef,
            self.job_dir(job_id)?.join("artifact-ref.json"),
            &artifact_ref,
        )?;
        Ok(artifact_ref)
    }

    pub fn register_artifact_ref(
        &self,
        state: &mut Value,
        key: &str,
        artifact_ref: &Value,
    ) -> Result<(), StateStoreError> {
        validate_safe_name(key)?;
        self.validate_artifact(
            CoreSchema::ArtifactRef,
            PathBuf::from("artifact-ref.json"),
            artifact_ref,
        )?;
        let Some(state_object) = state.as_object_mut() else {
            return Err(StateStoreError::InvalidArtifactShape {
                message: "RunState must be a JSON object".to_string(),
            });
        };
        let artifacts = state_object
            .entry("artifacts")
            .or_insert_with(|| Value::Object(Default::default()));
        let Some(artifacts_object) = artifacts.as_object_mut() else {
            return Err(StateStoreError::InvalidArtifactShape {
                message: "RunState artifacts must be a JSON object".to_string(),
            });
        };
        artifacts_object.insert(key.to_string(), artifact_ref.clone());
        Ok(())
    }
}
