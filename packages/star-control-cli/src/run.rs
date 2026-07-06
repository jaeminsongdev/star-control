use self::constants::{COMMAND_RUN, FAILED_STATE, JOB_SCHEMA_PATH};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::{DEFAULT_ENTRYPOINT, DEFAULT_PROVIDER};
use crate::error::CliError;
use crate::output::{status_for_state, success_envelope};
use crate::{required_project, string_field};
use execution::execute_routed_stage;
use registry::load_run_registry;
use route::{route_value_for_provider, workspec_value_for_provider};
use serde_json::{json, Value};
use star_control_router::{JobSpec, RouterEngine};
use star_control_state::StateStore;
use state::routed_state;

mod artifacts;
mod constants;
mod execution;
mod registry;
mod route;
mod state;

pub(crate) fn run_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let request = parsed
        .request
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--request is required for run".to_string(),
        })?;
    let provider = parsed.provider.as_deref().unwrap_or(DEFAULT_PROVIDER);
    let provider_instance_id = provider.to_string();

    let schemas = config.schema_root();
    let store = StateStore::open(&project, &schemas).map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;
    let registry = load_run_registry(parsed, config, provider)?;
    let job = store
        .create_job(
            request,
            parsed
                .entrypoint
                .clone()
                .unwrap_or_else(|| DEFAULT_ENTRYPOINT.to_string()),
            Vec::new(),
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let job_id = string_field(&job, "job_id", &parsed.command)?;
    let job_spec =
        JobSpec::from_value(job.clone(), JOB_SCHEMA_PATH, &schemas).map_err(|source| {
            CliError::Router {
                command: parsed.command.clone(),
                source,
            }
        })?;
    let router = RouterEngine::new(&registry, &schemas);
    let route_output = router.route(&job_spec).map_err(|source| CliError::Router {
        command: parsed.command.clone(),
        source,
    })?;
    let route_value = route_value_for_provider(route_output.route().value(), &provider_instance_id);
    store
        .save_route(&job_id, &route_value)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    for (stage, workspec) in route_output.workspecs() {
        let workspec_value = workspec_value_for_provider(workspec.value(), &provider_instance_id);
        store
            .save_workspec(&job_id, stage, &workspec_value)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
    }

    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/route.json", job_id),
    ];

    let (state, executed_stage) = if parsed.dry_run {
        let state = routed_state(&job_id);
        store
            .save_state(&job_id, &state)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        artifacts.push(format!(".ai-runs/{}/run-state.json", job_id));
        (state, None)
    } else {
        let (state, stage, execution_artifacts) = execute_routed_stage(
            &parsed.command,
            &store,
            &registry,
            &schemas,
            &route_output,
            &job_id,
            &provider_instance_id,
        )?;
        artifacts.push(format!(".ai-runs/{}/run-state.json", job_id));
        artifacts.extend(execution_artifacts);
        (state, Some(stage))
    };

    Ok(success_envelope(
        COMMAND_RUN,
        status_for_state(
            state
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or(FAILED_STATE),
        ),
        json!({
            "job_id": job_id,
            "state": state.get("state").cloned().unwrap_or_else(|| json!("")),
            "current_stage": state.get("current_stage").cloned().unwrap_or_else(|| json!("")),
            "run_dir": format!(".ai-runs/{}", job_id),
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "dry_run": parsed.dry_run,
            "executed_stage": executed_stage
        }),
        artifacts,
    ))
}
