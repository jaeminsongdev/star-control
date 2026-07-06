use super::super::{
    CloudApiOfflineProviderAdapter, CloudCliProviderAdapter, CloudProviderPreflightAdapter,
};
use super::registry::registry_with_instance;
use super::request::request_value;
use super::temp::{schema_root, temp_project};
use crate::cloud_constants::{CLI_TRANSPORT, CLOUD_API_KIND, CLOUD_CLI_KIND, HTTP_TRANSPORT};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
};
use serde_json::Value;
use star_control_state::StateStore;
use std::fs;
use std::path::PathBuf;

pub(crate) fn execute_cloud_provider(
    kind: &str,
    transport: &str,
    instance_value: Value,
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    let project = temp_project();
    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open store");
    store
        .create_job("use cloud provider", "codex", vec![])
        .expect("create job");
    let registry =
        registry_with_instance(kind, transport, instance_value).expect("register cloud provider");
    let request =
        ExecutionRequest::from_value(request_value(), "request.json", &schemas).expect("request");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    match CloudProviderPreflightAdapter.execute(&request, &context) {
        Ok(execution) => Ok((execution, project)),
        Err(error) => {
            fs::remove_dir_all(project).ok();
            Err(error)
        }
    }
}

pub(crate) fn execute_cloud_cli_transport(
    instance_value: Value,
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    let project = temp_project();
    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open store");
    store
        .create_job("use cloud CLI provider", "codex", vec![])
        .expect("create job");
    let registry = registry_with_instance(CLOUD_CLI_KIND, CLI_TRANSPORT, instance_value)
        .expect("register cloud provider");
    let request =
        ExecutionRequest::from_value(request_value(), "request.json", &schemas).expect("request");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    match CloudCliProviderAdapter.execute(&request, &context) {
        Ok(execution) => Ok((execution, project)),
        Err(error) => {
            fs::remove_dir_all(project).ok();
            Err(error)
        }
    }
}

pub(crate) fn execute_cloud_api_offline(
    instance_value: Value,
    fixture_relative_path: &str,
    fixture_value: &Value,
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    let project = temp_project();
    let fixture_path = project.join(fixture_relative_path);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(
        &fixture_path,
        serde_json::to_string_pretty(fixture_value).expect("serialize response fixture"),
    )
    .expect("write response fixture");
    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open store");
    store
        .create_job("use cloud API provider", "codex", vec![])
        .expect("create job");
    let registry = registry_with_instance(CLOUD_API_KIND, HTTP_TRANSPORT, instance_value)
        .expect("register cloud API provider");
    let request =
        ExecutionRequest::from_value(request_value(), "request.json", &schemas).expect("request");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    match CloudApiOfflineProviderAdapter.execute(&request, &context) {
        Ok(execution) => Ok((execution, project)),
        Err(error) => {
            fs::remove_dir_all(project).ok();
            Err(error)
        }
    }
}

pub(crate) fn execute_cloud_api_live_approval(
    instance_value: Value,
) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
    let project = temp_project();
    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open store");
    store
        .create_job("use cloud API provider", "codex", vec![])
        .expect("create job");
    let registry = registry_with_instance(CLOUD_API_KIND, HTTP_TRANSPORT, instance_value)
        .expect("register cloud API provider");
    let request =
        ExecutionRequest::from_value(request_value(), "request.json", &schemas).expect("request");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    match CloudApiOfflineProviderAdapter.execute(&request, &context) {
        Ok(execution) => Ok((execution, project)),
        Err(error) => {
            fs::remove_dir_all(project).ok();
            Err(error)
        }
    }
}
