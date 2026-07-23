//! P0 domain invariants shared by every entry adapter.

pub mod recovery;

use std::collections::BTreeSet;

use chrono::{Duration, Utc};
use serde::Serialize;
use star_contracts::{
    Sha256Hash, canonical_sha256,
    management::{
        Baseline, CoordinatedOperation, CoordinationParticipant, StoreVersionVector, Suppression,
    },
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("canonical fingerprint generation failed")]
    Fingerprint,
    #[error("baseline activation requires an explicitly reviewed complete scan")]
    BaselineNotReviewed,
    #[error("permanent suppression requires a non-empty justification")]
    PermanentSuppressionWithoutJustification,
    #[error("non-permanent suppression requires an expiry in the future")]
    InvalidSuppressionExpiry,
    #[error("suppression selector is invalid")]
    InvalidSuppressionSelector,
    #[error("coordination participants must be sorted, unique, and non-empty")]
    InvalidParticipants,
    #[error("coordination idempotency key must be 1 to 128 non-NUL characters")]
    InvalidIdempotencyKey,
    #[error("store version vector projects must be sorted and unique")]
    InvalidStoreVersionVector,
    #[error("a prohibited raw value reached the persistence boundary")]
    ProhibitedRawValue,
}

pub fn versioned_fingerprint(
    namespace: &str,
    contract_version: u32,
    payload: &impl Serialize,
) -> Result<Sha256Hash, DomainError> {
    let payload = serde_json::to_value(payload).map_err(|_| DomainError::Fingerprint)?;
    canonical_sha256(&serde_json::json!({
        "algorithm": namespace,
        "contract_version": contract_version,
        "payload": payload,
    }))
    .map_err(|_| DomainError::Fingerprint)
}

pub fn validate_baseline(baseline: &Baseline) -> Result<(), DomainError> {
    if !baseline.reviewed
        || baseline
            .finding_fingerprints
            .windows(2)
            .any(|v| v[0] >= v[1])
    {
        return Err(DomainError::BaselineNotReviewed);
    }
    Ok(())
}

pub fn validate_suppression(suppression: &Suppression) -> Result<(), DomainError> {
    if !valid_suppression_selector(&suppression.selector) {
        return Err(DomainError::InvalidSuppressionSelector);
    }
    if suppression.permanent {
        if suppression.expires_at.is_some()
            || suppression
                .justification
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(DomainError::PermanentSuppressionWithoutJustification);
        }
    } else if suppression
        .expires_at
        .is_none_or(|expiry| expiry <= suppression.created_at)
    {
        return Err(DomainError::InvalidSuppressionExpiry);
    }
    Ok(())
}

fn valid_suppression_selector(selector: &str) -> bool {
    let Some((kind, value)) = selector.split_once(':') else {
        return false;
    };
    if value.is_empty() || value.contains('\0') {
        return false;
    }
    match kind {
        "finding" => value.parse::<Sha256Hash>().is_ok(),
        "rule" => value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')),
        "symbol" => star_contracts::ids::SymbolId::parse(value).is_ok(),
        "path" => {
            !value.starts_with('/')
                && !value.ends_with('/')
                && !value.contains("//")
                && !value.contains('\\')
                && !value.contains(':')
                && value
                    .split('/')
                    .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
        }
        _ => false,
    }
}

pub fn default_suppression_expiry() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::days(90)
}

pub fn validate_coordination(operation: &CoordinatedOperation) -> Result<(), DomainError> {
    if operation.idempotency_key.trim().is_empty()
        || operation.idempotency_key.chars().count() > 128
        || operation.idempotency_key.contains('\0')
    {
        return Err(DomainError::InvalidIdempotencyKey);
    }
    validate_participants(&operation.participants)?;
    validate_version_vector(&operation.expected_version_vector)?;
    if let Some(vector) = &operation.committed_version_vector {
        validate_version_vector(vector)?;
    }
    Ok(())
}

fn validate_participants(participants: &[CoordinationParticipant]) -> Result<(), DomainError> {
    if participants.is_empty()
        || participants
            .windows(2)
            .any(|pair| pair[0].project_id >= pair[1].project_id)
    {
        return Err(DomainError::InvalidParticipants);
    }
    Ok(())
}

pub fn validate_version_vector(vector: &StoreVersionVector) -> Result<(), DomainError> {
    if vector
        .projects
        .windows(2)
        .any(|pair| pair[0].project_id >= pair[1].project_id)
    {
        return Err(DomainError::InvalidStoreVersionVector);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct PersistenceRedactor {
    current_user_tokens: BTreeSet<String>,
}

impl PersistenceRedactor {
    pub fn for_current_user() -> Self {
        let mut tokens = BTreeSet::new();
        for value in [
            std::env::var_os("USERNAME"),
            std::env::var_os("USERPROFILE"),
        ]
        .into_iter()
        .flatten()
        {
            let value = value.to_string_lossy().trim().to_lowercase();
            if value.len() >= 3 {
                tokens.insert(value.clone());
                if let Some(last) = value.replace('\\', "/").rsplit('/').next()
                    && last.len() >= 3
                {
                    tokens.insert(last.to_owned());
                }
            }
        }
        Self {
            current_user_tokens: tokens,
        }
    }

    pub fn validate(&self, value: &str) -> Result<(), DomainError> {
        let lower = value.to_lowercase();
        let secret_marker = [
            "password=",
            "password:",
            "token=",
            "api_key",
            "authorization:",
            "-----begin private key-----",
        ]
        .iter()
        .any(|marker| lower.contains(marker));
        let bytes = value.as_bytes();
        let absolute_path = value.contains("\\\\")
            || bytes.windows(3).enumerate().any(|(index, window)| {
                let token_boundary = index == 0
                    || !matches!(
                        bytes[index - 1],
                        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'-' | b'.'
                    );
                token_boundary
                    && window[0].is_ascii_alphabetic()
                    && window[1] == b':'
                    && matches!(window[2], b'\\' | b'/')
            });
        if secret_marker
            || absolute_path
            || self
                .current_user_tokens
                .iter()
                .any(|token| lower.contains(token))
        {
            return Err(DomainError::ProhibitedRawValue);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redactor_rejects_secret_and_absolute_path_without_hashing_them() {
        let redactor = PersistenceRedactor {
            current_user_tokens: BTreeSet::from(["alice".to_owned()]),
        };
        for value in ["token=secret", r"C:\Users\person\repo", "owner=alice"] {
            assert_eq!(
                redactor.validate(value),
                Err(DomainError::ProhibitedRawValue)
            );
        }
        assert!(redactor.validate("src/lib.rs").is_ok());
        assert!(redactor.validate("https://example.invalid/api").is_ok());
    }
}
