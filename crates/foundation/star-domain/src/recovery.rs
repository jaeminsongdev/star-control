//! Pure recovery invariants and canonical fingerprints.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::Serialize;
use star_contracts::{
    BackupSetId, LocalStateBundleId, RecoveryPlanId, Sha256Hash,
    management::{
        BaselineScope, ChangePlanStatus, ProjectStorePoint, StorePoint, StoreScope,
        StoreVersionVector, SuppressionScope,
    },
    recovery::{
        ACTIVE_SET_MANIFEST_SCHEMA_ID, ActiveSetManifest, ActiveStoreGeneration,
        BACKUP_PLAN_SCHEMA_ID, BACKUP_SET_MANIFEST_SCHEMA_ID, BackupPlan, BackupSetManifest,
        BackupStoreEntry, BackupStoreTarget, LOCAL_STATE_BUNDLE_SCHEMA_ID,
        LOCAL_STATE_EXPORT_PLAN_SCHEMA_ID, LOCAL_STATE_IMPORT_PLAN_SCHEMA_ID, LocalStateBundle,
        LocalStateConflict, LocalStateExportPlan, LocalStateImportPlan, REBUILD_PLAN_SCHEMA_ID,
        RESTORE_PLAN_SCHEMA_ID, RebuildPlan, RebuildProjectInput, RecoveryLossItem,
        RecoveryLossKind, RestorePlan,
    },
};
use thiserror::Error;

use crate::{PersistenceRedactor, versioned_fingerprint};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoveryDomainError {
    #[error("recovery contract schema identity is invalid")]
    InvalidSchema,
    #[error("store set must contain one global store and sorted unique project stores")]
    InvalidStoreSet,
    #[error("recovery locator must be non-empty, relative, slash-separated, and traversal-free")]
    InvalidRelativeLocator,
    #[error("recovery fingerprint does not match canonical content")]
    FingerprintMismatch,
    #[error("approved plan fingerprint does not exactly match the planned fingerprint")]
    ApprovalMismatch,
    #[error("backup plan does not match the active set or source store vector")]
    BackupSourceMismatch,
    #[error("restore plan does not match the verified backup set")]
    RestoreSourceMismatch,
    #[error("rebuild plan is not project-bound, unique, or canonically sealed")]
    InvalidRebuildPlan,
    #[error("local state bundle is not project-bound, active-only, or redaction-safe")]
    InvalidLocalStateBundle,
    #[error("local state recovery plan is not unique, current, or canonically sealed")]
    InvalidLocalStatePlan,
}

#[derive(Serialize)]
struct ActiveStoreHeader<'a> {
    scope: &'a StoreScope,
    store_id: &'a star_contracts::ids::ManagementStoreId,
    generation: u64,
    management_store_version: u32,
    relative_locator: &'a str,
}

pub fn active_store_header_fingerprint(
    entry: &ActiveStoreGeneration,
) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-store-generation-header",
        1,
        &ActiveStoreHeader {
            scope: &entry.scope,
            store_id: &entry.store_id,
            generation: entry.generation,
            management_store_version: entry.management_store_version,
            relative_locator: &entry.relative_locator,
        },
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn seal_active_set(
    mut entries: Vec<ActiveStoreGeneration>,
) -> Result<ActiveSetManifest, RecoveryDomainError> {
    entries.sort_by_key(|entry| store_scope_key(&entry.scope));
    for entry in &mut entries {
        entry.header_fingerprint = active_store_header_fingerprint(entry)?;
    }
    validate_store_generations(&entries)?;
    let manifest_fingerprint = versioned_fingerprint("star.management-active-set", 1, &entries)
        .map_err(|_| RecoveryDomainError::FingerprintMismatch)?;
    Ok(ActiveSetManifest {
        schema_id: ACTIVE_SET_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        entries,
        manifest_fingerprint,
    })
}

pub fn validate_active_set(manifest: &ActiveSetManifest) -> Result<(), RecoveryDomainError> {
    if manifest.schema_id != ACTIVE_SET_MANIFEST_SCHEMA_ID || manifest.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_store_generations(&manifest.entries)?;
    for entry in &manifest.entries {
        if entry.header_fingerprint != active_store_header_fingerprint(entry)? {
            return Err(RecoveryDomainError::FingerprintMismatch);
        }
    }
    let expected = versioned_fingerprint("star.management-active-set", 1, &manifest.entries)
        .map_err(|_| RecoveryDomainError::FingerprintMismatch)?;
    if manifest.manifest_fingerprint != expected {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn backup_plan_fingerprint(plan: &BackupPlan) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-backup-plan",
        1,
        &serde_json::json!({
            "backup_set_id":plan.backup_set_id,
            "created_at":plan.created_at,
            "source_active_set_fingerprint":plan.source_active_set_fingerprint,
            "source_store_vector":plan.source_store_vector,
            "destination_fingerprint":plan.destination_fingerprint,
            "stores":plan.stores,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn seal_backup_plan(
    backup_set_id: BackupSetId,
    created_at: DateTime<Utc>,
    active_set: &ActiveSetManifest,
    source_store_vector: StoreVersionVector,
    destination_fingerprint: Sha256Hash,
    mut stores: Vec<BackupStoreTarget>,
) -> Result<BackupPlan, RecoveryDomainError> {
    validate_active_set(active_set)?;
    stores.sort_by_key(|store| store_scope_key(&store.scope));
    let mut plan = BackupPlan {
        schema_id: BACKUP_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        backup_set_id,
        created_at,
        source_active_set_fingerprint: active_set.manifest_fingerprint.clone(),
        source_store_vector,
        destination_fingerprint,
        stores,
        plan_fingerprint: Sha256Hash::digest(b"unsealed"),
    };
    validate_backup_sources(&plan, active_set)?;
    plan.plan_fingerprint = backup_plan_fingerprint(&plan)?;
    Ok(plan)
}

pub fn validate_backup_plan(
    plan: &BackupPlan,
    active_set: &ActiveSetManifest,
) -> Result<(), RecoveryDomainError> {
    if plan.schema_id != BACKUP_PLAN_SCHEMA_ID || plan.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_active_set(active_set)?;
    validate_backup_sources(plan, active_set)?;
    if plan.plan_fingerprint != backup_plan_fingerprint(plan)? {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn require_exact_approval(
    expected: &Sha256Hash,
    approved: &str,
) -> Result<(), RecoveryDomainError> {
    let approved = approved
        .parse::<Sha256Hash>()
        .map_err(|_| RecoveryDomainError::ApprovalMismatch)?;
    if &approved != expected {
        return Err(RecoveryDomainError::ApprovalMismatch);
    }
    Ok(())
}

pub fn backup_set_fingerprint(
    backup_set_id: &BackupSetId,
    created_at: DateTime<Utc>,
    source_active_set_fingerprint: &Sha256Hash,
    entries: &[BackupStoreEntry],
) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-backup-set",
        1,
        &serde_json::json!({
            "backup_set_id":backup_set_id,
            "created_at":created_at,
            "source_active_set_fingerprint":source_active_set_fingerprint,
            "entries":entries,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn seal_backup_set(
    backup_set_id: BackupSetId,
    created_at: DateTime<Utc>,
    source_active_set_fingerprint: Sha256Hash,
    mut entries: Vec<BackupStoreEntry>,
) -> Result<BackupSetManifest, RecoveryDomainError> {
    entries.sort_by_key(|entry| store_scope_key(&entry.scope));
    validate_backup_entries(&entries)?;
    let set_fingerprint = backup_set_fingerprint(
        &backup_set_id,
        created_at,
        &source_active_set_fingerprint,
        &entries,
    )?;
    Ok(BackupSetManifest {
        schema_id: BACKUP_SET_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        backup_set_id,
        created_at,
        source_active_set_fingerprint,
        entries,
        set_fingerprint,
    })
}

pub fn validate_backup_set(manifest: &BackupSetManifest) -> Result<(), RecoveryDomainError> {
    if manifest.schema_id != BACKUP_SET_MANIFEST_SCHEMA_ID || manifest.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_backup_entries(&manifest.entries)?;
    let expected = backup_set_fingerprint(
        &manifest.backup_set_id,
        manifest.created_at,
        &manifest.source_active_set_fingerprint,
        &manifest.entries,
    )?;
    if manifest.set_fingerprint != expected {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn restore_plan_fingerprint(plan: &RestorePlan) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-restore-plan",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "backup_set_id":plan.backup_set_id,
            "created_at":plan.created_at,
            "backup_set_fingerprint":plan.backup_set_fingerprint,
            "expected_active_set_fingerprint":plan.expected_active_set_fingerprint,
            "stores":plan.stores,
            "candidate_active_set":plan.candidate_active_set,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn validate_restore_plan(
    plan: &RestorePlan,
    backup: &BackupSetManifest,
) -> Result<(), RecoveryDomainError> {
    if plan.schema_id != RESTORE_PLAN_SCHEMA_ID || plan.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_backup_set(backup)?;
    validate_active_set(&plan.candidate_active_set)?;
    if plan.backup_set_id != backup.backup_set_id
        || plan.backup_set_fingerprint != backup.set_fingerprint
        || plan.stores.len() != backup.entries.len()
        || plan
            .stores
            .iter()
            .zip(&backup.entries)
            .any(|(target, source)| {
                target.scope != source.scope
                    || target.store_id != source.store_id
                    || target.source_generation != source.generation
                    || target.management_store_version != source.management_store_version
                    || target.source_byte_sha256 != source.byte_sha256
            })
    {
        return Err(RecoveryDomainError::RestoreSourceMismatch);
    }
    if plan.plan_fingerprint != restore_plan_fingerprint(plan)? {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn rebuild_plan_fingerprint(plan: &RebuildPlan) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-rebuild-plan",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "created_at":plan.created_at,
            "expected_active_set_fingerprint":plan.expected_active_set_fingerprint,
            "candidate_generation":plan.candidate_generation,
            "projects":plan.projects,
            "predicted_losses":plan.predicted_losses,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn seal_rebuild_plan(
    recovery_plan_id: star_contracts::RecoveryPlanId,
    created_at: DateTime<Utc>,
    expected_active_set_fingerprint: Option<Sha256Hash>,
    candidate_generation: u64,
    mut projects: Vec<RebuildProjectInput>,
    mut predicted_losses: Vec<RecoveryLossItem>,
) -> Result<RebuildPlan, RecoveryDomainError> {
    projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    predicted_losses.sort_by_key(recovery_loss_key);
    let mut plan = RebuildPlan {
        schema_id: REBUILD_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id,
        created_at,
        expected_active_set_fingerprint,
        candidate_generation,
        projects,
        predicted_losses,
        plan_fingerprint: Sha256Hash::digest(b"unsealed"),
    };
    validate_rebuild_shape(&plan)?;
    plan.plan_fingerprint = rebuild_plan_fingerprint(&plan)?;
    Ok(plan)
}

pub fn validate_rebuild_plan(plan: &RebuildPlan) -> Result<(), RecoveryDomainError> {
    if plan.schema_id != REBUILD_PLAN_SCHEMA_ID || plan.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_rebuild_shape(plan)?;
    if plan.plan_fingerprint != rebuild_plan_fingerprint(plan)? {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn local_state_bundle_fingerprint(
    bundle: &LocalStateBundle,
) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-local-state-bundle",
        1,
        &serde_json::json!({
            "bundle_id":bundle.bundle_id,
            "project_id":bundle.project_id,
            "source_revision_id":bundle.source_revision_id,
            "effective_config_fingerprint":bundle.effective_config_fingerprint,
            "redaction_contract_version":bundle.redaction_contract_version,
            "local_suppressions":bundle.local_suppressions,
            "local_baselines":bundle.local_baselines,
            "local_dispositions":bundle.local_dispositions,
            "active_change_plans":bundle.active_change_plans,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

#[allow(clippy::too_many_arguments)]
pub fn seal_local_state_bundle(
    bundle_id: LocalStateBundleId,
    project_id: star_contracts::ProjectId,
    source_revision_id: star_contracts::ids::ProjectRevisionId,
    effective_config_fingerprint: Sha256Hash,
    mut local_suppressions: Vec<star_contracts::management::Suppression>,
    mut local_baselines: Vec<star_contracts::management::Baseline>,
    mut local_dispositions: Vec<star_contracts::management::Disposition>,
    mut active_change_plans: Vec<star_contracts::management::ChangePlan>,
    redactor: &PersistenceRedactor,
) -> Result<LocalStateBundle, RecoveryDomainError> {
    local_suppressions.sort_by(|left, right| left.suppression_id.cmp(&right.suppression_id));
    local_baselines.sort_by(|left, right| left.baseline_id.cmp(&right.baseline_id));
    local_dispositions.sort_by(|left, right| left.disposition_id.cmp(&right.disposition_id));
    active_change_plans.sort_by(|left, right| left.change_plan_id.cmp(&right.change_plan_id));
    let mut bundle = LocalStateBundle {
        schema_id: LOCAL_STATE_BUNDLE_SCHEMA_ID.to_owned(),
        schema_version: 1,
        bundle_id,
        project_id,
        source_revision_id,
        effective_config_fingerprint,
        redaction_contract_version: star_contracts::management::REDACTION_CONTRACT_VERSION,
        local_suppressions,
        local_baselines,
        local_dispositions,
        active_change_plans,
        content_fingerprint: Sha256Hash::digest(b"unsealed"),
    };
    validate_local_state_shape(&bundle)?;
    let value =
        serde_json::to_value(&bundle).map_err(|_| RecoveryDomainError::InvalidLocalStateBundle)?;
    validate_redacted_json(&value, redactor)?;
    bundle.content_fingerprint = local_state_bundle_fingerprint(&bundle)?;
    Ok(bundle)
}

pub fn validate_local_state_bundle(
    bundle: &LocalStateBundle,
    redactor: &PersistenceRedactor,
) -> Result<(), RecoveryDomainError> {
    if bundle.schema_id != LOCAL_STATE_BUNDLE_SCHEMA_ID
        || bundle.schema_version != 1
        || bundle.redaction_contract_version
            != star_contracts::management::REDACTION_CONTRACT_VERSION
    {
        return Err(RecoveryDomainError::InvalidLocalStateBundle);
    }
    validate_local_state_shape(bundle)?;
    let value =
        serde_json::to_value(bundle).map_err(|_| RecoveryDomainError::InvalidLocalStateBundle)?;
    validate_redacted_json(&value, redactor)?;
    if bundle.content_fingerprint != local_state_bundle_fingerprint(bundle)? {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

pub fn local_state_export_plan_fingerprint(
    plan: &LocalStateExportPlan,
) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-local-state-export-plan",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "bundle_id":plan.bundle_id,
            "project_id":plan.project_id,
            "source_revision_id":plan.source_revision_id,
            "effective_config_fingerprint":plan.effective_config_fingerprint,
            "expected_store_revision":plan.expected_store_revision,
            "destination_fingerprint":plan.destination_fingerprint,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

#[allow(clippy::too_many_arguments)]
pub fn seal_local_state_export_plan(
    recovery_plan_id: RecoveryPlanId,
    bundle_id: LocalStateBundleId,
    project_id: star_contracts::ProjectId,
    source_revision_id: star_contracts::ids::ProjectRevisionId,
    effective_config_fingerprint: Sha256Hash,
    expected_store_revision: u64,
    destination_fingerprint: Sha256Hash,
) -> Result<LocalStateExportPlan, RecoveryDomainError> {
    let mut plan = LocalStateExportPlan {
        schema_id: LOCAL_STATE_EXPORT_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id,
        bundle_id,
        project_id,
        source_revision_id,
        effective_config_fingerprint,
        expected_store_revision,
        destination_fingerprint,
        plan_fingerprint: Sha256Hash::digest(b"unsealed"),
    };
    plan.plan_fingerprint = local_state_export_plan_fingerprint(&plan)?;
    Ok(plan)
}

pub fn validate_local_state_export_plan(
    plan: &LocalStateExportPlan,
) -> Result<(), RecoveryDomainError> {
    if plan.schema_id != LOCAL_STATE_EXPORT_PLAN_SCHEMA_ID
        || plan.schema_version != 1
        || plan.plan_fingerprint != local_state_export_plan_fingerprint(plan)?
    {
        return Err(RecoveryDomainError::InvalidLocalStatePlan);
    }
    Ok(())
}

pub fn local_state_import_plan_fingerprint(
    plan: &LocalStateImportPlan,
) -> Result<Sha256Hash, RecoveryDomainError> {
    versioned_fingerprint(
        "star.management-local-state-import-plan",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "bundle_id":plan.bundle_id,
            "project_id":plan.project_id,
            "expected_source_revision_id":plan.expected_source_revision_id,
            "expected_config_fingerprint":plan.expected_config_fingerprint,
            "expected_store_revision":plan.expected_store_revision,
            "payload_sha256":plan.payload_sha256,
            "suppression_ids":plan.suppression_ids,
            "baseline_ids":plan.baseline_ids,
            "disposition_ids":plan.disposition_ids,
            "change_plan_ids":plan.change_plan_ids,
            "conflicts":plan.conflicts,
        }),
    )
    .map_err(|_| RecoveryDomainError::FingerprintMismatch)
}

pub fn seal_local_state_import_plan(
    recovery_plan_id: RecoveryPlanId,
    bundle: &LocalStateBundle,
    expected_store_revision: u64,
    payload_sha256: Sha256Hash,
    mut conflicts: Vec<LocalStateConflict>,
    redactor: &PersistenceRedactor,
) -> Result<LocalStateImportPlan, RecoveryDomainError> {
    validate_local_state_bundle(bundle, redactor)?;
    conflicts.sort_by(|left, right| {
        (&left.entity_kind, &left.entity_id, &left.reason_code).cmp(&(
            &right.entity_kind,
            &right.entity_id,
            &right.reason_code,
        ))
    });
    let mut plan = LocalStateImportPlan {
        schema_id: LOCAL_STATE_IMPORT_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id,
        bundle_id: bundle.bundle_id.clone(),
        project_id: bundle.project_id.clone(),
        expected_source_revision_id: bundle.source_revision_id.clone(),
        expected_config_fingerprint: bundle.effective_config_fingerprint.clone(),
        expected_store_revision,
        payload_sha256,
        suppression_ids: bundle
            .local_suppressions
            .iter()
            .map(|value| value.suppression_id.clone())
            .collect(),
        baseline_ids: bundle
            .local_baselines
            .iter()
            .map(|value| value.baseline_id.clone())
            .collect(),
        disposition_ids: bundle
            .local_dispositions
            .iter()
            .map(|value| value.disposition_id.clone())
            .collect(),
        change_plan_ids: bundle
            .active_change_plans
            .iter()
            .map(|value| value.change_plan_id.clone())
            .collect(),
        conflicts,
        plan_fingerprint: Sha256Hash::digest(b"unsealed"),
    };
    validate_local_state_import_shape(&plan)?;
    plan.plan_fingerprint = local_state_import_plan_fingerprint(&plan)?;
    Ok(plan)
}

pub fn validate_local_state_import_plan(
    plan: &LocalStateImportPlan,
) -> Result<(), RecoveryDomainError> {
    if plan.schema_id != LOCAL_STATE_IMPORT_PLAN_SCHEMA_ID || plan.schema_version != 1 {
        return Err(RecoveryDomainError::InvalidSchema);
    }
    validate_local_state_import_shape(plan)?;
    if plan.plan_fingerprint != local_state_import_plan_fingerprint(plan)? {
        return Err(RecoveryDomainError::FingerprintMismatch);
    }
    Ok(())
}

fn validate_local_state_shape(bundle: &LocalStateBundle) -> Result<(), RecoveryDomainError> {
    if bundle.local_suppressions.iter().any(|value| {
        value.project_id != bundle.project_id || value.scope_kind != SuppressionScope::Local
    }) || bundle.local_baselines.iter().any(|value| {
        value.project_id != bundle.project_id || value.scope_kind != BaselineScope::Local
    }) || bundle.active_change_plans.iter().any(|value| {
        value.project_id != bundle.project_id
            || !matches!(
                value.status,
                ChangePlanStatus::Draft | ChangePlanStatus::Ready | ChangePlanStatus::Blocked
            )
    }) || !strictly_sorted_unique(
        bundle
            .local_suppressions
            .iter()
            .map(|value| value.suppression_id.as_str()),
    ) || !strictly_sorted_unique(
        bundle
            .local_baselines
            .iter()
            .map(|value| value.baseline_id.as_str()),
    ) || !strictly_sorted_unique(
        bundle
            .local_dispositions
            .iter()
            .map(|value| value.disposition_id.as_str()),
    ) || !strictly_sorted_unique(
        bundle
            .active_change_plans
            .iter()
            .map(|value| value.change_plan_id.as_str()),
    ) {
        return Err(RecoveryDomainError::InvalidLocalStateBundle);
    }
    Ok(())
}

fn validate_local_state_import_shape(
    plan: &LocalStateImportPlan,
) -> Result<(), RecoveryDomainError> {
    if !strictly_sorted_unique(plan.suppression_ids.iter().map(|value| value.as_str()))
        || !strictly_sorted_unique(plan.baseline_ids.iter().map(|value| value.as_str()))
        || !strictly_sorted_unique(plan.disposition_ids.iter().map(|value| value.as_str()))
        || !strictly_sorted_unique(plan.change_plan_ids.iter().map(|value| value.as_str()))
        || plan.conflicts.iter().any(|conflict| {
            conflict.entity_kind.trim().is_empty()
                || conflict.entity_id.trim().is_empty()
                || conflict.reason_code.trim().is_empty()
                || conflict.entity_kind.contains('\0')
                || conflict.entity_id.contains('\0')
                || conflict.reason_code.contains('\0')
        })
        || plan.conflicts.windows(2).any(|pair| {
            (
                &pair[0].entity_kind,
                &pair[0].entity_id,
                &pair[0].reason_code,
            ) >= (
                &pair[1].entity_kind,
                &pair[1].entity_id,
                &pair[1].reason_code,
            )
        })
    {
        return Err(RecoveryDomainError::InvalidLocalStatePlan);
    }
    Ok(())
}

fn strictly_sorted_unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut previous: Option<&str> = None;
    for value in values {
        if value.is_empty() || previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn validate_backup_sources(
    plan: &BackupPlan,
    active_set: &ActiveSetManifest,
) -> Result<(), RecoveryDomainError> {
    if plan.source_active_set_fingerprint != active_set.manifest_fingerprint
        || plan.stores.len() != active_set.entries.len()
        || plan
            .stores
            .windows(2)
            .any(|pair| store_scope_key(&pair[0].scope) >= store_scope_key(&pair[1].scope))
        || plan
            .stores
            .iter()
            .zip(&active_set.entries)
            .any(|(target, active)| {
                target.scope != active.scope
                    || target.store_id != active.store_id
                    || target.generation != active.generation
                    || target.management_store_version != active.management_store_version
            })
        || store_vector_from_targets(&plan.stores) != plan.source_store_vector
    {
        return Err(RecoveryDomainError::BackupSourceMismatch);
    }
    Ok(())
}

fn validate_rebuild_shape(plan: &RebuildPlan) -> Result<(), RecoveryDomainError> {
    if plan.candidate_generation == 0 || plan.projects.is_empty() {
        return Err(RecoveryDomainError::InvalidRebuildPlan);
    }
    let mut projects = BTreeSet::new();
    let mut checkouts = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    let mut previous_project = None;
    for project in &plan.projects {
        if previous_project.is_some_and(|previous| previous >= &project.project_id)
            || !projects.insert(project.project_id.as_str())
            || !checkouts.insert(project.checkout_id.as_str())
            || !bindings.insert(project.root_binding_id.as_str())
        {
            return Err(RecoveryDomainError::InvalidRebuildPlan);
        }
        previous_project = Some(&project.project_id);
    }
    if plan.predicted_losses.iter().any(|loss| {
        loss.reason_code.trim().is_empty()
            || loss.reason_code.contains('\0')
            || loss
                .project_id
                .as_ref()
                .is_some_and(|project_id| !projects.contains(project_id.as_str()))
    }) || plan
        .predicted_losses
        .windows(2)
        .any(|pair| recovery_loss_key(&pair[0]) >= recovery_loss_key(&pair[1]))
    {
        return Err(RecoveryDomainError::InvalidRebuildPlan);
    }
    Ok(())
}

fn recovery_loss_key(item: &RecoveryLossItem) -> (String, u8) {
    let kind = match item.kind {
        RecoveryLossKind::LocalSuppression => 0,
        RecoveryLossKind::LocalBaseline => 1,
        RecoveryLossKind::LocalDisposition => 2,
        RecoveryLossKind::ActiveChangePlan => 3,
        RecoveryLossKind::IdempotencyHistory => 4,
        RecoveryLossKind::ActorHistory => 5,
        RecoveryLossKind::EventTimestamp => 6,
        RecoveryLossKind::ArtifactReference => 7,
    };
    (
        item.project_id
            .as_ref()
            .map_or_else(String::new, |project_id| project_id.as_str().to_owned()),
        kind,
    )
}

fn store_vector_from_targets(stores: &[BackupStoreTarget]) -> StoreVersionVector {
    let mut global = None;
    let mut projects = Vec::new();
    for store in stores {
        let point = StorePoint {
            store_id: store.store_id.clone(),
            generation: store.generation,
            revision: store.store_revision,
        };
        match &store.scope {
            StoreScope::Global => global = Some(point),
            StoreScope::Project { project_id } => projects.push(ProjectStorePoint {
                project_id: project_id.clone(),
                point,
            }),
        }
    }
    StoreVersionVector {
        global: global.unwrap_or_else(|| StorePoint {
            store_id: star_contracts::ids::ManagementStoreId::new(),
            generation: 0,
            revision: 0,
        }),
        projects,
    }
}

fn validate_store_generations(
    entries: &[ActiveStoreGeneration],
) -> Result<(), RecoveryDomainError> {
    if entries.is_empty()
        || entries
            .iter()
            .filter(|entry| matches!(entry.scope, StoreScope::Global))
            .count()
            != 1
        || entries
            .windows(2)
            .any(|pair| store_scope_key(&pair[0].scope) >= store_scope_key(&pair[1].scope))
        || !unique_store_ids(entries.iter().map(|entry| entry.store_id.as_str()))
        || !unique_store_ids(entries.iter().map(|entry| entry.relative_locator.as_str()))
        || entries.iter().any(|entry| {
            entry.generation == 0
                || entry.management_store_version == 0
                || !valid_relative_locator(&entry.relative_locator)
        })
    {
        return Err(RecoveryDomainError::InvalidStoreSet);
    }
    Ok(())
}

fn validate_backup_entries(entries: &[BackupStoreEntry]) -> Result<(), RecoveryDomainError> {
    if entries.is_empty()
        || entries
            .iter()
            .filter(|entry| matches!(entry.scope, StoreScope::Global))
            .count()
            != 1
        || entries
            .windows(2)
            .any(|pair| store_scope_key(&pair[0].scope) >= store_scope_key(&pair[1].scope))
        || !unique_store_ids(entries.iter().map(|entry| entry.store_id.as_str()))
        || !unique_store_ids(entries.iter().map(|entry| entry.relative_locator.as_str()))
        || entries.iter().any(|entry| {
            entry.generation == 0
                || entry.management_store_version == 0
                || entry.size_bytes == 0
                || !valid_relative_locator(&entry.relative_locator)
        })
    {
        return Err(RecoveryDomainError::InvalidStoreSet);
    }
    Ok(())
}

fn valid_relative_locator(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn unique_store_ids<'a>(mut values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.all(|value| seen.insert(value))
}

fn store_scope_key(scope: &StoreScope) -> String {
    match scope {
        StoreScope::Global => "0".to_owned(),
        StoreScope::Project { project_id } => format!("1:{}", project_id.as_str()),
    }
}

fn validate_redacted_json(
    value: &serde_json::Value,
    redactor: &PersistenceRedactor,
) -> Result<(), RecoveryDomainError> {
    match value {
        serde_json::Value::String(value) => redactor
            .validate(value)
            .map_err(|_| RecoveryDomainError::InvalidLocalStateBundle),
        serde_json::Value::Array(values) => {
            for value in values {
                validate_redacted_json(value, redactor)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                let lower = key.to_ascii_lowercase();
                if ["secret", "password", "token", "username", "root_binding"]
                    .iter()
                    .any(|marker| lower.contains(marker))
                {
                    return Err(RecoveryDomainError::InvalidLocalStateBundle);
                }
                validate_redacted_json(value, redactor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use star_contracts::{
        ProjectId,
        ids::ManagementStoreId,
        recovery::{ActiveStoreGeneration, BackupStoreEntry},
    };

    use super::*;

    fn hash(value: &str) -> Sha256Hash {
        Sha256Hash::digest(value.as_bytes())
    }

    fn entry(scope: StoreScope, locator: &str) -> ActiveStoreGeneration {
        ActiveStoreGeneration {
            scope,
            store_id: ManagementStoreId::new(),
            generation: 1,
            management_store_version: 2,
            relative_locator: locator.to_owned(),
            header_fingerprint: hash("placeholder"),
        }
    }

    #[test]
    fn active_set_is_sorted_sealed_and_tamper_evident() {
        let project = ProjectId::new();
        let manifest = seal_active_set(vec![
            entry(
                StoreScope::Project {
                    project_id: project,
                },
                "projects/one/generations/1/store",
            ),
            entry(StoreScope::Global, "global/generations/1/store"),
        ])
        .unwrap();
        validate_active_set(&manifest).unwrap();
        assert!(matches!(manifest.entries[0].scope, StoreScope::Global));

        let mut tampered = manifest;
        tampered.entries[0].relative_locator = "global/generations/2/store".to_owned();
        assert_eq!(
            validate_active_set(&tampered),
            Err(RecoveryDomainError::FingerprintMismatch)
        );
    }

    #[test]
    fn active_and_backup_sets_reject_traversal_and_mixed_store_sets() {
        assert_eq!(
            seal_active_set(vec![entry(StoreScope::Global, "../active/store")]),
            Err(RecoveryDomainError::InvalidStoreSet)
        );
        let project = ProjectId::new();
        let duplicate_project = ProjectId::new();
        let backup = vec![
            BackupStoreEntry {
                scope: StoreScope::Global,
                store_id: ManagementStoreId::new(),
                generation: 1,
                management_store_version: 2,
                store_revision: 1,
                relative_locator: "stores/global/store".to_owned(),
                size_bytes: 1,
                byte_sha256: hash("global"),
            },
            BackupStoreEntry {
                scope: StoreScope::Project {
                    project_id: project,
                },
                store_id: ManagementStoreId::new(),
                generation: 1,
                management_store_version: 2,
                store_revision: 1,
                relative_locator: "stores/project/store".to_owned(),
                size_bytes: 1,
                byte_sha256: hash("project"),
            },
            BackupStoreEntry {
                scope: StoreScope::Project {
                    project_id: duplicate_project,
                },
                store_id: ManagementStoreId::new(),
                generation: 1,
                management_store_version: 2,
                store_revision: 1,
                relative_locator: "stores/project/store".to_owned(),
                size_bytes: 1,
                byte_sha256: hash("other"),
            },
        ];
        assert_eq!(
            seal_backup_set(BackupSetId::new(), Utc::now(), hash("active"), backup),
            Err(RecoveryDomainError::InvalidStoreSet)
        );
    }

    #[test]
    fn approval_is_exact_and_parse_checked() {
        let expected = hash("plan");
        require_exact_approval(&expected, expected.as_str()).unwrap();
        assert_eq!(
            require_exact_approval(&expected, hash("other").as_str()),
            Err(RecoveryDomainError::ApprovalMismatch)
        );
        assert_eq!(
            require_exact_approval(&expected, "not-a-fingerprint"),
            Err(RecoveryDomainError::ApprovalMismatch)
        );
    }
}
