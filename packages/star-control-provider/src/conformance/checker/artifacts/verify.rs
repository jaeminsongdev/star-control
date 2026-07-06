use super::super::super::error::ProviderConformanceError;
use super::super::super::helpers::check_provider_relative_path;
use crate::ProviderRunContext;

pub(crate) fn verify_checked_artifacts(
    context: &ProviderRunContext<'_>,
    job_id: &str,
    provider_instance_id: &str,
    checked_artifacts: &mut Vec<String>,
) -> Result<(), ProviderConformanceError> {
    checked_artifacts.sort();
    checked_artifacts.dedup();
    for path in checked_artifacts {
        check_provider_relative_path("checked_artifacts[]", path, provider_instance_id)?;
        let absolute = context.state_store().resolve_job_path(job_id, path)?;
        if !absolute.is_file() {
            return Err(ProviderConformanceError::ArtifactMissing { path: absolute });
        }
    }
    Ok(())
}
