use crate::artifacts::timestamp_string;
use crate::constants::{SCHEMA_VERSION, TERMINAL_STATES};
use crate::paths::{ensure_standard_dirs, parse_job_number, validate_job_id};
use crate::{JobSummary, StateStore, StateStoreError};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

impl StateStore {
    pub fn allocate_job_id(&self) -> Result<String, StateStoreError> {
        let mut highest = 0_u64;
        for entry in fs::read_dir(&self.ai_runs_dir).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            }
        })? {
            let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            })?;
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some(number) = parse_job_number(&name) {
                highest = highest.max(number);
            }
        }

        Ok(format!("J-{:04}", highest + 1))
    }

    pub fn create_job(
        &self,
        request_text: impl Into<String>,
        entrypoint: impl Into<String>,
        user_constraints: Vec<String>,
    ) -> Result<Value, StateStoreError> {
        let job_id = self.allocate_job_id()?;
        let job_dir = self.create_job_dir(&job_id)?;
        ensure_standard_dirs(&job_dir)?;

        let timestamp = timestamp_string();
        let job = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "project_root": self.project_root.display().to_string(),
            "request_text": request_text.into(),
            "created_at": timestamp,
            "updated_at": timestamp,
            "entrypoint": entrypoint.into(),
            "state": "REQUESTED",
            "user_constraints": user_constraints,
        });

        self.save_job(&job_id, &job)?;
        self.append_event(
            &job_id,
            &json!({
                "schema_version": SCHEMA_VERSION,
                "event_id": format!("{}-0001", job_id),
                "job_id": job_id,
                "type": "JOB_CREATED",
                "created_at": timestamp,
                "state": "REQUESTED",
                "message": "Job created",
                "artifact_paths": ["job.json"],
                "details": {}
            }),
        )?;

        Ok(job)
    }

    pub fn create_job_dir(&self, job_id: &str) -> Result<PathBuf, StateStoreError> {
        validate_job_id(job_id)?;
        let job_dir = self.ai_runs_dir.join(job_id);
        if job_dir.exists() {
            return Err(StateStoreError::JobAlreadyExists {
                job_id: job_id.to_string(),
            });
        }
        fs::create_dir_all(&job_dir).map_err(|source| StateStoreError::AiRunsNotWritable {
            path: job_dir.clone(),
            source,
        })?;
        Ok(job_dir)
    }

    pub fn job_dir(&self, job_id: &str) -> Result<PathBuf, StateStoreError> {
        validate_job_id(job_id)?;
        let job_dir = self.ai_runs_dir.join(job_id);
        if !job_dir.is_dir() {
            return Err(StateStoreError::JobNotFound {
                job_id: job_id.to_string(),
            });
        }
        Ok(job_dir)
    }

    pub fn list_jobs(&self) -> Result<Vec<JobSummary>, StateStoreError> {
        let mut jobs = Vec::new();
        for entry in fs::read_dir(&self.ai_runs_dir).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            }
        })? {
            let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            })?;
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let Some(job_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !job_id.starts_with("J-") {
                continue;
            }
            jobs.push(self.job_summary(&job_id));
        }
        jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        Ok(jobs)
    }

    pub fn ensure_resume_allowed(&self, job_id: &str) -> Result<(), StateStoreError> {
        let state = self.load_state(job_id)?;
        let state_value = state
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if TERMINAL_STATES.contains(&state_value.as_str()) {
            return Err(StateStoreError::TerminalStateBlocked {
                job_id: job_id.to_string(),
                state: state_value,
            });
        }
        self.read_events(job_id)?;
        Ok(())
    }
}
