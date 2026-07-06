#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConformanceProfile {
    Basic,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConformanceReport {
    provider_instance_id: String,
    job_id: String,
    status: String,
    checked_artifacts: Vec<String>,
}

impl ProviderConformanceReport {
    pub(super) fn new(
        provider_instance_id: String,
        job_id: String,
        status: String,
        checked_artifacts: Vec<String>,
    ) -> Self {
        Self {
            provider_instance_id,
            job_id,
            status,
            checked_artifacts,
        }
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn checked_artifacts(&self) -> &[String] {
        &self.checked_artifacts
    }
}
