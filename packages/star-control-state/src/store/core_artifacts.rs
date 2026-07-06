use crate::artifacts::CoreSchema;
use crate::paths::{validate_job_id, validate_safe_name, validate_stage};
use crate::{StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn save_job(&self, job_id: &str, job: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(job, job_id)?;
        self.write_json_artifact(job_id, "job.json", CoreSchema::Job, job)
    }

    pub fn load_job(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "job.json", CoreSchema::Job)
    }

    pub fn save_state(&self, job_id: &str, state: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(state, job_id)?;
        self.write_json_artifact(job_id, "run-state.json", CoreSchema::RunState, state)
    }

    pub fn load_state(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "run-state.json", CoreSchema::RunState)
    }

    pub fn save_route(&self, job_id: &str, route: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(route, job_id)?;
        self.write_json_artifact(job_id, "route.json", CoreSchema::Route, route)
    }

    pub fn load_route(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "route.json", CoreSchema::Route)
    }

    pub fn save_workspec(
        &self,
        job_id: &str,
        stage: &str,
        workspec: &Value,
    ) -> Result<(), StateStoreError> {
        validate_stage(stage)?;
        ensure_artifact_job_id(workspec, job_id)?;
        self.write_json_artifact(
            job_id,
            &format!("workspecs/{}.json", stage),
            CoreSchema::WorkSpec,
            workspec,
        )
    }

    pub fn load_workspec(&self, job_id: &str, stage: &str) -> Result<Value, StateStoreError> {
        validate_stage(stage)?;
        self.read_json_artifact(
            job_id,
            &format!("workspecs/{}.json", stage),
            CoreSchema::WorkSpec,
        )
    }

    pub fn save_report(
        &self,
        job_id: &str,
        name: &str,
        report: &Value,
    ) -> Result<(), StateStoreError> {
        validate_safe_name(name)?;
        ensure_artifact_job_id(report, job_id)?;
        self.write_json_artifact(
            job_id,
            &format!("reports/{}.json", name),
            CoreSchema::Report,
            report,
        )
    }

    pub fn load_report(&self, job_id: &str, name: &str) -> Result<Value, StateStoreError> {
        validate_safe_name(name)?;
        self.read_json_artifact(
            job_id,
            &format!("reports/{}.json", name),
            CoreSchema::Report,
        )
    }
}

pub(crate) fn ensure_artifact_job_id(value: &Value, expected: &str) -> Result<(), StateStoreError> {
    validate_job_id(expected)?;
    let actual = value.get("job_id").and_then(Value::as_str).ok_or_else(|| {
        StateStoreError::JobIdMismatch {
            expected: expected.to_string(),
            actual: "<missing>".to_string(),
        }
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(StateStoreError::JobIdMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}
