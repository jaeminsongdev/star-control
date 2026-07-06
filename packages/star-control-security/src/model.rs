use crate::report::redaction_report;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionFinding {
    kind: String,
    path: String,
    action: String,
}

impl RedactionFinding {
    pub fn new(
        kind: impl Into<String>,
        path: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            path: path.into(),
            action: action.into(),
        }
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "path": self.path,
            "action": self.action
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedactionOutcome {
    pub(crate) value: Value,
    pub(crate) findings: Vec<RedactionFinding>,
}

impl RedactionOutcome {
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub fn findings(&self) -> &[RedactionFinding] {
        &self.findings
    }

    pub fn redacted(&self) -> bool {
        !self.findings.is_empty()
    }

    pub fn report(&self, job_id: &str, artifact_path: &str) -> Value {
        redaction_report(job_id, artifact_path, self.findings())
    }
}
