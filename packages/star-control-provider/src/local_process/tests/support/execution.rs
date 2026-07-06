use super::registry::registry_with_instance;
use super::request::request_value;
use super::temp::{open_store, schema_root, temp_project};
use crate::local_process::LocalProcessProviderAdapter;
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn execute_with_command(
    executable: &str,
    args: Vec<String>,
    allowed_executables: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    execute_with_command_after_setup(
        executable,
        args,
        allowed_executables,
        env_allowlist,
        timeout_seconds,
        |_store, _project| {},
    )
}

pub(crate) fn execute_with_command_after_setup(
    executable: &str,
    args: Vec<String>,
    allowed_executables: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
    setup: impl FnOnce(&StateStore, &Path),
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement local process feature", "codex", vec![])
        .expect("create job");
    setup(&store, &project);
    let registry = registry_with_instance(
        executable,
        args,
        allowed_executables,
        env_allowlist,
        timeout_seconds,
    )
    .expect("registry");
    let request = ExecutionRequest::from_value(request_value(), "request.json", schema_root())
        .expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    match LocalProcessProviderAdapter.execute(&request, &context) {
        Ok(execution) => Ok((execution, project)),
        Err(error) => {
            fs::remove_dir_all(project).ok();
            Err(error)
        }
    }
}
