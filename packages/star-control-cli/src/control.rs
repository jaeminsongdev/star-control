mod approve;
mod cancel;
mod helpers;
mod resume;

pub(crate) use approve::approve_command;
pub(crate) use cancel::cancel_command;
pub(crate) use resume::resume_command;

use serde_json::Value;

const APPROVAL_REQUEST_PATH: &str = "approvals/approval-request.json";
const APPROVAL_RESPONSE_FILE: &str = "approval-response.json";
const APPROVAL_RESPONSE_PATH: &str = "approvals/approval-response.json";
const RUN_STATE_PATH: &str = "run-state.json";
const CLI_REVIEWER: &str = "star-control-cli";

const RESPONSE_APPROVED: &str = "approved";
const RESPONSE_REJECTED: &str = "rejected";
const RESPONSE_NEEDS_CHANGES: &str = "needs_changes";
const RESPONSE_CANCELLED: &str = "cancelled";

const STATE_WAITING_APPROVAL: &str = "WAITING_APPROVAL";
const STATE_CANCELLED: &str = "CANCELLED";
const STATE_BLOCKED: &str = "BLOCKED";
const STATE_VALIDATED: &str = "VALIDATED";
const STATE_FAILED: &str = "FAILED";

const NEXT_ACTION_RESUME: &str = "resume";
const NEXT_ACTION_STOP: &str = "stop";
const NEXT_ACTION_REVISE: &str = "revise";
const NEXT_ACTION_REPORT: &str = "report";

const EVENT_APPROVAL_RECORDED: &str = "APPROVAL_RECORDED";
const EVENT_STATE_CHANGED: &str = "STATE_CHANGED";

const COMMAND_APPROVE: &str = "approve";
const COMMAND_CANCEL: &str = "cancel";
const COMMAND_RESUME: &str = "resume";

const STAGE_ROUTE: &str = "route";
const STAGE_PLAN: &str = "plan";
const STAGE_DESIGN: &str = "design";
const STAGE_IMPLEMENT: &str = "implement";
const STAGE_VALIDATE: &str = "validate";
const STAGE_REVIEW: &str = "review";
const STAGE_POLISH: &str = "polish";
const STAGE_REPORT: &str = "report";

const ALLOWED_NEXT_STAGES: &[(&str, &str)] = &[
    (STAGE_ROUTE, STAGE_PLAN),
    (STAGE_PLAN, STAGE_DESIGN),
    (STAGE_DESIGN, STAGE_IMPLEMENT),
    (STAGE_IMPLEMENT, STAGE_VALIDATE),
    (STAGE_VALIDATE, STAGE_REPORT),
    (STAGE_REVIEW, STAGE_POLISH),
    (STAGE_POLISH, STAGE_REPORT),
];

#[derive(Debug, Clone)]
struct CliEvent {
    event_id: String,
    event_type: &'static str,
    state: String,
    stage: String,
    message: &'static str,
    artifact_paths: Vec<String>,
    details: Value,
}
