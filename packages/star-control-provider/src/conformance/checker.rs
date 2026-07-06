mod artifacts;
mod stored;

use super::error::ProviderConformanceError;
use super::helpers::{check_result_field, check_safe_segment};
use super::types::{ProviderConformanceProfile, ProviderConformanceReport};
use crate::{ProviderExecution, ProviderRunContext};
use artifacts::{collect_checked_artifacts, verify_checked_artifacts};
use stored::{validate_cloud_sidecars, validate_stored_response_artifact};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderConformanceChecker;

impl ProviderConformanceChecker {
    pub fn check_execution(
        &self,
        execution: &ProviderExecution,
        context: &ProviderRunContext<'_>,
        profile: ProviderConformanceProfile,
    ) -> Result<ProviderConformanceReport, ProviderConformanceError> {
        let result = execution.result();
        let value = result.value();
        let provider_instance_id = result.provider_instance_id();
        let job_id = result.job_id();
        let mut checked_artifacts = Vec::new();

        check_safe_segment("provider_instance_id", provider_instance_id)?;
        check_result_field(value, "provider_instance_id", provider_instance_id)?;
        check_result_field(value, "job_id", job_id)?;
        check_result_field(value, "stage", result.stage())?;
        check_result_field(value, "status", result.status())?;

        collect_checked_artifacts(
            execution,
            value,
            provider_instance_id,
            profile,
            &mut checked_artifacts,
        )?;
        verify_checked_artifacts(
            context,
            job_id,
            provider_instance_id,
            &mut checked_artifacts,
        )?;
        validate_stored_response_artifact(context, job_id, provider_instance_id, value)?;

        if profile == ProviderConformanceProfile::Cloud {
            validate_cloud_sidecars(context, job_id, provider_instance_id, result.stage())?;
        }

        Ok(ProviderConformanceReport::new(
            provider_instance_id.to_string(),
            job_id.to_string(),
            result.status().to_string(),
            checked_artifacts,
        ))
    }
}
