use super::capability::CapabilityProfile;
use super::instance::ProviderInstance;
use super::manifest::ProviderManifest;
use crate::registry_error::ProviderRegistryError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderRegistry {
    manifests: BTreeMap<String, ProviderManifest>,
    pub(crate) capabilities: BTreeMap<String, CapabilityProfile>,
    instances: BTreeMap<String, ProviderInstance>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_manifest(
        &mut self,
        manifest: ProviderManifest,
    ) -> Result<(), ProviderRegistryError> {
        let provider_id = manifest.id().to_string();
        if self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::DuplicateProvider { provider_id });
        }

        self.manifests.insert(provider_id, manifest);
        Ok(())
    }

    pub fn register_capability_profile(
        &mut self,
        profile: CapabilityProfile,
    ) -> Result<(), ProviderRegistryError> {
        let provider_id = profile.provider_id().to_string();
        if !self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::ProviderNotFound { provider_id });
        }
        if self.capabilities.contains_key(&provider_id) {
            return Err(ProviderRegistryError::DuplicateCapabilityProfile { provider_id });
        }

        self.capabilities.insert(provider_id, profile);
        Ok(())
    }

    pub fn register_instance(
        &mut self,
        instance: ProviderInstance,
    ) -> Result<(), ProviderRegistryError> {
        let instance_id = instance.id().to_string();
        let provider_id = instance.provider_id().to_string();
        if !self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::ProviderNotFound { provider_id });
        }
        if self.instances.contains_key(&instance_id) {
            return Err(ProviderRegistryError::DuplicateInstance { instance_id });
        }

        self.instances.insert(instance_id, instance);
        Ok(())
    }

    pub fn manifest(&self, provider_id: &str) -> Option<&ProviderManifest> {
        self.manifests.get(provider_id)
    }

    pub fn capability_profile(&self, provider_id: &str) -> Option<&CapabilityProfile> {
        self.capabilities.get(provider_id)
    }

    pub fn instance(&self, instance_id: &str) -> Option<&ProviderInstance> {
        self.instances.get(instance_id)
    }

    pub fn providers(&self) -> Vec<&ProviderManifest> {
        self.manifests.values().collect()
    }

    pub fn manifest_for_instance(
        &self,
        instance_id: &str,
    ) -> Result<&ProviderManifest, ProviderRegistryError> {
        let instance =
            self.instance(instance_id)
                .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;
        self.manifest(instance.provider_id()).ok_or_else(|| {
            ProviderRegistryError::ProviderNotFound {
                provider_id: instance.provider_id().to_string(),
            }
        })
    }

    pub fn capability_for_instance(
        &self,
        instance_id: &str,
    ) -> Result<&CapabilityProfile, ProviderRegistryError> {
        let instance =
            self.instance(instance_id)
                .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;
        self.capability_profile(instance.provider_id())
            .ok_or_else(|| ProviderRegistryError::CapabilityProfileNotFound {
                provider_id: instance.provider_id().to_string(),
            })
    }

    pub fn providers_by_kind(&self, kind: &str) -> Vec<&ProviderManifest> {
        self.manifests
            .values()
            .filter(|manifest| manifest.kind() == kind)
            .collect()
    }

    pub fn providers_by_transport(&self, transport: &str) -> Vec<&ProviderManifest> {
        self.manifests
            .values()
            .filter(|manifest| manifest.transport() == transport)
            .collect()
    }

    pub fn instances_for_provider(&self, provider_id: &str) -> Vec<&ProviderInstance> {
        self.instances
            .values()
            .filter(|instance| instance.provider_id() == provider_id)
            .collect()
    }

    pub fn enabled_instances(&self) -> Vec<&ProviderInstance> {
        self.instances
            .values()
            .filter(|instance| instance.enabled())
            .collect()
    }
}
