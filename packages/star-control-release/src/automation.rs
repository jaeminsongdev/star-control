use crate::constants::{RELEASE_READINESS_PATH, RELEASE_REVIEW_PACK_PATH, SCHEMA_VERSION};
use crate::error::ReleaseReadinessError;
use crate::review_pack::ReleaseReviewPackWriter;
use crate::writer::ReleaseReadinessWriter;
use serde_json::{json, Value};
use star_control_state::{ArtifactKind, StateStore};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

pub const RELEASE_AUTOMATION_ACTIONS: &[&str] = &[
    "prepare",
    "signing-policy",
    "package-publish",
    "deploy",
    "rollback-checklist",
    "approval-record",
    "review-pack",
];

#[derive(Debug, Clone)]
pub struct ReleaseAutomationPlanner {
    readiness_writer: ReleaseReadinessWriter,
    review_pack_writer: ReleaseReviewPackWriter,
}

impl ReleaseAutomationPlanner {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        let schema_root = schema_root.into();
        Self {
            readiness_writer: ReleaseReadinessWriter::new(schema_root.clone()),
            review_pack_writer: ReleaseReviewPackWriter::new(schema_root),
        }
    }

    pub fn plan(
        &self,
        job_id: &str,
        readiness: &Value,
        action: &str,
        mode: &str,
    ) -> Result<Value, ReleaseReadinessError> {
        if !RELEASE_AUTOMATION_ACTIONS.contains(&action) {
            return Err(ReleaseReadinessError::InvalidReleaseEvidence {
                message: format!(
                    "unsupported release automation action {}; supported actions: {}",
                    action,
                    RELEASE_AUTOMATION_ACTIONS.join(", ")
                ),
            });
        }
        self.readiness_writer.validate_readiness(readiness)?;
        let release_id = string_field(readiness, "release_id");
        let target = string_field(readiness, "target");
        let version = string_field(readiness, "version");
        let readiness_status = string_field(readiness, "status");
        let approval_required = action_requires_approval(action);
        let status = if mode == "dry_run" {
            "preview"
        } else {
            "approval_required"
        };
        let steps = steps_for(action);
        let external_execution_policy = external_execution_policy(action, &steps);
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "release_id": release_id,
            "target": target,
            "version": version,
            "readiness_status": readiness_status,
            "action": action,
            "mode": mode,
            "status": status,
            "approval_required": approval_required,
            "approval_token": format!("approve:{}:{}", action, job_id),
            "external_execution_policy": external_execution_policy,
            "external_actions_performed": false,
            "release_actions_performed": false,
            "artifact_paths": [
                RELEASE_READINESS_PATH,
                RELEASE_REVIEW_PACK_PATH
            ],
            "steps": steps,
            "warnings": warnings_for(action, &readiness_status)
        }))
    }

    pub fn execute(
        &self,
        store: &StateStore,
        job_id: &str,
        readiness: &Value,
        action: &str,
        approval_token: &str,
    ) -> Result<Value, ReleaseReadinessError> {
        let plan = self.plan(job_id, readiness, action, "approved_execution")?;
        let approval_required = plan
            .get("approval_required")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let expected_token = plan
            .get("approval_token")
            .and_then(Value::as_str)
            .unwrap_or("");
        let approval_accepted = !approval_required || approval_token == expected_token;
        if !approval_accepted {
            return Ok(json!({
                "schema_version": SCHEMA_VERSION,
                "job_id": job_id,
                "action": action,
                "mode": "approval_required",
                "status": "blocked",
                "approval_required": approval_required,
                "approval_accepted": false,
                "action_execution_enabled": false,
                "external_execution_policy": plan["external_execution_policy"],
                "external_actions_performed": false,
                "release_actions_performed": false,
                "result_artifact": result_artifact_path(action),
                "executed_steps": [],
                "skipped_steps": [{
                    "operation": "approval_gate",
                    "performed": false,
                    "skip_reason": "approval token did not match release automation plan"
                }],
                "release_automation_plan": plan
            }));
        }

        let mut executed_steps = Vec::new();
        let mut skipped_steps = Vec::new();
        if action == "review-pack" {
            match self.review_pack_writer.write(store, job_id, readiness) {
                Ok(ref_value) => executed_steps.push(json!({
                    "operation": "release_review_pack",
                    "performed": true,
                    "artifact_ref": ref_value,
                    "external_effect": false
                })),
                Err(ReleaseReadinessError::WriteFailed { .. }) => skipped_steps.push(json!({
                    "operation": "release_review_pack",
                    "performed": false,
                    "skip_reason": "release review pack already exists or could not be written"
                })),
                Err(source) => return Err(source),
            }
        }

        for step in plan
            .get("steps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let operation = step
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            if action == "review-pack" && operation == "release_review_pack" {
                continue;
            }
            executed_steps.push(json!({
                "operation": operation,
                "performed": true,
                "external_effect": step.get("external_effect").and_then(Value::as_bool).unwrap_or(false),
                "execution_kind": if step.get("external_effect").and_then(Value::as_bool).unwrap_or(false) {
                    "local_plan_record_only"
                } else {
                    "local_artifact_record"
                },
                "external_policy_decision": if step.get("external_effect").and_then(Value::as_bool).unwrap_or(false) {
                    "record_only_reserved"
                } else {
                    "not_external"
                },
                "summary": step.get("summary").cloned().unwrap_or(Value::Null)
            }));
        }

        let result_artifact = result_artifact_path(action);
        let result = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "release_id": plan["release_id"],
            "target": plan["target"],
            "version": plan["version"],
            "action": action,
            "mode": "approved_execution",
            "status": if skipped_steps.is_empty() { "success" } else { "partial" },
            "approval_required": approval_required,
            "approval_accepted": approval_accepted,
            "action_execution_enabled": true,
            "external_execution_policy": plan["external_execution_policy"],
            "external_actions_performed": false,
            "release_actions_performed": true,
            "result_artifact": result_artifact,
            "executed_steps": executed_steps,
            "skipped_steps": skipped_steps,
            "release_automation_plan": plan
        });
        let path = store.resolve_job_path(job_id, result_artifact.as_str())?;
        write_new_json(&path, &result)?;
        let artifact_ref = store.artifact_ref(
            job_id,
            result_artifact.as_str(),
            ArtifactKind::Other,
            "star-control-release",
            None,
            Some("release automation execution record"),
        )?;
        let mut result_with_ref = result;
        result_with_ref["result_artifact_ref"] = artifact_ref;
        Ok(result_with_ref)
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn action_requires_approval(action: &str) -> bool {
    matches!(
        action,
        "prepare" | "signing-policy" | "package-publish" | "deploy" | "approval-record"
    )
}

fn steps_for(action: &str) -> Vec<Value> {
    let all_steps = vec![
        step(
            "signing_policy_execution",
            "Prepare signing policy command inputs without signing artifacts.",
            true,
            false,
        ),
        step(
            "package_registry_publish",
            "Prepare package registry publish flow without publishing.",
            true,
            true,
        ),
        step(
            "deploy_flow",
            "Prepare deploy flow without changing infrastructure.",
            true,
            true,
        ),
        step(
            "rollback_checklist",
            "Prepare rollback checklist and stop conditions.",
            false,
            false,
        ),
        step(
            "approval_record",
            "Prepare release approval record location without recording approval.",
            true,
            false,
        ),
        step(
            "release_review_pack",
            "Link release review pack artifact for human review.",
            false,
            false,
        ),
    ];
    match action {
        "prepare" => all_steps,
        "signing-policy" => vec![all_steps[0].clone()],
        "package-publish" => vec![all_steps[1].clone()],
        "deploy" => vec![all_steps[2].clone()],
        "rollback-checklist" => vec![all_steps[3].clone()],
        "approval-record" => vec![all_steps[4].clone()],
        "review-pack" => vec![all_steps[5].clone()],
        _ => Vec::new(),
    }
}

fn step(operation: &str, summary: &str, approval_required: bool, external_effect: bool) -> Value {
    json!({
        "operation": operation,
        "summary": summary,
        "approval_required": approval_required,
        "external_effect": external_effect,
        "status": "not_executed"
    })
}

fn warnings_for(action: &str, readiness_status: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if readiness_status != "reserved" {
        warnings.push(format!(
            "release readiness status is {}; release automation still remains approval-gated",
            readiness_status
        ));
    }
    if matches!(action, "package-publish" | "deploy" | "prepare") {
        warnings.push(
            "publish/deploy execution is not performed by this automation surface".to_string(),
        );
    }
    warnings
}

fn external_execution_policy(action: &str, steps: &[Value]) -> Value {
    let blocked_operations = steps
        .iter()
        .filter(|step| {
            step.get("external_effect")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|step| step.get("operation").and_then(Value::as_str))
        .map(|operation| json!(operation))
        .collect::<Vec<_>>();
    json!({
        "status": "reserved",
        "action": action,
        "live_execution_enabled": false,
        "external_actions_allowed": false,
        "requires_explicit_approval": true,
        "approval_scope": "local release automation artifact only",
        "blocked_operations": blocked_operations,
        "policy_reason": "external release/deploy/publish execution remains approval-gated and is not performed by Star-Control local automation"
    })
}

fn result_artifact_path(action: &str) -> String {
    format!("release/{}-automation-result.json", action)
}

fn write_new_json(path: &Path, value: &Value) -> Result<(), ReleaseReadinessError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ReleaseReadinessError::WriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| ReleaseReadinessError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })
}
