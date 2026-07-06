use super::super::super::helpers::{body_string, body_string_array};
use serde_json::Value;

const APPROVAL_RESPONSE_APPROVED: &str = "approved";
const APPROVAL_RESPONSE_REJECTED: &str = "rejected";
const APPROVAL_RESPONSE_NEEDS_CHANGES: &str = "needs_changes";
const APPROVAL_RESPONSE_CANCELLED: &str = "cancelled";
const DEFAULT_REVIEWER: &str = "star-control-api";

const SUPPORTED_APPROVAL_RESPONSES: &[&str] = &[
    APPROVAL_RESPONSE_APPROVED,
    APPROVAL_RESPONSE_REJECTED,
    APPROVAL_RESPONSE_NEEDS_CHANGES,
    APPROVAL_RESPONSE_CANCELLED,
];

#[derive(Debug, Clone)]
pub(super) struct ApprovalDecision {
    response: String,
    reason: String,
    reviewer: String,
    constraints: Vec<String>,
}

impl ApprovalDecision {
    pub(super) fn from_body(body: &Value) -> Result<Self, String> {
        let response = body_string(body, "response")?;
        if !SUPPORTED_APPROVAL_RESPONSES.contains(&response.as_str()) {
            return Err(format!("unsupported approval response {}", response));
        }
        Ok(Self {
            response,
            reason: body_string(body, "reason")?,
            reviewer: body_string(body, "reviewer")
                .unwrap_or_else(|_| DEFAULT_REVIEWER.to_string()),
            constraints: body_string_array(body, "constraints")?,
        })
    }

    pub(super) fn response(&self) -> &str {
        &self.response
    }

    pub(super) fn reason(&self) -> &str {
        &self.reason
    }

    pub(super) fn reviewer(&self) -> &str {
        &self.reviewer
    }

    pub(super) fn constraints(&self) -> &[String] {
        &self.constraints
    }
}
