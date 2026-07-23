//! Backend-neutral contracts for management backup, restore, rebuild, and local-state recovery.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BackupSetId, LocalStateBundleId, ProjectId, RecoveryPlanId, Sha256Hash,
    ids::{
        BaselineId, ChangePlanId, CheckoutId, DispositionId, ManagementStoreId, ProjectRevisionId,
        RootBindingId, ScanRunId, SuppressionId, WorkspaceSnapshotId,
    },
    management::{Baseline, ChangePlan, Disposition, StoreScope, StoreVersionVector, Suppression},
};

pub const ACTIVE_SET_MANIFEST_SCHEMA_ID: &str = "star.management-active-set";
pub const BACKUP_PLAN_SCHEMA_ID: &str = "star.management-backup-plan";
pub const BACKUP_SET_MANIFEST_SCHEMA_ID: &str = "star.management-backup-set-manifest";
pub const BACKUP_APPLY_RESULT_SCHEMA_ID: &str = "star.management-backup-apply-result";
pub const RECOVERY_STATUS_SCHEMA_ID: &str = "star.management-recovery-status";
pub const RESTORE_PLAN_SCHEMA_ID: &str = "star.management-restore-plan";
pub const RESTORE_APPLY_RESULT_SCHEMA_ID: &str = "star.management-restore-apply-result";
pub const REBUILD_PLAN_SCHEMA_ID: &str = "star.management-rebuild-plan";
pub const REBUILD_APPLY_RESULT_SCHEMA_ID: &str = "star.management-rebuild-apply-result";
pub const LOCAL_STATE_BUNDLE_SCHEMA_ID: &str = "star.management-local-state-bundle";
pub const LOCAL_STATE_EXPORT_PLAN_SCHEMA_ID: &str = "star.management-local-state-export-plan";
pub const LOCAL_STATE_EXPORT_RESULT_SCHEMA_ID: &str = "star.management-local-state-export-result";
pub const LOCAL_STATE_IMPORT_PLAN_SCHEMA_ID: &str = "star.management-local-state-import-plan";
pub const LOCAL_STATE_IMPORT_RESULT_SCHEMA_ID: &str = "star.management-local-state-import-result";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryInspection {
    Missing,
    Healthy,
    MigrationRequired,
    FutureVersion,
    Corrupt,
    ActiveSetMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControllerRecoveryMode {
    Normal,
    RecoveryOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOperation {
    Status,
    BackupPlan,
    BackupApply,
    RestorePlan,
    RestoreApply,
    RebuildPlan,
    RebuildApply,
    LocalStateExportPlan,
    LocalStateExportApply,
    LocalStateImportPlan,
    LocalStateImportApply,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActiveStoreGeneration {
    pub scope: StoreScope,
    pub store_id: ManagementStoreId,
    pub generation: u64,
    pub management_store_version: u32,
    pub relative_locator: String,
    pub header_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActiveSetManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub entries: Vec<ActiveStoreGeneration>,
    pub manifest_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupStoreTarget {
    pub scope: StoreScope,
    pub store_id: ManagementStoreId,
    pub generation: u64,
    pub management_store_version: u32,
    pub store_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub backup_set_id: BackupSetId,
    pub created_at: DateTime<Utc>,
    pub source_active_set_fingerprint: Sha256Hash,
    pub source_store_vector: StoreVersionVector,
    pub destination_fingerprint: Sha256Hash,
    pub stores: Vec<BackupStoreTarget>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupStoreEntry {
    pub scope: StoreScope,
    pub store_id: ManagementStoreId,
    pub generation: u64,
    pub management_store_version: u32,
    pub store_revision: u64,
    pub relative_locator: String,
    pub size_bytes: u64,
    pub byte_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupSetManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub backup_set_id: BackupSetId,
    pub created_at: DateTime<Utc>,
    pub source_active_set_fingerprint: Sha256Hash,
    pub entries: Vec<BackupStoreEntry>,
    pub set_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupApplyResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub backup_set_id: BackupSetId,
    pub applied_at: DateTime<Utc>,
    pub approved_plan_fingerprint: Sha256Hash,
    pub manifest: BackupSetManifest,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreRecoveryStatus {
    pub scope: Option<StoreScope>,
    pub relative_locator: Option<String>,
    pub inspection: RecoveryInspection,
    pub diagnostic_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryStatus {
    pub schema_id: String,
    pub schema_version: u32,
    pub mode: ControllerRecoveryMode,
    pub active_set: Option<ActiveSetManifest>,
    pub stores: Vec<StoreRecoveryStatus>,
    pub allowed_operations: Vec<RecoveryOperation>,
    pub status_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreStoreTarget {
    pub scope: StoreScope,
    pub store_id: ManagementStoreId,
    pub source_generation: u64,
    pub candidate_generation: u64,
    pub management_store_version: u32,
    pub source_byte_sha256: Sha256Hash,
    pub candidate_relative_locator: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestorePlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub backup_set_id: BackupSetId,
    pub created_at: DateTime<Utc>,
    pub backup_set_fingerprint: Sha256Hash,
    pub expected_active_set_fingerprint: Option<Sha256Hash>,
    pub stores: Vec<RestoreStoreTarget>,
    pub candidate_active_set: ActiveSetManifest,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreApplyResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub backup_set_id: BackupSetId,
    pub applied_at: DateTime<Utc>,
    pub approved_plan_fingerprint: Sha256Hash,
    pub previous_active_set_fingerprint: Option<Sha256Hash>,
    pub activated_set: ActiveSetManifest,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RebuildProjectInput {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub root_binding_id: RootBindingId,
    pub source_revision_id: ProjectRevisionId,
    pub effective_config_fingerprint: Sha256Hash,
    pub artifact_inventory_fingerprint: Sha256Hash,
    pub verified_artifact_count: u64,
    pub rejected_artifact_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryLossKind {
    LocalSuppression,
    LocalBaseline,
    LocalDisposition,
    ActiveChangePlan,
    IdempotencyHistory,
    ActorHistory,
    EventTimestamp,
    ArtifactReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryLossState {
    Preserved,
    RecoverableFromExport,
    Lost,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryLossItem {
    pub project_id: Option<ProjectId>,
    pub kind: RecoveryLossKind,
    pub state: RecoveryLossState,
    pub count: Option<u64>,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RebuildPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub created_at: DateTime<Utc>,
    pub expected_active_set_fingerprint: Option<Sha256Hash>,
    pub candidate_generation: u64,
    pub projects: Vec<RebuildProjectInput>,
    pub predicted_losses: Vec<RecoveryLossItem>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RebuiltProjectSummary {
    pub project_id: ProjectId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub scan_run_id: ScanRunId,
    pub canonical_source_count: u64,
    pub symbol_count: u64,
    pub finding_count: u64,
    pub reindexed_artifact_count: u64,
    pub rejected_artifact_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RebuildApplyResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub applied_at: DateTime<Utc>,
    pub approved_plan_fingerprint: Sha256Hash,
    pub rebuilt_projects: Vec<RebuiltProjectSummary>,
    pub loss_report: Vec<RecoveryLossItem>,
    pub activated_set: ActiveSetManifest,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateBundle {
    pub schema_id: String,
    pub schema_version: u32,
    pub bundle_id: LocalStateBundleId,
    pub project_id: ProjectId,
    pub source_revision_id: ProjectRevisionId,
    pub effective_config_fingerprint: Sha256Hash,
    pub redaction_contract_version: u32,
    pub local_suppressions: Vec<Suppression>,
    pub local_baselines: Vec<Baseline>,
    pub local_dispositions: Vec<Disposition>,
    pub active_change_plans: Vec<ChangePlan>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateExportPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub bundle_id: LocalStateBundleId,
    pub project_id: ProjectId,
    pub source_revision_id: ProjectRevisionId,
    pub effective_config_fingerprint: Sha256Hash,
    pub expected_store_revision: u64,
    pub destination_fingerprint: Sha256Hash,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateExportResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub applied_at: DateTime<Utc>,
    pub approved_plan_fingerprint: Sha256Hash,
    pub bundle: LocalStateBundle,
    pub payload_sha256: Sha256Hash,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateConflict {
    pub entity_kind: String,
    pub entity_id: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateImportPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub bundle_id: LocalStateBundleId,
    pub project_id: ProjectId,
    pub expected_source_revision_id: ProjectRevisionId,
    pub expected_config_fingerprint: Sha256Hash,
    pub expected_store_revision: u64,
    pub payload_sha256: Sha256Hash,
    pub suppression_ids: Vec<SuppressionId>,
    pub baseline_ids: Vec<BaselineId>,
    pub disposition_ids: Vec<DispositionId>,
    pub change_plan_ids: Vec<ChangePlanId>,
    pub conflicts: Vec<LocalStateConflict>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalStateImportResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: RecoveryPlanId,
    pub bundle_id: LocalStateBundleId,
    pub applied_at: DateTime<Utc>,
    pub approved_plan_fingerprint: Sha256Hash,
    pub imported_suppressions: u64,
    pub imported_baselines: u64,
    pub imported_dispositions: u64,
    pub imported_change_plans: u64,
    pub result_fingerprint: Sha256Hash,
}

#[cfg(test)]
mod tests {
    use std::{fmt::Debug, fs, path::Path};

    use serde::de::DeserializeOwned;

    use super::*;
    use crate::management::decode_current_management_document;

    fn assert_fixture_set<T: DeserializeOwned + Debug>(stem: &str, schema_id: &str) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1")
            .join(stem);
        for name in ["minimal.json", "full.json"] {
            let input = fs::read_to_string(root.join(name)).expect("generated recovery fixture");
            decode_current_management_document::<T>(&input, schema_id)
                .unwrap_or_else(|error| panic!("{stem}/{name} must decode: {error}"));
        }
        for name in ["invalid.json", "future.json"] {
            let input = fs::read_to_string(root.join(name)).expect("generated recovery fixture");
            assert!(
                decode_current_management_document::<T>(&input, schema_id).is_err(),
                "{stem}/{name} must be rejected"
            );
        }
    }

    #[test]
    fn generated_recovery_fixtures_round_trip_through_strict_types() {
        assert_fixture_set::<ActiveSetManifest>(
            "management-active-set",
            ACTIVE_SET_MANIFEST_SCHEMA_ID,
        );
        assert_fixture_set::<BackupPlan>("management-backup-plan", BACKUP_PLAN_SCHEMA_ID);
        assert_fixture_set::<BackupSetManifest>(
            "management-backup-set-manifest",
            BACKUP_SET_MANIFEST_SCHEMA_ID,
        );
        assert_fixture_set::<BackupApplyResult>(
            "management-backup-apply-result",
            BACKUP_APPLY_RESULT_SCHEMA_ID,
        );
        assert_fixture_set::<RecoveryStatus>(
            "management-recovery-status",
            RECOVERY_STATUS_SCHEMA_ID,
        );
        assert_fixture_set::<RestorePlan>("management-restore-plan", RESTORE_PLAN_SCHEMA_ID);
        assert_fixture_set::<RestoreApplyResult>(
            "management-restore-apply-result",
            RESTORE_APPLY_RESULT_SCHEMA_ID,
        );
        assert_fixture_set::<RebuildPlan>("management-rebuild-plan", REBUILD_PLAN_SCHEMA_ID);
        assert_fixture_set::<RebuildApplyResult>(
            "management-rebuild-apply-result",
            REBUILD_APPLY_RESULT_SCHEMA_ID,
        );
        assert_fixture_set::<LocalStateBundle>(
            "management-local-state-bundle",
            LOCAL_STATE_BUNDLE_SCHEMA_ID,
        );
        assert_fixture_set::<LocalStateExportPlan>(
            "management-local-state-export-plan",
            LOCAL_STATE_EXPORT_PLAN_SCHEMA_ID,
        );
        assert_fixture_set::<LocalStateExportResult>(
            "management-local-state-export-result",
            LOCAL_STATE_EXPORT_RESULT_SCHEMA_ID,
        );
        assert_fixture_set::<LocalStateImportPlan>(
            "management-local-state-import-plan",
            LOCAL_STATE_IMPORT_PLAN_SCHEMA_ID,
        );
        assert_fixture_set::<LocalStateImportResult>(
            "management-local-state-import-result",
            LOCAL_STATE_IMPORT_RESULT_SCHEMA_ID,
        );
    }
}
