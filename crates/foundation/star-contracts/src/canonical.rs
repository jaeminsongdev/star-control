use std::{fmt, io, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("JCS canonicalization failed: {0}")]
    Jcs(String),
    #[error("hash must have the sha256: prefix and 64 lowercase hexadecimal characters")]
    InvalidHash,
}

/// Contract representation of a SHA-256 digest.  The prefix is always present.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct Sha256Hash(String);

impl Sha256Hash {
    pub fn digest(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(format!("sha256:{}", hex::encode(digest)))
    }

    pub fn digest_reader(mut reader: impl io::Read) -> io::Result<Self> {
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(Self(format!("sha256:{}", hex::encode(hasher.finalize()))))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Sha256Hash {
    type Err = CanonicalError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = value.len() == 71
            && value.starts_with("sha256:")
            && value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        valid
            .then(|| Self(value.to_owned()))
            .ok_or(CanonicalError::InvalidHash)
    }
}

/// RFC 8785 canonical JSON bytes using the frozen canonicalizer dependency.
pub fn jcs_bytes(value: &serde_json::Value) -> Result<Vec<u8>, CanonicalError> {
    serde_json_canonicalizer::to_string(value)
        .map(|text| text.into_bytes())
        .map_err(|error| CanonicalError::Jcs(error.to_string()))
}

pub fn canonical_sha256(value: &serde_json::Value) -> Result<Sha256Hash, CanonicalError> {
    Ok(Sha256Hash::digest(&jcs_bytes(value)?))
}
