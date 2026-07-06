use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryEntry {
    pub(crate) id: String,
    pub(crate) manifest: String,
    pub(crate) capabilities: String,
}

impl ProviderRegistryEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn manifest(&self) -> &str {
        &self.manifest
    }

    pub fn capabilities(&self) -> &str {
        &self.capabilities
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRegistryDocument {
    pub(crate) schema_version: String,
    pub(crate) entries: Vec<ProviderRegistryEntry>,
    pub(crate) path: PathBuf,
    pub(crate) value: Value,
}

impl ProviderRegistryDocument {
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn entries(&self) -> &[ProviderRegistryEntry] {
        &self.entries
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
