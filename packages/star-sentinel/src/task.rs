use crate::json_fields::{
    optional_string, optional_string_array, required_string, required_string_array,
};
use crate::SentinelError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelTask {
    pub task_id: String,
    pub goal: String,
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub forbidden_change_types: Vec<String>,
    pub required_validation: Vec<String>,
    pub approval_required_changes: Vec<String>,
    pub notes: Option<String>,
}

impl SentinelTask {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        Ok(Self {
            task_id: required_string(value, "task_id", "SentinelTask")?,
            goal: required_string(value, "goal", "SentinelTask")?,
            allowed_paths: required_string_array(value, "allowed_paths", "SentinelTask")?,
            forbidden_paths: required_string_array(value, "forbidden_paths", "SentinelTask")?,
            forbidden_change_types: required_string_array(
                value,
                "forbidden_change_types",
                "SentinelTask",
            )?,
            required_validation: required_string_array(
                value,
                "required_validation",
                "SentinelTask",
            )?,
            approval_required_changes: optional_string_array(
                value,
                "approval_required_changes",
                "SentinelTask",
            )?,
            notes: optional_string(value, "notes", "SentinelTask")?,
        })
    }
}
