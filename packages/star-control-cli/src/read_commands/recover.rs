use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::{RecoverySourceSelection, StateStore, RECOVERY_ACTIONS};

pub(crate) fn recover_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    if parsed.recovery_list && parsed.action.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover accepts either --list or --action, not both".to_string(),
        });
    }
    if !parsed.recovery_list && parsed.action.is_none() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover requires --list or --action <name>".to_string(),
        });
    }
    if parsed.release_readiness
        || parsed.stage.is_some()
        || parsed.markdown
        || parsed.request.is_some()
        || parsed.entrypoint.is_some()
        || parsed.provider.is_some()
        || !parsed.provider_instances.is_empty()
        || parsed.response.is_some()
        || parsed.reason.is_some()
        || !parsed.constraints.is_empty()
    {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover only accepts recovery options plus --project, --job, and --json"
                .to_string(),
        });
    }
    if parsed.recovery_list
        && (parsed.dry_run
            || parsed.recovery_approval.is_some()
            || parsed.has_recovery_source_selection())
    {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message:
                "recover --list does not accept --dry-run, --approve-recovery-action, or source selection"
                    .to_string(),
        });
    }

    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    if let Some(action) = parsed.action.as_deref() {
        return recover_action_command(parsed, &store, &job_id, action);
    }

    let inspection = store
        .inspect_recovery(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let inspection_value = inspection.to_value();
    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/run-state.json", job_id),
        format!(".ai-runs/{}/events.jsonl", job_id),
    ];
    artifacts.extend(
        inspection
            .issues
            .iter()
            .map(|issue| format!(".ai-runs/{}/{}", job_id, issue.artifact_path)),
    );
    artifacts.sort();
    artifacts.dedup();

    Ok(success_envelope(
        "recover",
        "success",
        json!({
            "job_id": job_id,
            "mode": "inspect_only",
            "recovery_actions_enabled": false,
            "recovery": inspection_value
        }),
        artifacts,
    ))
}

fn recover_action_command(
    parsed: &ParsedArgs,
    store: &StateStore,
    job_id: &str,
    action: &str,
) -> Result<Value, CliError> {
    if !RECOVERY_ACTIONS.contains(&action) {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!(
                "unsupported recovery action {}; supported actions: {}",
                action,
                RECOVERY_ACTIONS.join(", ")
            ),
        });
    }
    let source_selection = recovery_source_selection(parsed, action)?;
    let mode = if parsed.dry_run {
        "dry_run"
    } else {
        "approval_required"
    };
    let plan = store
        .plan_recovery_action_with_source(job_id, action, mode, source_selection.as_ref())
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    ensure_source_selection_matches_plan(parsed, &plan, source_selection.as_ref())?;
    let approval_accepted = parsed
        .recovery_approval
        .as_deref()
        .is_some_and(|approval| approval == plan.approval_token);
    if !parsed.dry_run && (!plan.approval_required || approval_accepted) {
        if action == "artifact-replace" && source_selection.is_none() {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message:
                    "artifact-replace execution requires --recovery-artifact and --recovery-source"
                        .to_string(),
            });
        }
        let execution = store
            .execute_recovery_action_with_source(
                job_id,
                action,
                parsed.recovery_approval.as_deref().unwrap_or(""),
                source_selection.as_ref(),
            )
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        let execution_value = execution.to_value();
        let mut artifacts = recovery_action_artifacts(&execution_value["recovery_action"]);
        artifacts.push(format!(".ai-runs/{}/{}", job_id, execution.result_artifact));
        artifacts.sort();
        artifacts.dedup();
        return Ok(success_envelope(
            "recover",
            "success",
            json!({
                "job_id": job_id,
                "mode": "approved_execution",
                "recovery_actions_enabled": true,
                "action_execution_enabled": true,
                "approval_required": execution.approval_required,
                "approval_gate": {
                    "approval_token": plan.approval_token,
                    "approval_provided": parsed.recovery_approval.is_some(),
                    "approval_accepted": execution.approval_accepted,
                    "execution_after_approval": if plan.approval_required { "performed" } else { "not_required" }
                },
                "destructive_actions_performed": execution.destructive_actions_performed,
                "recovery_action": execution_value["recovery_action"].clone(),
                "recovery_execution": execution_value
            }),
            artifacts,
        ));
    }

    let status = if parsed.dry_run { "success" } else { "blocked" };
    let artifacts = recovery_action_artifacts(&plan.to_value());

    Ok(success_envelope(
        "recover",
        status,
        json!({
            "job_id": job_id,
            "mode": mode,
            "recovery_actions_enabled": true,
            "action_execution_enabled": false,
            "approval_required": plan.approval_required,
            "approval_gate": {
                "approval_token": plan.approval_token,
                "approval_provided": parsed.recovery_approval.is_some(),
                "approval_accepted": approval_accepted,
                "execution_after_approval": "reserved_for_action_executor"
            },
            "destructive_actions_performed": false,
            "recovery_action": plan.to_value()
        }),
        artifacts,
    ))
}

fn ensure_source_selection_matches_plan(
    parsed: &ParsedArgs,
    plan: &star_control_state::RecoveryActionPlan,
    source_selection: Option<&RecoverySourceSelection>,
) -> Result<(), CliError> {
    let Some(selection) = source_selection else {
        return Ok(());
    };
    let matched = plan.planned_changes.iter().any(|change| {
        change.get("operation").and_then(Value::as_str)
            == Some("replace_artifact_from_approved_source")
            && change.get("artifact_path").and_then(Value::as_str)
                == Some(selection.artifact_path.as_str())
            && change.get("source_path").and_then(Value::as_str)
                == Some(selection.source_path.as_str())
    });
    if matched {
        Ok(())
    } else {
        Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!(
                "recovery artifact {} is not replaceable in the current inspection",
                selection.artifact_path
            ),
        })
    }
}

fn recovery_source_selection(
    parsed: &ParsedArgs,
    action: &str,
) -> Result<Option<RecoverySourceSelection>, CliError> {
    if !parsed.has_recovery_source_selection() {
        return Ok(None);
    }
    if action != "artifact-replace" {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message:
                "--recovery-artifact and --recovery-source are only valid for artifact-replace"
                    .to_string(),
        });
    }
    let Some(artifact_path) = parsed.recovery_artifact.clone() else {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--recovery-artifact is required when --recovery-source is set".to_string(),
        });
    };
    let Some(source_path) = parsed.recovery_source.clone() else {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--recovery-source is required when --recovery-artifact is set".to_string(),
        });
    };
    Ok(Some(RecoverySourceSelection {
        artifact_path,
        source_path,
    }))
}

fn recovery_action_artifacts(plan: &Value) -> Vec<String> {
    let job_id = plan
        .get("job_id")
        .and_then(Value::as_str)
        .unwrap_or("J-0000");
    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/run-state.json", job_id),
        format!(".ai-runs/{}/events.jsonl", job_id),
    ];
    if let Some(changes) = plan.get("planned_changes").and_then(Value::as_array) {
        for change in changes {
            for key in ["artifact_path", "output_path", "source_path"] {
                if let Some(path) = change.get(key).and_then(Value::as_str) {
                    artifacts.push(format!(".ai-runs/{}/{}", job_id, path));
                }
            }
        }
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
}
