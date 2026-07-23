//! Public validation, diagnostic, gate, and evidence contracts.
//!
//! The decision in [`GateDecision`] is authoritative. Consumers may display or
//! route it, but must not derive a replacement completion decision from the
//! referenced validation runs.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    Sha256Hash,
    ids::{
        ArtifactId, DiagnosticId, EvidenceBundleId, GateId, GoalId, ProjectId, RunId, StageId,
        TaskInvocationId, ValidationRunId, WaiverId,
    },
};

mod validation_plan;
pub use validation_plan::*;

pub const VALIDATION_RUN_SCHEMA_ID: &str = "star.validation-run";
pub const GATE_DECISION_SCHEMA_ID: &str = "star.gate-decision";
pub const EVIDENCE_BUNDLE_SCHEMA_ID: &str = "star.evidence-bundle";
pub const DIAGNOSTIC_SCHEMA_ID: &str = "star.diagnostic";
pub const EVIDENCE_CONTRACT_SCHEMA_VERSION: u32 = 1;

macro_rules! singleton_schema_id {
    ($name:ident, $variant:ident, $value:literal) => {
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
        )]
        pub enum $name {
            #[default]
            #[serde(rename = $value)]
            $variant,
        }
    };
}

singleton_schema_id!(ValidationRunSchemaId, ValidationRun, "star.validation-run");
singleton_schema_id!(GateDecisionSchemaId, GateDecision, "star.gate-decision");
singleton_schema_id!(
    EvidenceBundleSchemaId,
    EvidenceBundle,
    "star.evidence-bundle"
);
singleton_schema_id!(DiagnosticSchemaId, Diagnostic, "star.diagnostic");

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractInvariantError {
    #[error("schema_version must be exactly 1")]
    SchemaVersion,
    #[error("document timestamps are not ordered")]
    DocumentTimeOrder,
    #[error("extension keys must be namespaced")]
    ExtensionNamespace,
    #[error("project-relative path is invalid: {0}")]
    ProjectRelativePath(String),
    #[error("line and column positions must be 1-based and ordered")]
    Location,
    #[error("task invocation limits and required values are invalid")]
    TaskInvocation,
    #[error("validation attempt must be positive")]
    ValidationAttempt,
    #[error("validation timestamps are not ordered")]
    ValidationTimeOrder,
    #[error("validation outcome and execution evidence are inconsistent")]
    ValidationExecution,
    #[error("a pass requires complete, exited, semantically successful execution evidence")]
    InvalidPass,
    #[error("gate references contain a duplicate")]
    DuplicateGateReference,
    #[error("a satisfied run is not one of the required runs")]
    SatisfiedRunNotRequired,
    #[error("a referenced validation run could not be resolved at the pinned revision")]
    MissingValidationRun,
    #[error("a satisfied validation run does not satisfy the required check")]
    UnsatisfiedValidationRun,
    #[error("auto_pass requires every required run to be satisfied")]
    IncompleteAutoPass,
    #[error("auto_pass cannot contain blocking diagnostics or waivers")]
    BlockedAutoPass,
    #[error("suppressed diagnostics require a suppression reference")]
    MissingSuppression,
    #[error("diagnostic observation timestamps are not ordered")]
    DiagnosticTimeOrder,
    #[error("evidence completeness and missing reasons are inconsistent")]
    EvidenceCompleteness,
    #[error("artifact manifest entries must be unique")]
    DuplicateArtifact,
}

pub type Extensions = BTreeMap<String, Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProducerRef {
    pub component: String,
    pub product_version: String,
    pub build_id: String,
    pub platform: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    Codex,
    System,
    Controller,
    Mcp,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorRef {
    pub actor_type: ActorType,
    pub actor_id: String,
    pub display_name: String,
    pub auth_source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentRef {
    pub schema_id: String,
    pub document_id: String,
    pub revision: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogRef {
    pub catalog_id: String,
    pub format_version: u32,
    pub item_version: String,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial,
    Unverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPathKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectPathRef {
    pub project_id: ProjectId,
    pub path: String,
    pub path_kind: ProjectPathKind,
}

impl ProjectPathRef {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        validate_relative_path(&self.path)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TextPosition {
    #[schemars(range(min = 1))]
    pub line: u32,
    #[schemars(range(min = 1))]
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationRef {
    pub path: ProjectPathRef,
    pub start: TextPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<TextPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

impl LocationRef {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        self.path.validate()?;
        if self.start.line == 0
            || self.start.column == 0
            || self
                .end
                .is_some_and(|end| end.line == 0 || end.column == 0 || end < self.start)
        {
            return Err(ContractInvariantError::Location);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Log,
    Report,
    Diff,
    Screenshot,
    Trace,
    Manifest,
    Input,
    Output,
    Checkpoint,
    ChangeSet,
    ReviewPack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStatus {
    NotNeeded,
    Redacted,
    Quarantined,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    Temporary,
    Run,
    Evidence,
    Hold,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub artifact_id: ArtifactId,
    pub kind: ArtifactKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    pub relative_path: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: Sha256Hash,
    pub created_at: DateTime<Utc>,
    pub producer: ProducerRef,
    pub redaction_status: RedactionStatus,
    pub retention_class: RetentionClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_ref: Option<Box<ArtifactRef>>,
}

impl ArtifactRef {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        validate_relative_path(&self.relative_path)?;
        if let Some(source) = &self.source_artifact_ref {
            source.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationRunRef {
    pub validation_run_id: ValidationRunId,
    pub revision: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticRef {
    pub diagnostic_id: DiagnosticId,
    pub sequence: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateDecisionRef {
    pub gate_id: GateId,
    pub revision: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBundleRef {
    pub evidence_bundle_id: EvidenceBundleId,
    pub revision: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvironmentValueRef {
    Literal { value: String },
    Secret { secret_ref: DocumentRef },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputLimits {
    #[schemars(range(min = 1))]
    pub stdout_bytes: u64,
    #[schemars(range(min = 1))]
    pub stderr_bytes: u64,
    #[schemars(range(min = 1))]
    pub artifact_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskInvocation {
    pub invocation_id: TaskInvocationId,
    pub tool_ref: CatalogRef,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: ProjectPathRef,
    #[serde(default)]
    pub env_refs: BTreeMap<String, EnvironmentValueRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_ref: Option<ArtifactRef>,
    #[schemars(range(min = 1))]
    pub timeout_ms: u64,
    pub permission_action: String,
    pub idempotency_key: String,
    pub expected_exit_codes: BTreeSet<i32>,
    pub output_limits: OutputLimits,
}

impl TaskInvocation {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        self.cwd.validate()?;
        if let Some(stdin) = &self.stdin_ref {
            stdin.validate()?;
        }
        if self.executable.is_empty()
            || self.timeout_ms == 0
            || self.permission_action.is_empty()
            || self.idempotency_key.is_empty()
            || self.expected_exit_codes.is_empty()
            || self.output_limits.stdout_bytes == 0
            || self.output_limits.stderr_bytes == 0
            || self.output_limits.artifact_bytes == 0
            || self.env_refs.keys().any(|name| name.is_empty())
        {
            return Err(ContractInvariantError::TaskInvocation);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservedTool {
    pub executable_path: String,
    pub version: String,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationCache {
    pub hit: bool,
    pub cache_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_validation_run_ref: Option<ValidationRunRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOutcome {
    Pass,
    Fail,
    NotRun,
    Error,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    Exited,
    Timeout,
    Cancelled,
    LaunchError,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationRun {
    pub schema_id: ValidationRunSchemaId,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub validation_run_id: ValidationRunId,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub producer: ProducerRef,
    #[serde(default)]
    pub extensions: Extensions,
    pub validation_plan_ref: DocumentRef,
    pub check_ref: CatalogRef,
    pub tool_ref: CatalogRef,
    #[schemars(range(min = 1))]
    pub attempt: u32,
    pub invocation: TaskInvocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub outcome: ValidationOutcome,
    pub completeness: Completeness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termination_reason: Option<TerminationReason>,
    #[serde(default)]
    pub diagnostic_refs: Vec<DiagnosticRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_ref: Option<ArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_ref: Option<ArtifactRef>,
    #[serde(default)]
    pub result_artifact_refs: Vec<ArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_tool: Option<ObservedTool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<ValidationCache>,
}

impl ValidationRun {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        validate_document(
            self.schema_version,
            self.created_at,
            self.updated_at,
            &self.extensions,
        )?;
        if self.attempt == 0 {
            return Err(ContractInvariantError::ValidationAttempt);
        }
        self.invocation.validate()?;
        for artifact in self
            .stdout_ref
            .iter()
            .chain(self.stderr_ref.iter())
            .chain(self.result_artifact_refs.iter())
        {
            artifact.validate()?;
        }
        if self
            .started_at
            .zip(self.finished_at)
            .is_some_and(|(started, finished)| finished < started)
        {
            return Err(ContractInvariantError::ValidationTimeOrder);
        }
        if self.outcome == ValidationOutcome::NotRun
            && (self.started_at.is_some()
                || self.finished_at.is_some()
                || self.exit_code.is_some()
                || self.termination_reason.is_some()
                || self.observed_tool.is_some())
        {
            return Err(ContractInvariantError::ValidationExecution);
        }
        if self.outcome == ValidationOutcome::Pass && !self.satisfies_required_check() {
            return Err(ContractInvariantError::InvalidPass);
        }
        Ok(())
    }

    /// Returns whether this immutable result can appear in
    /// `GateDecision.satisfied_run_refs`.
    ///
    /// `not_run`, partial, unverified, timeout, cancelled, launch errors, and
    /// unknown outcomes always return `false`.
    pub fn satisfies_required_check(&self) -> bool {
        self.outcome == ValidationOutcome::Pass
            && self.completeness == Completeness::Complete
            && self.started_at.is_some()
            && self.finished_at.is_some()
            && self.termination_reason == Some(TerminationReason::Exited)
            && self
                .exit_code
                .is_some_and(|code| self.invocation.expected_exit_codes.contains(&code))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticConfidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Confirmed,
    Suspected,
    Unverified,
    Suppressed,
    Resolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosticScope {
    Goal {
        goal_id: GoalId,
        revision: u64,
    },
    Run {
        goal_id: GoalId,
        run_id: RunId,
        revision: u64,
    },
    Stage {
        goal_id: GoalId,
        run_id: RunId,
        stage_id: StageId,
        revision: u64,
    },
    ValidationRun {
        validation_run_ref: ValidationRunRef,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Remediation {
    pub summary: String,
    pub automatic_fix_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_ref: Option<CatalogRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuppressionRef {
    pub waiver_id: WaiverId,
    pub fingerprint: Sha256Hash,
    pub project_id: ProjectId,
    pub scope_revision: u64,
    pub reason: String,
    pub approved_by: ActorRef,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub schema_id: DiagnosticSchemaId,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub diagnostic_id: DiagnosticId,
    pub sequence: u64,
    pub producer: ProducerRef,
    #[serde(default)]
    pub extensions: Extensions,
    pub rule_id: CatalogRef,
    pub title: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub confidence: DiagnosticConfidence,
    pub status: DiagnosticStatus,
    pub scope: DiagnosticScope,
    #[serde(default)]
    pub locations: Vec<LocationRef>,
    #[serde(default)]
    pub evidence_refs: Vec<ArtifactRef>,
    pub fingerprint: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<Remediation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression: Option<SuppressionRef>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

impl Diagnostic {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        validate_schema_and_extensions(self.schema_version, &self.extensions)?;
        if self.first_seen_at > self.last_seen_at {
            return Err(ContractInvariantError::DiagnosticTimeOrder);
        }
        if self.status == DiagnosticStatus::Suppressed && self.suppression.is_none() {
            return Err(ContractInvariantError::MissingSuppression);
        }
        for location in &self.locations {
            location.validate()?;
        }
        for artifact in &self.evidence_refs {
            artifact.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GateScope {
    Goal {
        goal_id: GoalId,
        run_id: RunId,
        revision: u64,
    },
    Stage {
        goal_id: GoalId,
        run_id: RunId,
        stage_id: StageId,
        revision: u64,
    },
    Merge {
        project_id: ProjectId,
        revision: u64,
    },
    Release {
        project_id: ProjectId,
        revision: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateDecisionKind {
    AutoPass,
    HumanReview,
    Block,
}

/// The only completion state downstream consumers may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthoritativeGateState {
    Passed,
    AwaitingHumanReview,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WaiverTargetRef {
    ValidationRun {
        validation_run_ref: ValidationRunRef,
    },
    Diagnostic {
        diagnostic_ref: DiagnosticRef,
    },
    OmittedCheck {
        check_ref: CatalogRef,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaiverRef {
    pub waiver_id: WaiverId,
    pub target: WaiverTargetRef,
    pub scope_revision: u64,
    pub reason: String,
    pub evidence_sha256: Sha256Hash,
    pub approved_by: ActorRef,
    pub approved_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OmissionImpact {
    None,
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OmittedCheck {
    pub check_ref: CatalogRef,
    pub reason: String,
    pub impact: OmissionImpact,
    #[serde(default)]
    pub alternative_evidence_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskRef {
    pub risk_id: String,
    pub title: String,
    pub severity: DiagnosticSeverity,
    #[serde(default)]
    pub evidence_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatePolicySnapshot {
    pub policy_ref: DocumentRef,
    pub policy_sha256: Sha256Hash,
    #[serde(default)]
    pub thresholds: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateDecision {
    pub schema_id: GateDecisionSchemaId,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub gate_id: GateId,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub producer: ProducerRef,
    #[serde(default)]
    pub extensions: Extensions,
    pub scope: GateScope,
    pub decision: GateDecisionKind,
    #[serde(default)]
    pub required_run_refs: Vec<ValidationRunRef>,
    #[serde(default)]
    pub satisfied_run_refs: Vec<ValidationRunRef>,
    #[serde(default)]
    pub blocking_diagnostic_refs: Vec<DiagnosticRef>,
    #[serde(default)]
    pub waivers: Vec<WaiverRef>,
    #[serde(default)]
    pub omissions: Vec<OmittedCheck>,
    #[serde(default)]
    pub remaining_risks: Vec<RiskRef>,
    pub policy_snapshot: GatePolicySnapshot,
    pub decided_by: ActorRef,
}

impl GateDecision {
    /// Returns the upstream-owned completion state without inspecting or
    /// reinterpreting any validation result.
    pub fn authoritative_state(&self) -> AuthoritativeGateState {
        match self.decision {
            GateDecisionKind::AutoPass => AuthoritativeGateState::Passed,
            GateDecisionKind::HumanReview => AuthoritativeGateState::AwaitingHumanReview,
            GateDecisionKind::Block => AuthoritativeGateState::Blocked,
        }
    }

    /// Validates that pinned satisfied refs are truthful. This verifies an
    /// existing decision; it never computes a replacement decision.
    pub fn validate_against(
        &self,
        validation_runs: &[ValidationRun],
    ) -> Result<(), ContractInvariantError> {
        validate_document(
            self.schema_version,
            self.created_at,
            self.updated_at,
            &self.extensions,
        )?;

        let required: BTreeSet<_> = self.required_run_refs.iter().collect();
        let satisfied: BTreeSet<_> = self.satisfied_run_refs.iter().collect();
        if required.len() != self.required_run_refs.len()
            || satisfied.len() != self.satisfied_run_refs.len()
        {
            return Err(ContractInvariantError::DuplicateGateReference);
        }
        if !satisfied.is_subset(&required) {
            return Err(ContractInvariantError::SatisfiedRunNotRequired);
        }

        for reference in &self.satisfied_run_refs {
            let run = validation_runs
                .iter()
                .find(|run| {
                    run.validation_run_id == reference.validation_run_id
                        && run.revision == reference.revision
                })
                .ok_or(ContractInvariantError::MissingValidationRun)?;
            run.validate()?;
            if !run.satisfies_required_check() {
                return Err(ContractInvariantError::UnsatisfiedValidationRun);
            }
        }

        if self.decision == GateDecisionKind::AutoPass {
            if required != satisfied {
                return Err(ContractInvariantError::IncompleteAutoPass);
            }
            if !self.blocking_diagnostic_refs.is_empty() || !self.waivers.is_empty() {
                return Err(ContractInvariantError::BlockedAutoPass);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageEvidenceRefs {
    pub stage_id: StageId,
    pub route_decision_ref: DocumentRef,
    pub permission_plan_ref: DocumentRef,
    pub stage_result_ref: DocumentRef,
    #[serde(default)]
    pub checkpoint_refs: Vec<DocumentRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvidenceRefs {
    pub before_fingerprint: Sha256Hash,
    pub after_fingerprint: Sha256Hash,
    pub change_set_ref: DocumentRef,
    pub changed_files_ref: ArtifactRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventRangeKind {
    Approval,
    Retry,
    Escalation,
    Pause,
    Recovery,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventRangeRef {
    pub kind: EventRangeKind,
    pub run_id: RunId,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifestEntry {
    pub artifact_id: ArtifactId,
    pub sha256: Sha256Hash,
    pub size_bytes: u64,
    pub redaction_status: RedactionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub manifest_ref: ArtifactRef,
    #[serde(default)]
    pub artifacts: Vec<ArtifactManifestEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBundle {
    pub schema_id: EvidenceBundleSchemaId,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub evidence_bundle_id: EvidenceBundleId,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub producer: ProducerRef,
    #[serde(default)]
    pub extensions: Extensions,
    pub goal_spec_ref: DocumentRef,
    pub stage_graph_ref: DocumentRef,
    pub final_revision_ref: DocumentRef,
    #[serde(default)]
    pub stage_evidence: Vec<StageEvidenceRefs>,
    pub change_evidence: ChangeEvidenceRefs,
    #[serde(default)]
    pub validation_plan_refs: Vec<DocumentRef>,
    #[serde(default)]
    pub validation_run_refs: Vec<ValidationRunRef>,
    #[serde(default)]
    pub diagnostic_refs: Vec<DiagnosticRef>,
    pub gate_decision_ref: GateDecisionRef,
    #[serde(default)]
    pub event_ranges: Vec<EventRangeRef>,
    #[serde(default)]
    pub cost_record_refs: Vec<DocumentRef>,
    #[serde(default)]
    pub unmeasured_usage: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_result_ref: Option<DocumentRef>,
    #[serde(default)]
    pub remaining_risks: Vec<RiskRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_ref: Option<DocumentRef>,
    pub artifact_manifest: ArtifactManifest,
    pub completeness: Completeness,
    #[serde(default)]
    pub missing_reasons: Vec<String>,
}

impl EvidenceBundle {
    pub fn validate(&self) -> Result<(), ContractInvariantError> {
        validate_document(
            self.schema_version,
            self.created_at,
            self.updated_at,
            &self.extensions,
        )?;
        match self.completeness {
            Completeness::Complete if !self.missing_reasons.is_empty() => {
                return Err(ContractInvariantError::EvidenceCompleteness);
            }
            Completeness::Partial | Completeness::Unverified if self.missing_reasons.is_empty() => {
                return Err(ContractInvariantError::EvidenceCompleteness);
            }
            _ => {}
        }
        self.change_evidence.changed_files_ref.validate()?;
        self.artifact_manifest.manifest_ref.validate()?;
        let artifact_ids: BTreeSet<_> = self
            .artifact_manifest
            .artifacts
            .iter()
            .map(|artifact| &artifact.artifact_id)
            .collect();
        if artifact_ids.len() != self.artifact_manifest.artifacts.len() {
            return Err(ContractInvariantError::DuplicateArtifact);
        }
        if self
            .event_ranges
            .iter()
            .any(|range| range.last_sequence < range.first_sequence)
        {
            return Err(ContractInvariantError::EvidenceCompleteness);
        }
        Ok(())
    }
}

fn validate_document(
    schema_version: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    extensions: &Extensions,
) -> Result<(), ContractInvariantError> {
    validate_schema_and_extensions(schema_version, extensions)?;
    if updated_at < created_at {
        return Err(ContractInvariantError::DocumentTimeOrder);
    }
    Ok(())
}

fn validate_schema_and_extensions(
    schema_version: u32,
    extensions: &Extensions,
) -> Result<(), ContractInvariantError> {
    if schema_version != EVIDENCE_CONTRACT_SCHEMA_VERSION {
        return Err(ContractInvariantError::SchemaVersion);
    }
    if extensions
        .keys()
        .any(|key| key.is_empty() || !key.contains('.'))
    {
        return Err(ContractInvariantError::ExtensionNamespace);
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), ContractInvariantError> {
    let invalid = path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains(':')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..");
    if invalid {
        Err(ContractInvariantError::ProjectRelativePath(path.to_owned()))
    } else {
        Ok(())
    }
}
