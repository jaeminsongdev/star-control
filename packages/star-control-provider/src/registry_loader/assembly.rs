use super::ProviderRegistryLoader;
use crate::registry_domain::ProviderRegistry;
use crate::registry_error::ProviderRegistryError;
use std::path::{Path, PathBuf};

impl ProviderRegistryLoader {
    pub fn load_registry(
        &self,
        registry_path: impl AsRef<Path>,
        instance_paths: &[PathBuf],
    ) -> Result<ProviderRegistry, ProviderRegistryError> {
        let registry_document = self.load_registry_document(registry_path)?;
        let mut registry = ProviderRegistry::new();

        for entry in registry_document.entries() {
            let manifest_path = self.resolve_registry_entry_path(entry.manifest())?;
            let manifest = self.load_manifest(&manifest_path)?;
            if manifest.id() != entry.id() {
                return Err(ProviderRegistryError::RegistryManifestIdMismatch {
                    registry_id: entry.id().to_string(),
                    manifest_id: manifest.id().to_string(),
                    manifest_path,
                });
            }
            registry.register_manifest(manifest)?;

            let capability_path = self.resolve_registry_entry_path(entry.capabilities())?;
            let profile = self.load_capability_profile(&capability_path)?;
            if profile.provider_id() != entry.id() {
                return Err(ProviderRegistryError::RegistryCapabilityProviderMismatch {
                    registry_id: entry.id().to_string(),
                    capability_provider: profile.provider_id().to_string(),
                    capability_path,
                });
            }
            registry.register_capability_profile(profile)?;
        }

        for instance_path in instance_paths {
            let instance = self.load_instance(instance_path)?;
            registry.register_instance(instance)?;
        }

        Ok(registry)
    }

    pub fn load_fake_default_registry(&self) -> Result<ProviderRegistry, ProviderRegistryError> {
        self.load_registry(
            "examples/provider-contracts/provider-registry.example.json",
            &[PathBuf::from(
                "examples/provider-contracts/provider-instance.fake.example.json",
            )],
        )
    }
}
