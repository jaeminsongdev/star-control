mod cloud;
mod declared;
mod optional;
mod verify;

use super::super::error::ProviderConformanceError;
use super::super::helpers::{
    check_path_equals, check_ref_contract, provider_path, required_string,
};
use super::super::types::ProviderConformanceProfile;
use super::super::{LOG_KIND, PROVIDER_OUTPUT_KIND, REQUEST_FILE, RESPONSE_FILE, STDOUT_FILE};
use crate::ProviderExecution;
use serde_json::Value;

use cloud::collect_required_cloud_artifacts;
use declared::collect_declared_artifacts;
use optional::collect_stderr_artifact;
pub(super) use verify::verify_checked_artifacts;

pub(super) fn collect_checked_artifacts(
    execution: &ProviderExecution,
    value: &Value,
    provider_instance_id: &str,
    profile: ProviderConformanceProfile,
    checked_artifacts: &mut Vec<String>,
) -> Result<(), ProviderConformanceError> {
    check_ref_contract(
        execution.request_ref(),
        "request_ref",
        &provider_path(provider_instance_id, REQUEST_FILE),
        PROVIDER_OUTPUT_KIND,
        provider_instance_id,
    )?;
    check_ref_contract(
        execution.response_ref(),
        "response_ref",
        &provider_path(provider_instance_id, RESPONSE_FILE),
        PROVIDER_OUTPUT_KIND,
        provider_instance_id,
    )?;
    check_ref_contract(
        execution.stdout_ref(),
        "stdout_ref",
        &provider_path(provider_instance_id, STDOUT_FILE),
        LOG_KIND,
        provider_instance_id,
    )?;

    let stdout_path = required_string(value, "stdout_path")?;
    check_path_equals(
        "stdout_path",
        &stdout_path,
        &provider_path(provider_instance_id, STDOUT_FILE),
    )?;

    checked_artifacts.push(provider_path(provider_instance_id, REQUEST_FILE));
    checked_artifacts.push(provider_path(provider_instance_id, RESPONSE_FILE));
    checked_artifacts.push(provider_path(provider_instance_id, STDOUT_FILE));
    collect_stderr_artifact(execution, value, provider_instance_id, checked_artifacts)?;
    collect_declared_artifacts(value, provider_instance_id, checked_artifacts)?;

    if profile == ProviderConformanceProfile::Cloud {
        collect_required_cloud_artifacts(value, provider_instance_id, checked_artifacts)?;
    }
    Ok(())
}
