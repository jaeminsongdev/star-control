use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityProfile {
    pub(crate) provider_id: String,
    pub(crate) routing_tags: Vec<String>,
    pub(crate) path: PathBuf,
    pub(crate) value: Value,
}

impl CapabilityProfile {
    pub fn provider_id(&self) -> &str {
        &self.provider_id
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

    pub fn capability(&self, name: &str) -> Option<CapabilityValue<'_>> {
        self.value
            .pointer("/capability_profile/can")
            .and_then(Value::as_object)
            .and_then(|can| can.get(name))
            .and_then(CapabilityValue::from_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityValue<'a> {
    Bool(bool),
    Mode(&'a str),
}

impl<'a> CapabilityValue<'a> {
    pub fn from_value(value: &'a Value) -> Option<Self> {
        if let Some(flag) = value.as_bool() {
            return Some(Self::Bool(flag));
        }

        value.as_str().map(Self::Mode)
    }

    pub fn is_enabled(self) -> bool {
        match self {
            Self::Bool(flag) => flag,
            Self::Mode(mode) => matches!(mode, "true" | "partial" | "manual"),
        }
    }
}
