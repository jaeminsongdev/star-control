use crate::constants::{
    APPROVAL_FILE, DIAGNOSTICS_FILE, LEDGER_EVENT_SCHEMA, LEDGER_FILE,
    STAR_SENTINEL_TOOL_OUTPUT_DIR,
};
use crate::model::{Decision, EvaluationResult, LedgerEvent, Severity};
use crate::schema_io::validate_against_schema;
use crate::{SentinelError, SentinelTask};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::path::Path;

pub fn build_gate_ledger_event(
    event_id: &str,
    task: &SentinelTask,
    result: &EvaluationResult,
    created_at: &str,
) -> LedgerEvent {
    let severity = match result.decision {
        Decision::AutoPass => Severity::Info,
        Decision::HumanReview => Severity::Warn,
        Decision::Block => Severity::Block,
    };
    let message = match result.decision {
        Decision::AutoPass => "Approval gate auto-passed the task.",
        Decision::HumanReview => "Approval gate requires human review.",
        Decision::Block => "Approval gate blocked the task because a P0 diagnostic was emitted.",
    };

    LedgerEvent::new(
        event_id,
        task.task_id.clone(),
        "GATE_DECIDED",
        "validate",
        severity,
        message,
        created_at,
    )
    .artifacts([
        format!(
            "tool-output/{}/{}",
            STAR_SENTINEL_TOOL_OUTPUT_DIR, APPROVAL_FILE
        ),
        format!(
            "tool-output/{}/{}",
            STAR_SENTINEL_TOOL_OUTPUT_DIR, DIAGNOSTICS_FILE
        ),
    ])
    .metadata(json!({
        "decision": result.decision.as_str(),
        "diagnostic_count": result.diagnostics.len()
    }))
}

pub fn ledger_events_jsonl(events: &[LedgerEvent]) -> Result<String, SentinelError> {
    let mut lines = String::new();
    for event in events {
        let line = serde_json::to_string(&event.to_value()).map_err(|source| {
            SentinelError::InvalidField {
                artifact: LEDGER_FILE.to_string(),
                field: "event".to_string(),
                message: source.to_string(),
            }
        })?;
        lines.push_str(&line);
        lines.push('\n');
    }
    Ok(lines)
}

pub fn validate_ledger_events(
    events: &[LedgerEvent],
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    for (index, event) in events.iter().enumerate() {
        validate_against_schema(
            &event.to_value(),
            schema_root.as_ref(),
            LEDGER_EVENT_SCHEMA,
            &format!("{}[{}]", LEDGER_FILE, index),
        )?;
    }
    Ok(())
}

pub fn write_ledger_artifact(
    store: &StateStore,
    job_id: &str,
    events: &[LedgerEvent],
    schema_root: impl AsRef<Path>,
) -> Result<Value, SentinelError> {
    validate_ledger_events(events, schema_root.as_ref())?;
    let jsonl = ledger_events_jsonl(events)?;
    store
        .write_tool_text(job_id, STAR_SENTINEL_TOOL_OUTPUT_DIR, LEDGER_FILE, &jsonl)
        .map_err(|source| SentinelError::State { source })
}
