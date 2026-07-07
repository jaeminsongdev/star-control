use super::{RecoveryInspection, RecoveryIssue};
use crate::{StateStore, StateStoreError, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::fs;

pub const RECOVERY_ACTIONS: &[&str] = &[
    "tmp-cleanup",
    "recovered-copy",
    "event-log-trim",
    "artifact-replace",
    "retention-cleanup",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryActionPlan {
    pub job_id: String,
    pub action: String,
    pub mode: String,
    pub status: String,
    pub supported: bool,
    pub approval_required: bool,
    pub approval_token: String,
    pub destructive: bool,
    pub destructive_actions_performed: bool,
    pub planned_changes: Vec<Value>,
    pub warnings: Vec<String>,
    pub inspection: RecoveryInspection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryActionExecution {
    pub job_id: String,
    pub action: String,
    pub mode: String,
    pub status: String,
    pub approval_required: bool,
    pub approval_accepted: bool,
    pub action_execution_enabled: bool,
    pub destructive_actions_performed: bool,
    pub result_artifact: String,
    pub executed_changes: Vec<Value>,
    pub skipped_changes: Vec<Value>,
    pub plan: RecoveryActionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySourceSelection {
    pub artifact_path: String,
    pub source_path: String,
}

impl RecoveryActionPlan {
    fn new(
        inspection: RecoveryInspection,
        action: &str,
        mode: &str,
        source_selection: Option<&RecoverySourceSelection>,
    ) -> Self {
        let supported = RECOVERY_ACTIONS.contains(&action);
        let mut planned_changes = if supported {
            planned_changes_for(action, &inspection.issues)
        } else {
            Vec::new()
        };
        if let Some(selection) = source_selection {
            apply_source_selection(&mut planned_changes, selection);
        }
        let mut warnings = Vec::new();
        if !supported {
            warnings.push(format!(
                "unsupported recovery action {}; supported actions: {}",
                action,
                RECOVERY_ACTIONS.join(", ")
            ));
        }
        if supported && planned_changes.is_empty() {
            warnings.push(format!(
                "recovery action {} has no matching issue in the current inspection",
                action
            ));
        }
        if action == "artifact-replace" && source_selection.is_none() {
            warnings.push(format!(
                "recovery action {} requires a future action-specific executor before mutation",
                action
            ));
        }
        if action == "artifact-replace" && source_selection.is_some() {
            let source_matched = planned_changes.iter().any(|change| {
                change.get("operation").and_then(Value::as_str)
                    == Some("replace_artifact_from_approved_source")
                    && change.get("source_path").and_then(Value::as_str).is_some()
            });
            if !source_matched {
                warnings.push(
                    "artifact replacement source selection did not match a current recovery issue"
                        .to_string(),
                );
            }
        }
        if action == "retention-cleanup" {
            warnings.push(format!(
                "recovery action {} requires a future action-specific executor before mutation",
                action
            ));
        }
        let destructive = matches!(
            action,
            "tmp-cleanup" | "event-log-trim" | "artifact-replace" | "retention-cleanup"
        );
        let status = if !supported {
            "unsupported"
        } else if mode == "dry_run" {
            "preview"
        } else if mode == "approved_execution" {
            "approved"
        } else {
            "approval_required"
        };
        let job_id = inspection.job_id.clone();
        Self {
            job_id: job_id.clone(),
            action: action.to_string(),
            mode: mode.to_string(),
            status: status.to_string(),
            supported,
            approval_required: destructive,
            approval_token: format!("approve:{}:{}", action, job_id),
            destructive,
            destructive_actions_performed: false,
            planned_changes,
            warnings,
            inspection,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": self.job_id,
            "action": self.action,
            "mode": self.mode,
            "status": self.status,
            "supported": self.supported,
            "approval_required": self.approval_required,
            "approval_token": self.approval_token,
            "destructive": self.destructive,
            "destructive_actions_performed": self.destructive_actions_performed,
            "planned_changes": self.planned_changes,
            "warnings": self.warnings,
            "recovery": self.inspection.to_value()
        })
    }
}

impl RecoveryActionExecution {
    pub fn to_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": self.job_id,
            "action": self.action,
            "mode": self.mode,
            "status": self.status,
            "approval_required": self.approval_required,
            "approval_accepted": self.approval_accepted,
            "action_execution_enabled": self.action_execution_enabled,
            "destructive_actions_performed": self.destructive_actions_performed,
            "result_artifact": self.result_artifact,
            "executed_changes": self.executed_changes,
            "skipped_changes": self.skipped_changes,
            "recovery_action": self.plan.to_value()
        })
    }
}

impl StateStore {
    pub fn plan_recovery_action(
        &self,
        job_id: &str,
        action: &str,
        mode: &str,
    ) -> Result<RecoveryActionPlan, StateStoreError> {
        self.plan_recovery_action_with_source(job_id, action, mode, None)
    }

    pub fn plan_recovery_action_with_source(
        &self,
        job_id: &str,
        action: &str,
        mode: &str,
        source_selection: Option<&RecoverySourceSelection>,
    ) -> Result<RecoveryActionPlan, StateStoreError> {
        let inspection = self.inspect_recovery(job_id)?;
        Ok(RecoveryActionPlan::new(
            inspection,
            action,
            mode,
            source_selection,
        ))
    }

    pub fn execute_recovery_action(
        &self,
        job_id: &str,
        action: &str,
        approval_token: &str,
    ) -> Result<RecoveryActionExecution, StateStoreError> {
        self.execute_recovery_action_with_source(job_id, action, approval_token, None)
    }

    pub fn execute_recovery_action_with_source(
        &self,
        job_id: &str,
        action: &str,
        approval_token: &str,
        source_selection: Option<&RecoverySourceSelection>,
    ) -> Result<RecoveryActionExecution, StateStoreError> {
        let plan = self.plan_recovery_action_with_source(
            job_id,
            action,
            "approved_execution",
            source_selection,
        )?;
        let approval_accepted = !plan.approval_required || approval_token == plan.approval_token;
        if !approval_accepted || !plan.supported {
            return Ok(RecoveryActionExecution {
                job_id: job_id.to_string(),
                action: action.to_string(),
                mode: "approval_required".to_string(),
                status: if plan.supported {
                    "blocked".to_string()
                } else {
                    "unsupported".to_string()
                },
                approval_required: plan.approval_required,
                approval_accepted,
                action_execution_enabled: false,
                destructive_actions_performed: false,
                result_artifact: result_artifact_path(action),
                executed_changes: Vec::new(),
                skipped_changes: vec![json!({
                    "operation": "approval_gate",
                    "reason": "approval token did not match recovery action plan"
                })],
                plan,
            });
        }

        let mut execution = RecoveryActionExecution {
            job_id: job_id.to_string(),
            action: action.to_string(),
            mode: "approved_execution".to_string(),
            status: "success".to_string(),
            approval_required: plan.approval_required,
            approval_accepted,
            action_execution_enabled: true,
            destructive_actions_performed: false,
            result_artifact: result_artifact_path(action),
            executed_changes: Vec::new(),
            skipped_changes: Vec::new(),
            plan,
        };

        for change in execution.plan.planned_changes.clone() {
            self.execute_planned_recovery_change(job_id, &change, &mut execution)?;
        }
        if execution.executed_changes.is_empty() && execution.skipped_changes.is_empty() {
            execution.status = "noop".to_string();
        }
        let result = execution.to_value();
        self.write_json_value_atomic(job_id, &execution.result_artifact, &result)?;
        Ok(execution)
    }

    fn execute_planned_recovery_change(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        match change.get("operation").and_then(Value::as_str) {
            Some("delete_file") | Some("retention_delete_tmp_artifact") => {
                self.execute_delete_file(job_id, change, execution)
            }
            Some("write_recovered_copy") => {
                self.execute_write_recovered_copy(job_id, change, execution)
            }
            Some("write_trimmed_event_log_preview") => {
                self.execute_write_trimmed_event_log_preview(job_id, change, execution)
            }
            Some("replace_event_log_with_trimmed_copy") => {
                self.execute_replace_event_log(job_id, change, execution)
            }
            Some("replace_artifact_from_approved_source") => {
                self.execute_replace_artifact_from_source(job_id, change, execution)
            }
            Some(_) | None => {
                push_skipped_change(execution, change, "unsupported recovery operation");
                Ok(())
            }
        }
    }

    fn execute_delete_file(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        let Some(artifact_path) = change.get("artifact_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "artifact_path is required");
            return Ok(());
        };
        let path = self.resolve_job_path(job_id, artifact_path)?;
        if !path.is_file() {
            push_skipped_change(execution, change, "file is already absent");
            return Ok(());
        }
        fs::remove_file(&path).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: path.clone(),
            source,
        })?;
        execution.destructive_actions_performed = true;
        push_executed_change(execution, change, "deleted file");
        Ok(())
    }

    fn execute_write_recovered_copy(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        let Some(artifact_path) = change.get("artifact_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "artifact_path is required");
            return Ok(());
        };
        let Some(output_path) = change.get("output_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "output_path is required");
            return Ok(());
        };
        let source_path = self.resolve_job_path(job_id, artifact_path)?;
        if !source_path.is_file() {
            push_skipped_change(execution, change, "source artifact is absent");
            return Ok(());
        }
        let target_path = self.resolve_job_path(job_id, output_path)?;
        if target_path.exists() {
            push_skipped_change(execution, change, "recovered copy already exists");
            return Ok(());
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(&source_path, &target_path).map_err(|source| {
            StateStoreError::AtomicWriteFailed {
                path: target_path,
                source,
            }
        })?;
        push_executed_change(execution, change, "wrote recovered copy");
        Ok(())
    }

    fn execute_write_trimmed_event_log_preview(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        let Some(artifact_path) = change.get("artifact_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "artifact_path is required");
            return Ok(());
        };
        let Some(output_path) = change.get("output_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "output_path is required");
            return Ok(());
        };
        let trimmed = trimmed_event_log(self, job_id, artifact_path)?;
        self.write_text_artifact(job_id, output_path, &trimmed)?;
        push_executed_change(execution, change, "wrote trimmed event log preview");
        Ok(())
    }

    fn execute_replace_event_log(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        let Some(source_path) = change.get("source_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "source_path is required");
            return Ok(());
        };
        let Some(artifact_path) = change.get("artifact_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "artifact_path is required");
            return Ok(());
        };
        let source = self.resolve_job_path(job_id, source_path)?;
        if !source.is_file() {
            push_skipped_change(execution, change, "trimmed event log preview is absent");
            return Ok(());
        }
        let content =
            fs::read_to_string(&source).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: source_path.into(),
                source,
            })?;
        self.write_text_artifact(job_id, artifact_path, &content)?;
        execution.destructive_actions_performed = true;
        push_executed_change(execution, change, "replaced event log with trimmed copy");
        Ok(())
    }

    fn execute_replace_artifact_from_source(
        &self,
        job_id: &str,
        change: &Value,
        execution: &mut RecoveryActionExecution,
    ) -> Result<(), StateStoreError> {
        let Some(source_path) = change.get("source_path").and_then(Value::as_str) else {
            push_skipped_change(
                execution,
                change,
                "artifact replacement requires an explicit approved source path",
            );
            return Ok(());
        };
        let Some(artifact_path) = change.get("artifact_path").and_then(Value::as_str) else {
            push_skipped_change(execution, change, "artifact_path is required");
            return Ok(());
        };
        if source_path == artifact_path {
            push_skipped_change(
                execution,
                change,
                "source_path must differ from artifact_path",
            );
            return Ok(());
        }
        let source = self.resolve_job_path(job_id, source_path)?;
        if !source.is_file() {
            push_skipped_change(execution, change, "approved source artifact is absent");
            return Ok(());
        }
        let target = self.resolve_job_path(job_id, artifact_path)?;
        let bytes = fs::read(&source).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: source_path.into(),
            source,
        })?;
        self.write_bytes_atomic(job_id, &target, &bytes)?;
        execution.destructive_actions_performed = true;
        push_executed_change(execution, change, "replaced artifact from approved source");
        Ok(())
    }
}

fn apply_source_selection(changes: &mut [Value], selection: &RecoverySourceSelection) {
    for change in changes {
        if change.get("operation").and_then(Value::as_str)
            == Some("replace_artifact_from_approved_source")
            && change.get("artifact_path").and_then(Value::as_str)
                == Some(selection.artifact_path.as_str())
        {
            change["source_path"] = json!(selection.source_path);
        }
    }
}

fn planned_changes_for(action: &str, issues: &[RecoveryIssue]) -> Vec<Value> {
    match action {
        "tmp-cleanup" => issues
            .iter()
            .filter(|issue| issue.kind == "partial_tmp_file")
            .map(|issue| {
                json!({
                    "operation": "delete_file",
                    "artifact_path": issue.artifact_path,
                    "destructive": true,
                    "requires_approval": true
                })
            })
            .collect(),
        "recovered-copy" => issues
            .iter()
            .filter(|issue| issue.severity == "block")
            .map(|issue| {
                json!({
                    "operation": "write_recovered_copy",
                    "artifact_path": issue.artifact_path,
                    "output_path": recovered_copy_path(&issue.artifact_path),
                    "destructive": false,
                    "requires_approval": false
                })
            })
            .collect(),
        "event-log-trim" => issues
            .iter()
            .filter(|issue| issue.kind == "corrupt_event_log")
            .flat_map(|issue| {
                [
                    json!({
                        "operation": "write_trimmed_event_log_preview",
                        "artifact_path": issue.artifact_path,
                        "output_path": "recovery/events.trimmed.jsonl",
                        "destructive": false,
                        "requires_approval": false
                    }),
                    json!({
                        "operation": "replace_event_log_with_trimmed_copy",
                        "artifact_path": issue.artifact_path,
                        "source_path": "recovery/events.trimmed.jsonl",
                        "destructive": true,
                        "requires_approval": true
                    }),
                ]
            })
            .collect(),
        "artifact-replace" => issues
            .iter()
            .filter(|issue| {
                matches!(
                    issue.kind.as_str(),
                    "missing_required_file" | "invalid_json" | "schema_mismatch"
                )
            })
            .map(|issue| {
                json!({
                    "operation": "replace_artifact_from_approved_source",
                    "artifact_path": issue.artifact_path,
                    "source_path": null,
                    "destructive": true,
                    "requires_approval": true
                })
            })
            .collect(),
        "retention-cleanup" => issues
            .iter()
            .filter(|issue| issue.kind == "partial_tmp_file")
            .map(|issue| {
                json!({
                    "operation": "retention_delete_tmp_artifact",
                    "artifact_path": issue.artifact_path,
                    "destructive": true,
                    "requires_approval": true
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn recovered_copy_path(artifact_path: &str) -> String {
    let safe = artifact_path
        .replace('\\', "/")
        .replace('/', "__")
        .replace(':', "_");
    format!("recovery/{}.recovered-copy", safe)
}

fn result_artifact_path(action: &str) -> String {
    format!("recovery/{}-result.json", action)
}

fn trimmed_event_log(
    store: &StateStore,
    job_id: &str,
    artifact_path: &str,
) -> Result<String, StateStoreError> {
    let path = store.resolve_job_path(job_id, artifact_path)?;
    if !path.is_file() {
        return Ok(String::new());
    }
    let text = fs::read_to_string(&path).map_err(|source| StateStoreError::AtomicWriteFailed {
        path: path.clone(),
        source,
    })?;
    let mut output = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if serde_json::from_str::<Value>(line).is_err() {
            break;
        }
        output.push_str(line);
        output.push('\n');
    }
    Ok(output)
}

fn push_executed_change(execution: &mut RecoveryActionExecution, change: &Value, message: &str) {
    let mut value = change.clone();
    value["performed"] = json!(true);
    value["message"] = json!(message);
    execution.executed_changes.push(value);
}

fn push_skipped_change(execution: &mut RecoveryActionExecution, change: &Value, reason: &str) {
    let mut value = change.clone();
    value["performed"] = json!(false);
    value["skip_reason"] = json!(reason);
    execution.skipped_changes.push(value);
    if execution.status == "success" {
        execution.status = "partial".to_string();
    }
}
