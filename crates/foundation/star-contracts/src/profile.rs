use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Sha256Hash, canonical_sha256};

pub const DEVELOPMENT_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const DEVELOPMENT_PROFILE_DESCRIPTOR_SCHEMA_ID: &str = "star.development-profile-descriptor";
pub const DEVELOPMENT_PROFILE_CATALOG_SCHEMA_ID: &str = "star.development-profile-catalog-snapshot";
pub const DEVELOPMENT_PROFILE_RESOLUTION_SCHEMA_ID: &str = "star.development-profile-resolution";

pub const BUILTIN_DEVELOPMENT_PROFILE_IDS: [&str; 16] = [
    "ai_development_validation",
    "api_contract_change",
    "architecture_quality",
    "change_planning",
    "ci_release_deploy",
    "data_config_db_migration",
    "debug_recovery",
    "dependency_upgrade",
    "docs_config_environment",
    "language_platform_migration",
    "performance_build",
    "project_understanding",
    "refactor_codemod",
    "rust_style_auto_fix",
    "security_supply_chain",
    "test_correctness",
];

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

string_enum!(ProfileGatePhaseV1 {
    DuringStage,
    StageExit,
    GoalExit,
    PatchPreApply,
    PatchPostApply,
    Merge,
    MigrationPreExecute,
    MigrationPostExecute,
    MigrationPostRollback,
    PerformanceCompare,
    LanguageCutover,
    Release,
    ReleasePreflight,
    ReleaseBuild,
    ReleaseVerify,
    ReleasePackage,
    ReleaseInstallLifecycle,
    ReleaseReady,
    ReleasePublishPreflight,
    ReleasePublishVerify,
});

string_enum!(ProfileBaselinePolicyV1 {
    Off,
    ReportOnly,
    RatchetNew,
    RatchetNewAndWorsened,
    CleanOnly,
});

string_enum!(ProfileSuppressionPolicyV1 {
    Exact,
    BoundedExpiring,
    ApprovedExpiring,
});

string_enum!(ProfileStabilityPolicyV1 {
    ReportFlaky,
    HumanReviewFlaky,
    BlockRequiredFlaky,
});

string_enum!(ProfileReviewFloorV1 {
    None,
    HumanSemantic,
    IndependentHuman,
    Block,
});

string_enum!(ProfilePermissionFloorV1 {
    LocalReadOnly,
    ExactPrompt,
    ExactDurableApproval,
    DenyExternal,
});

string_enum!(ProfileEffectClassV1 {
    Read,
    Plan,
    Validate,
    LocalStateWrite,
    WorkspaceMutation,
    NetworkRead,
    RemoteWrite,
    Publish,
});

string_enum!(ProfileApprovalCheckpointV1 {
    Network,
    Download,
    DebugAttach,
    DependencyChange,
    PatchApply,
    MigrationExecute,
    DestructiveMigration,
    UnknownFieldLoss,
    CrossProjectEffect,
    LanguageCutover,
    RemoteWrite,
    ReleasePublish,
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileRefV1 {
    pub profile_id: String,
    pub profile_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileReviewPolicyV1 {
    pub cli_only: ProfileReviewFloorV1,
    pub codex_managed: ProfileReviewFloorV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationProfileMetadataV1 {
    pub target_kinds: Vec<String>,
    pub invariant_families: Vec<String>,
    pub strategy_floor: String,
    pub backup_policy: String,
    pub restore_policy: String,
    pub rehearsal_policy: String,
    pub resume_policy: String,
    pub destructive_policy: String,
    pub default_pending_action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseProfileMetadataV1 {
    pub validation_layer_refs: Vec<String>,
    pub release_policy_ref: String,
    pub target_environment_refs: Vec<String>,
    pub package_lifecycle_refs: Vec<String>,
    pub supply_chain_applicability_ref: String,
    pub approval_state_policy_ref: String,
    pub evaluation_policy_refs: Vec<String>,
    pub evaluation_contexts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustStyleProfileMetadataV1 {
    pub profile_kind: String,
    pub pipeline_ref: String,
    pub toolchain_release: String,
    pub default_policy: String,
    pub supported_policies: Vec<String>,
    pub tool_role_refs: Vec<String>,
    pub source_effect: String,
    pub network: String,
    pub steps: Vec<String>,
    pub required_gates: Vec<String>,
    pub allowed_live_operations: Vec<String>,
    pub built_in_clippy_fix_allowlist: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileExtensionsV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<MigrationProfileMetadataV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseProfileMetadataV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_style: Option<RustStyleProfileMetadataV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileDescriptorV1 {
    pub schema_version: u32,
    pub profile_id: String,
    pub profile_version: String,
    pub display_name: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_profile: Option<DevelopmentProfileRefV1>,
    pub triggers: Vec<String>,
    pub stage_template: Vec<String>,
    pub context_rules: Vec<String>,
    pub route_hints: Vec<String>,
    pub permission_actions: Vec<String>,
    pub gate_phases: Vec<ProfileGatePhaseV1>,
    pub required_rule_families: Vec<String>,
    pub required_check_families: Vec<String>,
    pub optional_check_families: Vec<String>,
    pub always_run_for: Vec<String>,
    pub baseline_policy: ProfileBaselinePolicyV1,
    pub suppression_policy: ProfileSuppressionPolicyV1,
    pub claim_policy: Vec<String>,
    pub stability_policy: ProfileStabilityPolicyV1,
    pub review_policy: ProfileReviewPolicyV1,
    pub evidence_requirements: Vec<String>,
    pub corpus_requirements: Vec<String>,
    pub approval_checkpoints: Vec<ProfileApprovalCheckpointV1>,
    pub default_stop_state: String,
    pub allowed_effect_classes: Vec<ProfileEffectClassV1>,
    pub permission_floor: ProfilePermissionFloorV1,
    #[serde(default)]
    pub extensions: DevelopmentProfileExtensionsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileCatalogEntryV1 {
    pub profile_ref: DevelopmentProfileRefV1,
    pub definition_hash: Sha256Hash,
    pub descriptor: DevelopmentProfileDescriptorV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileCatalogSnapshotV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub entries: Vec<DevelopmentProfileCatalogEntryV1>,
    pub catalog_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileDefinitionRefV1 {
    pub profile_ref: DevelopmentProfileRefV1,
    pub definition_hash: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentProfileResolutionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub catalog_fingerprint: Sha256Hash,
    pub selected_profiles: Vec<DevelopmentProfileRefV1>,
    pub parent_closure: Vec<DevelopmentProfileRefV1>,
    pub definition_refs: Vec<DevelopmentProfileDefinitionRefV1>,
    pub triggers: Vec<String>,
    pub stage_refs: Vec<String>,
    pub context_rules: Vec<String>,
    pub route_hints: Vec<String>,
    pub permission_actions: Vec<String>,
    pub gate_phases: Vec<ProfileGatePhaseV1>,
    pub required_rule_families: Vec<String>,
    pub required_check_families: Vec<String>,
    pub optional_check_families: Vec<String>,
    pub always_run_for: Vec<String>,
    pub baseline_policy: ProfileBaselinePolicyV1,
    pub suppression_policy: ProfileSuppressionPolicyV1,
    pub claim_policy: Vec<String>,
    pub stability_policy: ProfileStabilityPolicyV1,
    pub review_policy: ProfileReviewPolicyV1,
    pub evidence_requirements: Vec<String>,
    pub corpus_requirements: Vec<String>,
    pub approval_checkpoints: Vec<ProfileApprovalCheckpointV1>,
    pub allowed_effect_classes: Vec<ProfileEffectClassV1>,
    pub permission_floor: ProfilePermissionFloorV1,
    pub default_stop_states: Vec<String>,
    pub profile_resolution_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DevelopmentProfileContractError {
    #[error("profile descriptor is invalid")]
    Invalid,
    #[error("profile descriptor contains an invalid version")]
    Version,
    #[error("profile catalog contains a duplicate profile id")]
    Duplicate,
    #[error("profile catalog does not contain the exact built-in profile set")]
    BuiltinSet,
    #[error("profile parent reference is missing or version-mismatched")]
    ParentReference,
    #[error("profile parent graph contains a cycle")]
    ParentCycle,
    #[error("requested profile was not found")]
    NotFound,
    #[error("profile fingerprint could not be calculated")]
    Fingerprint,
    #[error("profile TOML is invalid")]
    Toml,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.contains('\0')
        && !value.chars().any(char::is_whitespace)
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn unique_non_empty(values: &[String], max_items: usize) -> bool {
    values.len() <= max_items
        && values.iter().all(|value| valid_ref(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

impl DevelopmentProfileDescriptorV1 {
    pub fn parse_toml(source: &str) -> Result<Self, DevelopmentProfileContractError> {
        let descriptor: Self =
            toml::from_str(source).map_err(|_| DevelopmentProfileContractError::Toml)?;
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn profile_ref(&self) -> DevelopmentProfileRefV1 {
        DevelopmentProfileRefV1 {
            profile_id: self.profile_id.clone(),
            profile_version: self.profile_version.clone(),
        }
    }

    pub fn definition_hash(&self) -> Result<Sha256Hash, DevelopmentProfileContractError> {
        canonical_sha256(
            &serde_json::to_value(self)
                .map_err(|_| DevelopmentProfileContractError::Fingerprint)?,
        )
        .map_err(|_| DevelopmentProfileContractError::Fingerprint)
    }

    pub fn validate(&self) -> Result<(), DevelopmentProfileContractError> {
        if self.schema_version != DEVELOPMENT_PROFILE_SCHEMA_VERSION
            || !valid_id(&self.profile_id)
            || !valid_text(&self.display_name, 160)
            || !valid_text(&self.summary, 1_024)
            || semver::Version::parse(&self.profile_version).is_err()
            || self.parent_profile.as_ref().is_some_and(|parent| {
                !valid_id(&parent.profile_id)
                    || parent.profile_id == self.profile_id
                    || semver::Version::parse(&parent.profile_version).is_err()
            })
            || !unique_non_empty(&self.triggers, 64)
            || self.triggers.is_empty()
            || !unique_non_empty(&self.stage_template, 32)
            || self.stage_template.is_empty()
            || !unique_non_empty(&self.context_rules, 64)
            || !unique_non_empty(&self.route_hints, 64)
            || !unique_non_empty(&self.permission_actions, 64)
            || self.gate_phases.is_empty()
            || self.gate_phases.iter().collect::<BTreeSet<_>>().len() != self.gate_phases.len()
            || !unique_non_empty(&self.required_rule_families, 128)
            || self.required_rule_families.is_empty()
            || !unique_non_empty(&self.required_check_families, 128)
            || self.required_check_families.is_empty()
            || !unique_non_empty(&self.optional_check_families, 128)
            || !unique_non_empty(&self.always_run_for, 64)
            || !unique_non_empty(&self.claim_policy, 64)
            || !unique_non_empty(&self.evidence_requirements, 64)
            || self.evidence_requirements.is_empty()
            || !unique_non_empty(&self.corpus_requirements, 64)
            || self
                .approval_checkpoints
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.approval_checkpoints.len()
            || !valid_ref(&self.default_stop_state)
            || self.allowed_effect_classes.is_empty()
            || self
                .allowed_effect_classes
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.allowed_effect_classes.len()
            || self.review_policy.cli_only > ProfileReviewFloorV1::HumanSemantic
        {
            return Err(DevelopmentProfileContractError::Invalid);
        }
        self.validate_extensions()
    }

    fn validate_extensions(&self) -> Result<(), DevelopmentProfileContractError> {
        let migration_profile = matches!(
            self.profile_id.as_str(),
            "data_config_db_migration" | "language_platform_migration" | "performance_build"
        );
        if migration_profile != self.extensions.migration.is_some()
            || (self.profile_id == "ci_release_deploy") != self.extensions.release.is_some()
            || (self.profile_id == "rust_style_auto_fix") != self.extensions.rust_style.is_some()
        {
            return Err(DevelopmentProfileContractError::Invalid);
        }
        if let Some(metadata) = &self.extensions.migration
            && (!unique_non_empty(&metadata.target_kinds, 32)
                || metadata.target_kinds.is_empty()
                || !unique_non_empty(&metadata.invariant_families, 64)
                || metadata.invariant_families.is_empty()
                || [
                    &metadata.strategy_floor,
                    &metadata.backup_policy,
                    &metadata.restore_policy,
                    &metadata.rehearsal_policy,
                    &metadata.resume_policy,
                    &metadata.destructive_policy,
                    &metadata.default_pending_action,
                ]
                .iter()
                .any(|value| !valid_ref(value)))
        {
            return Err(DevelopmentProfileContractError::Invalid);
        }
        if let Some(metadata) = &self.extensions.release
            && (!unique_non_empty(&metadata.validation_layer_refs, 16)
                || metadata.validation_layer_refs.is_empty()
                || !unique_non_empty(&metadata.target_environment_refs, 32)
                || !unique_non_empty(&metadata.package_lifecycle_refs, 32)
                || !unique_non_empty(&metadata.evaluation_policy_refs, 32)
                || !unique_non_empty(&metadata.evaluation_contexts, 8)
                || [
                    &metadata.release_policy_ref,
                    &metadata.supply_chain_applicability_ref,
                    &metadata.approval_state_policy_ref,
                ]
                .iter()
                .any(|value| !valid_ref(value)))
        {
            return Err(DevelopmentProfileContractError::Invalid);
        }
        if let Some(metadata) = &self.extensions.rust_style
            && (metadata.profile_kind != "rust_style_auto_fix"
                || !valid_ref(&metadata.pipeline_ref)
                || semver::Version::parse(&metadata.toolchain_release).is_err()
                || !valid_ref(&metadata.default_policy)
                || !unique_non_empty(&metadata.supported_policies, 16)
                || !metadata
                    .supported_policies
                    .contains(&metadata.default_policy)
                || !unique_non_empty(&metadata.tool_role_refs, 16)
                || !valid_ref(&metadata.source_effect)
                || metadata.network != "denied"
                || !unique_non_empty(&metadata.steps, 64)
                || metadata.steps.is_empty()
                || !unique_non_empty(&metadata.required_gates, 16)
                || !unique_non_empty(&metadata.allowed_live_operations, 16)
                || !unique_non_empty(&metadata.built_in_clippy_fix_allowlist, 128))
        {
            return Err(DevelopmentProfileContractError::Invalid);
        }
        Ok(())
    }
}

pub fn build_development_profile_catalog(
    descriptors: Vec<DevelopmentProfileDescriptorV1>,
) -> Result<DevelopmentProfileCatalogSnapshotV1, DevelopmentProfileContractError> {
    let mut by_id = BTreeMap::new();
    for descriptor in descriptors {
        descriptor.validate()?;
        if by_id
            .insert(descriptor.profile_id.clone(), descriptor)
            .is_some()
        {
            return Err(DevelopmentProfileContractError::Duplicate);
        }
    }
    let actual = by_id.keys().map(String::as_str).collect::<Vec<_>>();
    if actual != BUILTIN_DEVELOPMENT_PROFILE_IDS {
        return Err(DevelopmentProfileContractError::BuiltinSet);
    }
    validate_parent_graph(&by_id)?;
    let mut entries = Vec::with_capacity(by_id.len());
    for descriptor in by_id.into_values() {
        entries.push(DevelopmentProfileCatalogEntryV1 {
            profile_ref: descriptor.profile_ref(),
            definition_hash: descriptor.definition_hash()?,
            descriptor,
        });
    }
    let catalog_fingerprint = canonical_sha256(
        &serde_json::to_value(&entries)
            .map_err(|_| DevelopmentProfileContractError::Fingerprint)?,
    )
    .map_err(|_| DevelopmentProfileContractError::Fingerprint)?;
    Ok(DevelopmentProfileCatalogSnapshotV1 {
        schema_id: DEVELOPMENT_PROFILE_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: DEVELOPMENT_PROFILE_SCHEMA_VERSION,
        entries,
        catalog_fingerprint,
    })
}

fn validate_parent_graph(
    descriptors: &BTreeMap<String, DevelopmentProfileDescriptorV1>,
) -> Result<(), DevelopmentProfileContractError> {
    for descriptor in descriptors.values() {
        if let Some(parent) = &descriptor.parent_profile {
            let Some(parent_descriptor) = descriptors.get(&parent.profile_id) else {
                return Err(DevelopmentProfileContractError::ParentReference);
            };
            if parent_descriptor.profile_version != parent.profile_version {
                return Err(DevelopmentProfileContractError::ParentReference);
            }
        }
    }
    let mut states = BTreeMap::<String, u8>::new();
    for id in descriptors.keys() {
        visit_parent(id, descriptors, &mut states)?;
    }
    Ok(())
}

fn visit_parent(
    id: &str,
    descriptors: &BTreeMap<String, DevelopmentProfileDescriptorV1>,
    states: &mut BTreeMap<String, u8>,
) -> Result<(), DevelopmentProfileContractError> {
    match states.get(id).copied() {
        Some(1) => return Err(DevelopmentProfileContractError::ParentCycle),
        Some(2) => return Ok(()),
        _ => {}
    }
    states.insert(id.to_owned(), 1);
    if let Some(parent) = descriptors
        .get(id)
        .and_then(|descriptor| descriptor.parent_profile.as_ref())
    {
        visit_parent(&parent.profile_id, descriptors, states)?;
    }
    states.insert(id.to_owned(), 2);
    Ok(())
}

pub fn resolve_development_profiles(
    catalog: &DevelopmentProfileCatalogSnapshotV1,
    selected_profile_ids: &[String],
) -> Result<DevelopmentProfileResolutionV1, DevelopmentProfileContractError> {
    catalog.validate()?;
    if selected_profile_ids.is_empty()
        || selected_profile_ids.len() > BUILTIN_DEVELOPMENT_PROFILE_IDS.len()
    {
        return Err(DevelopmentProfileContractError::Invalid);
    }
    let entries = catalog
        .entries
        .iter()
        .map(|entry| (entry.profile_ref.profile_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut selected_ids = selected_profile_ids.to_vec();
    selected_ids.sort();
    selected_ids.dedup();
    if selected_ids.len() != selected_profile_ids.len() {
        return Err(DevelopmentProfileContractError::Duplicate);
    }
    let selected_set = selected_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut closure_order = Vec::new();
    let mut closure_set = BTreeSet::new();
    for id in &selected_ids {
        collect_profile_closure(id, &entries, &mut closure_set, &mut closure_order)?;
    }
    let selected_profiles = selected_ids
        .iter()
        .map(|id| entries.get(id).map(|entry| entry.profile_ref.clone()))
        .collect::<Option<Vec<_>>>()
        .ok_or(DevelopmentProfileContractError::NotFound)?;
    let parent_closure = closure_order
        .iter()
        .filter(|id| !selected_set.contains(*id))
        .map(|id| entries[id].profile_ref.clone())
        .collect::<Vec<_>>();

    let mut triggers = BTreeSet::new();
    let mut stages = BTreeSet::new();
    let mut context_rules = BTreeSet::new();
    let mut route_hints = BTreeSet::new();
    let mut permission_actions = BTreeSet::new();
    let mut gate_phases = BTreeSet::new();
    let mut required_rules = BTreeSet::new();
    let mut required_checks = BTreeSet::new();
    let mut optional_checks = BTreeSet::new();
    let mut always_run = BTreeSet::new();
    let mut claims = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut corpus = BTreeSet::new();
    let mut checkpoints = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut stop_states = BTreeSet::new();
    let mut baseline = ProfileBaselinePolicyV1::Off;
    let mut suppression = ProfileSuppressionPolicyV1::Exact;
    let mut stability = ProfileStabilityPolicyV1::ReportFlaky;
    let mut cli_review = ProfileReviewFloorV1::None;
    let mut codex_review = ProfileReviewFloorV1::None;
    let mut permission_floor = ProfilePermissionFloorV1::LocalReadOnly;
    let mut definition_refs = Vec::new();
    for id in &closure_order {
        let entry = entries[id];
        let descriptor = &entry.descriptor;
        definition_refs.push(DevelopmentProfileDefinitionRefV1 {
            profile_ref: entry.profile_ref.clone(),
            definition_hash: entry.definition_hash.clone(),
        });
        triggers.extend(descriptor.triggers.iter().cloned());
        stages.extend(descriptor.stage_template.iter().cloned());
        context_rules.extend(descriptor.context_rules.iter().cloned());
        route_hints.extend(descriptor.route_hints.iter().cloned());
        permission_actions.extend(descriptor.permission_actions.iter().cloned());
        gate_phases.extend(descriptor.gate_phases.iter().copied());
        required_rules.extend(descriptor.required_rule_families.iter().cloned());
        required_checks.extend(descriptor.required_check_families.iter().cloned());
        optional_checks.extend(descriptor.optional_check_families.iter().cloned());
        always_run.extend(descriptor.always_run_for.iter().cloned());
        claims.extend(descriptor.claim_policy.iter().cloned());
        evidence.extend(descriptor.evidence_requirements.iter().cloned());
        corpus.extend(descriptor.corpus_requirements.iter().cloned());
        checkpoints.extend(descriptor.approval_checkpoints.iter().copied());
        effects.extend(descriptor.allowed_effect_classes.iter().copied());
        stop_states.insert(descriptor.default_stop_state.clone());
        baseline = baseline.max(descriptor.baseline_policy);
        suppression = suppression.max(descriptor.suppression_policy);
        stability = stability.max(descriptor.stability_policy);
        cli_review = cli_review.max(descriptor.review_policy.cli_only);
        codex_review = codex_review.max(descriptor.review_policy.codex_managed);
        permission_floor = permission_floor.max(descriptor.permission_floor);
    }
    let mut resolution = DevelopmentProfileResolutionV1 {
        schema_id: DEVELOPMENT_PROFILE_RESOLUTION_SCHEMA_ID.to_owned(),
        schema_version: DEVELOPMENT_PROFILE_SCHEMA_VERSION,
        catalog_fingerprint: catalog.catalog_fingerprint.clone(),
        selected_profiles,
        parent_closure,
        definition_refs,
        triggers: triggers.into_iter().collect(),
        stage_refs: stages.into_iter().collect(),
        context_rules: context_rules.into_iter().collect(),
        route_hints: route_hints.into_iter().collect(),
        permission_actions: permission_actions.into_iter().collect(),
        gate_phases: gate_phases.into_iter().collect(),
        required_rule_families: required_rules.into_iter().collect(),
        required_check_families: required_checks.into_iter().collect(),
        optional_check_families: optional_checks.into_iter().collect(),
        always_run_for: always_run.into_iter().collect(),
        baseline_policy: baseline,
        suppression_policy: suppression,
        claim_policy: claims.into_iter().collect(),
        stability_policy: stability,
        review_policy: ProfileReviewPolicyV1 {
            cli_only: cli_review,
            codex_managed: codex_review,
        },
        evidence_requirements: evidence.into_iter().collect(),
        corpus_requirements: corpus.into_iter().collect(),
        approval_checkpoints: checkpoints.into_iter().collect(),
        allowed_effect_classes: effects.into_iter().collect(),
        permission_floor,
        default_stop_states: stop_states.into_iter().collect(),
        profile_resolution_fingerprint: Sha256Hash::digest(b""),
    };
    resolution.profile_resolution_fingerprint = resolution_fingerprint(&resolution)?;
    Ok(resolution)
}

impl DevelopmentProfileCatalogSnapshotV1 {
    pub fn validate(&self) -> Result<(), DevelopmentProfileContractError> {
        let rebuilt = build_development_profile_catalog(
            self.entries
                .iter()
                .map(|entry| entry.descriptor.clone())
                .collect(),
        )?;
        if rebuilt != *self {
            return Err(DevelopmentProfileContractError::Fingerprint);
        }
        Ok(())
    }
}

impl DevelopmentProfileResolutionV1 {
    pub fn validate(&self) -> Result<(), DevelopmentProfileContractError> {
        if self.schema_id != DEVELOPMENT_PROFILE_RESOLUTION_SCHEMA_ID
            || self.schema_version != DEVELOPMENT_PROFILE_SCHEMA_VERSION
            || self.selected_profiles.is_empty()
            || self.profile_resolution_fingerprint != resolution_fingerprint(self)?
        {
            return Err(DevelopmentProfileContractError::Fingerprint);
        }
        Ok(())
    }
}

fn resolution_fingerprint(
    resolution: &DevelopmentProfileResolutionV1,
) -> Result<Sha256Hash, DevelopmentProfileContractError> {
    let mut value = serde_json::to_value(resolution)
        .map_err(|_| DevelopmentProfileContractError::Fingerprint)?;
    value
        .as_object_mut()
        .ok_or(DevelopmentProfileContractError::Fingerprint)?
        .remove("profile_resolution_fingerprint");
    canonical_sha256(&value).map_err(|_| DevelopmentProfileContractError::Fingerprint)
}

fn collect_profile_closure(
    id: &str,
    entries: &BTreeMap<String, &DevelopmentProfileCatalogEntryV1>,
    seen: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), DevelopmentProfileContractError> {
    let entry = entries
        .get(id)
        .ok_or(DevelopmentProfileContractError::NotFound)?;
    if seen.contains(id) {
        return Ok(());
    }
    if let Some(parent) = &entry.descriptor.parent_profile {
        collect_profile_closure(&parent.profile_id, entries, seen, order)?;
    }
    seen.insert(id.to_owned());
    order.push(id.to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: &str, parent: Option<&str>) -> DevelopmentProfileDescriptorV1 {
        let migration = matches!(
            id,
            "data_config_db_migration" | "language_platform_migration" | "performance_build"
        )
        .then(|| MigrationProfileMetadataV1 {
            target_kinds: vec!["source".to_owned()],
            invariant_families: vec!["equivalence".to_owned()],
            strategy_floor: "staged".to_owned(),
            backup_policy: "required".to_owned(),
            restore_policy: "verified".to_owned(),
            rehearsal_policy: "required".to_owned(),
            resume_policy: "checkpointed".to_owned(),
            destructive_policy: "prompt".to_owned(),
            default_pending_action: "execute".to_owned(),
        });
        let release = (id == "ci_release_deploy").then(|| ReleaseProfileMetadataV1 {
            validation_layer_refs: vec!["full".to_owned()],
            release_policy_ref: "packaging/release.toml@1".to_owned(),
            target_environment_refs: vec!["windows_x64_stable".to_owned()],
            package_lifecycle_refs: vec!["install".to_owned()],
            supply_chain_applicability_ref: "supply_chain@1".to_owned(),
            approval_state_policy_ref: "release_state@2".to_owned(),
            evaluation_policy_refs: vec!["evaluation@2".to_owned()],
            evaluation_contexts: vec!["cli_only".to_owned()],
        });
        let rust_style = (id == "rust_style_auto_fix").then(|| RustStyleProfileMetadataV1 {
            profile_kind: "rust_style_auto_fix".to_owned(),
            pipeline_ref: "rust_style_v1@1".to_owned(),
            toolchain_release: "1.96.0".to_owned(),
            default_policy: "safe_default".to_owned(),
            supported_policies: vec!["safe_default".to_owned()],
            tool_role_refs: vec!["cargo_fmt@1".to_owned()],
            source_effect: "isolated_preview_then_exact_patch".to_owned(),
            network: "denied".to_owned(),
            steps: vec!["resolve".to_owned()],
            required_gates: vec!["patch_pre_apply".to_owned()],
            allowed_live_operations: vec!["modify_handwritten_in_scope_rs".to_owned()],
            built_in_clippy_fix_allowlist: vec![],
        });
        DevelopmentProfileDescriptorV1 {
            schema_version: 1,
            profile_id: id.to_owned(),
            profile_version: "1.0.0".to_owned(),
            display_name: id.to_owned(),
            summary: "fixture profile".to_owned(),
            parent_profile: parent.map(|id| DevelopmentProfileRefV1 {
                profile_id: id.to_owned(),
                profile_version: "1.0.0".to_owned(),
            }),
            triggers: vec!["code_change".to_owned()],
            stage_template: vec!["m3_gate".to_owned()],
            context_rules: vec!["current_index".to_owned()],
            route_hints: vec!["common_validation".to_owned()],
            permission_actions: vec![],
            gate_phases: vec![ProfileGatePhaseV1::GoalExit],
            required_rule_families: vec!["correctness".to_owned()],
            required_check_families: vec!["test".to_owned()],
            optional_check_families: vec![],
            always_run_for: vec![],
            baseline_policy: ProfileBaselinePolicyV1::RatchetNew,
            suppression_policy: ProfileSuppressionPolicyV1::BoundedExpiring,
            claim_policy: vec!["change".to_owned()],
            stability_policy: ProfileStabilityPolicyV1::HumanReviewFlaky,
            review_policy: ProfileReviewPolicyV1 {
                cli_only: ProfileReviewFloorV1::HumanSemantic,
                codex_managed: ProfileReviewFloorV1::IndependentHuman,
            },
            evidence_requirements: vec!["gate".to_owned()],
            corpus_requirements: vec![],
            approval_checkpoints: vec![],
            default_stop_state: "ready".to_owned(),
            allowed_effect_classes: vec![ProfileEffectClassV1::Validate],
            permission_floor: ProfilePermissionFloorV1::LocalReadOnly,
            extensions: DevelopmentProfileExtensionsV1 {
                migration,
                release,
                rust_style,
            },
        }
    }

    fn catalog() -> DevelopmentProfileCatalogSnapshotV1 {
        build_development_profile_catalog(
            BUILTIN_DEVELOPMENT_PROFILE_IDS
                .iter()
                .map(|id| descriptor(id, None))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn exact_builtin_set_resolves_deterministically() {
        let catalog = catalog();
        let first = resolve_development_profiles(
            &catalog,
            &["test_correctness".to_owned(), "change_planning".to_owned()],
        )
        .unwrap();
        let second = resolve_development_profiles(
            &catalog,
            &["change_planning".to_owned(), "test_correctness".to_owned()],
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_profiles.len(), 2);
    }

    #[test]
    fn missing_parent_and_cycle_are_rejected() {
        let mut missing = BUILTIN_DEVELOPMENT_PROFILE_IDS
            .iter()
            .map(|id| descriptor(id, None))
            .collect::<Vec<_>>();
        missing[0].parent_profile = Some(DevelopmentProfileRefV1 {
            profile_id: "missing".to_owned(),
            profile_version: "1.0.0".to_owned(),
        });
        assert_eq!(
            build_development_profile_catalog(missing).unwrap_err(),
            DevelopmentProfileContractError::ParentReference
        );

        let mut cycle = BUILTIN_DEVELOPMENT_PROFILE_IDS
            .iter()
            .map(|id| descriptor(id, None))
            .collect::<Vec<_>>();
        cycle[0].parent_profile = Some(cycle[1].profile_ref());
        cycle[1].parent_profile = Some(cycle[0].profile_ref());
        assert_eq!(
            build_development_profile_catalog(cycle).unwrap_err(),
            DevelopmentProfileContractError::ParentCycle
        );
    }
}
