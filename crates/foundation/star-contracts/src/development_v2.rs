//! Product-grade M6 contract, documentation, configuration, and environment contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ProjectId, Sha256Hash};

pub const PROJECT_CONTRACT_MANIFEST_SCHEMA_ID: &str = "star.project-contract-manifest";
pub const CONTRACT_SURFACE_SNAPSHOT_SCHEMA_ID: &str = "star.contract-surface-snapshot";
pub const COMPATIBILITY_REPORT_V2_SCHEMA_ID: &str = "star.compatibility-report";
pub const DOCUMENTATION_SNAPSHOT_SCHEMA_ID: &str = "star.documentation-snapshot";
pub const CONFIG_KEY_TRACE_SCHEMA_ID: &str = "star.config-key-trace";
pub const ENVIRONMENT_SNAPSHOT_SCHEMA_ID: &str = "star.environment-snapshot";
pub const PROJECT_DOCTOR_REPORT_SCHEMA_ID: &str = "star.project-doctor-report";
pub const CLEAN_ROOM_SPECIFICATION_SCHEMA_ID: &str = "star.clean-room-specification";
pub const DEPENDENCY_SECURITY_INPUT_MANIFEST_SCHEMA_ID: &str =
    "star.dependency-security-input-manifest";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContractSurfaceKind {
    Api,
    Cli,
    Schema,
    FileFormat,
    Config,
    ErrorCode,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Complete,
    Partial,
    Unverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceSnapshotRole {
    Baseline,
    Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityClass {
    Unchanged,
    Compatible,
    Additive,
    Breaking,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceChangeKind {
    Unchanged,
    Added,
    Removed,
    Modified,
    Renamed,
    CoverageChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationState {
    Pass,
    Block,
    HumanReview,
    Unknown,
    NotApplicable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselinePolicy {
    pub baseline_ref: String,
    pub baseline_sha256: Sha256Hash,
    pub approval_ref: String,
    pub activated_at: String,
    pub supported_until: Option<String>,
    pub minimum_consumer_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContractSurfaceDescriptor {
    pub surface_id: String,
    pub kind: ContractSurfaceKind,
    pub owner: String,
    pub source_path: String,
    pub source_selector: String,
    pub declaration_ref: Option<String>,
    pub schema_ref: Option<String>,
    #[serde(default)]
    pub generated_refs: Vec<String>,
    #[serde(default)]
    pub documentation_refs: Vec<String>,
    #[serde(default)]
    pub consumer_contract_refs: Vec<String>,
    pub compatibility_policy_ref: String,
    pub visibility_policy: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentationTarget {
    pub target_id: String,
    pub source_path: String,
    #[serde(default)]
    pub required_commands: Vec<String>,
    #[serde(default)]
    pub required_config_keys: Vec<String>,
    #[serde(default)]
    pub required_references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssumptionSpec {
    pub assumption_id: String,
    pub kind: String,
    pub subject: String,
    pub expected: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentConstraint {
    pub constraint_id: String,
    pub kind: String,
    pub subject: String,
    pub accepted: Vec<String>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectContractManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub manifest_id: String,
    pub manifest_version: String,
    pub project_id: ProjectId,
    pub baseline_policy: BaselinePolicy,
    pub surfaces: Vec<ContractSurfaceDescriptor>,
    #[serde(default)]
    pub documentation: Vec<DocumentationTarget>,
    #[serde(default)]
    pub assumptions: Vec<AssumptionSpec>,
    #[serde(default)]
    pub environment_constraints: Vec<EnvironmentConstraint>,
    pub clean_room_spec_ref: Option<String>,
    pub source_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SurfaceObservation {
    pub surface_id: String,
    pub kind: ContractSurfaceKind,
    pub normalized_shape: String,
    pub visibility: String,
    pub source_path: String,
    pub source_sha256: Sha256Hash,
    #[serde(default)]
    pub binding_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub coverage: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub observation_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContractSurfaceSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub snapshot_role: SurfaceSnapshotRole,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub manifest_fingerprint: Sha256Hash,
    pub registry_snapshot_ref: Option<String>,
    pub surfaces: Vec<SurfaceObservation>,
    pub coverage: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContractChangeRecord {
    pub surface_id: String,
    pub kind: ContractSurfaceKind,
    pub change_kind: SurfaceChangeKind,
    pub classification: CompatibilityClass,
    pub before_fingerprint: Option<Sha256Hash>,
    pub after_fingerprint: Option<Sha256Hash>,
    pub rule_id: String,
    pub summary: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerImpactRecord {
    pub consumer_ref: String,
    pub surface_id: String,
    pub observed_revision: Option<String>,
    pub minimum_version: Option<String>,
    pub classification: CompatibilityClass,
    pub migration_required: bool,
    pub state: EvaluationState,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityReportV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub report_id: String,
    pub project_id: ProjectId,
    pub manifest_ref: String,
    pub manifest_fingerprint: Sha256Hash,
    pub baseline_snapshot_ref: String,
    pub baseline_snapshot_fingerprint: Sha256Hash,
    pub current_snapshot_ref: String,
    pub current_snapshot_fingerprint: Sha256Hash,
    pub changes: Vec<ContractChangeRecord>,
    #[serde(default)]
    pub consumer_impacts: Vec<ConsumerImpactRecord>,
    pub outcome: CompatibilityClass,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub report_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationObservationKind {
    Link,
    Anchor,
    Command,
    Snippet,
    Example,
    ConfigKey,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentationObservation {
    pub target_id: String,
    pub kind: DocumentationObservationKind,
    pub subject: String,
    pub state: EvaluationState,
    pub source_path: String,
    pub source_sha256: Sha256Hash,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentationSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub observations: Vec<DocumentationObservation>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigReaderObservation {
    pub reader_ref: String,
    pub source_path: String,
    pub source_sha256: Sha256Hash,
    pub state: EvaluationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigOverrideObservation {
    pub provenance: String,
    pub present: bool,
    pub precedence: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigKeyTrace {
    pub schema_id: String,
    pub schema_version: u32,
    pub trace_id: String,
    pub project_id: ProjectId,
    pub key_ref: String,
    pub lifecycle: String,
    pub declaration_ref: Option<String>,
    pub readers: Vec<ConfigReaderObservation>,
    pub overrides: Vec<ConfigOverrideObservation>,
    pub effective_provenance: Option<String>,
    pub value_redacted: bool,
    pub state: EvaluationState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub trace_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Present,
    Missing,
    VersionMismatch,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolchainObservation {
    pub toolchain_id: String,
    pub discovered_from: String,
    pub declared_range: Option<String>,
    pub observed_version: Option<String>,
    pub executable_fingerprint: Option<Sha256Hash>,
    pub state: ObservationState,
    pub evidence_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManifestObservation {
    pub ecosystem: String,
    pub manifest_kind: String,
    pub logical_path: String,
    pub content_sha256: Sha256Hash,
    pub owner: String,
    pub relation: Option<String>,
    pub completeness: CoverageState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentContractPresence {
    pub declaration_ref: String,
    pub required: bool,
    pub scope: String,
    pub state: ObservationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub os_family: String,
    pub os_release: String,
    pub architecture: String,
    pub filesystem_kind: String,
    pub case_behavior: String,
    pub symlink_capability: String,
    pub long_path_capability: String,
    pub path_kind: String,
    pub path_depth: u32,
    pub path_length_bucket: String,
    pub text_encoding_policy: String,
    pub line_ending_policy: String,
    #[serde(default)]
    pub toolchains: Vec<ToolchainObservation>,
    #[serde(default)]
    pub manifests: Vec<ManifestObservation>,
    #[serde(default)]
    pub task_descriptor_refs: Vec<String>,
    #[serde(default)]
    pub environment_contract_presence: Vec<EnvironmentContractPresence>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub environment_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanRoomTaskSpec {
    pub task_id: String,
    pub order: u32,
    pub timeout_ms: u64,
    pub resource_bound: String,
    pub expected_result: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanRoomSpecification {
    pub schema_id: String,
    pub schema_version: u32,
    pub specification_id: String,
    pub project_id: ProjectId,
    pub source_revision: String,
    pub source_sha256: Sha256Hash,
    pub target_os: Vec<String>,
    pub architectures: Vec<String>,
    #[serde(default)]
    pub required_toolchains: Vec<ToolchainObservation>,
    #[serde(default)]
    pub manifest_refs: Vec<String>,
    pub tasks: Vec<CleanRoomTaskSpec>,
    #[serde(default)]
    pub required_environment_contracts: Vec<String>,
    pub test_network_policy: String,
    pub dependency_download: String,
    pub package_install: String,
    pub system_mutation: String,
    pub cache_state: String,
    #[serde(default)]
    pub writable_output_roots: Vec<String>,
    #[serde(default)]
    pub forbidden_actions: Vec<String>,
    pub specification_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CleanRoomReadiness {
    Ready,
    NotReady,
    Unknown,
    NotRequired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConstraintEvaluation {
    pub constraint_id: String,
    pub observed: String,
    pub required: String,
    pub state: EvaluationState,
    pub diagnostic_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectDoctorReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub report_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub environment_snapshot_ref: String,
    pub environment_snapshot_fingerprint: Sha256Hash,
    pub constraint_evaluations: Vec<ConstraintEvaluation>,
    pub toolchain_observations: Vec<ToolchainObservation>,
    pub manifest_observations: Vec<ManifestObservation>,
    #[serde(default)]
    pub command_availability: Vec<ConstraintEvaluation>,
    #[serde(default)]
    pub windows_compatibility: Vec<ConstraintEvaluation>,
    pub clean_room_readiness: CleanRoomReadiness,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub forbidden_actions_observed: Vec<String>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub state: EvaluationState,
    pub report_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencySecurityInputManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub manifest_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub environment_snapshot_ref: String,
    pub manifest_observations: Vec<ManifestObservation>,
    pub toolchain_observations: Vec<ToolchainObservation>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

fn default_true() -> bool {
    true
}
