//! Closed ErrorEnvelope code registry.
//!
//! The checked-in catalog is embedded in every binary. Runtime input can only
//! become a `StableErrorCode` when it is an exact catalog member; unknown
//! values remain compatibility strings and cannot be advertised as stable.

use std::{borrow::Cow, fmt, str::FromStr, sync::OnceLock};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Sha256Hash, canonical_sha256};

pub const STABLE_ERROR_CODE_CATALOG_SCHEMA_ID: &str = "star.stable-error-code-catalog";
pub const STABLE_ERROR_CODE_CATALOG_SCHEMA_VERSION: u32 = 1;

const STABLE_ERROR_CODE_SOURCE: &str = include_str!("../../../../catalog/stable-error-codes.txt");

pub fn stable_error_codes() -> &'static [&'static str] {
    static CODES: OnceLock<Vec<&'static str>> = OnceLock::new();
    CODES
        .get_or_init(|| {
            STABLE_ERROR_CODE_SOURCE
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect()
        })
        .as_slice()
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct StableErrorCode(String);

impl StableErrorCode {
    pub fn parse(value: impl Into<String>) -> Result<Self, StableErrorCodeError> {
        let value = value.into();
        stable_error_codes()
            .binary_search(&value.as_str())
            .map(|_| Self(value))
            .map_err(|_| StableErrorCodeError::Unknown)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn family(&self) -> StableErrorFamily {
        let code = self.0.as_str();
        let belongs = |prefix: &str| {
            code == prefix
                || code
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('_'))
        };
        if [
            "CHANGE_BUNDLE",
            "COORDINATION",
            "MERGE",
            "REMOTE",
            "VCS",
            "WORKTREE",
        ]
        .into_iter()
        .any(belongs)
        {
            StableErrorFamily::Coordination
        } else if ["PLANNING", "AFFECTED", "IMPACT", "CHANGE_PLAN", "GOAL"]
            .into_iter()
            .any(belongs)
        {
            StableErrorFamily::Planning
        } else if ["VALIDATION", "RULE"].into_iter().any(belongs) {
            StableErrorFamily::Validation
        } else if ["PATCH", "RECIPE"].into_iter().any(belongs) {
            StableErrorFamily::Patch
        } else if ["REGISTRY", "MANAGED_REGISTRY", "CATALOG"]
            .into_iter()
            .any(belongs)
        {
            StableErrorFamily::Registry
        } else if ["CONTRACT", "DOCUMENTATION"].into_iter().any(belongs) {
            StableErrorFamily::Contract
        } else if ["MAINTENANCE", "RETENTION"].into_iter().any(belongs) {
            StableErrorFamily::Maintenance
        } else if ["MIGRATION", "LANGUAGE", "PLATFORM"]
            .into_iter()
            .any(belongs)
        {
            StableErrorFamily::Migration
        } else if ["RELEASE", "EVALUATION"].into_iter().any(belongs) {
            StableErrorFamily::Release
        } else if belongs("RUST") {
            StableErrorFamily::RustStyle
        } else if belongs("TOOL") {
            StableErrorFamily::Tool
        } else if ["MANAGEMENT", "RECOVERY"].into_iter().any(belongs) {
            StableErrorFamily::Management
        } else if ["PROJECT", "WORKSPACE", "SCAN"].into_iter().any(belongs) {
            StableErrorFamily::Project
        } else if belongs("SECURITY") {
            StableErrorFamily::Security
        } else if belongs("DEPENDENCY") {
            StableErrorFamily::Dependency
        } else if belongs("PERFORMANCE") {
            StableErrorFamily::Performance
        } else if ["REPRODUCTION", "REGRESSION", "CLEAN_ROOM", "DOCTOR"]
            .into_iter()
            .any(belongs)
        {
            StableErrorFamily::Reproduction
        } else if belongs("CONFIG") {
            StableErrorFamily::Configuration
        } else if belongs("POLICY") {
            StableErrorFamily::Policy
        } else if belongs("ROUTE") {
            StableErrorFamily::Routing
        } else if belongs("IPC") {
            StableErrorFamily::Ipc
        } else if belongs("STATE") {
            StableErrorFamily::State
        } else if belongs("CODEX") {
            StableErrorFamily::Codex
        } else if belongs("UPDATE") {
            StableErrorFamily::Update
        } else if belongs("DEVELOPMENT") {
            StableErrorFamily::Development
        } else if belongs("ARTIFACT") {
            StableErrorFamily::Artifact
        } else {
            StableErrorFamily::Other
        }
    }
}

impl fmt::Display for StableErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for StableErrorCode {
    type Err = StableErrorCodeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for StableErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for StableErrorCode {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("StableErrorCode")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        serde_json::from_value(serde_json::json!({
            "type":"string",
            "enum":stable_error_codes(),
        }))
        .expect("stable error-code catalog produces a JSON Schema string enum")
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StableErrorFamily {
    Configuration,
    Policy,
    Routing,
    Ipc,
    State,
    Codex,
    Update,
    Development,
    Artifact,
    Planning,
    Validation,
    Patch,
    Registry,
    Contract,
    Maintenance,
    Migration,
    Coordination,
    Release,
    RustStyle,
    Tool,
    Management,
    Project,
    Security,
    Dependency,
    Performance,
    Reproduction,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StableErrorCodeDescriptor {
    pub code: StableErrorCode,
    pub family: StableErrorFamily,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StableErrorCodeCatalog {
    pub schema_id: String,
    pub schema_version: u32,
    pub codes: Vec<StableErrorCodeDescriptor>,
    pub content_fingerprint: Sha256Hash,
}

impl StableErrorCodeCatalog {
    pub fn builtin() -> Result<Self, StableErrorCodeError> {
        Self {
            schema_id: STABLE_ERROR_CODE_CATALOG_SCHEMA_ID.to_owned(),
            schema_version: STABLE_ERROR_CODE_CATALOG_SCHEMA_VERSION,
            codes: stable_error_codes()
                .iter()
                .map(|code| {
                    let code = StableErrorCode::parse(*code)?;
                    Ok(StableErrorCodeDescriptor {
                        family: code.family(),
                        code,
                    })
                })
                .collect::<Result<Vec<_>, StableErrorCodeError>>()?,
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
        .seal()
    }

    pub fn seal(mut self) -> Result<Self, StableErrorCodeError> {
        self.codes.sort_by(|left, right| left.code.cmp(&right.code));
        let actual = self
            .codes
            .iter()
            .map(|descriptor| descriptor.code.as_str())
            .collect::<Vec<_>>();
        if self.schema_id != STABLE_ERROR_CODE_CATALOG_SCHEMA_ID
            || self.schema_version != STABLE_ERROR_CODE_CATALOG_SCHEMA_VERSION
            || actual != stable_error_codes()
            || self
                .codes
                .iter()
                .any(|descriptor| descriptor.family != descriptor.code.family())
        {
            return Err(StableErrorCodeError::Catalog);
        }
        self.content_fingerprint = canonical_sha256(&serde_json::json!({
            "schema_id":self.schema_id,
            "schema_version":self.schema_version,
            "codes":self.codes,
        }))
        .map_err(|_| StableErrorCodeError::Fingerprint)?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum StableErrorCodeError {
    #[error("error code is not in the stable catalog")]
    Unknown,
    #[error("stable error-code catalog is invalid")]
    Catalog,
    #[error("stable error-code catalog fingerprint failed")]
    Fingerprint,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn source_catalog_is_sorted_unique_and_matches_the_canonical_document_tables() {
        let codes = stable_error_codes();
        assert_eq!(codes.len(), 533);
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(codes.iter().all(|code| {
            !code.is_empty()
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        }));
        let documented = include_str!("../../../../docs/contracts/errors-and-diagnostics.md")
            .split('`')
            .enumerate()
            .filter_map(|(index, token)| {
                (index % 2 == 1
                    && token.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                    && token.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    && !matches!(token, "AUTO_PASS" | "HUMAN_REVIEW"))
                .then_some(token)
            })
            .collect::<BTreeSet<_>>();
        let catalog = codes.iter().copied().collect::<BTreeSet<_>>();
        assert!(documented.is_subset(&catalog));
    }

    #[test]
    fn closed_type_and_catalog_reject_unknown_or_drifted_values() {
        assert!(StableErrorCode::parse("VALIDATION_PLAN_INCOHERENT").is_ok());
        assert!(StableErrorCode::parse("CONFIG_USER_INVALID").is_ok());
        assert!(StableErrorCode::parse("IPC_AUTH_FAILED").is_ok());
        assert!(StableErrorCode::parse("CHANGE_PLAN_MIGRATION_PLAN_STALE").is_ok());
        assert!(StableErrorCode::parse("VALIDATION_MADE_UP").is_err());
        let catalog = StableErrorCodeCatalog::builtin().unwrap();
        assert_eq!(catalog.clone().seal().unwrap(), catalog);
        let mut drifted = catalog;
        drifted.codes.pop();
        assert_eq!(drifted.seal(), Err(StableErrorCodeError::Catalog));
    }

    #[test]
    fn checked_in_rust_machine_codes_are_closed_by_the_catalog() {
        const PREFIXES: &[&str] = &[
            "CONFIG",
            "CONTRACT",
            "DOCTOR",
            "CLEAN_ROOM",
            "STATE",
            "POLICY",
            "ROUTE",
            "PLANNING",
            "IMPACT",
            "AFFECTED",
            "REGISTRY",
            "MANAGED_REGISTRY",
            "TOOL",
            "CODEX",
            "VALIDATION",
            "REPRODUCTION",
            "RECOVERY",
            "SECURITY",
            "DEPENDENCY",
            "MAINTENANCE",
            "MIGRATION",
            "PERFORMANCE",
            "LANGUAGE",
            "PLATFORM",
            "VCS",
            "WORKTREE",
            "MERGE",
            "REMOTE",
            "CHANGE_BUNDLE",
            "RELEASE",
            "EVALUATION",
            "IPC",
            "INTERNAL",
            "PROJECT",
            "WORKSPACE",
            "SCAN",
            "INDEX",
            "PATCH",
            "RECIPE",
            "RUST",
            "DEVELOPMENT",
            "MANAGEMENT",
            "OPERATION",
            "UPDATE",
            "ARTIFACT",
            "COORDINATION",
            "LIFECYCLE",
            "GOAL",
        ];

        fn collect_rs_files(root: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
            let mut entries = std::fs::read_dir(root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    collect_rs_files(&path, output);
                } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs") {
                    output.push(path);
                }
            }
        }

        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .unwrap();
        let mut files = Vec::new();
        for root in ["apps", "crates", "tools"] {
            collect_rs_files(&workspace.join(root), &mut files);
        }
        let mut observed = BTreeSet::new();
        for file in files {
            let source = std::fs::read_to_string(file).unwrap();
            for token in source.split('"').skip(1).step_by(2) {
                if token.len() >= 3
                    && token.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    && PREFIXES
                        .iter()
                        .any(|prefix| token.starts_with(&format!("{prefix}_")))
                    && !matches!(token, "POLICY_" | "RECOVERY_FIXTURE" | "VALIDATION_MADE_UP")
                {
                    observed.insert(token.to_owned());
                }
            }
        }
        let missing = observed
            .iter()
            .filter(|code| StableErrorCode::parse((*code).clone()).is_err())
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "machine codes missing from catalog: {missing:?}"
        );
    }
}
