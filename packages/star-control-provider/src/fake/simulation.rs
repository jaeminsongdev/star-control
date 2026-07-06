use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeProviderSimulation {
    Success,
    Failed(String),
    Blocked(String),
}

impl FakeProviderSimulation {
    pub(super) fn status(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed(_) => "failed",
            Self::Blocked(_) => "blocked",
        }
    }

    pub(super) fn summary(&self) -> String {
        match self {
            Self::Success => "fake provider completed".to_string(),
            Self::Failed(message) => format!("fake provider failed: {}", message),
            Self::Blocked(reason) => format!("fake provider blocked: {}", reason),
        }
    }

    pub(super) fn stdout(&self) -> String {
        match self {
            Self::Success => "fake provider completed\n".to_string(),
            Self::Failed(message) => format!("fake provider failed: {}\n", message),
            Self::Blocked(reason) => format!("fake provider blocked: {}\n", reason),
        }
    }

    pub(super) fn stderr(&self) -> Option<String> {
        match self {
            Self::Success => None,
            Self::Failed(message) => Some(format!("fake failure: {}\n", message)),
            Self::Blocked(reason) => Some(format!("fake blocked: {}\n", reason)),
        }
    }

    pub(super) fn error(&self) -> Value {
        match self {
            Self::Success => Value::Null,
            Self::Failed(message) => json!({
                "kind": "fake_failed",
                "message": message,
            }),
            Self::Blocked(reason) => json!({
                "kind": "fake_blocked",
                "message": reason,
            }),
        }
    }
}
