mod adapter;
mod model;
mod output;
mod simulation;

pub use adapter::FakeProviderAdapter;
pub use model::{
    load_execution_request, ExecutionRequest, ProviderAdapter, ProviderAdapterError,
    ProviderExecution, ProviderRunContext, ProviderRunResult,
};
pub(crate) use output::{ensure_output_files_absent, provider_output_path};
pub use simulation::FakeProviderSimulation;

#[cfg(test)]
mod tests;
