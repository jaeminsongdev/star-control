use crate::constants::STAR_SENTINEL_TOOL_OUTPUT_DIR;
use crate::constants::{APPROVAL_FILE, APPROVAL_SCHEMA, DIAGNOSTICS_FILE, DIAGNOSTIC_SCHEMA};
use crate::model::{Decision, Diagnostic, EvaluationResult, GateArtifactRefs};
use crate::schema_io::validate_against_schema;
use crate::{SentinelError, SentinelTask};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::collections::BTreeSet;
use std::path::Path;

pub fn build_diagnostics_artifact(result: &EvaluationResult) -> Value {
    Value::Array(
        result
            .diagnostics
            .iter()
            .map(Diagnostic::to_value)
            .collect(),
    )
}

pub fn build_approval_artifact(task: &SentinelTask, result: &EvaluationResult) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task.task_id,
        "decision": result.decision.as_str(),
        "reasons": approval_reasons(result),
        "diagnostics": build_diagnostics_artifact(result),
        "required_human_actions": required_human_actions(result.decision)
    })
}

pub fn validate_diagnostics_artifact(
    diagnostics: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    let Value::Array(items) = diagnostics else {
        return Err(SentinelError::InvalidField {
            artifact: DIAGNOSTICS_FILE.to_string(),
            field: "$".to_string(),
            message: "expected array of diagnostic objects".to_string(),
        });
    };

    for (index, diagnostic) in items.iter().enumerate() {
        validate_against_schema(
            diagnostic,
            schema_root.as_ref(),
            DIAGNOSTIC_SCHEMA,
            &format!("{}[{}]", DIAGNOSTICS_FILE, index),
        )?;
    }

    Ok(())
}

pub fn validate_approval_artifact(
    approval: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    validate_against_schema(
        approval,
        schema_root.as_ref(),
        APPROVAL_SCHEMA,
        APPROVAL_FILE,
    )
}

pub fn write_gate_artifacts(
    store: &StateStore,
    job_id: &str,
    task: &SentinelTask,
    result: &EvaluationResult,
    schema_root: impl AsRef<Path>,
) -> Result<GateArtifactRefs, SentinelError> {
    let diagnostics = build_diagnostics_artifact(result);
    validate_diagnostics_artifact(&diagnostics, schema_root.as_ref())?;
    let approval = build_approval_artifact(task, result);
    validate_approval_artifact(&approval, schema_root.as_ref())?;

    let diagnostics_ref = store
        .write_tool_json(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            DIAGNOSTICS_FILE,
            &diagnostics,
        )
        .map_err(|source| SentinelError::State { source })?;
    let approval_ref = store
        .write_tool_json(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            APPROVAL_FILE,
            &approval,
        )
        .map_err(|source| SentinelError::State { source })?;

    Ok(GateArtifactRefs {
        diagnostics_ref,
        approval_ref,
    })
}

fn approval_reasons(result: &EvaluationResult) -> Vec<String> {
    if result.diagnostics.is_empty() {
        return vec!["No P0 diagnostics were produced.".to_string()];
    }

    let mut rules = BTreeSet::new();
    for diagnostic in &result.diagnostics {
        rules.insert((diagnostic.severity.as_str(), diagnostic.rule_id.as_str()));
    }

    rules
        .into_iter()
        .map(|(severity, rule_id)| format!("{} diagnostic from {}", severity, rule_id))
        .collect()
}

fn required_human_actions(decision: Decision) -> Vec<String> {
    match decision {
        Decision::AutoPass => Vec::new(),
        Decision::HumanReview => vec![
            "Review HUMAN_REVIEW diagnostics and record approval before continuing.".to_string(),
        ],
        Decision::Block => {
            vec!["Resolve BLOCK diagnostics before continuing automatically.".to_string()]
        }
    }
}
