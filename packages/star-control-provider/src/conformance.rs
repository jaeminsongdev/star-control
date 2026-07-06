mod checker;
mod error;
mod helpers;
mod types;

pub use checker::ProviderConformanceChecker;
pub use error::ProviderConformanceError;
pub use types::{ProviderConformanceProfile, ProviderConformanceReport};

#[cfg(test)]
pub(crate) use helpers::{check_provider_relative_path, provider_path};

const PROVIDER_RUN_RESULT_SCHEMA: &str = "provider-run-result.schema.json";
const REQUEST_FILE: &str = "request.json";
const RESPONSE_FILE: &str = "response.json";
const STDOUT_FILE: &str = "stdout.txt";
const STDERR_FILE: &str = "stderr.txt";
const PRIVACY_HANDOFF_FILE: &str = "privacy-handoff.json";
const COST_METRIC_FILE: &str = "cost-metric.json";
const PRIVACY_HANDOFF_SCHEMA: &str = "privacy-handoff.schema.json";
const COST_METRIC_SCHEMA: &str = "cost-metric.schema.json";
const PROVIDER_OUTPUT_KIND: &str = "provider_output";
const LOG_KIND: &str = "log";

#[cfg(test)]
mod tests;
