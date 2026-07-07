use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_release::{
    ReleaseAutomationPlanner, ReleaseReadinessWriter, RELEASE_AUTOMATION_ACTIONS,
    RELEASE_READINESS_PATH,
};
use star_control_state::StateStore;

pub(crate) fn release_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let action = parsed
        .action
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "release requires --action <name>".to_string(),
        })?;
    if !RELEASE_AUTOMATION_ACTIONS.contains(&action) {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!(
                "unsupported release action {}; supported actions: {}",
                action,
                RELEASE_AUTOMATION_ACTIONS.join(", ")
            ),
        });
    }
    reject_unrelated_options(parsed)?;

    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let readiness_writer = ReleaseReadinessWriter::new(config.schema_root());
    let readiness = readiness_writer
        .read(&store, &job_id)
        .map_err(|source| CliError::ReleaseReadiness {
            command: parsed.command.clone(),
            source,
        })?
        .ok_or_else(|| CliError::MissingArtifact {
            command: parsed.command.clone(),
            message: "release readiness artifact is required before release automation".to_string(),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)],
        })?;
    let mode = if parsed.dry_run {
        "dry_run"
    } else {
        "approval_required"
    };
    let planner = ReleaseAutomationPlanner::new(config.schema_root());
    let plan = planner
        .plan(&job_id, &readiness, action, mode)
        .map_err(|source| CliError::ReleaseReadiness {
            command: parsed.command.clone(),
            source,
        })?;
    let approval_token = plan
        .get("approval_token")
        .and_then(Value::as_str)
        .unwrap_or("");
    let approval_accepted = parsed
        .release_approval
        .as_deref()
        .is_some_and(|approval| approval == approval_token);
    if !parsed.dry_run
        && (!plan["approval_required"].as_bool().unwrap_or(true) || approval_accepted)
    {
        let execution = planner
            .execute(
                &store,
                &job_id,
                &readiness,
                action,
                parsed.release_approval.as_deref().unwrap_or(""),
            )
            .map_err(|source| CliError::ReleaseReadiness {
                command: parsed.command.clone(),
                source,
            })?;
        let mut artifacts = release_action_artifacts(&plan);
        if let Some(path) = execution.get("result_artifact").and_then(Value::as_str) {
            artifacts.push(format!(".ai-runs/{}/{}", job_id, path));
        }
        artifacts.sort();
        artifacts.dedup();
        return Ok(success_envelope(
            "release",
            "success",
            json!({
                "job_id": job_id,
                "mode": "approved_execution",
                "release_actions_enabled": true,
                "action_execution_enabled": true,
                "approval_required": execution["approval_required"],
                "approval_gate": {
                    "approval_token": approval_token,
                    "approval_provided": parsed.release_approval.is_some(),
                    "approval_accepted": execution["approval_accepted"],
                    "execution_after_approval": if plan["approval_required"].as_bool().unwrap_or(true) {
                        "performed"
                    } else {
                        "not_required"
                    }
                },
                "external_actions_performed": execution["external_actions_performed"],
                "release_actions_performed": execution["release_actions_performed"],
                "external_execution_policy": execution["external_execution_policy"],
                "release_automation_plan": plan,
                "release_execution": execution
            }),
            artifacts,
        ));
    }
    let status = if parsed.dry_run { "success" } else { "blocked" };
    let artifacts = release_action_artifacts(&plan);

    Ok(success_envelope(
        "release",
        status,
        json!({
            "job_id": job_id,
            "mode": mode,
            "release_actions_enabled": true,
            "action_execution_enabled": false,
            "approval_required": plan["approval_required"],
            "approval_gate": {
                "approval_token": approval_token,
                "approval_provided": parsed.release_approval.is_some(),
                "approval_accepted": approval_accepted,
                "execution_after_approval": "reserved_for_release_action_executor"
            },
            "external_actions_performed": false,
            "release_actions_performed": false,
            "external_execution_policy": plan["external_execution_policy"],
            "release_automation_plan": plan
        }),
        artifacts,
    ))
}

fn reject_unrelated_options(parsed: &ParsedArgs) -> Result<(), CliError> {
    if parsed.recovery_list
        || parsed.release_readiness
        || parsed.stage.is_some()
        || parsed.markdown
        || parsed.request.is_some()
        || parsed.entrypoint.is_some()
        || parsed.provider.is_some()
        || !parsed.provider_instances.is_empty()
        || parsed.response.is_some()
        || parsed.reason.is_some()
        || !parsed.constraints.is_empty()
        || parsed.recovery_approval.is_some()
        || parsed.has_recovery_source_selection()
    {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "release only accepts --project, --job, --action, --dry-run, --approve-release-action, and --json".to_string(),
        });
    }
    Ok(())
}

fn release_action_artifacts(plan: &Value) -> Vec<String> {
    let job_id = plan
        .get("job_id")
        .and_then(Value::as_str)
        .unwrap_or("J-0000");
    let mut artifacts = Vec::new();
    if let Some(paths) = plan.get("artifact_paths").and_then(Value::as_array) {
        for path in paths {
            if let Some(path) = path.as_str() {
                artifacts.push(format!(".ai-runs/{}/{}", job_id, path));
            }
        }
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
}
