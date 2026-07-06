pub(crate) const SCHEMA_VERSION: &str = "1.0.0";
pub(crate) const API_RESPONSE_SCHEMA: &str = "api-response.schema.json";
pub(crate) const APPROVAL_REQUEST_SCHEMA: &str = "approval-request.schema.json";
pub(crate) const APPROVAL_RESPONSE_SCHEMA: &str = "approval-response.schema.json";
pub(crate) const DEFAULT_REPORT_STAGE: &str = "implement";
pub(crate) const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];
pub(crate) const CANONICAL_STAGES: &[&str] = &[
    "route",
    "plan",
    "design",
    "implement",
    "validate",
    "review",
    "polish",
    "report",
];
