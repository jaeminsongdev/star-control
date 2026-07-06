use super::issue::{recovery_issue_from_error, RecoveryIssue};
use crate::{StateStore, StateStoreError, SCHEMA_VERSION};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryInspection {
    pub job_id: String,
    pub mode: String,
    pub status: String,
    pub manual_followup_required: bool,
    pub destructive_actions_performed: bool,
    pub issues: Vec<RecoveryIssue>,
}

impl RecoveryInspection {
    fn inspect_only(job_id: impl Into<String>, issues: Vec<RecoveryIssue>) -> Self {
        let manual_followup_required = !issues.is_empty();
        Self {
            job_id: job_id.into(),
            mode: "inspect_only".to_string(),
            status: if manual_followup_required {
                "needs_recovery".to_string()
            } else {
                "ok".to_string()
            },
            manual_followup_required,
            destructive_actions_performed: false,
            issues,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": self.job_id,
            "mode": self.mode,
            "status": self.status,
            "manual_followup_required": self.manual_followup_required,
            "destructive_actions_performed": self.destructive_actions_performed,
            "issues": self.issues.iter().map(RecoveryIssue::to_value).collect::<Vec<_>>()
        })
    }
}

impl StateStore {
    pub fn inspect_recovery(&self, job_id: &str) -> Result<RecoveryInspection, StateStoreError> {
        self.job_dir(job_id)?;
        let mut issues = Vec::new();

        if let Err(error) = self.load_job(job_id) {
            issues.push(recovery_issue_from_error("job.json", &error));
        }
        if let Err(error) = self.load_state(job_id) {
            issues.push(recovery_issue_from_error("run-state.json", &error));
        }
        if let Err(error) = self.read_events(job_id) {
            issues.push(recovery_issue_from_error("events.jsonl", &error));
        } else {
            let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
            if !events_path.is_file() {
                issues.push(RecoveryIssue::new(
                    "events.jsonl",
                    "missing_required_file",
                    "block",
                    "required event log is missing",
                    "inspect the job and recreate only through an explicit recovery command",
                ));
            }
        }

        issues.extend(self.tmp_file_issues(job_id)?);
        Ok(RecoveryInspection::inspect_only(job_id, issues))
    }
}
