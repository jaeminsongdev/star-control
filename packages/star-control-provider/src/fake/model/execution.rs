use super::error::ProviderAdapterError;
use super::request::ExecutionRequest;
use super::result::ProviderRunResult;
use crate::ProviderRegistry;
use serde_json::Value;
use star_control_state::StateStore;
use std::path::Path;

pub trait ProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError>;
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderRunContext<'a> {
    registry: &'a ProviderRegistry,
    state_store: &'a StateStore,
    schema_root: &'a Path,
}

impl<'a> ProviderRunContext<'a> {
    pub fn new(
        registry: &'a ProviderRegistry,
        state_store: &'a StateStore,
        schema_root: &'a Path,
    ) -> Self {
        Self {
            registry,
            state_store,
            schema_root,
        }
    }

    pub fn registry(&self) -> &ProviderRegistry {
        self.registry
    }

    pub fn state_store(&self) -> &StateStore {
        self.state_store
    }

    pub fn schema_root(&self) -> &Path {
        self.schema_root
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderExecution {
    result: ProviderRunResult,
    request_ref: Value,
    response_ref: Value,
    stdout_ref: Value,
    stderr_ref: Option<Value>,
}

impl ProviderExecution {
    pub(crate) fn new(
        result: ProviderRunResult,
        request_ref: Value,
        response_ref: Value,
        stdout_ref: Value,
        stderr_ref: Option<Value>,
    ) -> Self {
        Self {
            result,
            request_ref,
            response_ref,
            stdout_ref,
            stderr_ref,
        }
    }

    pub fn result(&self) -> &ProviderRunResult {
        &self.result
    }

    pub fn request_ref(&self) -> &Value {
        &self.request_ref
    }

    pub fn response_ref(&self) -> &Value {
        &self.response_ref
    }

    pub fn stdout_ref(&self) -> &Value {
        &self.stdout_ref
    }

    pub fn stderr_ref(&self) -> Option<&Value> {
        self.stderr_ref.as_ref()
    }
}
