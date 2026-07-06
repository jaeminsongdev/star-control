use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInstance {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) enabled: bool,
    pub(crate) routing_tags: Vec<String>,
    pub(crate) path: PathBuf,
    pub(crate) value: Value,
}

impl ProviderInstance {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn routing_tags(&self) -> &[String] {
        &self.routing_tags
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
