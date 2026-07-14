//! Windows package, installation and Codex integration contracts.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Sha256Hash, ids::InstallationId};

pub const RELEASE_FILE_MANIFEST_SCHEMA_ID: &str = "star.release-file-manifest";
pub const INSTALLATION_RECORD_SCHEMA_ID: &str = "star.installation-record";
pub const CODEX_INTEGRATION_RECORD_SCHEMA_ID: &str = "star.codex-integration-record";
pub const INSTALLATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetArchitecture {
    X64,
    Arm64,
}

impl TargetArchitecture {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        }
    }
}

impl std::fmt::Display for TargetArchitecture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TargetArchitecture {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "x64" => Ok(Self::X64),
            "arm64" => Ok(Self::Arm64),
            _ => Err("architecture must be x64 or arm64"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageSigningState {
    UnsignedLocal,
    Signed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseFileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseFileManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub product_version: String,
    pub target_architecture: TargetArchitecture,
    pub created_at: DateTime<Utc>,
    pub source_revision: String,
    pub files: Vec<ReleaseFileEntry>,
    pub generated_files: Vec<String>,
    pub set_sha256: Sha256Hash,
    pub signing: PackageSigningState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControllerInstallManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub product_version: String,
    pub gateway_sha256: Sha256Hash,
    pub controller_path: String,
    pub controller_sha256: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodexRegistrationState {
    Registered,
    ManualActionRequired,
    Removed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexIntegrationSummary {
    pub record_path: String,
    pub registration_state: CodexRegistrationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallationRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub installation_id: InstallationId,
    pub product_version: String,
    pub target_architecture: TargetArchitecture,
    pub install_root: String,
    pub release_manifest_sha256: Sha256Hash,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub codex_integration: Option<CodexIntegrationSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexIntegrationRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub product_version: String,
    pub install_root: String,
    pub integration_root: String,
    pub marketplace_root: String,
    pub marketplace_name: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub render_sha256: Sha256Hash,
    pub registration_state: CodexRegistrationState,
    pub manual_commands: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_parser_is_closed() {
        assert_eq!("x64".parse(), Ok(TargetArchitecture::X64));
        assert_eq!("arm64".parse(), Ok(TargetArchitecture::Arm64));
        assert!("amd64".parse::<TargetArchitecture>().is_err());
    }

    #[test]
    fn persisted_contracts_reject_unknown_fields() {
        let text = format!(
            r#"{{"schema_id":"{INSTALLATION_RECORD_SCHEMA_ID}","schema_version":1,"installation_id":"ins_01J00000000000000000000000","product_version":"0.1.0","target_architecture":"x64","install_root":"C:\\\\Tools","release_manifest_sha256":"sha256:{}","installed_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","codex_integration":null,"extra":true}}"#,
            "0".repeat(64)
        );
        assert!(serde_json::from_str::<InstallationRecord>(&text).is_err());
    }
}
