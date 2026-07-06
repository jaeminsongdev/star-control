use super::Severity;
use crate::constants::STAR_SENTINEL_TOOL_OUTPUT_DIR;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvent {
    pub event_id: String,
    pub task_id: String,
    pub event_type: String,
    pub stage: String,
    pub severity: Severity,
    pub message: String,
    pub created_at: String,
    pub artifacts: Vec<String>,
    pub metadata: Value,
}

impl LedgerEvent {
    pub fn new(
        event_id: impl Into<String>,
        task_id: impl Into<String>,
        event_type: impl Into<String>,
        stage: impl Into<String>,
        severity: Severity,
        message: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            task_id: task_id.into(),
            event_type: event_type.into(),
            stage: stage.into(),
            severity,
            message: message.into(),
            created_at: created_at.into(),
            artifacts: Vec::new(),
            metadata: json!({}),
        }
    }

    pub fn artifacts(mut self, artifacts: impl IntoIterator<Item = String>) -> Self {
        self.artifacts = artifacts.into_iter().collect();
        self
    }

    pub fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn to_value(&self) -> Value {
        json!({
            "schema_version": "1.0.0",
            "event_id": self.event_id,
            "task_id": self.task_id,
            "event_type": self.event_type,
            "stage": self.stage,
            "severity": self.severity.as_str(),
            "message": self.message,
            "created_at": self.created_at,
            "source": {
                "kind": "tool",
                "name": STAR_SENTINEL_TOOL_OUTPUT_DIR
            },
            "artifacts": self.artifacts,
            "metadata": self.metadata
        })
    }
}
