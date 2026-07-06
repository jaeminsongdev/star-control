use super::ValidationEngine;
use crate::error::ValidationEngineError;

impl<'a> ValidationEngine<'a> {
    pub fn ensure_provider_response(
        &self,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<(), ValidationEngineError> {
        let path = self.state_store.resolve_job_path(
            job_id,
            &format!("provider-output/{}/response.json", provider_instance_id),
        )?;
        if path.is_file() {
            Ok(())
        } else {
            Err(ValidationEngineError::ProviderOutputMissing { path })
        }
    }
}
