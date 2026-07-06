mod api_live;
mod api_offline_adapter;
mod cli_adapter;
mod manifest;
mod preflight_adapter;

pub use api_offline_adapter::CloudApiOfflineProviderAdapter;
pub use cli_adapter::CloudCliProviderAdapter;
pub use manifest::{is_cloud_api_manifest, is_cloud_cli_manifest, is_cloud_provider_manifest};
pub use preflight_adapter::CloudProviderPreflightAdapter;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;
