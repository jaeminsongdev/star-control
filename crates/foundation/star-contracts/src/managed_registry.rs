//! M5 managed declaration registry contracts.
//!
//! Git manifests are canonical. Snapshots and consistency records are derived
//! evidence and cannot be used as a source-writing authority.

use std::{collections::BTreeMap, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CheckoutId, ManagedRegistrySnapshotId, ProjectId, RegistryConsistencyRecordId, Sha256Hash,
    evidence::{ArtifactRef, CatalogRef, DocumentRef},
    ids::{CodeIndexSnapshotId, ProjectRevisionId, WorkspaceSnapshotId},
    management::ProjectPathRef,
};

pub const MANAGED_REGISTRY_MANIFEST_SCHEMA_ID: &str = "star.managed-registry-manifest";
pub const MANAGED_REGISTRY_FRAGMENT_SCHEMA_ID: &str = "star.managed-registry-fragment";
pub const MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID: &str = "star.managed-registry-snapshot";
pub const REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID: &str = "star.registry-consistency-record";
pub const MANAGED_DECLARATION_CHANGE_INTENT_SCHEMA_ID: &str =
    "star.managed-declaration-change-intent";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedDeclarationIdError;

impl fmt::Display for ManagedDeclarationIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("managed declaration ID is invalid")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ManagedDeclarationId(String);

impl ManagedDeclarationId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ManagedDeclarationIdError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 160
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
            && value
                .split('.')
                .all(|segment| !segment.is_empty() && !segment.starts_with('-'));
        valid
            .then_some(Self(value))
            .ok_or(ManagedDeclarationIdError)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ManagedDeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for ManagedDeclarationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCompleteness {
    Complete,
    Partial,
    Unverified,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedDeclarationKind {
    ErrorCode,
    DiagnosticId,
    SchemaId,
    SchemaVersion,
    ConfigKey,
    ConfigDefault,
    CliCommand,
    CliExitCode,
    EventId,
    CapabilityId,
    PermissionId,
    FeatureFlag,
    FormatId,
    ResourceId,
    GlobalConstant,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedDeclarationClassification {
    ManagedDeclaration,
    Candidate,
    LocalImplementationConstant,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedLifecycle {
    Active,
    Deprecated,
    Reserved,
    Removed,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedValueRole {
    StableIdentifier,
    ConfigDefault,
    CompileTimeContract,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedBindingKind {
    Definition,
    Reference,
    Schema,
    Docs,
    GeneratedOutput,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedConsumerRequirement {
    Required,
    Optional,
    ObservedOnly,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedConsumerState {
    Bound,
    Alias,
    TransitionRequired,
    BelowMinimum,
    Unresolved,
    Stale,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedOwnerRef {
    pub project_id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_key: Option<String>,
    pub approval_policy_ref: CatalogRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_owner: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DelegatedNamespaceClaim {
    pub namespace: String,
    pub owner_project_id: ProjectId,
    pub allowed_kinds: Vec<ManagedDeclarationKind>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceClaimStatus {
    Active,
    Reserved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamespaceClaim {
    pub namespace: String,
    pub owner_project_id: ProjectId,
    pub allowed_kinds: Vec<ManagedDeclarationKind>,
    #[serde(default)]
    pub delegated_child_namespaces: Vec<DelegatedNamespaceClaim>,
    pub status: NamespaceClaimStatus,
    pub introduced_in_registry_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_ref: Option<DocumentRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleRecord {
    pub introduced_in_registry_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_in_registry_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_in_registry_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_id: Option<ManagedDeclarationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_record_ref: Option<DocumentRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AliasRecord {
    pub value: String,
    pub introduced_in_registry_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_registry_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindingSpec {
    pub binding_id: String,
    pub kind: ManagedBindingKind,
    pub path: ProjectPathRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_key: Option<String>,
    pub expected_value: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerContract {
    pub consumer_surface_id: String,
    pub project_id: ProjectId,
    pub requirement: ManagedConsumerRequirement,
    pub minimum_item_version: String,
    #[serde(default)]
    pub accepted_values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_window_end: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclaration {
    pub managed_declaration_id: ManagedDeclarationId,
    pub item_version: String,
    pub namespace: String,
    pub semantic_key: String,
    pub kind: ManagedDeclarationKind,
    pub owner: ManagedOwnerRef,
    pub value_type: String,
    pub value_role: ManagedValueRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_value: Option<String>,
    pub description: String,
    pub status: ManagedLifecycle,
    pub lifecycle: ManagedLifecycleRecord,
    #[serde(default)]
    pub aliases: Vec<AliasRecord>,
    #[serde(default)]
    pub binding_specs: Vec<BindingSpec>,
    #[serde(default)]
    pub consumer_contracts: Vec<ConsumerContract>,
    pub uniqueness_scope: String,
    pub source_path: ProjectPathRef,
    pub source_sha256: Sha256Hash,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclarationSource {
    pub managed_declaration_id: ManagedDeclarationId,
    pub item_version: String,
    pub namespace: String,
    pub semantic_key: String,
    pub kind: ManagedDeclarationKind,
    pub owner: ManagedOwnerRef,
    pub value_type: String,
    pub value_role: ManagedValueRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_value: Option<String>,
    pub description: String,
    pub status: ManagedLifecycle,
    pub lifecycle: ManagedLifecycleRecord,
    #[serde(default)]
    pub aliases: Vec<AliasRecord>,
    #[serde(default)]
    pub binding_specs: Vec<BindingSpec>,
    #[serde(default)]
    pub consumer_contracts: Vec<ConsumerContract>,
    pub uniqueness_scope: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedRegistryManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_id: String,
    pub registry_version: String,
    pub owner_project_id: ProjectId,
    pub namespace_claims: Vec<NamespaceClaim>,
    pub declaration_files: Vec<ProjectPathRef>,
    pub compatibility_policy_ref: CatalogRef,
    pub required_check_families: Vec<String>,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedRegistryFragment {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_id: String,
    pub namespace: String,
    #[serde(default)]
    pub declarations: Vec<ManagedDeclarationSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistrySourceRef {
    pub path: ProjectPathRef,
    pub source_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedConsumer {
    pub declaration_id: ManagedDeclarationId,
    pub project_id: ProjectId,
    pub path: ProjectPathRef,
    pub observed_value: String,
    pub observed_item_version: String,
    pub state: ManagedConsumerState,
    pub source_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindingObservation {
    pub declaration_id: ManagedDeclarationId,
    pub binding_id: String,
    pub path: ProjectPathRef,
    pub expected_value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_value: Option<String>,
    pub current: bool,
    pub source_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedCandidate {
    pub candidate_id: String,
    pub classification: ManagedDeclarationClassification,
    pub kind: ManagedDeclarationKind,
    pub observed_value: String,
    pub path: ProjectPathRef,
    pub source_sha256: Sha256Hash,
    pub reason_codes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryTombstone {
    pub declaration_id: ManagedDeclarationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserved_value: Option<String>,
    pub removed_in_registry_version: String,
    pub tombstone_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RegistryResolutionState {
    Valid,
    Conflicted,
    Invalid,
    Partial,
    Unverified,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RegistryFreshness {
    Current,
    StaleSource,
    StaleCatalog,
    Partial,
    Unverified,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedRegistrySnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub managed_registry_snapshot_id: ManagedRegistrySnapshotId,
    pub registry_id: String,
    pub registry_version: String,
    pub owner_project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub git_revision: String,
    pub manifest_sha256: Sha256Hash,
    pub manifest_source_refs: Vec<RegistrySourceRef>,
    pub namespace_claims: Vec<NamespaceClaim>,
    pub declarations: Vec<ManagedDeclaration>,
    #[serde(default)]
    pub binding_observations: Vec<BindingObservation>,
    #[serde(default)]
    pub consumers: Vec<ManagedConsumer>,
    #[serde(default)]
    pub candidates: Vec<ManagedCandidate>,
    #[serde(default)]
    pub local_constants: Vec<ManagedCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_index_snapshot_id: Option<CodeIndexSnapshotId>,
    #[serde(default)]
    pub tombstones: Vec<RegistryTombstone>,
    pub tombstone_set_fingerprint: Sha256Hash,
    pub resolution_state: RegistryResolutionState,
    pub freshness: RegistryFreshness,
    pub completeness: EvidenceCompleteness,
    #[serde(default)]
    pub limitations: Vec<String>,
    #[serde(default)]
    pub diagnostic_refs: Vec<DocumentRef>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclarationRef {
    pub managed_declaration_id: ManagedDeclarationId,
    pub item_version: String,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RegistryConsistencyStatus {
    Current,
    BindingDrift,
    ConsumerDrift,
    RemovedReference,
    AliasWindowExpired,
    GeneratedOutputStale,
    DocsSchemaDrift,
    StaleRegistryIndex,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryConsistencyRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_consistency_record_id: RegistryConsistencyRecordId,
    pub registry_snapshot_ref: DocumentRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_ref: Option<ManagedDeclarationRef>,
    pub status: RegistryConsistencyStatus,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_value: Option<String>,
    pub completeness: EvidenceCompleteness,
    #[serde(default)]
    pub evidence_refs: Vec<ArtifactRef>,
    pub remediation: String,
    pub record_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedDeclarationChangeKind {
    Create,
    UpdateDescription,
    ChangePrimaryValue,
    Deprecate,
    AddAlias,
    Remove,
    AddBinding,
    ChangeConsumerFloor,
    ClassifyCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedDesiredFields {
    Create {
        declaration: Box<ManagedDeclaration>,
    },
    UpdateDescription {
        description: String,
    },
    ChangePrimaryValue {
        primary_value: String,
        new_item_version: String,
    },
    Deprecate {
        deprecated_in_registry_version: String,
        replacement_id: Option<ManagedDeclarationId>,
    },
    AddAlias {
        alias: AliasRecord,
    },
    Remove {
        removed_in_registry_version: String,
    },
    AddBinding {
        binding: BindingSpec,
    },
    ChangeConsumerFloor {
        consumer_surface_id: String,
        minimum_item_version: String,
    },
    ClassifyCandidate {
        candidate_id: String,
        classification: ManagedDeclarationClassification,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclarationChangeIntent {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_snapshot_ref: DocumentRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_ref: Option<ManagedDeclarationRef>,
    pub change_kind: ManagedDeclarationChangeKind,
    pub desired_fields: ManagedDesiredFields,
    pub reason: String,
    pub requested_consumer_scope: Vec<ProjectId>,
    pub expected_manifest_fingerprint: Sha256Hash,
    pub intent_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerMigrationState {
    Ready,
    Blocked,
    NoChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRewrite {
    pub project_id: ProjectId,
    pub path: ProjectPathRef,
    pub expected_source_sha256: Sha256Hash,
    pub before_value: String,
    pub after_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerMigrationPlan {
    pub declaration_id: ManagedDeclarationId,
    pub from_snapshot: Sha256Hash,
    pub to_snapshot: Sha256Hash,
    pub state: ConsumerMigrationState,
    pub rewrites: Vec<ConsumerRewrite>,
    pub blockers: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ManagedRegistryContractError {
    #[error("managed registry contract invariant failed")]
    Invalid,
    #[error("managed registry fingerprint calculation failed")]
    Fingerprint,
}

impl ManagedRegistrySnapshot {
    pub fn seal(mut self) -> Result<Self, ManagedRegistryContractError> {
        let mut declaration_ids = self
            .declarations
            .iter()
            .map(|declaration| &declaration.managed_declaration_id)
            .collect::<Vec<_>>();
        declaration_ids.sort();
        let mut source_paths = self
            .manifest_source_refs
            .iter()
            .map(|source| &source.path)
            .collect::<Vec<_>>();
        source_paths.sort();
        if self.schema_id != MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
            || self.schema_version != 2
            || self.registry_id.trim().is_empty()
            || self.registry_version.trim().is_empty()
            || self.git_revision.len() != 40
            || self.manifest_source_refs.is_empty()
            || declaration_ids.windows(2).any(|pair| pair[0] == pair[1])
            || source_paths.windows(2).any(|pair| pair[0] == pair[1])
            || (self.completeness == EvidenceCompleteness::Complete && !self.limitations.is_empty())
            || (self.freshness == RegistryFreshness::Current
                && self.resolution_state != RegistryResolutionState::Valid)
        {
            return Err(ManagedRegistryContractError::Invalid);
        }
        self.content_fingerprint = managed_fingerprint(
            MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID,
            &serde_json::json!({
                "registry_id":self.registry_id,
                "registry_version":self.registry_version,
                "owner_project_id":self.owner_project_id,
                "checkout_id":self.checkout_id,
                "project_revision_id":self.project_revision_id,
                "workspace_snapshot_id":self.workspace_snapshot_id,
                "git_revision":self.git_revision,
                "manifest_sha256":self.manifest_sha256,
                "manifest_source_refs":self.manifest_source_refs,
                "namespace_claims":self.namespace_claims,
                "declarations":self.declarations,
                "binding_observations":self.binding_observations,
                "consumers":self.consumers,
                "candidates":self.candidates,
                "local_constants":self.local_constants,
                "code_index_snapshot_id":self.code_index_snapshot_id,
                "tombstones":self.tombstones,
                "tombstone_set_fingerprint":self.tombstone_set_fingerprint,
                "resolution_state":self.resolution_state,
                "freshness":self.freshness,
                "completeness":self.completeness,
                "limitations":self.limitations,
                "diagnostic_refs":self.diagnostic_refs,
            }),
        )?;
        self.managed_registry_snapshot_id =
            ManagedRegistrySnapshotId::from_fingerprint(&self.content_fingerprint);
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, ManagedRegistryContractError> {
        if self.clone().seal()? != *self {
            return Err(ManagedRegistryContractError::Invalid);
        }
        Ok(DocumentRef {
            schema_id: MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID.to_owned(),
            document_id: self.managed_registry_snapshot_id.to_string(),
            revision: 1,
            sha256: self.content_fingerprint.clone(),
        })
    }
}

impl RegistryConsistencyRecord {
    pub fn seal(mut self) -> Result<Self, ManagedRegistryContractError> {
        if self.schema_id != REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID
            || self.schema_version != 1
            || self.registry_snapshot_ref.schema_id != MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
            || self.registry_snapshot_ref.revision != 1
            || self.subject.trim().is_empty()
            || self.remediation.trim().is_empty()
        {
            return Err(ManagedRegistryContractError::Invalid);
        }
        self.record_fingerprint = managed_fingerprint(
            REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID,
            &serde_json::json!({
                "registry_snapshot_ref":self.registry_snapshot_ref,
                "declaration_ref":self.declaration_ref,
                "status":self.status,
                "subject":self.subject,
                "expected_value":self.expected_value,
                "observed_value":self.observed_value,
                "completeness":self.completeness,
                "remediation":self.remediation,
            }),
        )?;
        self.registry_consistency_record_id =
            RegistryConsistencyRecordId::from_fingerprint(&self.record_fingerprint);
        Ok(self)
    }
}

impl ManagedDeclarationChangeIntent {
    pub fn seal(mut self) -> Result<Self, ManagedRegistryContractError> {
        if self.schema_id != MANAGED_DECLARATION_CHANGE_INTENT_SCHEMA_ID
            || self.schema_version != 1
            || self.registry_snapshot_ref.schema_id != MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
            || self.registry_snapshot_ref.revision != 1
            || self.reason.trim().is_empty()
        {
            return Err(ManagedRegistryContractError::Invalid);
        }
        let mut scope = self.requested_consumer_scope.clone();
        scope.sort();
        scope.dedup();
        if scope != self.requested_consumer_scope {
            return Err(ManagedRegistryContractError::Invalid);
        }
        self.intent_fingerprint = managed_fingerprint(
            MANAGED_DECLARATION_CHANGE_INTENT_SCHEMA_ID,
            &serde_json::json!({
                "registry_snapshot_ref":self.registry_snapshot_ref,
                "declaration_ref":self.declaration_ref,
                "change_kind":self.change_kind,
                "desired_fields":self.desired_fields,
                "reason":self.reason,
                "requested_consumer_scope":self.requested_consumer_scope,
                "expected_manifest_fingerprint":self.expected_manifest_fingerprint,
            }),
        )?;
        Ok(self)
    }
}

fn managed_fingerprint(
    algorithm: &str,
    payload: &impl Serialize,
) -> Result<Sha256Hash, ManagedRegistryContractError> {
    crate::canonical_sha256(&serde_json::json!({
        "algorithm":algorithm,
        "contract_version":1,
        "payload":payload,
    }))
    .map_err(|_| ManagedRegistryContractError::Fingerprint)
}
