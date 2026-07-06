use super::fields::{
    nested_required_string, pointer_string_array, required_bool, required_string,
    required_string_array,
};
use super::ProviderRegistryLoader;
use crate::registry_domain::{
    CapabilityProfile, ProviderInstance, ProviderManifest, ProviderRegistryDocument,
    ProviderRegistryEntry,
};
use crate::registry_error::ProviderRegistryError;
use serde_json::Value;
use std::path::Path;

const PROVIDER_MANIFEST_SCHEMA: &str = "provider-manifest.schema.json";
const PROVIDER_INSTANCE_SCHEMA: &str = "provider-instance.schema.json";
const CAPABILITY_PROFILE_SCHEMA: &str = "capability-profile.schema.json";
const PROVIDER_REGISTRY_SCHEMA: &str = "provider-registry.schema.json";

impl ProviderRegistryLoader {
    pub fn load_manifest(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderManifest, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_MANIFEST_SCHEMA)?;

        Ok(ProviderManifest {
            id: required_string(&value, &path, "id")?,
            kind: required_string(&value, &path, "kind")?,
            transport: required_string(&value, &path, "transport")?,
            adapter: required_string(&value, &path, "adapter")?,
            path,
            value,
        })
    }

    pub fn load_instance(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderInstance, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_INSTANCE_SCHEMA)?;

        Ok(ProviderInstance {
            id: required_string(&value, &path, "id")?,
            provider_id: required_string(&value, &path, "provider")?,
            enabled: required_bool(&value, &path, "enabled")?,
            routing_tags: required_string_array(&value, &path, "routing_tags")?,
            path,
            value,
        })
    }

    pub fn load_capability_profile(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<CapabilityProfile, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, CAPABILITY_PROFILE_SCHEMA)?;

        Ok(CapabilityProfile {
            provider_id: required_string(&value, &path, "provider")?,
            routing_tags: pointer_string_array(&value, &path, "/capability_profile/routing_tags")?,
            path,
            value,
        })
    }

    pub fn load_registry_document(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderRegistryDocument, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_REGISTRY_SCHEMA)?;

        let schema_version = match value.get("schema_version") {
            Some(Value::String(version)) => version.clone(),
            Some(Value::Number(version)) => version.to_string(),
            Some(_) => {
                return Err(ProviderRegistryError::InvalidFieldType {
                    path,
                    field: "schema_version".to_string(),
                    expected: "string or number".to_string(),
                });
            }
            None => {
                return Err(ProviderRegistryError::MissingField {
                    path,
                    field: "schema_version".to_string(),
                });
            }
        };

        let providers = value
            .get("providers")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
                path: path.clone(),
                field: "providers".to_string(),
                expected: "array".to_string(),
            })?;
        let mut entries = Vec::with_capacity(providers.len());
        for (index, provider) in providers.iter().enumerate() {
            let entry_path = format!("providers[{}]", index);
            entries.push(ProviderRegistryEntry {
                id: nested_required_string(provider, &path, &entry_path, "id")?,
                manifest: nested_required_string(provider, &path, &entry_path, "manifest")?,
                capabilities: nested_required_string(provider, &path, &entry_path, "capabilities")?,
            });
        }

        Ok(ProviderRegistryDocument {
            schema_version,
            entries,
            path,
            value,
        })
    }
}
