use crate::contract::required_string;
use crate::error::ExecutionError;
use serde_json::Value;
use star_control_provider::{ExecutionRequest, ProviderExecution};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionOutcome {
    pub(crate) request: ExecutionRequest,
    pub(crate) provider_execution: ProviderExecution,
    pub(crate) attempt: Value,
    pub(crate) state: Value,
}

impl ExecutionOutcome {
    pub fn request(&self) -> &ExecutionRequest {
        &self.request
    }

    pub fn provider_execution(&self) -> &ProviderExecution {
        &self.provider_execution
    }

    pub fn attempt(&self) -> &Value {
        &self.attempt
    }

    pub fn state(&self) -> &Value {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderAssignment {
    pub(crate) provider: String,
    pub(crate) provider_instance: String,
}

impl ProviderAssignment {
    pub(crate) fn from_workspec(workspec: &Value, stage: &str) -> Result<Self, ExecutionError> {
        let path = Path::new("workspec.json");
        let provider = required_string(workspec, path, "provider").map_err(|_| {
            ExecutionError::ProviderAssignmentMissing {
                stage: stage.to_string(),
            }
        })?;
        let provider_instance =
            required_string(workspec, path, "provider_instance").map_err(|_| {
                ExecutionError::ProviderAssignmentMissing {
                    stage: stage.to_string(),
                }
            })?;
        if provider != provider_instance {
            return Err(ExecutionError::ProviderAssignmentMismatch {
                provider,
                provider_instance,
            });
        }
        Ok(Self {
            provider,
            provider_instance,
        })
    }
}
