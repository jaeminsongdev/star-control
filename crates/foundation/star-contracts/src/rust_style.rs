use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ProjectId, Sha256Hash, management::ProjectPathRef};

pub const RUST_TOOLCHAIN_BINDING_SCHEMA_ID: &str = "star.rust-toolchain-binding";
pub const RUST_STYLE_POLICY_SNAPSHOT_SCHEMA_ID: &str = "star.rust-style-policy-snapshot";
pub const RUST_STYLE_COVERAGE_MATRIX_SCHEMA_ID: &str = "star.rust-style-coverage-matrix";
pub const RUST_STYLE_STEP_EXECUTION_SCHEMA_ID: &str = "star.rust-style-step-execution";
pub const RUST_STYLE_PIPELINE_ID: &str = "rust_style_v1";
pub const RUST_STYLE_PIPELINE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustCompleteness {
    Complete,
    Partial,
    Unverified,
    Ambiguous,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustToolchainSource {
    RustToolchainToml,
    LegacyRustToolchain,
    ProjectCatalog,
    RustupDirectoryOverride,
    EnvironmentOverride,
    Default,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustToolchainPinState {
    PinnedStable,
    MovingStable,
    Beta,
    Nightly,
    Custom,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustAvailabilityState {
    Available,
    Missing,
    Unsupported,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustSourceBinding {
    pub source_ref: String,
    pub content_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustExecutableBinding {
    pub logical_id: String,
    pub opaque_file_identity: String,
    pub version: String,
    pub sha256: Sha256Hash,
    pub component_state: RustAvailabilityState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustEditionBinding {
    pub subject_ref: String,
    pub edition: String,
    pub provenance: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustTargetState {
    pub target_triple: String,
    pub state: RustAvailabilityState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustToolchainBinding {
    pub schema_id: String,
    pub schema_version: u32,
    pub contract_version: u32,
    pub workspace_root_ref: String,
    pub manifest_refs: Vec<RustSourceBinding>,
    pub toolchain_source: RustToolchainSource,
    pub toolchain_source_ref: String,
    pub toolchain_pin_state: RustToolchainPinState,
    pub channel: String,
    pub release: Option<String>,
    pub host_triple: String,
    pub cargo: RustExecutableBinding,
    pub rustc: RustExecutableBinding,
    pub rustfmt: RustExecutableBinding,
    pub clippy_driver: RustExecutableBinding,
    pub parsing_editions: Vec<RustEditionBinding>,
    pub style_editions: Vec<RustEditionBinding>,
    pub msrv_bindings: Vec<RustEditionBinding>,
    pub host_target: String,
    pub requested_target_triples: Vec<String>,
    pub config_bindings: Vec<RustSourceBinding>,
    pub component_states: Vec<RustTargetState>,
    pub target_states: Vec<RustTargetState>,
    pub completeness: RustCompleteness,
    pub limitations: Vec<String>,
    pub binding_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClippyAllowlistSource {
    ProjectCatalog,
    UserCatalog,
    BuiltinVerified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustCatalogLifecycle {
    Active,
    Deprecated,
    Retired,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClippyFixAllowlistEntry {
    pub lint_id: String,
    pub entry_version: String,
    pub source: ClippyAllowlistSource,
    pub source_ref: String,
    pub clippy_release: String,
    pub clippy_executable_sha256: Sha256Hash,
    pub required_applicability: SuggestionApplicability,
    pub allowed_scope: Vec<ProjectPathRef>,
    pub public_api_policy: String,
    pub required_check_families: Vec<String>,
    pub corpus_ref: String,
    pub lifecycle: RustCatalogLifecycle,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustAutoPolicy {
    SafeDefault,
    PersonalAuto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustStylePolicySnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub contract_version: u32,
    pub profile_ref: String,
    pub profile_definition_hash: Sha256Hash,
    pub pipeline_ref: String,
    pub fixed_adapter_definition_fingerprint: Sha256Hash,
    pub formatting_sources: Vec<RustSourceBinding>,
    pub lint_level_sources: Vec<RustSourceBinding>,
    pub clippy_parameter_sources: Vec<RustSourceBinding>,
    pub clippy_fix_allowlist: Vec<ClippyFixAllowlistEntry>,
    pub coverage_policy_ref: String,
    pub scope_project_id: ProjectId,
    pub scope_packages: Vec<String>,
    pub scope_paths: Vec<ProjectPathRef>,
    pub auto_policy: RustAutoPolicy,
    pub standing_grant_ref: Option<String>,
    pub max_files: u32,
    pub max_hunks: u32,
    pub max_changed_bytes: u64,
    pub forbidden_operations: Vec<String>,
    pub policy_completeness: RustCompleteness,
    pub limitations: Vec<String>,
    pub policy_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustTargetKind {
    Lib,
    Bin,
    Test,
    Example,
    Bench,
    CustomBuild,
    ProcMacro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustSourceOwnership {
    Handwritten,
    Generated,
    Vendor,
    OutOfScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustCoveragePhase {
    DiagnosticCheck,
    IsolatedFix,
    CandidateFinalCheck,
    ActualAfterPostCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustCoverageExecution {
    Executed,
    Skipped,
    Unavailable,
    Conflicted,
    Invalidated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustStyleCoverageCell {
    pub cell_id: String,
    pub workspace_ref: String,
    pub package_id: String,
    pub manifest_sha256: Sha256Hash,
    pub target_kind: RustTargetKind,
    pub target_name: String,
    pub source_root: ProjectPathRef,
    pub feature_set_id: String,
    pub default_features: bool,
    pub features: Vec<String>,
    pub required_features_satisfied: bool,
    pub host_triple: String,
    pub target_triple: String,
    pub cfg_observation_ref: String,
    pub ownership: RustSourceOwnership,
    pub phase: RustCoveragePhase,
    pub execution: RustCoverageExecution,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustStyleCoverageMatrix {
    pub schema_id: String,
    pub schema_version: u32,
    pub contract_version: u32,
    pub policy_ref: String,
    pub cells: Vec<RustStyleCoverageCell>,
    pub required_cell_ids: Vec<String>,
    pub cfg_frontier: Vec<String>,
    pub conflicts: Vec<String>,
    pub completeness: RustCompleteness,
    pub limitations: Vec<String>,
    pub coverage_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustByteEdit {
    pub start_byte: u64,
    pub end_byte: u64,
    pub replacement: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClippySuggestion {
    pub lint_id: String,
    pub coverage_cell_id: String,
    pub path: ProjectPathRef,
    pub before_file_sha256: Sha256Hash,
    pub applicability: SuggestionApplicability,
    pub edits: Vec<RustByteEdit>,
    pub expansion_origin: Option<String>,
    pub suggestion_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustSideEffectResult {
    Pass,
    Violation,
    Unverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RustStepResult {
    Succeeded,
    Failed,
    Blocked,
    Stale,
    Cancelled,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustStyleStepExecution {
    pub schema_id: String,
    pub schema_version: u32,
    pub contract_version: u32,
    pub step_execution_id: String,
    pub ordinal: u32,
    pub step_id: String,
    pub pipeline_ref: String,
    pub adapter_fingerprint: Sha256Hash,
    pub subject_before: Sha256Hash,
    pub subject_after: Sha256Hash,
    pub tool_descriptor_ref: Option<String>,
    pub task_invocation_ref: Option<String>,
    pub execution_result_ref: Option<String>,
    pub coverage_cell_refs: Vec<String>,
    pub diagnostic_set_ref: Option<String>,
    pub suggestion_manifest_ref: Option<String>,
    pub diff_artifact_ref: Option<String>,
    pub filesystem_manifest_ref: String,
    pub side_effect_result: RustSideEffectResult,
    pub result: RustStepResult,
    pub started_at: String,
    pub finished_at: String,
    pub step_execution_fingerprint: Sha256Hash,
}
