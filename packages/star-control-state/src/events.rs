use crate::artifacts::CoreSchema;
use crate::store::ensure_artifact_job_id;
use crate::{StateStore, StateStoreError};
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

impl StateStore {
    pub fn append_event(&self, job_id: &str, event: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(event, job_id)?;
        self.validate_artifact(
            CoreSchema::Event,
            self.job_dir(job_id)?.join("events.jsonl"),
            event,
        )?;
        let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path.clone(),
                source,
            })?;
        serde_json::to_writer(&mut file, event).map_err(|source| StateStoreError::InvalidJson {
            path: events_path.clone(),
            source,
        })?;
        file.write_all(b"\n")
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path,
                source,
            })
    }

    pub fn read_events(&self, job_id: &str) -> Result<Vec<Value>, StateStoreError> {
        let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
        if !events_path.is_file() {
            return Ok(Vec::new());
        }
        let file =
            File::open(&events_path).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path.clone(),
                source,
            })?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line_number = index + 1;
            let line = line.map_err(|source| StateStoreError::CorruptEventLog {
                path: events_path.clone(),
                line: line_number,
                message: source.to_string(),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Value =
                serde_json::from_str(&line).map_err(|source| StateStoreError::CorruptEventLog {
                    path: events_path.clone(),
                    line: line_number,
                    message: source.to_string(),
                })?;
            self.validate_artifact(CoreSchema::Event, events_path.clone(), &event)?;
            events.push(event);
        }
        Ok(events)
    }
}
