mod cloud;
mod cloud_api_artifacts;
mod cloud_cli;
mod cloud_constants;
mod cloud_io;
mod cloud_policy;
mod cloud_sidecars;
mod conformance;
mod fake;
mod local_process;
mod openai_compatible;
mod provider_cost;
mod provider_redaction;
mod registry_domain;
mod registry_error;
mod registry_loader;
#[cfg(test)]
mod registry_tests;
mod registry_yaml;

pub use cloud::{
    is_cloud_api_manifest, is_cloud_cli_manifest, is_cloud_provider_manifest,
    CloudApiOfflineProviderAdapter, CloudCliProviderAdapter, CloudProviderPreflightAdapter,
};
pub use conformance::{
    ProviderConformanceChecker, ProviderConformanceError, ProviderConformanceProfile,
    ProviderConformanceReport,
};
pub use fake::{
    load_execution_request, ExecutionRequest, FakeProviderAdapter, FakeProviderSimulation,
    ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
pub use local_process::{LocalProcessCommandPolicy, LocalProcessProviderAdapter};
pub use openai_compatible::{
    OpenAiCompatibleParseError, OpenAiCompatibleParsedResponse, OpenAiCompatiblePreparedRequest,
    OpenAiCompatibleRequestApi, OpenAiCompatibleRequestBuilder, OpenAiCompatibleRequestError,
    OpenAiCompatibleResponseKind, OpenAiCompatibleResponseParser,
};
pub use registry_domain::{
    CapabilityProfile, CapabilityValue, ProviderInstance, ProviderManifest, ProviderRegistry,
    ProviderRegistryDocument, ProviderRegistryEntry,
};
pub use registry_error::ProviderRegistryError;
pub use registry_loader::ProviderRegistryLoader;
