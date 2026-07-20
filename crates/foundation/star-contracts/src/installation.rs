//! Windows package, installation and Codex integration contracts.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Sha256Hash, ids::InstallationId};

pub const RELEASE_FILE_MANIFEST_SCHEMA_ID: &str = "star.release-file-manifest";
pub const INSTALLATION_RECORD_SCHEMA_ID: &str = "star.installation-record";
pub const CODEX_INTEGRATION_RECORD_SCHEMA_ID: &str = "star.codex-integration-record";
pub const RUNTIME_GENERATION_MANIFEST_SCHEMA_ID: &str = "star.runtime-generation-manifest";
pub const RUNTIME_ACTIVATION_RECORD_SCHEMA_ID: &str = "star.runtime-activation-record";
pub const RUNTIME_CANDIDATE_REVIEW_SCHEMA_ID: &str = "star.runtime-candidate-review";
pub const INTEGRATION_CANDIDATE_REVIEW_SCHEMA_ID: &str = "star.integration-candidate-review";
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_activation_record_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_contract_version: Option<u32>,
}

/// A content-addressed Runtime Generation that can be selected by the stable
/// Bootstrap Bridge without changing the MCP or Plugin entrypoints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGenerationRef {
    pub generation_id: String,
    pub runtime_root: String,
    pub release_manifest_sha256: Sha256Hash,
}

/// Files and compatibility facts that a staged Runtime Generation must expose
/// before it can become the active Controller/CLI runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGenerationManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub generation: RuntimeGenerationRef,
    pub product_version: String,
    pub target_architecture: TargetArchitecture,
    pub controller_path: String,
    pub controller_sha256: Sha256Hash,
    pub cli_runtime_path: String,
    pub catalog_path: String,
    pub schemas_root: String,
    pub bridge_contract_version: u32,
}

/// The only persisted selector read by the stable bridge.  The activation
/// writer replaces this record atomically after the old Controller quiesces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeActivationRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub activation_revision: u64,
    pub active: RuntimeGenerationRef,
    pub previous: Option<RuntimeGenerationRef>,
    pub state_generation_id: String,
    pub bridge_contract_version: u32,
    pub activated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeUpdateClass {
    ToolHotReload,
    RuntimeGeneration,
    BridgeUpdate,
    PluginUpdate,
}

/// Full release-stage classification for a Codex restart transaction. This is
/// intentionally separate from RuntimeGeneration review: a stable Bridge and
/// Plugin update changes files outside a generation selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationCandidateClass {
    CodexIntegrationUpdate,
    UpdaterUpdate,
    RuntimeUpdate,
    MixedUpdate,
    NoChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationCandidateReview {
    pub schema_id: String,
    pub schema_version: u32,
    pub candidate_release_manifest_sha256: Sha256Hash,
    pub target_architecture: TargetArchitecture,
    pub candidate_class: IntegrationCandidateClass,
    pub changed_files: Vec<String>,
    pub rollback_available: bool,
    pub requires_codex_restart: bool,
    pub approval_scope_sha256: Sha256Hash,
}

/// Public candidate review used by both the stable CLI and Registry actions.
/// It intentionally describes a candidate without authorizing its mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCandidateReview {
    pub schema_id: String,
    pub schema_version: u32,
    pub candidate: RuntimeGenerationRef,
    pub update_class: RuntimeUpdateClass,
    pub added_actions: Vec<String>,
    pub removed_actions: Vec<String>,
    pub changed_actions: Vec<String>,
    pub breaking_schema: bool,
    pub risk_lane_widened: bool,
    pub permission_widened: bool,
    pub handler_ready: bool,
    pub bridge_compatible: bool,
    pub rollback_available: bool,
    pub requires_codex_restart: bool,
    pub requires_new_task: bool,
    pub hook_review_required: bool,
    pub approval_scope_sha256: Sha256Hash,
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

    #[test]
    fn runtime_activation_record_rejects_unknown_fields_and_keeps_rollback_reference() {
        let text = format!(
            r#"{{"schema_id":"{RUNTIME_ACTIVATION_RECORD_SCHEMA_ID}","schema_version":1,"activation_revision":2,"active":{{"generation_id":"rt_active","runtime_root":"runtime/generations/rt_active","release_manifest_sha256":"sha256:{}"}},"previous":{{"generation_id":"rt_previous","runtime_root":"runtime/generations/rt_previous","release_manifest_sha256":"sha256:{}"}},"state_generation_id":"state_2","bridge_contract_version":2,"activated_at":"2026-07-18T00:00:00Z"}}"#,
            "1".repeat(64),
            "2".repeat(64),
        );
        let record: RuntimeActivationRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(record.previous.unwrap().generation_id, "rt_previous");
        let mut invalid: serde_json::Value = serde_json::from_str(&text).unwrap();
        invalid
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), true.into());
        assert!(serde_json::from_value::<RuntimeActivationRecord>(invalid).is_err());
    }

    #[test]
    fn candidate_review_keeps_restart_requirements_separate_from_runtime_changes() {
        let review = RuntimeCandidateReview {
            schema_id: RUNTIME_CANDIDATE_REVIEW_SCHEMA_ID.to_owned(),
            schema_version: 1,
            candidate: RuntimeGenerationRef {
                generation_id: "rt_candidate".to_owned(),
                runtime_root: "runtime/generations/rt_candidate".to_owned(),
                release_manifest_sha256: Sha256Hash::digest(b"candidate"),
            },
            update_class: RuntimeUpdateClass::RuntimeGeneration,
            added_actions: vec!["star.core.runtime.update.status".to_owned()],
            removed_actions: Vec::new(),
            changed_actions: Vec::new(),
            breaking_schema: false,
            risk_lane_widened: false,
            permission_widened: false,
            handler_ready: true,
            bridge_compatible: true,
            rollback_available: true,
            requires_codex_restart: false,
            requires_new_task: false,
            hook_review_required: false,
            approval_scope_sha256: Sha256Hash::digest(b"approval-scope"),
        };
        assert_eq!(review.update_class, RuntimeUpdateClass::RuntimeGeneration);
        assert!(!review.requires_codex_restart);
    }
}
