mod constants;
mod evidence;
mod policy;
mod runner;
mod sidecars;

use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use constants::{LOCAL_PROCESS_KIND, PROCESS_TRANSPORT, STDERR_FILE, STDOUT_FILE};
use evidence::forbidden_action_evidence;
pub use policy::LocalProcessCommandPolicy;
use runner::{run_process, LocalProcessRunResult};
use sidecars::{artifact_ref, create_new_output_file, planned_output_files, response_value};
use std::fs;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalProcessProviderAdapter;

impl ProviderAdapter for LocalProcessProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.kind() != LOCAL_PROCESS_KIND || manifest.transport() != PROCESS_TRANSPORT {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }

        let instance = context
            .registry()
            .instance(request.provider_instance_id())
            .ok_or_else(|| crate::ProviderRegistryError::InstanceNotFound {
                instance_id: request.provider_instance_id().to_string(),
            })?;
        let policy = LocalProcessCommandPolicy::from_instance(instance)?;

        let output_files = planned_output_files(request.provider_instance_id());
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let output_dir = context
            .state_store()
            .resolve_provider_output_dir(request.job_id(), request.provider_instance_id())?;
        fs::create_dir_all(&output_dir).map_err(|source| ProviderAdapterError::Io {
            path: output_dir.clone(),
            source,
        })?;

        let stdout_path = output_dir.join(STDOUT_FILE);
        let stderr_path = output_dir.join(STDERR_FILE);
        let stdout_file = create_new_output_file(&stdout_path)?;
        let stderr_file = create_new_output_file(&stderr_path)?;

        let mut process_result = run_process(&policy, request, context, stdout_file, stderr_file)?;
        if matches!(process_result, LocalProcessRunResult::Exited { .. }) {
            if let Some(evidence) = forbidden_action_evidence(request, &stdout_path, &stderr_path)?
            {
                process_result = LocalProcessRunResult::BlockedForbiddenAction { evidence };
            }
        }
        let response_value = response_value(request, &policy, &process_result);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            provider_output_path(request.provider_instance_id(), "response.json"),
            context.schema_root(),
        )?;

        let stdout_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        )?;
        let stderr_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDERR_FILE),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            &response_value,
        )?;

        Ok(ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        ))
    }
}

#[cfg(test)]
mod tests;
