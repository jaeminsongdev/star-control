use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderManifest {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) transport: String,
    pub(crate) adapter: String,
    pub(crate) path: PathBuf,
    pub(crate) value: Value,
}

impl ProviderManifest {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn transport(&self) -> &str {
        &self.transport
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
