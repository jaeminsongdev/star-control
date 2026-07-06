use crate::cloud_constants::{CLI_TRANSPORT, CLOUD_API_KIND, CLOUD_CLI_KIND, HTTP_TRANSPORT};
use crate::ProviderManifest;

pub fn is_cloud_provider_manifest(manifest: &ProviderManifest) -> bool {
    (manifest.kind() == CLOUD_CLI_KIND && manifest.transport() == CLI_TRANSPORT)
        || (manifest.kind() == CLOUD_API_KIND && manifest.transport() == HTTP_TRANSPORT)
}

pub fn is_cloud_cli_manifest(manifest: &ProviderManifest) -> bool {
    manifest.kind() == CLOUD_CLI_KIND && manifest.transport() == CLI_TRANSPORT
}

pub fn is_cloud_api_manifest(manifest: &ProviderManifest) -> bool {
    manifest.kind() == CLOUD_API_KIND && manifest.transport() == HTTP_TRANSPORT
}
