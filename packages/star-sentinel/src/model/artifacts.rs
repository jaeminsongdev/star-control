use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateArtifactRefs {
    pub diagnostics_ref: Value,
    pub approval_ref: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewValidation {
    pub command: String,
    pub result: String,
}

impl ReviewValidation {
    pub fn new(command: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            result: result.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPackArtifactRefs {
    pub tool_json_ref: Value,
    pub tool_markdown_ref: Value,
    pub review_json_ref: Value,
    pub review_markdown_ref: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfcheckReport {
    pub ok: bool,
    pub diagnostics: Vec<String>,
}
