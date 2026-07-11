use std::{fmt, sync::Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;

#[derive(Debug, Error)]
pub enum IdError {
    #[error("invalid {kind}: {value}")]
    Invalid { kind: &'static str, value: String },
}

macro_rules! prefixed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                let raw = value
                    .strip_prefix($prefix)
                    .ok_or_else(|| IdError::Invalid {
                        kind: stringify!($name),
                        value: value.clone(),
                    })?;
                Ulid::from_string(raw).map_err(|_| IdError::Invalid {
                    kind: stringify!($name),
                    value: value.clone(),
                })?;
                Ok(Self(value))
            }
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, Ulid::new()))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

prefixed_id!(RequestId, "req_");
prefixed_id!(OperationId, "opn_");
prefixed_id!(ApprovalId, "apr_");
prefixed_id!(ToolTrustId, "trt_");
prefixed_id!(ToolCacheId, "trc_");
prefixed_id!(ProjectId, "prj_");
prefixed_id!(GoalId, "gol_");
prefixed_id!(RunId, "run_");
prefixed_id!(StageId, "stg_");
prefixed_id!(ArtifactId, "art_");
prefixed_id!(DiagnosticId, "dia_");
prefixed_id!(ValidationRunId, "val_");
prefixed_id!(GateId, "gat_");
prefixed_id!(EvidenceBundleId, "evb_");
prefixed_id!(TaskInvocationId, "inv_");
prefixed_id!(WaiverId, "wav_");

/// Monotonic process-local ULID source for IDs where clock regression must not
/// reorder IDs created by this process.
#[derive(Default)]
pub struct MonotonicUlids(Mutex<Option<Ulid>>);

impl MonotonicUlids {
    pub fn next(&self) -> Ulid {
        let mut previous = self.0.lock().expect("monotonic ULID mutex poisoned");
        let next = Ulid::new();
        let chosen = match *previous {
            Some(last) if next <= last => Ulid::from_parts(last.timestamp_ms() + 1, 0),
            _ => next,
        };
        *previous = Some(chosen);
        chosen
    }
}
