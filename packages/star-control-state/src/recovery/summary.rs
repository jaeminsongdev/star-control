use crate::StateStore;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSummary {
    pub job_id: String,
    pub state: Option<String>,
    pub current_stage: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub summary: Option<String>,
    pub corrupt: bool,
    pub corrupt_reason: Option<String>,
}

impl StateStore {
    pub(crate) fn job_summary(&self, job_id: &str) -> JobSummary {
        match self.load_job(job_id) {
            Ok(job) => {
                let state = self.load_state(job_id).ok();
                JobSummary {
                    job_id: job_id.to_string(),
                    state: state
                        .as_ref()
                        .and_then(|state| state.get("state"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    current_stage: state
                        .as_ref()
                        .and_then(|state| state.get("current_stage"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    created_at: job
                        .get("created_at")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    updated_at: state
                        .as_ref()
                        .and_then(|state| state.get("updated_at"))
                        .and_then(Value::as_str)
                        .or_else(|| job.get("updated_at").and_then(Value::as_str))
                        .map(str::to_owned),
                    summary: job
                        .get("request_text")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    corrupt: false,
                    corrupt_reason: None,
                }
            }
            Err(error) => JobSummary {
                job_id: job_id.to_string(),
                state: None,
                current_stage: None,
                created_at: None,
                updated_at: None,
                summary: None,
                corrupt: true,
                corrupt_reason: Some(error.to_string()),
            },
        }
    }
}
