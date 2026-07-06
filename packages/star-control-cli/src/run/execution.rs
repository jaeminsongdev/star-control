use super::artifacts::provider_result_artifacts;
use super::constants::IMPLEMENT_STAGE;
use super::state::report_from_provider_result;
use crate::error::CliError;
use serde_json::Value;
use star_control_execution::ExecutionEngine;
use star_control_provider::ProviderRegistry;
use star_control_router::RouterOutput;
use star_control_state::StateStore;
use std::path::Path;

pub(super) fn execute_routed_stage(
    command: &str,
    store: &StateStore,
    registry: &ProviderRegistry,
    schemas: &Path,
    route_output: &RouterOutput,
    job_id: &str,
    provider_instance_id: &str,
) -> Result<(Value, String, Vec<String>), CliError> {
    let stage = route_output
        .workspec(IMPLEMENT_STAGE)
        .map(|workspec| workspec.stage().to_string())
        .or_else(|| route_output.workspecs().keys().next().cloned())
        .ok_or_else(|| CliError::Internal {
            command: command.to_string(),
            message: "route produced no executable WorkSpec".to_string(),
        })?;
    let engine = ExecutionEngine::new(store, registry, schemas);
    let outcome = engine
        .execute_stage(job_id, &stage)
        .map_err(|source| CliError::Execution {
            command: command.to_string(),
            source,
        })?;
    let provider_result = outcome.provider_execution().result().value();
    let report = report_from_provider_result(provider_result);
    store
        .save_report(job_id, &format!("{}-report", stage), &report)
        .map_err(|source| CliError::State {
            command: command.to_string(),
            source,
        })?;
    let mut artifacts = vec![format!(
        ".ai-runs/{}/provider-output/{}/request.json",
        job_id, provider_instance_id
    )];
    artifacts.extend(provider_result_artifacts(provider_result, job_id));
    artifacts.push(format!(".ai-runs/{}/reports/{}-report.json", job_id, stage));
    Ok((outcome.state().clone(), stage, artifacts))
}
