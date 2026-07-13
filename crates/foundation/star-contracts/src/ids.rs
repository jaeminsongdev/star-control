use std::{
    fmt,
    sync::{Mutex, OnceLock},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
                if raw.len() != 26 || raw.bytes().any(|byte| byte.is_ascii_lowercase()) {
                    return Err(IdError::Invalid {
                        kind: stringify!($name),
                        value,
                    });
                }
                Ulid::from_string(raw).map_err(|_| IdError::Invalid {
                    kind: stringify!($name),
                    value: value.clone(),
                })?;
                Ok(Self(value))
            }
            pub fn new() -> Self {
                static SOURCE: OnceLock<MonotonicUlids> = OnceLock::new();
                Self(format!(
                    "{}{}",
                    $prefix,
                    SOURCE.get_or_init(MonotonicUlids::default).next()
                ))
            }
            pub fn from_stable_bytes(bytes: &[u8]) -> Self {
                let digest = Sha256::digest(bytes);
                let mut truncated = [0_u8; 16];
                truncated.copy_from_slice(&digest[..16]);
                Self(format!("{}{}", $prefix, Ulid::from_bytes(truncated)))
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

macro_rules! derived_id {
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
                if raw.len() != 52
                    || raw
                        .bytes()
                        .any(|byte| !matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
                {
                    return Err(IdError::Invalid {
                        kind: stringify!($name),
                        value,
                    });
                }
                Ok(Self(value))
            }
            pub fn new() -> Self {
                static SOURCE: OnceLock<MonotonicUlids> = OnceLock::new();
                let seed = SOURCE.get_or_init(MonotonicUlids::default).next();
                Self::from_stable_bytes(&seed.to_bytes())
            }
            pub fn from_stable_bytes(bytes: &[u8]) -> Self {
                let digest = Sha256::digest(bytes);
                Self(format!("{}{}", $prefix, base32_lower_no_pad(&digest)))
            }
            pub fn from_fingerprint(fingerprint: &crate::Sha256Hash) -> Self {
                let bytes = hex::decode(
                    fingerprint
                        .as_str()
                        .strip_prefix("sha256:")
                        .expect("Sha256Hash always has its prefix"),
                )
                .expect("Sha256Hash always contains valid hexadecimal");
                Self(format!("{}{}", $prefix, base32_lower_no_pad(&bytes)))
            }
            pub fn as_str(&self) -> &str {
                &self.0
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
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn base32_lower_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut output = String::with_capacity(bytes.len().saturating_mul(8).div_ceil(5));
    let mut buffer = 0_u16;
    let mut bits = 0_u8;
    for byte in bytes {
        buffer = (buffer << 8) | u16::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
            buffer &= (1_u16 << bits).saturating_sub(1);
        }
    }
    if bits > 0 {
        output.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    output
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
derived_id!(ProjectRevisionId, "prv_");
derived_id!(WorkspaceSnapshotId, "wsp_");
prefixed_id!(ScanRunId, "scn_");
derived_id!(FindingId, "fnd_");
derived_id!(OccurrenceId, "occ_");
derived_id!(SymbolId, "sym_");
derived_id!(SymbolReferenceId, "srf_");
derived_id!(CanonicalSourceId, "src_");
prefixed_id!(SuppressionId, "sup_");
prefixed_id!(BaselineId, "bas_");
prefixed_id!(DispositionId, "dsp_");
prefixed_id!(ChangePlanId, "cpl_");
prefixed_id!(PatchSetId, "pat_");
prefixed_id!(ValidationResultId, "vrs_");
prefixed_id!(ManagementStoreId, "mst_");
prefixed_id!(CoordinatedOperationId, "cop_");
prefixed_id!(RootBindingId, "rtb_");
prefixed_id!(GenerationId, "gen_");
prefixed_id!(EventId, "evt_");

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
