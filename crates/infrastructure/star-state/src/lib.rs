//! Private embedded-relational repository and Windows root-binding adapters.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
    backup::Backup, limits::Limit, params,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use star_contracts::recovery::RecoveryInspection;
use star_contracts::{
    BackupSetId, LocalStateBundleId, RecoveryPlanId, Sha256Hash, canonical_sha256,
    evidence::{
        ArtifactRef, Completeness, GateDecision, GateScope,
        RetentionClass as EvidenceRetentionClass,
    },
    evidence_v2::{
        BaselineV2, DecisionSubjectKindV2, DiagnosticEvidenceRefV2, DiagnosticV2, DispositionV2,
        EvidenceBundleV2, EvidenceFreshnessV2, GateDecisionV2, ReviewPackV1, SuppressionV2,
        ValidationResultV2, ValidationRunV2, rule_set_fingerprint_for_subject_binding,
    },
    ids::{
        CheckoutId, CodeIndexSnapshotId, CoordinatedOperationId, DiagnosticId, EventId,
        EvidenceBundleId, FindingId, GateId, ManagementStoreId, PatchSetId, ProjectId,
        ProjectRevisionId, ReviewPackId, RootBindingId, ScanRunId, TaskSpecId, ValidationResultId,
        ValidationRunId, WorkspaceSnapshotId,
    },
    index::{CodeIndexSnapshot, IndexEdge, IndexEntity, ProjectCatalogSnapshot, SourceEntry},
    maintenance_v2::StaticAnalysisImportReport,
    managed_registry::{ManagedRegistrySnapshot, RegistryConsistencyRecord},
    management::{
        Baseline, CanonicalSource, ChangePlan, CheckoutAttachmentState, CheckoutHeadState,
        CheckoutKind, CoordinatedOperation, Disposition, DispositionStatus, Finding,
        FindingLifecycle, IntegrityState, MANAGEMENT_COMPACTION_PLAN_SCHEMA_ID,
        MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION, MANAGEMENT_STATUS_DEFAULT_MAX_ITEMS,
        MANAGEMENT_STATUS_PAGE_SCHEMA_ID, MANAGEMENT_STATUS_PAGE_SCHEMA_VERSION,
        MANAGEMENT_STORE_VERSION, ManagementCompactionApplyResultV1,
        ManagementCompactionBackupBindingV1, ManagementCompactionPlanV1, ManagementStatusPageV1,
        ManagementStatusQueryV1, ManagementStoreStatus, MigrationApplyState, Occurrence,
        ParticipantReceipt, PatchSet, Project, ProjectCheckout, ProjectRevision, ProjectStorePoint,
        ProjectV1, ProjectV1ToV2MigrationEntry, ProjectV1ToV2MigrationPlan,
        ProjectV1ToV2MigrationResult, REDACTION_CONTRACT_VERSION, RETENTION_CHECKPOINT_SCHEMA_ID,
        RETENTION_CHECKPOINT_SCHEMA_VERSION, RETENTION_PLAN_V2_SCHEMA_ID,
        RETENTION_PLAN_V2_SCHEMA_VERSION, RegistrationState, RepositoryKind,
        RetentionBatchResultV1, RetentionCandidateV2, RetentionCheckpointV1,
        RetentionMigrationBackupCandidateV1, RetentionPlanV2, RetentionPolicyV2,
        RetentionProtectedMigrationBackupV1, RetentionProtectedTargetV1, RetentionTargetKindV2,
        ScanRun, ScanStatus, StoreOpenMode, StorePoint, StoreScope, StoreVersionVector,
        Suppression, SuppressionScope, SuppressionStatus, Symbol, SymbolReference,
        ValidationResult, WorkspaceSnapshot,
    },
    planning::PlanningBundle,
    recovery::{
        ACTIVE_SET_MANIFEST_SCHEMA_ID, ActiveSetManifest, ActiveStoreGeneration,
        BACKUP_APPLY_RESULT_SCHEMA_ID, BackupApplyResult, BackupPlan, BackupSetManifest,
        BackupStoreEntry, BackupStoreTarget, ControllerRecoveryMode,
        LOCAL_STATE_EXPORT_RESULT_SCHEMA_ID, LOCAL_STATE_IMPORT_RESULT_SCHEMA_ID, LocalStateBundle,
        LocalStateConflict, LocalStateExportPlan, LocalStateExportResult, LocalStateImportPlan,
        LocalStateImportResult, REBUILD_APPLY_RESULT_SCHEMA_ID, RECOVERY_STATUS_SCHEMA_ID,
        RESTORE_APPLY_RESULT_SCHEMA_ID, RESTORE_PLAN_SCHEMA_ID, RebuildApplyResult, RebuildPlan,
        RebuildProjectInput, RebuiltProjectSummary, RecoveryLossItem, RecoveryOperation,
        RecoveryStatus, RestoreApplyResult, RestorePlan, RestoreStoreTarget, StoreRecoveryStatus,
    },
};
use star_domain::{
    PersistenceRedactor,
    recovery::{
        require_exact_approval, restore_plan_fingerprint, seal_active_set, seal_backup_plan,
        seal_backup_set, seal_local_state_bundle, seal_local_state_export_plan,
        seal_local_state_import_plan, seal_rebuild_plan, validate_active_set, validate_backup_plan,
        validate_backup_set, validate_local_state_bundle, validate_local_state_export_plan,
        validate_local_state_import_plan, validate_rebuild_plan, validate_restore_plan,
    },
    validate_baseline, validate_coordination, validate_suppression, versioned_fingerprint,
};
use star_ports::{
    CheckGraphEvidenceTransaction, CodeIndexCache, DevelopmentRecord, GlobalManagementRepository,
    ManagementRecovery, ManagementRepositorySet, ProjectManagementRepository,
    ProjectRootAttachment, ProjectRootBindingStore, RepositoryError, RepositoryErrorCategory,
    RetentionApplyResult, RetentionCandidate, RetentionPlan, RetentionPolicy, ScanCommit,
    StoredCodeIndexProjection,
};
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            Cryptography::{
                CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
            },
            DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
            SetFileSecurityW,
        },
        Storage::FileSystem::{GetDiskFreeSpaceExW, REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    },
    core::{HSTRING, PCWSTR, w},
};

const STORE_FILENAME: &str = "management.v1.db";
const ACTIVE_SET_FILENAME: &str = "active-set.json";
const RETENTION_EXECUTION_PLAN_FILENAME: &str = "retention-plan-v2.json";
const RETENTION_CHECKPOINT_FILENAME: &str = "retention-checkpoint-v1.json";
const MIGRATION_BACKUPS_DIRECTORY: &str = "migration-backups";
const MIGRATION_BACKUP_MANIFEST_FILENAME: &str = "migration-backup.json";
const MIGRATION_BACKUP_PREFIX: &str = "project-v1-to-v2-";
const RETENTION_DELETE_TOMBSTONE_PREFIX: &str = ".retention-deleting-";
const RETENTION_BACKUP_DIRECTORY_LIMIT: usize = 4_096;
const RETENTION_BACKUP_FILE_SCAN_LIMIT: usize = 10_001;
const FIRST_GENERATION_LOCATOR: &str = "generations/00000000000000000001";
const APPLICATION_ID: i32 = 0x5354_4152;
const MANAGEMENT_DOCUMENT_MAX_BYTES: i32 = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct FileCodeIndexCache {
    root: PathBuf,
    max_entries_per_project: usize,
    max_entry_bytes: u64,
    max_project_bytes: u64,
    max_age_days: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeIndexCacheEnvelope {
    schema_version: u32,
    project_id: ProjectId,
    cache_key: Sha256Hash,
    projection: StoredCodeIndexProjection,
    stored_at: DateTime<Utc>,
    content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedCodeIndexCacheFile {
    schema_version: u32,
    protection: String,
    protected_payload_base64: String,
}

impl FileCodeIndexCache {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RepositoryError> {
        Self::open_with_limits(root, 8, 256 * 1024 * 1024, 512 * 1024 * 1024)
    }

    pub fn open_with_limits(
        root: impl Into<PathBuf>,
        max_entries_per_project: usize,
        max_entry_bytes: u64,
        max_project_bytes: u64,
    ) -> Result<Self, RepositoryError> {
        Self::open_with_policy(
            root,
            max_entries_per_project,
            max_entry_bytes,
            max_project_bytes,
            30,
        )
    }

    pub fn open_with_policy(
        root: impl Into<PathBuf>,
        max_entries_per_project: usize,
        max_entry_bytes: u64,
        max_project_bytes: u64,
        max_age_days: u64,
    ) -> Result<Self, RepositoryError> {
        if max_entries_per_project == 0
            || max_entry_bytes == 0
            || max_project_bytes < max_entry_bytes
            || max_age_days == 0
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "code index cache limits are invalid",
            ));
        }
        let root = root.into();
        create_private_dir(&root)?;
        Ok(Self {
            root,
            max_entries_per_project,
            max_entry_bytes,
            max_project_bytes,
            max_age_days,
        })
    }

    fn project_root(&self, project_id: &ProjectId) -> PathBuf {
        self.root.join(project_id.as_str())
    }

    fn entry_path(&self, project_id: &ProjectId, cache_key: &Sha256Hash) -> PathBuf {
        self.project_root(project_id).join(format!(
            "{}.json",
            cache_key
                .as_str()
                .strip_prefix("sha256:")
                .expect("Sha256Hash always has its prefix")
        ))
    }

    fn envelope_fingerprint(
        project_id: &ProjectId,
        cache_key: &Sha256Hash,
        projection: &StoredCodeIndexProjection,
    ) -> Result<Sha256Hash, RepositoryError> {
        versioned_fingerprint(
            "star.code-index-cache-entry",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "cache_key":cache_key,
                "projection":projection,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "code index cache fingerprint failed",
            )
        })
    }

    fn protection_entropy(project_id: &ProjectId, cache_key: &Sha256Hash) -> Vec<u8> {
        format!(
            "Star-Control/code-index-cache/v1/{}/{}",
            project_id.as_str(),
            cache_key.as_str()
        )
        .into_bytes()
    }

    fn evict(&self, project_id: &ProjectId) -> Result<(), RepositoryError> {
        let project_root = self.project_root(project_id);
        let mut entries = Vec::new();
        let oldest_allowed = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(
                self.max_age_days.saturating_mul(86_400),
            ))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        for entry in fs::read_dir(&project_root).map_err(map_io)? {
            let entry = entry.map_err(map_io)?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let metadata = entry.metadata().map_err(map_io)?;
            if metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                < oldest_allowed
            {
                fs::remove_file(&path).map_err(map_io)?;
                continue;
            }
            entries.push((
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                path,
                metadata.len(),
            ));
        }
        entries.sort_by(|left, right| {
            (left.0, left.1.as_os_str()).cmp(&(right.0, right.1.as_os_str()))
        });
        let mut total: u64 = entries.iter().map(|entry| entry.2).sum();
        while entries.len() > self.max_entries_per_project || total > self.max_project_bytes {
            let (_, path, bytes) = entries.remove(0);
            fs::remove_file(path).map_err(map_io)?;
            total = total.saturating_sub(bytes);
        }
        Ok(())
    }
}

impl CodeIndexCache for FileCodeIndexCache {
    fn load(
        &self,
        project_id: &ProjectId,
        cache_key: &Sha256Hash,
    ) -> Result<Option<StoredCodeIndexProjection>, RepositoryError> {
        let path = self.entry_path(project_id, cache_key);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(map_io(error)),
        };
        if metadata.len() > self.max_entry_bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "code index cache entry exceeds its read limit",
            ));
        }
        let bytes = fs::read(&path).map_err(map_io)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "code index cache entry encoding is invalid",
            )
        })?;
        let value = star_contracts::parse_no_duplicate_keys(text).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "code index cache entry JSON is invalid",
            )
        })?;
        let protected_file: ProtectedCodeIndexCacheFile =
            serde_json::from_value(value).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "code index cache entry shape is invalid",
                )
            })?;
        if protected_file.schema_version != 1 || protected_file.protection != "dpapi_current_user" {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "code index cache protection identity is invalid",
            ));
        }
        let ciphertext = BASE64
            .decode(protected_file.protected_payload_base64.as_bytes())
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "code index cache protected payload encoding is invalid",
                )
            })?;
        let mut plaintext = unprotect_current_user(
            &ciphertext,
            &Self::protection_entropy(project_id, cache_key),
        )?;
        let envelope = (|| {
            let text = std::str::from_utf8(&plaintext).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "code index cache protected payload is invalid",
                )
            })?;
            let value = star_contracts::parse_no_duplicate_keys(text).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "code index cache protected payload JSON is invalid",
                )
            })?;
            serde_json::from_value::<CodeIndexCacheEnvelope>(value).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "code index cache protected payload shape is invalid",
                )
            })
        })();
        plaintext.fill(0);
        let envelope = envelope?;
        let expected = Self::envelope_fingerprint(project_id, cache_key, &envelope.projection)?;
        if envelope.schema_version != 1
            || envelope.project_id != *project_id
            || envelope.cache_key != *cache_key
            || envelope.projection.snapshot.project_id != *project_id
            || envelope.content_fingerprint != expected
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "code index cache entry identity is invalid",
            ));
        }
        if envelope.stored_at
            < Utc::now() - chrono::Duration::days(self.max_age_days.min(i64::MAX as u64) as i64)
        {
            fs::remove_file(&path).map_err(map_io)?;
            return Ok(None);
        }
        Ok(Some(envelope.projection))
    }

    fn store(
        &self,
        project_id: &ProjectId,
        cache_key: &Sha256Hash,
        projection: &StoredCodeIndexProjection,
    ) -> Result<(), RepositoryError> {
        if projection.snapshot.project_id != *project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "code index cache entry crosses a ProjectId partition",
            ));
        }
        let project_root = self.project_root(project_id);
        create_private_dir(&project_root)?;
        let envelope = CodeIndexCacheEnvelope {
            schema_version: 1,
            project_id: project_id.clone(),
            cache_key: cache_key.clone(),
            projection: projection.clone(),
            stored_at: Utc::now(),
            content_fingerprint: Self::envelope_fingerprint(project_id, cache_key, projection)?,
        };
        let mut plaintext = serde_json::to_vec(&envelope).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "code index cache serialization failed",
            )
        })?;
        if plaintext.len() as u64 > self.max_entry_bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "code index cache entry exceeds its write limit",
            ));
        }
        let ciphertext =
            protect_current_user(&plaintext, &Self::protection_entropy(project_id, cache_key));
        plaintext.fill(0);
        let ciphertext = ciphertext?;
        let protected_file = ProtectedCodeIndexCacheFile {
            schema_version: 1,
            protection: "dpapi_current_user".to_owned(),
            protected_payload_base64: BASE64.encode(ciphertext),
        };
        let bytes = serde_json::to_vec(&protected_file).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "code index cache protected file serialization failed",
            )
        })?;
        if bytes.len() as u64 > self.max_entry_bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "code index cache protected entry exceeds its write limit",
            ));
        }
        let destination = self.entry_path(project_id, cache_key);
        if destination.exists() {
            match self.load(project_id, cache_key) {
                Ok(Some(existing)) => {
                    if existing.snapshot.content_fingerprint
                        != projection.snapshot.content_fingerprint
                        || existing.snapshot.code_index_snapshot_id
                            != projection.snapshot.code_index_snapshot_id
                    {
                        return Err(repository_error(
                            RepositoryErrorCategory::IntegrityFailed,
                            "code index cache key resolved to conflicting content",
                        ));
                    }
                    return Ok(());
                }
                Ok(None) => {}
                Err(error)
                    if matches!(
                        error.category,
                        RepositoryErrorCategory::Corrupt
                            | RepositoryErrorCategory::IntegrityFailed
                            | RepositoryErrorCategory::QuotaExceeded
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        write_private_atomic(&destination, &bytes)?;
        self.evict(project_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MigrationBackupKind {
    Global,
    Project { project_id: ProjectId },
    RootBinding { root_binding_id: RootBindingId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationBackupFile {
    kind: MigrationBackupKind,
    relative_path: String,
    content_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationBackupManifest {
    schema_id: String,
    schema_version: u32,
    plan_fingerprint: Sha256Hash,
    files: Vec<MigrationBackupFile>,
    backup_fingerprint: Sha256Hash,
}

pub fn inspect_store_read_only(path: &Path) -> RecoveryInspection {
    if !path.is_file() {
        return RecoveryInspection::Missing;
    }
    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return RecoveryInspection::Corrupt;
    };
    if connection.execute_batch("PRAGMA query_only=ON;").is_err() {
        return RecoveryInspection::Corrupt;
    }
    let application_id: rusqlite::Result<i32> =
        connection.pragma_query_value(None, "application_id", |row| row.get(0));
    let version: rusqlite::Result<u32> =
        connection.pragma_query_value(None, "user_version", |row| row.get(0));
    if application_id.ok() != Some(APPLICATION_ID) {
        return RecoveryInspection::Corrupt;
    }
    let Ok(version) = version else {
        return RecoveryInspection::Corrupt;
    };
    if version > MANAGEMENT_STORE_VERSION {
        return RecoveryInspection::FutureVersion;
    }
    if version < MANAGEMENT_STORE_VERSION {
        return RecoveryInspection::MigrationRequired;
    }
    if verify_connection(&connection).is_err() || status_from_connection(&connection).is_err() {
        RecoveryInspection::Corrupt
    } else {
        RecoveryInspection::Healthy
    }
}

pub fn inspect_management_root(root: &Path) -> Option<RecoveryInspection> {
    let manifest_path = root.join(ACTIVE_SET_FILENAME);
    if !manifest_path.is_file() {
        let legacy = root.join("global").join("active").join(STORE_FILENAME);
        return legacy.exists().then(|| inspect_store_read_only(&legacy));
    }
    let input = match fs::read_to_string(&manifest_path) {
        Ok(input) => input,
        Err(_) => return Some(RecoveryInspection::ActiveSetMismatch),
    };
    let parsed = match parse_active_set(&input) {
        Ok(parsed) => parsed,
        Err(_) => return Some(RecoveryInspection::ActiveSetMismatch),
    };
    for entry in &parsed.manifest.entries {
        let store = active_store_file(root, entry);
        let status = match read_store_status_read_only(&store) {
            Ok(status) => status,
            Err(_) => {
                let inspection = inspect_store_read_only(&store);
                return Some(if inspection == RecoveryInspection::Healthy {
                    RecoveryInspection::Corrupt
                } else {
                    inspection
                });
            }
        };
        if status.store_scope != entry.scope
            || status.store_id != entry.store_id
            || status.generation != entry.generation
            || status.management_store_version != entry.management_store_version
        {
            return Some(RecoveryInspection::ActiveSetMismatch);
        }
    }
    if validate_active_set_relationships(root, &parsed.manifest).is_err() {
        return Some(RecoveryInspection::ActiveSetMismatch);
    }
    Some(RecoveryInspection::Healthy)
}

/// Inspects the active management set without walking healthy stores during normal startup.
///
/// Clean stores are admitted from bounded header/status snapshots. An unclean store or any
/// snapshot/materialization failure falls back to the explicit deep recovery inspection so crash
/// recovery remains fail closed.
pub fn inspect_management_root_for_startup(root: &Path) -> Option<RecoveryInspection> {
    let manifest_path = root.join(ACTIVE_SET_FILENAME);
    if !manifest_path.is_file() {
        let legacy = root.join("global").join("active").join(STORE_FILENAME);
        if !legacy.exists() {
            return None;
        }
        return Some(match read_store_status_snapshot(&legacy) {
            Ok(status) if status.last_clean_shutdown => RecoveryInspection::Healthy,
            _ => inspect_store_read_only(&legacy),
        });
    }
    let input = match fs::read_to_string(&manifest_path) {
        Ok(input) => input,
        Err(_) => return Some(RecoveryInspection::ActiveSetMismatch),
    };
    let parsed = match parse_active_set(&input) {
        Ok(parsed) => parsed,
        Err(_) => return Some(RecoveryInspection::ActiveSetMismatch),
    };
    match validate_active_set_snapshot_materialization(root, &parsed.manifest) {
        Ok(true) => Some(RecoveryInspection::Healthy),
        Ok(false) | Err(_) => inspect_management_root(root),
    }
}

fn parse_active_set(input: &str) -> Result<ParsedActiveSet, RepositoryError> {
    if let Ok(manifest) = star_contracts::management::decode_current_management_document::<
        ActiveSetManifest,
    >(input, ACTIVE_SET_MANIFEST_SCHEMA_ID)
    {
        validate_active_set(&manifest).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "active set manifest fingerprint or store set is invalid",
            )
        })?;
        return Ok(ParsedActiveSet {
            manifest,
            needs_upgrade: false,
        });
    }

    let value = star_contracts::parse_no_duplicate_keys(input).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "active set manifest JSON is invalid",
        )
    })?;
    let legacy: LegacyActiveSetManifest = serde_json::from_value(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "active set manifest shape is invalid",
        )
    })?;
    if legacy.schema_version != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "active set manifest version is unsupported",
        ));
    }
    let manifest = ActiveSetManifest {
        schema_id: ACTIVE_SET_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        entries: legacy
            .entries
            .into_iter()
            .map(|entry| ActiveStoreGeneration {
                scope: entry.scope,
                store_id: entry.store_id,
                generation: entry.generation,
                management_store_version: entry.management_store_version,
                relative_locator: entry.relative_locator,
                header_fingerprint: entry.header_fingerprint,
            })
            .collect(),
        manifest_fingerprint: legacy.manifest_fingerprint,
    };
    validate_active_set(&manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "legacy active set manifest fingerprint or store set is invalid",
        )
    })?;
    Ok(ParsedActiveSet {
        manifest,
        needs_upgrade: true,
    })
}

fn read_active_set(root: &Path) -> Result<Option<ParsedActiveSet>, RepositoryError> {
    let path = root.join(ACTIVE_SET_FILENAME);
    match fs::read_to_string(path) {
        Ok(input) => parse_active_set(&input).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(map_io(error)),
    }
}

fn write_active_set_document(
    root: &Path,
    manifest: &ActiveSetManifest,
) -> Result<(), RepositoryError> {
    validate_active_set(manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "active set manifest is not sealed",
        )
    })?;
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "active set serialization failed",
        )
    })?;
    write_private_atomic(&root.join(ACTIVE_SET_FILENAME), &bytes)
}

fn active_store_file(root: &Path, entry: &ActiveStoreGeneration) -> PathBuf {
    root.join(&entry.relative_locator).join(STORE_FILENAME)
}

fn read_store_status_read_only(path: &Path) -> Result<ManagementStoreStatus, RepositoryError> {
    match inspect_store_read_only(path) {
        RecoveryInspection::Healthy => {}
        RecoveryInspection::FutureVersion | RecoveryInspection::MigrationRequired => {
            return Err(repository_error(
                RepositoryErrorCategory::IncompatibleVersion,
                "management store version is not supported by this reader",
            ));
        }
        RecoveryInspection::Missing => {
            return Err(repository_error(
                RepositoryErrorCategory::NotFound,
                "management store is missing",
            ));
        }
        RecoveryInspection::Corrupt | RecoveryInspection::ActiveSetMismatch => {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "management store failed read-only inspection",
            ));
        }
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    status_from_connection(&connection)
}

/// Reads only the StoreStatus metadata needed by a normal management status request.
///
/// Unlike recovery inspection and explicit verification, this deliberately does not run
/// `PRAGMA quick_check` or the event-chain walk. It opens the active store read-only and
/// leaves both store metadata and the active-set document untouched.
fn read_store_status_snapshot(path: &Path) -> Result<ManagementStoreStatus, RepositoryError> {
    if !path.is_file() {
        return Err(repository_error(
            RepositoryErrorCategory::NotFound,
            "management store is missing",
        ));
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    let application_id: i32 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(map_sql)?;
    if application_id != APPLICATION_ID {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "management store application ID is invalid",
        ));
    }
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_sql)?;
    if version != MANAGEMENT_STORE_VERSION {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "management store version is not supported by this reader",
        ));
    }
    status_from_connection(&connection)
}

fn validate_active_set_relationships(
    root: &Path,
    manifest: &ActiveSetManifest,
) -> Result<(), RepositoryError> {
    let global = manifest
        .entries
        .iter()
        .find(|entry| matches!(entry.scope, StoreScope::Global))
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "active set has no global store",
            )
        })?;
    let connection = Connection::open_with_flags(
        active_store_file(root, global),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    let mut statement = connection
        .prepare("SELECT project_id FROM projects ORDER BY project_id")
        .map_err(map_sql)?;
    let projects = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_sql)?;
    let mut expected = BTreeSet::new();
    for project in projects {
        expected.insert(ProjectId::parse(project.map_err(map_sql)?).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "global project relationship has an invalid project ID",
            )
        })?);
    }
    let actual: BTreeSet<_> = manifest
        .entries
        .iter()
        .filter_map(|entry| match &entry.scope {
            StoreScope::Project { project_id } => Some(project_id.clone()),
            StoreScope::Global => None,
        })
        .collect();
    if expected != actual {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "active set project relationships do not match the global store",
        ));
    }
    Ok(())
}

fn validate_active_set_materialization(
    root: &Path,
    manifest: &ActiveSetManifest,
) -> Result<(), RepositoryError> {
    validate_active_set(manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "active set manifest is invalid",
        )
    })?;
    for entry in &manifest.entries {
        let status = read_store_status_read_only(&active_store_file(root, entry))?;
        if status.store_scope != entry.scope
            || status.store_id != entry.store_id
            || status.generation != entry.generation
            || status.management_store_version != entry.management_store_version
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "active set store header does not match the manifest",
            ));
        }
    }
    validate_active_set_relationships(root, manifest)
}

fn validate_active_set_snapshot_materialization(
    root: &Path,
    manifest: &ActiveSetManifest,
) -> Result<bool, RepositoryError> {
    validate_active_set(manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "active set manifest is invalid",
        )
    })?;
    let mut all_clean = true;
    for entry in &manifest.entries {
        let status =
            read_store_status_snapshot(&active_store_file(root, entry)).map_err(|error| {
                if matches!(
                    error.category,
                    RepositoryErrorCategory::IncompatibleVersion
                        | RepositoryErrorCategory::NotFound
                ) {
                    error
                } else {
                    repository_error(
                        RepositoryErrorCategory::IntegrityFailed,
                        "management store failed bounded header inspection",
                    )
                }
            })?;
        if status.store_scope != entry.scope
            || status.store_id != entry.store_id
            || status.generation != entry.generation
            || status.management_store_version != entry.management_store_version
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "active set store header does not match the manifest",
            ));
        }
        all_clean &= status.last_clean_shutdown;
    }
    validate_active_set_relationships(root, manifest)?;
    Ok(all_clean)
}

pub fn restore_verified_backup_side_by_side(
    backup: &Path,
    destination: &Path,
) -> Result<RecoveryInspection, RepositoryError> {
    if inspect_store_read_only(backup) != RecoveryInspection::Healthy || destination.exists() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "backup is not healthy or restore destination already exists",
        ));
    }
    if let Some(parent) = destination.parent() {
        create_private_dir(parent)?;
    }
    fs::copy(backup, destination).map_err(map_io)?;
    apply_owner_system_dacl(destination)?;
    let inspection = inspect_store_read_only(destination);
    if inspection != RecoveryInspection::Healthy {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restored side-by-side generation failed integrity inspection",
        ));
    }
    Ok(inspection)
}

fn repository_error(category: RepositoryErrorCategory, message: &'static str) -> RepositoryError {
    RepositoryError::new(category, message)
}

fn gate_project_id(decision: &GateDecision) -> Option<&ProjectId> {
    match &decision.scope {
        GateScope::Merge { project_id, .. } | GateScope::Release { project_id, .. } => {
            Some(project_id)
        }
        GateScope::Goal { .. } | GateScope::Stage { .. } => None,
    }
}

fn gate_workspace_snapshot_id(
    decision: &GateDecision,
) -> Result<WorkspaceSnapshotId, RepositoryError> {
    let value = decision
        .extensions
        .get("star.management")
        .and_then(|extension| extension.get("workspace_snapshot_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management GateDecision is missing its workspace snapshot reference",
            )
        })?;
    WorkspaceSnapshotId::parse(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management GateDecision has an invalid workspace snapshot reference",
        )
    })
}

fn serialized_enum_label<T: Serialize>(value: &T) -> Result<String, RepositoryError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management enum serialization failed",
            )
        })
}

fn validate_decision_strings<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), RepositoryError> {
    let redactor = PersistenceRedactor::for_current_user();
    for value in values {
        redactor.validate(value).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "decision contains a prohibited raw value",
            )
        })?;
    }
    Ok(())
}

fn map_sql(error: rusqlite::Error) -> RepositoryError {
    let category = match &error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => RepositoryErrorCategory::Busy,
            ErrorCode::ConstraintViolation => RepositoryErrorCategory::Invalid,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => {
                RepositoryErrorCategory::Corrupt
            }
            ErrorCode::ReadOnly => RepositoryErrorCategory::ReadOnly,
            ErrorCode::TooBig => RepositoryErrorCategory::QuotaExceeded,
            _ => RepositoryErrorCategory::Unavailable,
        },
        _ => RepositoryErrorCategory::Unavailable,
    };
    repository_error(category, "embedded management store operation failed")
}

fn map_io(_: std::io::Error) -> RepositoryError {
    repository_error(
        RepositoryErrorCategory::Unavailable,
        "management state file operation failed",
    )
}

struct WriterLease {
    _file: fs::File,
}

impl WriterLease {
    fn acquire(path: &Path) -> Result<Self, RepositoryError> {
        use std::os::windows::fs::OpenOptionsExt;
        let parent = path.parent().ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "writer lease has no parent directory",
            )
        })?;
        create_private_dir(parent)?;
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0)
            .open(path)
            .map_err(|error| {
                if error.raw_os_error() == Some(32) {
                    repository_error(
                        RepositoryErrorCategory::Busy,
                        "another Controller owns the management writer lease",
                    )
                } else {
                    map_io(error)
                }
            })?;
        write!(file, "{}", std::process::id()).map_err(map_io)?;
        file.sync_all().map_err(map_io)?;
        apply_owner_system_dacl(path)?;
        Ok(Self { _file: file })
    }
}

pub struct SqliteManagementRecovery {
    root: PathBuf,
    product_version: String,
    _lease: WriterLease,
}

impl SqliteManagementRecovery {
    pub fn open(
        management_root: impl Into<PathBuf>,
        product_version: impl Into<String>,
    ) -> Result<Self, RepositoryError> {
        let root = management_root.into();
        create_private_dir(&root)?;
        let lease = WriterLease::acquire(&root.join("writer.lock"))?;
        let recovery = Self {
            root,
            product_version: product_version.into(),
            _lease: lease,
        };
        if let Ok(Some(parsed)) = read_active_set(&recovery.root)
            && parsed.needs_upgrade
        {
            write_active_set_document(&recovery.root, &parsed.manifest)?;
        }
        Ok(recovery)
    }

    pub fn status(&self) -> Result<RecoveryStatus, RepositoryError> {
        ManagementRecovery::status(self)
    }

    pub fn plan_restore(&self, backup_root: &Path) -> Result<RestorePlan, RepositoryError> {
        ManagementRecovery::plan_restore(self, backup_root)
    }

    pub fn apply_restore(
        &self,
        backup_root: &Path,
        plan: &RestorePlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RestoreApplyResult, RepositoryError> {
        ManagementRecovery::apply_restore(self, backup_root, plan, approved_plan_fingerprint)
    }

    pub fn plan_rebuild(
        &self,
        projects: Vec<RebuildProjectInput>,
        predicted_losses: Vec<RecoveryLossItem>,
    ) -> Result<RebuildPlan, RepositoryError> {
        ManagementRecovery::plan_rebuild(self, projects, predicted_losses)
    }

    pub fn begin_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<Arc<dyn ManagementRepositorySet>, RepositoryError> {
        ManagementRecovery::begin_rebuild(self, plan, approved_plan_fingerprint)
    }

    pub fn apply_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
        rebuilt_projects: Vec<RebuiltProjectSummary>,
    ) -> Result<RebuildApplyResult, RepositoryError> {
        ManagementRecovery::apply_rebuild(self, plan, approved_plan_fingerprint, rebuilt_projects)
    }

    pub fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, RepositoryError> {
        ManagementRecovery::plan_local_state_export(self, project_id, destination)
    }

    pub fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, RepositoryError> {
        ManagementRecovery::apply_local_state_export(
            self,
            destination,
            plan,
            approved_plan_fingerprint,
        )
    }
}

pub struct SqliteManagementRepositorySet {
    root: PathBuf,
    product_version: String,
    _lease: WriterLease,
    global: SqliteGlobalRepository,
    active_set: Mutex<ActiveSetManifest>,
    projects: Mutex<BTreeMap<ProjectId, Weak<SqliteProjectRepository>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagementStatusCursorV1 {
    schema_version: u32,
    global_store_id: ManagementStoreId,
    snapshot_revision: u64,
    global_returned: bool,
    after_project_id: Option<ProjectId>,
}

fn encode_management_status_cursor(
    cursor: &ManagementStatusCursorV1,
) -> Result<String, RepositoryError> {
    serde_json::to_vec(cursor)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management status cursor serialization failed",
            )
        })
}

fn decode_management_status_cursor(
    encoded: &str,
) -> Result<ManagementStatusCursorV1, RepositoryError> {
    if encoded.is_empty() || encoded.len() > 1_024 || !encoded.is_ascii() {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        ));
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        )
    })?;
    let input = std::str::from_utf8(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        )
    })?;
    let value = star_contracts::parse_no_duplicate_keys(input).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        )
    })?;
    let cursor: ManagementStatusCursorV1 = serde_json::from_value(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        )
    })?;
    let canonical = serde_json::to_vec(&cursor).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        )
    })?;
    if canonical != bytes || cursor.schema_version != 1 || !cursor.global_returned {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "management status cursor is invalid",
        ));
    }
    Ok(cursor)
}

fn validate_retention_plan_v2(plan: &RetentionPlanV2) -> Result<(), RepositoryError> {
    plan.validate().map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "retention plan shape is invalid",
        )
    })?;
    let policy_fingerprint =
        versioned_fingerprint("star.management-retention-policy", 2, &plan.policy).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "retention policy fingerprint failed",
                )
            },
        )?;
    let source_fingerprint = versioned_fingerprint(
        "star.management-retention-source",
        2,
        &serde_json::json!({
            "expected_store_revisions":plan.expected_store_revisions,
            "migration_backup_candidates":plan.migration_backup_candidates,
            "protected_migration_backups":plan.protected_migration_backups,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "retention source fingerprint failed",
        )
    })?;
    let plan_fingerprint = versioned_fingerprint(
        "star.management-retention-plan",
        RETENTION_PLAN_V2_SCHEMA_VERSION,
        &serde_json::json!({
            "policy_fingerprint":policy_fingerprint,
            "source_fingerprint":source_fingerprint,
            "expected_store_revisions":plan.expected_store_revisions,
            "candidates":plan.candidates,
            "protected_targets":plan.protected_targets,
            "migration_backup_candidates":plan.migration_backup_candidates,
            "protected_migration_backups":plan.protected_migration_backups,
            "estimated_rows":plan.estimated_rows,
            "estimated_bytes":plan.estimated_bytes,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "retention plan fingerprint failed",
        )
    })?;
    if policy_fingerprint != plan.policy_fingerprint
        || source_fingerprint != plan.source_fingerprint
        || plan_fingerprint != plan.plan_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "retention plan fingerprint is invalid",
        ));
    }
    Ok(())
}

fn validate_retention_checkpoint(
    plan: &RetentionPlanV2,
    checkpoint: &RetentionCheckpointV1,
) -> Result<(), RepositoryError> {
    let expected_last_candidate_id = checkpoint
        .next_candidate_index
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| {
            plan.candidates
                .iter()
                .map(|candidate| candidate.candidate_id.as_str())
                .chain(
                    plan.migration_backup_candidates
                        .iter()
                        .map(|candidate| candidate.candidate_id.as_str()),
                )
                .nth(index)
        });
    let active_candidate = usize::try_from(checkpoint.next_candidate_index)
        .ok()
        .and_then(|index| plan.candidates.get(index));
    let active_candidate_valid = match (checkpoint.active_candidate_id.as_deref(), active_candidate)
    {
        (None, _) => {
            checkpoint.active_candidate_applied_rows == 0
                && checkpoint.active_candidate_applied_bytes == 0
        }
        (Some(active_id), Some(candidate)) => {
            active_id == candidate.candidate_id
                && checkpoint.active_candidate_applied_rows > 0
                && checkpoint.active_candidate_applied_rows < candidate.estimated_rows
                && checkpoint.active_candidate_applied_bytes <= candidate.estimated_bytes
        }
        (Some(_), None) => false,
    };
    if checkpoint.schema_id != RETENTION_CHECKPOINT_SCHEMA_ID
        || checkpoint.schema_version != RETENTION_CHECKPOINT_SCHEMA_VERSION
        || checkpoint.plan_fingerprint != plan.plan_fingerprint
        || checkpoint.next_candidate_index > plan.total_candidate_count() as u64
        || checkpoint.committed_batch_count < checkpoint.next_candidate_index
        || checkpoint.committed_batch_count > checkpoint.applied_rows
        || checkpoint.applied_rows > plan.estimated_rows
        || checkpoint.applied_bytes > plan.estimated_bytes
        || checkpoint.last_candidate_id.as_deref() != expected_last_candidate_id
        || !active_candidate_valid
        || checkpoint.expected_store_revisions.len() != plan.expected_store_revisions.len()
        || checkpoint
            .expected_store_revisions
            .iter()
            .any(|(key, revision)| {
                plan.expected_store_revisions
                    .get(key)
                    .is_none_or(|planned_revision| revision < planned_revision)
            })
        || (checkpoint.complete
            && (checkpoint.next_candidate_index != plan.total_candidate_count() as u64
                || checkpoint.active_candidate_id.is_some()
                || checkpoint.applied_rows != plan.estimated_rows
                || checkpoint.applied_bytes != plan.estimated_bytes))
        || (checkpoint.complete && checkpoint.cancelled)
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "retention checkpoint is invalid",
        ));
    }
    Ok(())
}

fn read_strict_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, RepositoryError> {
    let input = fs::read_to_string(path).map_err(map_io)?;
    let value = star_contracts::parse_no_duplicate_keys(&input).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "management execution document is invalid",
        )
    })?;
    serde_json::from_value(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "management execution document shape is invalid",
        )
    })
}

fn available_disk_bytes(path: &Path) -> Result<u64, RepositoryError> {
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management store path has no parent",
            )
        })?
    };
    let directory = HSTRING::from(directory.as_os_str());
    let mut available = 0_u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(directory.as_ptr()), Some(&mut available), None, None) }
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "available disk space could not be determined",
            )
        })?;
    Ok(available)
}

fn compaction_space_snapshot(path: &Path) -> Result<(u64, u64, u64), RepositoryError> {
    let database_bytes = fs::metadata(path).map_err(map_io)?.len();
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    let page_size: i64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(map_sql)?;
    let freelist_count: i64 = connection
        .query_row("PRAGMA freelist_count", [], |row| row.get(0))
        .map_err(map_sql)?;
    let page_size = u64::try_from(page_size).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "management store page size is invalid",
        )
    })?;
    let freelist_count = u64::try_from(freelist_count).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "management store freelist count is invalid",
        )
    })?;
    let reclaimable = page_size.checked_mul(freelist_count).ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "management compaction estimate overflowed",
        )
    })?;
    let required_free_bytes = database_bytes.checked_mul(2).ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "management compaction space requirement overflowed",
        )
    })?;
    Ok((database_bytes, reclaimable, required_free_bytes))
}

fn validate_compaction_plan(plan: &ManagementCompactionPlanV1) -> Result<(), RepositoryError> {
    let expected = versioned_fingerprint(
        "star.management-compaction-plan",
        MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION,
        &serde_json::json!({
            "store_id":plan.store_id,
            "store_scope":plan.store_scope,
            "store_revision":plan.store_revision,
            "source_fingerprint":plan.source_fingerprint,
            "store_fingerprint":plan.store_fingerprint,
            "database_bytes":plan.database_bytes,
            "estimated_reclaimable_bytes":plan.estimated_reclaimable_bytes,
            "required_free_bytes":plan.required_free_bytes,
            "available_free_bytes_at_plan":plan.available_free_bytes_at_plan,
            "backup_binding":plan.backup_binding,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management compaction plan fingerprint failed",
        )
    })?;
    if plan.schema_id != MANAGEMENT_COMPACTION_PLAN_SCHEMA_ID
        || plan.schema_version != MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION
        || plan.required_free_bytes < plan.database_bytes
        || plan.backup_binding.as_ref().is_some_and(|binding| {
            binding.store_id != plan.store_id || binding.store_revision != plan.store_revision
        })
        || plan.plan_fingerprint != expected
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "management compaction plan is invalid",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyActiveSetEntry {
    scope: StoreScope,
    store_id: ManagementStoreId,
    generation: u64,
    management_store_version: u32,
    relative_locator: String,
    header_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyActiveSetManifest {
    schema_version: u32,
    entries: Vec<LegacyActiveSetEntry>,
    manifest_fingerprint: Sha256Hash,
}

struct ParsedActiveSet {
    manifest: ActiveSetManifest,
    needs_upgrade: bool,
}

impl SqliteManagementRepositorySet {
    pub fn open(
        management_root: impl Into<PathBuf>,
        product_version: impl Into<String>,
    ) -> Result<Self, RepositoryError> {
        let root = management_root.into();
        create_private_dir(&root)?;
        let lease = WriterLease::acquire(&root.join("writer.lock"))?;
        let product_version = product_version.into();
        let parsed_active_set = read_active_set(&root)?;
        if let Some(parsed) = &parsed_active_set {
            validate_active_set_snapshot_materialization(&root, &parsed.manifest)?;
        }
        let global_locator = parsed_active_set
            .as_ref()
            .and_then(|parsed| {
                parsed.manifest.entries.iter().find_map(|entry| {
                    matches!(entry.scope, StoreScope::Global)
                        .then(|| entry.relative_locator.clone())
                })
            })
            .unwrap_or_else(|| initial_global_locator(&root));
        let global_path = root.join(&global_locator).join(STORE_FILENAME);
        let global = SqliteGlobalRepository::open(&global_path, &product_version)?;
        let active_set = if let Some(parsed) = &parsed_active_set {
            parsed.manifest.clone()
        } else {
            seal_active_set(vec![active_entry_for_status(
                &global.status()?,
                global_locator,
            )])
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "initial active set could not be sealed",
                )
            })?
        };
        let repositories = Self {
            root,
            product_version,
            _lease: lease,
            global,
            active_set: Mutex::new(active_set),
            projects: Mutex::new(BTreeMap::new()),
        };
        if parsed_active_set
            .as_ref()
            .is_some_and(|parsed| parsed.needs_upgrade)
        {
            let active_set = repositories.active_set.lock().map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "active set cache is unavailable",
                )
            })?;
            write_active_set_document(&repositories.root, &active_set)?;
        }
        repositories.refresh_active_set()?;
        Ok(repositories)
    }

    fn project_path(&self, project_id: &ProjectId) -> Result<PathBuf, RepositoryError> {
        let active_set = self.active_set.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "active set cache is unavailable",
            )
        })?;
        if let Some(entry) = active_set.entries.iter().find(|entry| {
            matches!(
                &entry.scope,
                StoreScope::Project { project_id: active_project_id }
                    if active_project_id == project_id
            )
        }) {
            return Ok(active_store_file(&self.root, entry));
        }
        drop(active_set);
        let locator = initial_project_locator(&self.root, project_id);
        Ok(self.root.join(locator).join(STORE_FILENAME))
    }

    fn store_path_and_status(
        &self,
        store_id: &ManagementStoreId,
    ) -> Result<(PathBuf, ManagementStoreStatus), RepositoryError> {
        let entry = self
            .active_set
            .lock()
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "active set cache is unavailable",
                )
            })?
            .entries
            .iter()
            .find(|entry| &entry.store_id == store_id)
            .cloned()
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::NotFound,
                    "management store is not active",
                )
            })?;
        let path = active_store_file(&self.root, &entry);
        let status = read_store_status_snapshot(&path)?;
        Ok((path, status))
    }

    fn project_repository(
        &self,
        project_id: &ProjectId,
    ) -> Result<Arc<SqliteProjectRepository>, RepositoryError> {
        let mut projects = self.projects.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project repository cache is unavailable",
            )
        })?;
        if let Some(repository) = projects.get(project_id).and_then(Weak::upgrade) {
            return Ok(repository);
        }
        let repository = Arc::new(SqliteProjectRepository::open(
            &self.project_path(project_id)?,
            project_id,
            &self.product_version,
        )?);
        projects.insert(project_id.clone(), Arc::downgrade(&repository));
        drop(projects);
        if self.global.get_project(project_id)?.is_some() {
            self.refresh_active_set()?;
        }
        Ok(repository)
    }

    fn refresh_active_set(&self) -> Result<(), RepositoryError> {
        let mut statuses = vec![self.global.status()?];
        for project in self.global.list_projects()? {
            statuses.push(self.project_repository(&project.project_id)?.status()?);
        }
        self.write_active_set(&statuses)
    }

    fn write_active_set(&self, statuses: &[ManagementStoreStatus]) -> Result<(), RepositoryError> {
        let mut entries = Vec::new();
        let current = self.active_set.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "active set cache is unavailable",
            )
        })?;
        for status in statuses {
            let relative_locator = current
                .entries
                .iter()
                .find(|entry| entry.scope == status.store_scope)
                .map(|entry| entry.relative_locator.clone())
                .unwrap_or_else(|| match &status.store_scope {
                    StoreScope::Global => initial_global_locator(&self.root),
                    StoreScope::Project { project_id } => {
                        initial_project_locator(&self.root, project_id)
                    }
                });
            entries.push(active_entry_for_status(status, relative_locator));
        }
        drop(current);
        let manifest = seal_active_set(entries).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "active set could not be sealed",
            )
        })?;
        write_active_set_document(&self.root, &manifest)?;
        let mut current = self.active_set.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "active set cache is unavailable",
            )
        })?;
        *current = manifest;
        Ok(())
    }
}

fn initial_global_locator(root: &Path) -> String {
    if root
        .join("global")
        .join("active")
        .join(STORE_FILENAME)
        .is_file()
    {
        "global/active".to_owned()
    } else {
        format!("global/{FIRST_GENERATION_LOCATOR}")
    }
}

fn initial_project_locator(root: &Path, project_id: &ProjectId) -> String {
    let legacy = root
        .join("projects")
        .join(project_id.as_str())
        .join("active")
        .join(STORE_FILENAME);
    if legacy.is_file() {
        format!("projects/{}/active", project_id.as_str())
    } else {
        format!(
            "projects/{}/{}",
            project_id.as_str(),
            FIRST_GENERATION_LOCATOR
        )
    }
}

fn active_entry_for_status(
    status: &ManagementStoreStatus,
    relative_locator: String,
) -> ActiveStoreGeneration {
    ActiveStoreGeneration {
        scope: status.store_scope.clone(),
        store_id: status.store_id.clone(),
        generation: status.generation,
        management_store_version: status.management_store_version,
        relative_locator,
        header_fingerprint: Sha256Hash::digest(b"unsealed-active-store-header"),
    }
}

fn refresh_migration_active_set(management_root: &Path) -> Result<(), RepositoryError> {
    let Some(parsed) = read_active_set(management_root)? else {
        return Ok(());
    };
    let mut entries = Vec::with_capacity(parsed.manifest.entries.len());
    for mut current in parsed.manifest.entries {
        let connection = Connection::open_with_flags(
            active_store_file(management_root, &current),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        let version = checked_store_version(&connection)?;
        let store_id =
            ManagementStoreId::parse(get_meta(&connection, "store_id")?).map_err(|_| {
                repository_error(RepositoryErrorCategory::Corrupt, "store ID is invalid")
            })?;
        let scope: StoreScope = serde_json::from_str(&get_meta(&connection, "store_scope")?)
            .map_err(|_| {
                repository_error(RepositoryErrorCategory::Corrupt, "store scope is invalid")
            })?;
        let generation: u64 = get_meta(&connection, "generation")?.parse().map_err(|_| {
            repository_error(RepositoryErrorCategory::Corrupt, "generation is invalid")
        })?;
        if current.store_id != store_id
            || current.scope != scope
            || current.generation != generation
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration store identity changed outside the active set",
            ));
        }
        current.management_store_version = version;
        entries.push(current);
    }
    let refreshed = seal_active_set(entries).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration active set could not be sealed",
        )
    })?;
    write_active_set_document(management_root, &refreshed)
}

fn backup_destination_fingerprint(
    management_root: &Path,
    destination: &Path,
) -> Result<Sha256Hash, RepositoryError> {
    if !destination.is_absolute()
        || destination.as_os_str().is_empty()
        || destination.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "backup destination must be an absolute normalized path",
        ));
    }
    let canonical_management_root = management_root.canonicalize().map_err(map_io)?;
    let parent = destination.parent().ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "backup destination has no parent directory",
        )
    })?;
    let canonical_parent = parent.canonicalize().map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "backup destination parent must already exist",
        )
    })?;
    if canonical_parent.starts_with(&canonical_management_root) {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "backup destination must be outside the management root",
        ));
    }
    let normalized = destination
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    versioned_fingerprint("star.management-backup-destination", 1, &normalized).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "backup destination fingerprint failed",
        )
    })
}

fn local_state_path_fingerprint(
    path: &Path,
    contract: &str,
) -> Result<Sha256Hash, RepositoryError> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "local state document path must be absolute and normalized",
        ));
    }
    let normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();
    versioned_fingerprint(contract, 1, &normalized).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state document path fingerprint failed",
        )
    })
}

fn recovery_receipt_path(root: &Path, operation: &str, plan_fingerprint: &Sha256Hash) -> PathBuf {
    let token = plan_fingerprint.as_str().trim_start_matches("sha256:");
    root.join("recovery-receipts")
        .join(format!("{operation}-{}.json", &token[..32]))
}

fn read_recovery_receipt<T: DeserializeOwned>(
    root: &Path,
    operation: &str,
    plan_fingerprint: &Sha256Hash,
) -> Result<Option<T>, RepositoryError> {
    let path = recovery_receipt_path(root, operation, plan_fingerprint);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(map_io(error)),
    };
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery receipt exceeds its read limit",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery receipt is not UTF-8",
        )
    })?;
    let value = star_contracts::parse_no_duplicate_keys(text).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery receipt JSON is invalid",
        )
    })?;
    serde_json::from_value(value).map(Some).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery receipt shape is invalid",
        )
    })
}

fn write_recovery_receipt<T>(
    root: &Path,
    operation: &str,
    plan_fingerprint: &Sha256Hash,
    value: &T,
) -> Result<(), RepositoryError>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    if let Some(existing) = read_recovery_receipt::<T>(root, operation, plan_fingerprint)? {
        if existing == *value {
            return Ok(());
        }
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery receipt conflicts with the completed operation",
        ));
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "recovery receipt serialization failed",
        )
    })?;
    write_private_atomic(
        &recovery_receipt_path(root, operation, plan_fingerprint),
        &bytes,
    )
}

fn backup_apply_result(
    plan: &BackupPlan,
    manifest: BackupSetManifest,
    applied_at: DateTime<Utc>,
) -> Result<BackupApplyResult, RepositoryError> {
    let result_fingerprint = backup_apply_result_fingerprint(
        &plan.backup_set_id,
        applied_at,
        &plan.plan_fingerprint,
        &manifest,
    )?;
    Ok(BackupApplyResult {
        schema_id: BACKUP_APPLY_RESULT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        backup_set_id: plan.backup_set_id.clone(),
        applied_at,
        approved_plan_fingerprint: plan.plan_fingerprint.clone(),
        manifest,
        result_fingerprint,
    })
}

fn backup_apply_result_fingerprint(
    backup_set_id: &BackupSetId,
    applied_at: DateTime<Utc>,
    approved_plan_fingerprint: &Sha256Hash,
    manifest: &BackupSetManifest,
) -> Result<Sha256Hash, RepositoryError> {
    versioned_fingerprint(
        "star.management-backup-apply-result",
        1,
        &serde_json::json!({
            "backup_set_id":backup_set_id,
            "applied_at":applied_at,
            "approved_plan_fingerprint":approved_plan_fingerprint,
            "manifest":manifest,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "backup result fingerprint failed",
        )
    })
}

fn local_state_export_result(
    plan: &LocalStateExportPlan,
    bundle: LocalStateBundle,
    payload_sha256: Sha256Hash,
    applied_at: DateTime<Utc>,
) -> Result<LocalStateExportResult, RepositoryError> {
    let result_fingerprint = versioned_fingerprint(
        "star.management-local-state-export-result",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "applied_at":applied_at,
            "approved_plan_fingerprint":plan.plan_fingerprint,
            "bundle":bundle,
            "payload_sha256":payload_sha256,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state export result fingerprint failed",
        )
    })?;
    Ok(LocalStateExportResult {
        schema_id: LOCAL_STATE_EXPORT_RESULT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id: plan.recovery_plan_id.clone(),
        applied_at,
        approved_plan_fingerprint: plan.plan_fingerprint.clone(),
        bundle,
        payload_sha256,
        result_fingerprint,
    })
}

fn local_state_import_result(
    plan: &LocalStateImportPlan,
    bundle: &LocalStateBundle,
    applied_at: DateTime<Utc>,
) -> Result<LocalStateImportResult, RepositoryError> {
    let imported_suppressions = u64::try_from(
        bundle
            .local_suppressions
            .len()
            .checked_add(bundle.local_suppressions_v2.len())
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "local state suppression count exceeds its supported range",
                )
            })?,
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "local state suppression count exceeds its supported range",
        )
    })?;
    let imported_baselines = u64::try_from(
        bundle
            .local_baselines
            .len()
            .checked_add(bundle.local_baselines_v2.len())
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "local state baseline count exceeds its supported range",
                )
            })?,
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "local state baseline count exceeds its supported range",
        )
    })?;
    let imported_dispositions = u64::try_from(
        bundle
            .local_dispositions
            .len()
            .checked_add(bundle.local_dispositions_v2.len())
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "local state disposition count exceeds its supported range",
                )
            })?,
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "local state disposition count exceeds its supported range",
        )
    })?;
    let imported_change_plans = u64::try_from(bundle.active_change_plans.len()).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "local state change plan count exceeds its supported range",
        )
    })?;
    let result_fingerprint = versioned_fingerprint(
        "star.management-local-state-import-result",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "bundle_id":plan.bundle_id,
            "applied_at":applied_at,
            "approved_plan_fingerprint":plan.plan_fingerprint,
            "imported_suppressions":imported_suppressions,
            "imported_baselines":imported_baselines,
            "imported_dispositions":imported_dispositions,
            "imported_change_plans":imported_change_plans,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import result fingerprint failed",
        )
    })?;
    Ok(LocalStateImportResult {
        schema_id: LOCAL_STATE_IMPORT_RESULT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id: plan.recovery_plan_id.clone(),
        bundle_id: plan.bundle_id.clone(),
        applied_at,
        approved_plan_fingerprint: plan.plan_fingerprint.clone(),
        imported_suppressions,
        imported_baselines,
        imported_dispositions,
        imported_change_plans,
        result_fingerprint,
    })
}

fn restore_apply_result(
    plan: &RestorePlan,
    applied_at: DateTime<Utc>,
) -> Result<RestoreApplyResult, RepositoryError> {
    let result_fingerprint = versioned_fingerprint(
        "star.management-restore-apply-result",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "backup_set_id":plan.backup_set_id,
            "applied_at":applied_at,
            "approved_plan_fingerprint":plan.plan_fingerprint,
            "previous_active_set_fingerprint":plan.expected_active_set_fingerprint,
            "activated_set":plan.candidate_active_set,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "restore result fingerprint failed",
        )
    })?;
    Ok(RestoreApplyResult {
        schema_id: RESTORE_APPLY_RESULT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id: plan.recovery_plan_id.clone(),
        backup_set_id: plan.backup_set_id.clone(),
        applied_at,
        approved_plan_fingerprint: plan.plan_fingerprint.clone(),
        previous_active_set_fingerprint: plan.expected_active_set_fingerprint.clone(),
        activated_set: plan.candidate_active_set.clone(),
        result_fingerprint,
    })
}

fn rebuild_apply_result(
    plan: &RebuildPlan,
    rebuilt_projects: Vec<RebuiltProjectSummary>,
    activated_set: ActiveSetManifest,
    applied_at: DateTime<Utc>,
) -> Result<RebuildApplyResult, RepositoryError> {
    let result_fingerprint = versioned_fingerprint(
        "star.management-rebuild-apply-result",
        1,
        &serde_json::json!({
            "recovery_plan_id":plan.recovery_plan_id,
            "applied_at":applied_at,
            "approved_plan_fingerprint":plan.plan_fingerprint,
            "rebuilt_projects":rebuilt_projects,
            "loss_report":plan.predicted_losses,
            "activated_set":activated_set,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "source rebuild result fingerprint failed",
        )
    })?;
    Ok(RebuildApplyResult {
        schema_id: REBUILD_APPLY_RESULT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        recovery_plan_id: plan.recovery_plan_id.clone(),
        applied_at,
        approved_plan_fingerprint: plan.plan_fingerprint.clone(),
        rebuilt_projects,
        loss_report: plan.predicted_losses.clone(),
        activated_set,
        result_fingerprint,
    })
}

fn local_state_snapshot(
    repository: &dyn ProjectManagementRepository,
    project_id: &ProjectId,
    bundle_id: LocalStateBundleId,
) -> Result<(LocalStateBundle, ManagementStoreStatus), RepositoryError> {
    let before = repository.status()?;
    if before.store_scope
        != (StoreScope::Project {
            project_id: project_id.clone(),
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "local state repository project scope does not match",
        ));
    }
    let scan = repository.latest_scan()?.ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::NotFound,
            "local state project has no current scan",
        )
    })?;
    let local_suppressions = repository
        .list_suppressions()?
        .into_iter()
        .filter(|value| value.scope_kind == star_contracts::management::SuppressionScope::Local)
        .collect();
    let local_baselines = repository
        .list_baselines()?
        .into_iter()
        .filter(|value| value.scope_kind == star_contracts::management::BaselineScope::Local)
        .collect();
    let local_dispositions = repository.list_dispositions()?;
    let local_suppressions_v2 = repository.list_suppressions_v2()?;
    let local_baselines_v2 = repository.list_baselines_v2()?;
    let local_dispositions_v2 = repository.list_dispositions_v2()?;
    let active_change_plans = repository
        .list_change_plans()?
        .into_iter()
        .filter(|value| {
            matches!(
                value.status,
                star_contracts::management::ChangePlanStatus::Draft
                    | star_contracts::management::ChangePlanStatus::Ready
                    | star_contracts::management::ChangePlanStatus::Blocked
            )
        })
        .collect();
    let after = repository.status()?;
    if before.store_revision != after.store_revision {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state changed while the recovery snapshot was read",
        ));
    }
    let bundle = seal_local_state_bundle(
        bundle_id,
        project_id.clone(),
        scan.project_revision_id,
        scan.effective_config_fingerprint,
        local_suppressions,
        local_baselines,
        local_dispositions,
        active_change_plans,
        local_suppressions_v2,
        local_baselines_v2,
        local_dispositions_v2,
        &PersistenceRedactor::for_current_user(),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state snapshot is not redaction-safe",
        )
    })?;
    Ok((bundle, after))
}

fn plan_local_state_export_for_repository(
    repository: &dyn ProjectManagementRepository,
    project_id: &ProjectId,
    destination: &Path,
) -> Result<LocalStateExportPlan, RepositoryError> {
    if destination.exists() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state export destination already exists",
        ));
    }
    let destination_fingerprint = local_state_path_fingerprint(
        destination,
        "star.management-local-state-export-destination",
    )?;
    let bundle_id = LocalStateBundleId::new();
    let (bundle, status) = local_state_snapshot(repository, project_id, bundle_id.clone())?;
    seal_local_state_export_plan(
        RecoveryPlanId::new(),
        bundle_id,
        project_id.clone(),
        bundle.source_revision_id,
        bundle.effective_config_fingerprint,
        status.store_revision,
        destination_fingerprint,
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state export plan could not be sealed",
        )
    })
}

fn apply_local_state_export_for_repository(
    receipt_root: &Path,
    repository: &dyn ProjectManagementRepository,
    destination: &Path,
    plan: &LocalStateExportPlan,
    approved_plan_fingerprint: &str,
) -> Result<LocalStateExportResult, RepositoryError> {
    require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state export approval fingerprint is stale",
        )
    })?;
    validate_local_state_export_plan(plan).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state export plan is invalid",
        )
    })?;
    if local_state_path_fingerprint(
        destination,
        "star.management-local-state-export-destination",
    )? != plan.destination_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state export destination changed after planning",
        ));
    }
    if let Some(completed) = read_recovery_receipt::<LocalStateExportResult>(
        receipt_root,
        "local-state-export",
        &plan.plan_fingerprint,
    )? {
        let (bundle, payload_sha256) = read_local_state_bundle(destination)?;
        let expected = local_state_export_result(
            plan,
            completed.bundle.clone(),
            completed.payload_sha256.clone(),
            completed.applied_at,
        )?;
        if completed != expected
            || bundle != completed.bundle
            || payload_sha256 != completed.payload_sha256
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "completed local state export receipt or payload is invalid",
            ));
        }
        return Ok(completed);
    }
    if destination.exists() {
        let (bundle, payload_sha256) = read_local_state_bundle(destination)?;
        if bundle.bundle_id != plan.bundle_id
            || bundle.project_id != plan.project_id
            || bundle.source_revision_id != plan.source_revision_id
            || bundle.effective_config_fingerprint != plan.effective_config_fingerprint
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "local state export destination belongs to a different plan",
            ));
        }
        let result = local_state_export_result(plan, bundle, payload_sha256, Utc::now())?;
        write_recovery_receipt(
            receipt_root,
            "local-state-export",
            &plan.plan_fingerprint,
            &result,
        )?;
        return Ok(result);
    }
    let (bundle, status) =
        local_state_snapshot(repository, &plan.project_id, plan.bundle_id.clone())?;
    if status.store_revision != plan.expected_store_revision
        || bundle.source_revision_id != plan.source_revision_id
        || bundle.effective_config_fingerprint != plan.effective_config_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state changed after export planning",
        ));
    }
    let payload = serde_json::to_vec_pretty(&bundle).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state export serialization failed",
        )
    })?;
    write_private_new(destination, &payload, plan.recovery_plan_id.as_str())?;
    let payload_sha256 = Sha256Hash::digest(&payload);
    let result = local_state_export_result(plan, bundle, payload_sha256, Utc::now())?;
    write_recovery_receipt(
        receipt_root,
        "local-state-export",
        &plan.plan_fingerprint,
        &result,
    )?;
    Ok(result)
}

fn read_local_state_bundle(
    source: &Path,
) -> Result<(LocalStateBundle, Sha256Hash), RepositoryError> {
    let _ = local_state_path_fingerprint(source, "star.management-local-state-import-source")?;
    let metadata = fs::symlink_metadata(source).map_err(map_io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 16 * 1024 * 1024
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import source must be a regular file no larger than 16 MiB",
        ));
    }
    let bytes = fs::read(source).map_err(map_io)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import source is not UTF-8",
        )
    })?;
    let value = star_contracts::parse_no_duplicate_keys(text).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import source is invalid or has duplicate keys",
        )
    })?;
    let bundle: LocalStateBundle = serde_json::from_value(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import contract is invalid",
        )
    })?;
    validate_local_state_bundle(&bundle, &PersistenceRedactor::for_current_user()).map_err(
        |_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "local state import bundle failed integrity or redaction checks",
            )
        },
    )?;
    Ok((bundle, Sha256Hash::digest(&bytes)))
}

fn diagnostic_evidence_ref_is_current(
    repository: &dyn ProjectManagementRepository,
    reference: &DiagnosticEvidenceRefV2,
    project_id: &ProjectId,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> Result<bool, RepositoryError> {
    match reference {
        DiagnosticEvidenceRefV2::Finding {
            finding_id,
            finding_fingerprint,
        } => Ok(repository
            .get_finding(finding_id)?
            .is_some_and(|finding| finding.finding_fingerprint == *finding_fingerprint)),
        DiagnosticEvidenceRefV2::Diagnostic { diagnostic_ref } => {
            let Some(diagnostic) = repository.get_diagnostic_v2(&diagnostic_ref.diagnostic_id)?
            else {
                return Ok(false);
            };
            Ok(diagnostic.reference().as_ref() == Ok(diagnostic_ref)
                && current_diagnostic_matches(
                    &diagnostic,
                    &repository.list_validation_runs_v2()?,
                    project_id,
                    source_revision_id,
                    effective_config_fingerprint,
                ))
        }
        DiagnosticEvidenceRefV2::Artifact { .. } | DiagnosticEvidenceRefV2::Document { .. } => {
            Ok(true)
        }
    }
}

fn baseline_v2_subject_is_current(
    repository: &dyn ProjectManagementRepository,
    baseline: &BaselineV2,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> Result<bool, RepositoryError> {
    if baseline.entries.is_empty() {
        return Ok(true);
    }
    Ok(baseline_v2_subject_matches(
        baseline,
        &repository.list_validation_runs_v2()?,
        source_revision_id,
        effective_config_fingerprint,
    ))
}

fn current_validation_run_matches(
    run: &ValidationRunV2,
    project_id: &ProjectId,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    run.project_id == *project_id
        && run.completeness == Completeness::Complete
        && run.subject_binding.freshness == EvidenceFreshnessV2::Current
        && run.subject_binding.project_revision_id == *source_revision_id
        && run.subject_binding.effective_config_fingerprint == *effective_config_fingerprint
}

fn current_diagnostic_matches(
    diagnostic: &DiagnosticV2,
    runs: &[ValidationRunV2],
    project_id: &ProjectId,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    diagnostic.project_id == *project_id
        && diagnostic
            .subject_binding_fingerprint
            .as_ref()
            .is_some_and(|binding| {
                runs.iter().any(|run| {
                    run.validation_run_id == diagnostic.validation_run_id
                        && run.subject_binding.binding_fingerprint == *binding
                        && current_validation_run_matches(
                            run,
                            project_id,
                            source_revision_id,
                            effective_config_fingerprint,
                        )
                })
            })
}

fn baseline_v2_subject_matches(
    baseline: &BaselineV2,
    runs: &[ValidationRunV2],
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    baseline.entries.is_empty()
        || runs.iter().any(|run| {
            current_validation_run_matches(
                run,
                &baseline.project_id,
                source_revision_id,
                effective_config_fingerprint,
            ) && run.subject_binding.binding_fingerprint == baseline.subject_binding_fingerprint
                && rule_set_fingerprint_for_subject_binding(&run.subject_binding)
                    .is_ok_and(|fingerprint| fingerprint == baseline.rule_set_fingerprint)
        })
}

fn suppression_v2_subject_is_current(
    repository: &dyn ProjectManagementRepository,
    suppression: &SuppressionV2,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> Result<bool, RepositoryError> {
    Ok(suppression_v2_subject_matches(
        suppression,
        &repository.list_findings()?,
        &repository.list_diagnostics_v2()?,
        &repository.list_validation_runs_v2()?,
        source_revision_id,
        effective_config_fingerprint,
    ))
}

fn disposition_v2_subject_is_current(
    repository: &dyn ProjectManagementRepository,
    disposition: &DispositionV2,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> Result<bool, RepositoryError> {
    Ok(disposition_v2_subject_matches(
        disposition,
        &repository.list_findings()?,
        &repository.list_diagnostics_v2()?,
        &repository.list_validation_runs_v2()?,
        source_revision_id,
        effective_config_fingerprint,
    ))
}

fn diagnostic_evidence_ref_matches(
    reference: &DiagnosticEvidenceRefV2,
    findings: &[Finding],
    diagnostics: &[DiagnosticV2],
    runs: &[ValidationRunV2],
    project_id: &ProjectId,
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    match reference {
        DiagnosticEvidenceRefV2::Finding {
            finding_id,
            finding_fingerprint,
        } => findings.iter().any(|finding| {
            finding.finding_id == *finding_id && finding.finding_fingerprint == *finding_fingerprint
        }),
        DiagnosticEvidenceRefV2::Diagnostic { diagnostic_ref } => diagnostics
            .iter()
            .find(|diagnostic| diagnostic.diagnostic_id == diagnostic_ref.diagnostic_id)
            .is_some_and(|diagnostic| {
                diagnostic.reference().as_ref() == Ok(diagnostic_ref)
                    && current_diagnostic_matches(
                        diagnostic,
                        runs,
                        project_id,
                        source_revision_id,
                        effective_config_fingerprint,
                    )
            }),
        DiagnosticEvidenceRefV2::Artifact { .. } | DiagnosticEvidenceRefV2::Document { .. } => true,
    }
}

fn suppression_v2_subject_matches(
    suppression: &SuppressionV2,
    findings: &[Finding],
    diagnostics: &[DiagnosticV2],
    runs: &[ValidationRunV2],
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    match suppression.subject_kind {
        DecisionSubjectKindV2::Finding => findings.iter().any(|finding| {
            suppression
                .subject_fingerprint
                .as_ref()
                .is_some_and(|fingerprint| fingerprint == &finding.finding_fingerprint)
        }),
        DecisionSubjectKindV2::Diagnostic => diagnostics.iter().any(|diagnostic| {
            let subject_matches = suppression
                .subject_fingerprint
                .as_ref()
                .is_some_and(|fingerprint| fingerprint == &diagnostic.fingerprint)
                || suppression
                    .rule_ref_constraint
                    .as_ref()
                    .is_some_and(|rule| rule == &diagnostic.rule_ref);
            let binding_matches =
                suppression
                    .subject_binding_constraint
                    .as_ref()
                    .is_some_and(|binding| {
                        diagnostic.subject_binding_fingerprint.as_ref() == Some(binding)
                    });
            subject_matches
                && binding_matches
                && current_diagnostic_matches(
                    diagnostic,
                    runs,
                    &suppression.project_id,
                    source_revision_id,
                    effective_config_fingerprint,
                )
        }),
    }
}

fn disposition_v2_subject_matches(
    disposition: &DispositionV2,
    findings: &[Finding],
    diagnostics: &[DiagnosticV2],
    runs: &[ValidationRunV2],
    source_revision_id: &ProjectRevisionId,
    effective_config_fingerprint: &Sha256Hash,
) -> bool {
    match disposition.subject_kind {
        DecisionSubjectKindV2::Finding => {
            disposition.finding_id.as_ref().is_some_and(|finding_id| {
                findings.iter().any(|finding| {
                    finding.finding_id == *finding_id
                        && finding.finding_fingerprint == disposition.subject_fingerprint
                })
            })
        }
        DecisionSubjectKindV2::Diagnostic => diagnostics.iter().any(|diagnostic| {
            diagnostic.fingerprint == disposition.subject_fingerprint
                && disposition
                    .subject_binding_constraint
                    .as_ref()
                    .is_some_and(|binding| {
                        diagnostic.subject_binding_fingerprint.as_ref() == Some(binding)
                    })
                && current_diagnostic_matches(
                    diagnostic,
                    runs,
                    &disposition.project_id,
                    source_revision_id,
                    effective_config_fingerprint,
                )
        }),
    }
}

fn plan_local_state_import_for_repository(
    repository: &dyn ProjectManagementRepository,
    bundle: &LocalStateBundle,
    payload_sha256: Sha256Hash,
) -> Result<LocalStateImportPlan, RepositoryError> {
    let status = repository.status()?;
    let current = repository.latest_scan()?.ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::NotFound,
            "local state import target has no current scan",
        )
    })?;
    let mut conflicts = Vec::new();
    if current.project_revision_id != bundle.source_revision_id {
        conflicts.push(local_state_conflict(
            "bundle",
            bundle.bundle_id.as_str(),
            "SOURCE_REVISION_MISMATCH",
        ));
    }
    if current.effective_config_fingerprint != bundle.effective_config_fingerprint {
        conflicts.push(local_state_conflict(
            "bundle",
            bundle.bundle_id.as_str(),
            "CONFIG_FINGERPRINT_MISMATCH",
        ));
    }
    let mut suppressions = repository
        .list_suppressions()?
        .into_iter()
        .map(|value| value.suppression_id)
        .collect::<BTreeSet<_>>();
    suppressions.extend(
        repository
            .list_suppressions_v2()?
            .into_iter()
            .map(|value| value.suppression_id),
    );
    for value in &bundle.local_suppressions {
        if suppressions.contains(&value.suppression_id) {
            conflicts.push(local_state_conflict(
                "suppression",
                value.suppression_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
    }
    for value in &bundle.local_suppressions_v2 {
        if suppressions.contains(&value.suppression_id) {
            conflicts.push(local_state_conflict(
                "suppression_v2",
                value.suppression_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        let mut target_missing = !suppression_v2_subject_is_current(
            repository,
            value,
            &bundle.source_revision_id,
            &bundle.effective_config_fingerprint,
        )?;
        for reference in &value.review_evidence_refs {
            target_missing |= !diagnostic_evidence_ref_is_current(
                repository,
                reference,
                &bundle.project_id,
                &bundle.source_revision_id,
                &bundle.effective_config_fingerprint,
            )?;
        }
        if target_missing {
            conflicts.push(local_state_conflict(
                "suppression_v2",
                value.suppression_id.as_str(),
                "TARGET_SUBJECT_OR_EVIDENCE_NOT_CURRENT",
            ));
        }
    }
    let mut baselines = repository
        .list_baselines()?
        .into_iter()
        .map(|value| value.baseline_id)
        .collect::<BTreeSet<_>>();
    baselines.extend(
        repository
            .list_baselines_v2()?
            .into_iter()
            .map(|value| value.baseline_id),
    );
    for value in &bundle.local_baselines {
        if baselines.contains(&value.baseline_id) {
            conflicts.push(local_state_conflict(
                "baseline",
                value.baseline_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        if repository
            .get_workspace_snapshot(&value.workspace_snapshot_id)?
            .is_none()
        {
            conflicts.push(local_state_conflict(
                "baseline",
                value.baseline_id.as_str(),
                "TARGET_SNAPSHOT_NOT_CURRENT",
            ));
        }
    }
    for value in &bundle.local_baselines_v2 {
        if baselines.contains(&value.baseline_id) {
            conflicts.push(local_state_conflict(
                "baseline_v2",
                value.baseline_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        if !baseline_v2_subject_is_current(
            repository,
            value,
            &bundle.source_revision_id,
            &bundle.effective_config_fingerprint,
        )? {
            conflicts.push(local_state_conflict(
                "baseline_v2",
                value.baseline_id.as_str(),
                "TARGET_SUBJECT_OR_RULE_SET_NOT_CURRENT",
            ));
        }
    }
    let mut dispositions = repository
        .list_dispositions()?
        .into_iter()
        .map(|value| value.disposition_id)
        .collect::<BTreeSet<_>>();
    dispositions.extend(
        repository
            .list_dispositions_v2()?
            .into_iter()
            .map(|value| value.disposition_id),
    );
    for value in &bundle.local_dispositions {
        if dispositions.contains(&value.disposition_id) {
            conflicts.push(local_state_conflict(
                "disposition",
                value.disposition_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        if repository.get_finding(&value.finding_id)?.is_none() {
            conflicts.push(local_state_conflict(
                "disposition",
                value.disposition_id.as_str(),
                "TARGET_FINDING_NOT_CURRENT",
            ));
        }
        if let Some(duplicate_id) = value.duplicate_of_finding_id.as_ref()
            && repository.get_finding(duplicate_id)?.is_none()
        {
            conflicts.push(local_state_conflict(
                "disposition",
                value.disposition_id.as_str(),
                "DUPLICATE_TARGET_FINDING_NOT_CURRENT",
            ));
        }
    }
    for value in &bundle.local_dispositions_v2 {
        if dispositions.contains(&value.disposition_id) {
            conflicts.push(local_state_conflict(
                "disposition_v2",
                value.disposition_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        let mut target_missing = !disposition_v2_subject_is_current(
            repository,
            value,
            &bundle.source_revision_id,
            &bundle.effective_config_fingerprint,
        )?;
        if let Some(duplicate_id) = value.duplicate_of_finding_id.as_ref() {
            target_missing |= repository.get_finding(duplicate_id)?.is_none();
        }
        for reference in &value.review_evidence_refs {
            target_missing |= !diagnostic_evidence_ref_is_current(
                repository,
                reference,
                &bundle.project_id,
                &bundle.source_revision_id,
                &bundle.effective_config_fingerprint,
            )?;
        }
        if target_missing {
            conflicts.push(local_state_conflict(
                "disposition_v2",
                value.disposition_id.as_str(),
                "TARGET_SUBJECT_OR_EVIDENCE_NOT_CURRENT",
            ));
        }
    }
    let change_plans = repository
        .list_change_plans()?
        .into_iter()
        .map(|value| value.change_plan_id)
        .collect::<BTreeSet<_>>();
    for value in &bundle.active_change_plans {
        if change_plans.contains(&value.change_plan_id) {
            conflicts.push(local_state_conflict(
                "change_plan",
                value.change_plan_id.as_str(),
                "ENTITY_ALREADY_EXISTS",
            ));
        }
        let mut target_missing = repository
            .get_workspace_snapshot(&value.target_workspace_snapshot_id)?
            .is_none();
        for finding_id in &value.finding_refs {
            target_missing |= repository.get_finding(finding_id)?.is_none();
        }
        if target_missing {
            conflicts.push(local_state_conflict(
                "change_plan",
                value.change_plan_id.as_str(),
                "TARGET_STATE_NOT_CURRENT",
            ));
        }
    }
    seal_local_state_import_plan(
        RecoveryPlanId::new(),
        bundle,
        status.store_revision,
        payload_sha256,
        conflicts,
        &PersistenceRedactor::for_current_user(),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import plan could not be sealed",
        )
    })
}

fn apply_local_state_import_for_repository(
    receipt_root: &Path,
    repository: &dyn ProjectManagementRepository,
    bundle: &LocalStateBundle,
    payload_sha256: Sha256Hash,
    plan: &LocalStateImportPlan,
    approved_plan_fingerprint: &str,
) -> Result<LocalStateImportResult, RepositoryError> {
    require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state import approval fingerprint is stale",
        )
    })?;
    validate_local_state_import_plan(plan).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import plan is invalid",
        )
    })?;
    if !plan.conflicts.is_empty() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state import plan contains unresolved conflicts",
        ));
    }
    let bound_plan = seal_local_state_import_plan(
        plan.recovery_plan_id.clone(),
        bundle,
        plan.expected_store_revision,
        payload_sha256.clone(),
        Vec::new(),
        &PersistenceRedactor::for_current_user(),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state import source could not be resealed",
        )
    })?;
    if &bound_plan != plan || payload_sha256 != plan.payload_sha256 {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state import source changed after planning",
        ));
    }
    if let Some(completed) = read_recovery_receipt::<LocalStateImportResult>(
        receipt_root,
        "local-state-import",
        &plan.plan_fingerprint,
    )? {
        let expected = local_state_import_result(plan, bundle, completed.applied_at)?;
        if completed != expected {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "completed local state import receipt is invalid",
            ));
        }
        return Ok(completed);
    }
    let status = repository.status()?;
    let current = repository.latest_scan()?.ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::NotFound,
            "local state import target has no current scan",
        )
    })?;
    if status.store_revision != plan.expected_store_revision
        || current.project_revision_id != plan.expected_source_revision_id
        || current.effective_config_fingerprint != plan.expected_config_fingerprint
    {
        let suppressions = repository.list_suppressions()?;
        let baselines = repository.list_baselines()?;
        let dispositions = repository.list_dispositions()?;
        let change_plans = repository.list_change_plans()?;
        let suppressions_v2 = repository.list_suppressions_v2()?;
        let baselines_v2 = repository.list_baselines_v2()?;
        let dispositions_v2 = repository.list_dispositions_v2()?;
        let already_applied = status.store_revision > plan.expected_store_revision
            && current.project_revision_id == plan.expected_source_revision_id
            && current.effective_config_fingerprint == plan.expected_config_fingerprint
            && bundle
                .local_suppressions
                .iter()
                .all(|value| suppressions.contains(value))
            && bundle
                .local_baselines
                .iter()
                .all(|value| baselines.contains(value))
            && bundle
                .local_dispositions
                .iter()
                .all(|value| dispositions.contains(value))
            && bundle
                .active_change_plans
                .iter()
                .all(|value| change_plans.contains(value))
            && bundle
                .local_suppressions_v2
                .iter()
                .all(|value| suppressions_v2.contains(value))
            && bundle
                .local_baselines_v2
                .iter()
                .all(|value| baselines_v2.contains(value))
            && bundle
                .local_dispositions_v2
                .iter()
                .all(|value| dispositions_v2.contains(value));
        if already_applied {
            let result = local_state_import_result(plan, bundle, Utc::now())?;
            write_recovery_receipt(
                receipt_root,
                "local-state-import",
                &plan.plan_fingerprint,
                &result,
            )?;
            return Ok(result);
        }
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state import source or target changed after planning",
        ));
    }
    repository.import_local_state(bundle, plan.expected_store_revision)?;
    let result = local_state_import_result(plan, bundle, Utc::now())?;
    write_recovery_receipt(
        receipt_root,
        "local-state-import",
        &plan.plan_fingerprint,
        &result,
    )?;
    Ok(result)
}

fn local_state_conflict(kind: &str, id: &str, reason: &str) -> LocalStateConflict {
    LocalStateConflict {
        entity_kind: kind.to_owned(),
        entity_id: id.to_owned(),
        reason_code: reason.to_owned(),
    }
}

fn backup_targets_from_statuses(statuses: &[ManagementStoreStatus]) -> Vec<BackupStoreTarget> {
    let mut targets: Vec<_> = statuses
        .iter()
        .map(|status| BackupStoreTarget {
            scope: status.store_scope.clone(),
            store_id: status.store_id.clone(),
            generation: status.generation,
            management_store_version: status.management_store_version,
            store_revision: status.store_revision,
        })
        .collect();
    targets.sort_by_key(|target| match &target.scope {
        StoreScope::Global => "0".to_owned(),
        StoreScope::Project { project_id } => format!("1:{}", project_id.as_str()),
    });
    targets
}

fn store_vector_from_statuses(
    statuses: &[ManagementStoreStatus],
) -> Result<StoreVersionVector, RepositoryError> {
    let global = statuses
        .iter()
        .find(|status| matches!(status.store_scope, StoreScope::Global))
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "management store set has no global status",
            )
        })?;
    let mut projects: Vec<_> = statuses
        .iter()
        .filter_map(|status| match &status.store_scope {
            StoreScope::Project { project_id } => Some(ProjectStorePoint {
                project_id: project_id.clone(),
                point: StorePoint {
                    store_id: status.store_id.clone(),
                    generation: status.generation,
                    revision: status.store_revision,
                },
            }),
            StoreScope::Global => None,
        })
        .collect();
    projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    Ok(StoreVersionVector {
        global: StorePoint {
            store_id: global.store_id.clone(),
            generation: global.generation,
            revision: global.store_revision,
        },
        projects,
    })
}

fn backup_store_locator(scope: &StoreScope) -> String {
    match scope {
        StoreScope::Global => "stores/global/store".to_owned(),
        StoreScope::Project { project_id } => {
            format!("stores/projects/{}/store", project_id.as_str())
        }
    }
}

fn validate_backup_set_relationships(
    root: &Path,
    manifest: &BackupSetManifest,
) -> Result<(), RepositoryError> {
    validate_backup_set(manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "backup set manifest is invalid",
        )
    })?;
    for entry in &manifest.entries {
        let path = root.join(&entry.relative_locator);
        let metadata = fs::metadata(&path).map_err(map_io)?;
        let byte_sha256 =
            Sha256Hash::digest_reader(fs::File::open(&path).map_err(map_io)?).map_err(map_io)?;
        let status = read_store_status_read_only(&path)?;
        if !metadata.is_file()
            || metadata.len() != entry.size_bytes
            || byte_sha256 != entry.byte_sha256
            || status.store_scope != entry.scope
            || status.store_id != entry.store_id
            || status.generation != entry.generation
            || status.management_store_version != entry.management_store_version
            || status.store_revision != entry.store_revision
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "backup store bytes or header do not match the manifest",
            ));
        }
    }
    let global = manifest
        .entries
        .iter()
        .find(|entry| matches!(entry.scope, StoreScope::Global))
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "backup set has no global store",
            )
        })?;
    let connection = Connection::open_with_flags(
        root.join(&global.relative_locator),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    let mut statement = connection
        .prepare("SELECT project_id FROM projects ORDER BY project_id")
        .map_err(map_sql)?;
    let mut expected = BTreeSet::new();
    for project_id in statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_sql)?
    {
        expected.insert(ProjectId::parse(project_id.map_err(map_sql)?).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "backup global relationship has an invalid project ID",
            )
        })?);
    }
    let actual: BTreeSet<_> = manifest
        .entries
        .iter()
        .filter_map(|entry| match &entry.scope {
            StoreScope::Global => None,
            StoreScope::Project { project_id } => Some(project_id.clone()),
        })
        .collect();
    if expected != actual {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "backup project relationships do not match the global store",
        ));
    }
    Ok(())
}

fn validate_current_retention_store_revisions(
    repositories: &SqliteManagementRepositorySet,
    expected: &BTreeMap<String, u64>,
    replay_project: Option<&ProjectId>,
) -> Result<(), RepositoryError> {
    let global_revision = expected.get("global").copied().ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "retention plan omits the global store revision",
        )
    })?;
    if repositories.global.status()?.store_revision != global_revision {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "retention global revision is stale",
        ));
    }
    for (project_id, expected_revision) in expected {
        if project_id == "global" {
            continue;
        }
        let project_id = ProjectId::parse(project_id.clone()).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "retention plan has an invalid project revision key",
            )
        })?;
        if replay_project == Some(&project_id) {
            continue;
        }
        let status = read_store_status_snapshot(&repositories.project_path(&project_id)?)?;
        if status.store_revision != *expected_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention project revision is stale",
            ));
        }
    }
    Ok(())
}

impl ManagementRepositorySet for SqliteManagementRepositorySet {
    fn global(&self) -> &dyn GlobalManagementRepository {
        &self.global
    }

    fn project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Arc<dyn ProjectManagementRepository>, RepositoryError> {
        let repository = self.project_repository(project_id)?;
        self.refresh_active_set()?;
        Ok(repository)
    }

    fn active_set(&self) -> Result<ActiveSetManifest, RepositoryError> {
        self.active_set
            .lock()
            .map(|manifest| manifest.clone())
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "active set cache is unavailable",
                )
            })
    }

    fn status_all(&self) -> Result<Vec<ManagementStoreStatus>, RepositoryError> {
        let (project_total, _) = self.global.project_ids_page(None, 0)?;
        if project_total.saturating_add(1) > u64::from(MANAGEMENT_STATUS_DEFAULT_MAX_ITEMS) {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "management status requires the paged status query",
            ));
        }
        let page = self.status_page(&ManagementStatusQueryV1::default())?;
        if page.truncated {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "management status requires the paged status query",
            ));
        }
        Ok(page.items)
    }

    fn status_page(
        &self,
        query: &ManagementStatusQueryV1,
    ) -> Result<ManagementStatusPageV1, RepositoryError> {
        query.validate().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management status query is invalid",
            )
        })?;
        let global = self.global.status()?;
        let cursor = query
            .cursor
            .as_deref()
            .map(decode_management_status_cursor)
            .transpose()?;
        if cursor.as_ref().is_some_and(|cursor| {
            cursor.global_store_id != global.store_id
                || cursor.snapshot_revision != global.store_revision
        }) {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management status cursor is stale",
            ));
        }
        let global_returned = cursor.is_some();
        let after_project_id = cursor
            .as_ref()
            .and_then(|cursor| cursor.after_project_id.as_ref());
        let project_slots = usize::try_from(query.max_items)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "management status page limit is invalid",
                )
            })?
            .saturating_sub(usize::from(!global_returned));
        let fetch_limit = project_slots.saturating_add(1).max(1);
        let (project_total, project_ids) = self
            .global
            .project_ids_page(after_project_id, fetch_limit)?;
        let snapshot_hash = versioned_fingerprint(
            "star.management-status-snapshot",
            MANAGEMENT_STATUS_PAGE_SCHEMA_VERSION,
            &serde_json::json!({
                "global_store_id":global.store_id,
                "snapshot_revision":global.store_revision,
                "project_count":project_total,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management status snapshot fingerprint failed",
            )
        })?;
        let mut items = if global_returned {
            Vec::new()
        } else {
            vec![global.clone()]
        };
        let mut consumed = 0_usize;
        let mut last_project_id = after_project_id.cloned();

        let page_with = |items: Vec<ManagementStoreStatus>,
                         last_project_id: Option<ProjectId>,
                         has_more: bool|
         -> Result<ManagementStatusPageV1, RepositoryError> {
            let next_cursor = has_more
                .then(|| {
                    encode_management_status_cursor(&ManagementStatusCursorV1 {
                        schema_version: 1,
                        global_store_id: global.store_id.clone(),
                        snapshot_revision: global.store_revision,
                        global_returned: true,
                        after_project_id: last_project_id,
                    })
                })
                .transpose()?;
            Ok(ManagementStatusPageV1 {
                schema_id: MANAGEMENT_STATUS_PAGE_SCHEMA_ID.to_owned(),
                schema_version: MANAGEMENT_STATUS_PAGE_SCHEMA_VERSION,
                snapshot_revision: global.store_revision,
                snapshot_hash: snapshot_hash.clone(),
                returned_count: u32::try_from(items.len()).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "management status page item count overflowed",
                    )
                })?,
                total_count: project_total.saturating_add(1),
                items,
                truncated: has_more,
                next_cursor,
                max_items: query.max_items,
                max_bytes: query.max_bytes,
            })
        };

        for project_id in project_ids.iter().take(project_slots) {
            let status = read_store_status_snapshot(&self.project_path(project_id)?)?;
            let mut tentative = items.clone();
            tentative.push(status);
            let tentative_consumed = consumed + 1;
            let has_more = tentative_consumed < project_ids.len();
            let tentative_page = page_with(tentative.clone(), Some(project_id.clone()), has_more)?;
            let encoded_len = serde_json::to_vec(&tentative_page)
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "management status page serialization failed",
                    )
                })?
                .len() as u64;
            if encoded_len > query.max_bytes {
                break;
            }
            items = tentative;
            consumed = tentative_consumed;
            last_project_id = Some(project_id.clone());
        }
        let has_more = consumed < project_ids.len();
        if has_more && consumed == 0 && cursor.is_some() {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "management status page byte budget cannot fit the next item",
            ));
        }
        let page = page_with(items, last_project_id, has_more)?;
        page.validate().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "management status page exceeds its transport budget",
            )
        })?;
        let final_global = self.global.status()?;
        if final_global.store_id != global.store_id
            || final_global.store_revision != global.store_revision
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management status cursor is stale",
            ));
        }
        Ok(page)
    }

    fn verify_all(&self) -> Result<Vec<ManagementStoreStatus>, RepositoryError> {
        let mut statuses = vec![self.global.verify_integrity()?];
        for project in self.global.list_projects()? {
            statuses.push(
                self.project_repository(&project.project_id)?
                    .verify_integrity()?,
            );
        }
        self.write_active_set(&statuses)?;
        Ok(statuses)
    }

    fn plan_backup(&self, destination: &Path) -> Result<BackupPlan, RepositoryError> {
        match fs::symlink_metadata(destination) {
            Ok(_) => {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "backup destination must not exist when the plan is created",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(map_io(error)),
        }
        let destination_fingerprint = backup_destination_fingerprint(&self.root, destination)?;
        let statuses = self.verify_all()?;
        let active_set = self.active_set()?;
        seal_backup_plan(
            BackupSetId::new(),
            Utc::now(),
            &active_set,
            store_vector_from_statuses(&statuses)?,
            destination_fingerprint,
            backup_targets_from_statuses(&statuses),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "backup plan could not be sealed",
            )
        })
    }

    fn apply_backup(
        &self,
        destination: &Path,
        plan: &BackupPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<BackupApplyResult, RepositoryError> {
        require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "backup approval fingerprint is stale",
                )
            },
        )?;
        if backup_destination_fingerprint(&self.root, destination)? != plan.destination_fingerprint
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "backup destination changed after planning",
            ));
        }
        if let Some(completed) = read_recovery_receipt::<BackupApplyResult>(
            &self.root,
            "backup",
            &plan.plan_fingerprint,
        )? {
            let expected =
                backup_apply_result(plan, completed.manifest.clone(), completed.applied_at)?;
            if completed != expected || read_verified_backup_set(destination)? != completed.manifest
            {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "completed backup receipt or backup set is invalid",
                ));
            }
            return Ok(completed);
        }
        if destination.exists() {
            let manifest = read_verified_backup_set(destination)?;
            let matches_plan = manifest.backup_set_id == plan.backup_set_id
                && manifest.source_active_set_fingerprint == plan.source_active_set_fingerprint
                && manifest.entries.len() == plan.stores.len()
                && manifest
                    .entries
                    .iter()
                    .zip(&plan.stores)
                    .all(|(entry, target)| {
                        entry.scope == target.scope
                            && entry.store_id == target.store_id
                            && entry.generation == target.generation
                            && entry.management_store_version == target.management_store_version
                            && entry.store_revision == target.store_revision
                    });
            if !matches_plan {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "backup destination belongs to a different plan",
                ));
            }
            let result = backup_apply_result(plan, manifest, Utc::now())?;
            write_recovery_receipt(&self.root, "backup", &plan.plan_fingerprint, &result)?;
            return Ok(result);
        }
        let statuses = self.verify_all()?;
        let active_set = self.active_set()?;
        validate_backup_plan(plan, &active_set).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "backup plan no longer matches the active store set",
            )
        })?;
        if backup_targets_from_statuses(&statuses) != plan.stores {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management store revision changed after backup planning",
            ));
        }

        create_private_dir(destination)?;
        let mut entries = Vec::with_capacity(plan.stores.len());
        for target in &plan.stores {
            let relative_locator = backup_store_locator(&target.scope);
            let backup_path = destination.join(&relative_locator);
            match &target.scope {
                StoreScope::Global => self.global.backup(&backup_path)?,
                StoreScope::Project { project_id } => {
                    self.project_repository(project_id)?.backup(&backup_path)?;
                }
            }
            let status = read_store_status_read_only(&backup_path)?;
            if status.store_scope != target.scope
                || status.store_id != target.store_id
                || status.generation != target.generation
                || status.management_store_version != target.management_store_version
                || status.store_revision != target.store_revision
            {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "backup store header does not match the approved plan",
                ));
            }
            let size_bytes = fs::metadata(&backup_path).map_err(map_io)?.len();
            let byte_sha256 =
                Sha256Hash::digest_reader(fs::File::open(&backup_path).map_err(map_io)?)
                    .map_err(map_io)?;
            entries.push(BackupStoreEntry {
                scope: target.scope.clone(),
                store_id: target.store_id.clone(),
                generation: target.generation,
                management_store_version: target.management_store_version,
                store_revision: target.store_revision,
                relative_locator,
                size_bytes,
                byte_sha256,
            });
        }
        let manifest = seal_backup_set(
            plan.backup_set_id.clone(),
            plan.created_at,
            plan.source_active_set_fingerprint.clone(),
            entries,
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "backup set manifest could not be sealed",
            )
        })?;
        validate_backup_set_relationships(destination, &manifest)?;
        validate_backup_set(&manifest).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "backup set validation failed",
            )
        })?;
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "backup set manifest serialization failed",
            )
        })?;
        write_private_atomic(&destination.join("backup-set.json"), &manifest_bytes)?;
        let result = backup_apply_result(plan, manifest, Utc::now())?;
        write_recovery_receipt(&self.root, "backup", &plan.plan_fingerprint, &result)?;
        Ok(result)
    }

    fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, RepositoryError> {
        plan_local_state_export_for_repository(
            self.project_repository(project_id)?.as_ref(),
            project_id,
            destination,
        )
    }

    fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, RepositoryError> {
        apply_local_state_export_for_repository(
            &self.root,
            self.project_repository(&plan.project_id)?.as_ref(),
            destination,
            plan,
            approved_plan_fingerprint,
        )
    }

    fn plan_local_state_import(
        &self,
        source: &Path,
    ) -> Result<LocalStateImportPlan, RepositoryError> {
        let (bundle, payload_sha256) = read_local_state_bundle(source)?;
        let repository = self.project_repository(&bundle.project_id)?;
        plan_local_state_import_for_repository(repository.as_ref(), &bundle, payload_sha256)
    }

    fn apply_local_state_import(
        &self,
        source: &Path,
        plan: &LocalStateImportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateImportResult, RepositoryError> {
        let (bundle, payload_sha256) = read_local_state_bundle(source)?;
        let repository = self.project_repository(&plan.project_id)?;
        apply_local_state_import_for_repository(
            &self.root,
            repository.as_ref(),
            &bundle,
            payload_sha256,
            plan,
            approved_plan_fingerprint,
        )
    }

    fn plan_retention(&self) -> Result<RetentionPlan, RepositoryError> {
        self.plan_retention_with_policy(&RetentionPolicy::default())
    }

    fn plan_retention_with_policy(
        &self,
        policy: &RetentionPolicy,
    ) -> Result<RetentionPlan, RepositoryError> {
        if policy.keep_latest_successful_scans == 0
            || policy.incomplete_staging_retention_days == 0
            || policy.scan_detail_retention_days == 0
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "retention policy is invalid",
            ));
        }
        let created_at = Utc::now();
        let cutoff = created_at
            - chrono::Duration::days(
                policy
                    .incomplete_staging_retention_days
                    .min(i64::MAX as u64) as i64,
            );
        let scan_detail_cutoff = created_at
            - chrono::Duration::days(policy.scan_detail_retention_days.min(i64::MAX as u64) as i64);
        let global = self.global.status()?;
        let mut expected_store_revisions =
            BTreeMap::from([("global".to_owned(), global.store_revision)]);
        let mut candidates = Vec::new();
        for project in self.global.list_projects()? {
            let repository = self.project_repository(&project.project_id)?;
            let status = repository.status()?;
            expected_store_revisions.insert(
                project.project_id.as_str().to_owned(),
                status.store_revision,
            );
            candidates.extend(repository.retention_candidates(
                cutoff,
                scan_detail_cutoff,
                policy.keep_latest_successful_scans,
            )?);
        }
        candidates.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.generation_id.cmp(&right.generation_id))
        });
        let fingerprint = versioned_fingerprint(
            "star.management-retention-plan",
            1,
            &serde_json::json!({
                "expected_store_revisions":expected_store_revisions,
                "candidates":candidates,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention plan fingerprint failed",
            )
        })?;
        Ok(RetentionPlan {
            schema_version: 1,
            created_at: created_at.to_rfc3339(),
            expected_store_revisions,
            candidates,
            plan_fingerprint: fingerprint,
        })
    }

    fn apply_retention(
        &self,
        plan: &RetentionPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RetentionApplyResult, RepositoryError> {
        if plan.schema_version != 1
            || plan.plan_fingerprint.as_str() != approved_plan_fingerprint
            || self.global.status()?.store_revision
                != plan
                    .expected_store_revisions
                    .get("global")
                    .copied()
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::Invalid,
                            "retention plan omits global store revision",
                        )
                    })?
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention plan approval or global revision is stale",
            ));
        }
        let mut by_project: BTreeMap<ProjectId, Vec<&RetentionCandidate>> = BTreeMap::new();
        for candidate in &plan.candidates {
            by_project
                .entry(candidate.project_id.clone())
                .or_default()
                .push(candidate);
        }
        let mut applied_count = 0;
        for (project_id, candidates) in by_project {
            let expected_revision = plan
                .expected_store_revisions
                .get(project_id.as_str())
                .copied()
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "retention plan omits a project store revision",
                    )
                })?;
            applied_count += self.project_repository(&project_id)?.apply_retention(
                expected_revision,
                &candidates,
                &plan.plan_fingerprint,
            )?;
        }
        Ok(RetentionApplyResult {
            applied_count,
            plan_fingerprint: plan.plan_fingerprint.clone(),
        })
    }

    fn plan_retention_v2(
        &self,
        policy: &RetentionPolicyV2,
    ) -> Result<RetentionPlanV2, RepositoryError> {
        self.plan_retention_v2_controlled(policy, &|| false)
    }

    fn plan_retention_v2_controlled(
        &self,
        policy: &RetentionPolicyV2,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RetentionPlanV2, RepositoryError> {
        policy.validate().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention policy is invalid",
            )
        })?;
        let created_at = Utc::now();
        let global = self.global.status()?;
        let mut expected_store_revisions =
            BTreeMap::from([("global".to_owned(), global.store_revision)]);
        let mut candidates = Vec::new();
        let mut protected_targets = Vec::new();
        for project in self.global.list_projects()? {
            if is_cancelled() {
                return Err(repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "retention planning was cancelled",
                ));
            }
            let repository = self.project_repository(&project.project_id)?;
            let status = repository.status()?;
            expected_store_revisions.insert(
                project.project_id.as_str().to_owned(),
                status.store_revision,
            );
            let (mut project_candidates, mut project_protected) =
                repository.retention_analysis_v2(policy, created_at, is_cancelled)?;
            candidates.append(&mut project_candidates);
            protected_targets.append(&mut project_protected);
        }
        candidates.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.candidate_id.cmp(&right.candidate_id))
        });
        protected_targets.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.target_ref.cmp(&right.target_ref))
                .then_with(|| left.reason_code.cmp(&right.reason_code))
        });
        let (migration_backup_candidates, protected_migration_backups) =
            retention_migration_backup_inventory(&self.root, policy)?;
        let (estimated_rows, estimated_bytes) = candidates
            .iter()
            .map(|candidate| (candidate.estimated_rows, candidate.estimated_bytes))
            .chain(
                migration_backup_candidates
                    .iter()
                    .map(|candidate| (candidate.estimated_rows, candidate.estimated_bytes)),
            )
            .try_fold(
                (0_u64, 0_u64),
                |(rows, bytes), (candidate_rows, candidate_bytes)| {
                    Ok::<_, RepositoryError>((
                        rows.checked_add(candidate_rows).ok_or_else(|| {
                            repository_error(
                                RepositoryErrorCategory::QuotaExceeded,
                                "retention row estimate overflowed",
                            )
                        })?,
                        bytes.checked_add(candidate_bytes).ok_or_else(|| {
                            repository_error(
                                RepositoryErrorCategory::QuotaExceeded,
                                "retention byte estimate overflowed",
                            )
                        })?,
                    ))
                },
            )?;
        let policy_fingerprint =
            versioned_fingerprint("star.management-retention-policy", 2, policy).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "retention policy fingerprint failed",
                )
            })?;
        let source_fingerprint = versioned_fingerprint(
            "star.management-retention-source",
            2,
            &serde_json::json!({
                "expected_store_revisions":expected_store_revisions,
                "migration_backup_candidates":migration_backup_candidates,
                "protected_migration_backups":protected_migration_backups,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention source fingerprint failed",
            )
        })?;
        let plan_fingerprint = versioned_fingerprint(
            "star.management-retention-plan",
            RETENTION_PLAN_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "policy_fingerprint":policy_fingerprint,
                "source_fingerprint":source_fingerprint,
                "expected_store_revisions":expected_store_revisions,
                "candidates":candidates,
                "protected_targets":protected_targets,
                "migration_backup_candidates":migration_backup_candidates,
                "protected_migration_backups":protected_migration_backups,
                "estimated_rows":estimated_rows,
                "estimated_bytes":estimated_bytes,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention plan fingerprint failed",
            )
        })?;
        let plan = RetentionPlanV2 {
            schema_id: RETENTION_PLAN_V2_SCHEMA_ID.to_owned(),
            schema_version: RETENTION_PLAN_V2_SCHEMA_VERSION,
            created_at,
            policy: policy.clone(),
            policy_fingerprint,
            source_fingerprint,
            expected_store_revisions,
            candidates,
            protected_targets,
            migration_backup_candidates,
            protected_migration_backups,
            estimated_rows,
            estimated_bytes,
            plan_fingerprint,
        };
        validate_retention_plan_v2(&plan)?;
        Ok(plan)
    }

    fn load_retention_execution(
        &self,
    ) -> Result<Option<(RetentionPlanV2, RetentionCheckpointV1)>, RepositoryError> {
        let plan_path = self.root.join(RETENTION_EXECUTION_PLAN_FILENAME);
        let checkpoint_path = self.root.join(RETENTION_CHECKPOINT_FILENAME);
        let plan_exists = plan_path.try_exists().map_err(map_io)?;
        let checkpoint_exists = checkpoint_path.try_exists().map_err(map_io)?;
        if !plan_exists && !checkpoint_exists {
            return Ok(None);
        }
        if !plan_exists || !checkpoint_exists {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "retention execution documents are incomplete",
            ));
        }
        let plan: RetentionPlanV2 = read_strict_json_file(&plan_path)?;
        let checkpoint: RetentionCheckpointV1 = read_strict_json_file(&checkpoint_path)?;
        validate_retention_plan_v2(&plan)?;
        validate_retention_checkpoint(&plan, &checkpoint)?;
        if checkpoint.complete || checkpoint.cancelled {
            return Ok(None);
        }
        Ok(Some((plan, checkpoint)))
    }

    fn start_retention_execution(
        &self,
        plan: &RetentionPlanV2,
        checkpoint: &RetentionCheckpointV1,
    ) -> Result<(), RepositoryError> {
        validate_retention_plan_v2(plan)?;
        validate_retention_checkpoint(plan, checkpoint)?;
        let plan_path = self.root.join(RETENTION_EXECUTION_PLAN_FILENAME);
        let checkpoint_path = self.root.join(RETENTION_CHECKPOINT_FILENAME);
        if plan_path.try_exists().map_err(map_io)?
            && checkpoint_path.try_exists().map_err(map_io)?
        {
            let existing_plan: RetentionPlanV2 = read_strict_json_file(&plan_path)?;
            let existing_checkpoint: RetentionCheckpointV1 =
                read_strict_json_file(&checkpoint_path)?;
            if !existing_checkpoint.complete
                && !existing_checkpoint.cancelled
                && existing_plan.plan_fingerprint != plan.plan_fingerprint
            {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "another retention execution is active",
                ));
            }
        }
        let plan_bytes = serde_json::to_vec_pretty(plan).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention plan serialization failed",
            )
        })?;
        let checkpoint_bytes = serde_json::to_vec_pretty(checkpoint).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention checkpoint serialization failed",
            )
        })?;
        write_private_atomic(&plan_path, &plan_bytes)?;
        write_private_atomic(&checkpoint_path, &checkpoint_bytes)
    }

    fn apply_retention_batch_v2(
        &self,
        plan: &RetentionPlanV2,
        checkpoint: &RetentionCheckpointV1,
    ) -> Result<RetentionBatchResultV1, RepositoryError> {
        validate_retention_plan_v2(plan)?;
        validate_retention_checkpoint(plan, checkpoint)?;
        let mut next = checkpoint.clone();
        if next.complete {
            return Ok(RetentionBatchResultV1 {
                processed_candidates: 0,
                applied_rows: 0,
                applied_bytes: 0,
                checkpoint: next,
            });
        }
        let candidate_index = usize::try_from(next.next_candidate_index).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "retention checkpoint cursor overflowed",
            )
        })?;
        if candidate_index == plan.total_candidate_count() {
            if next.active_candidate_id.is_some()
                || next.applied_rows != plan.estimated_rows
                || next.applied_bytes != plan.estimated_bytes
            {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "retention checkpoint totals are incomplete at the terminal cursor",
                ));
            }
            validate_current_retention_store_revisions(
                self,
                &checkpoint.expected_store_revisions,
                None,
            )?;
            if !migration_backup_inventory_matches(
                &self.root,
                plan,
                plan.migration_backup_candidates.len(),
            )? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "migration backup inventory changed before retention completion",
                ));
            }
            next.complete = true;
            next.updated_at = Utc::now();
            self.save_retention_checkpoint(&next)?;
            return Ok(RetentionBatchResultV1 {
                processed_candidates: 0,
                applied_rows: 0,
                applied_bytes: 0,
                checkpoint: next,
            });
        }
        let (applied_rows, applied_bytes, candidate_id, candidate_complete) =
            if let Some(candidate) = plan.candidates.get(candidate_index) {
                validate_current_retention_store_revisions(
                    self,
                    &checkpoint.expected_store_revisions,
                    Some(&candidate.project_id),
                )?;
                if !migration_backup_inventory_matches(&self.root, plan, 0)? {
                    return Err(repository_error(
                        RepositoryErrorCategory::RevisionConflict,
                        "migration backup inventory changed after retention planning",
                    ));
                }
                let expected_revision = next
                    .expected_store_revisions
                    .get(candidate.project_id.as_str())
                    .copied()
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::IntegrityFailed,
                            "retention checkpoint omits project revision",
                        )
                    })?;
                let (applied_rows, applied_bytes, committed_revision, candidate_complete) = self
                    .project_repository(&candidate.project_id)?
                    .apply_retention_candidate_v2(
                        expected_revision,
                        candidate,
                        &plan.plan_fingerprint,
                        RetentionBatchLimits {
                            rows: plan.policy.max_batch_rows,
                            bytes: plan.policy.max_batch_bytes,
                        },
                        RetentionCandidateProgress {
                            rows: next.active_candidate_applied_rows,
                            bytes: next.active_candidate_applied_bytes,
                        },
                    )?;
                next.expected_store_revisions
                    .insert(candidate.project_id.as_str().to_owned(), committed_revision);
                (
                    applied_rows,
                    applied_bytes,
                    candidate.candidate_id.clone(),
                    candidate_complete,
                )
            } else {
                validate_current_retention_store_revisions(
                    self,
                    &checkpoint.expected_store_revisions,
                    None,
                )?;
                let migration_index = candidate_index
                    .checked_sub(plan.candidates.len())
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::IntegrityFailed,
                            "retention migration backup cursor is invalid",
                        )
                    })?;
                let candidate = plan
                    .migration_backup_candidates
                    .get(migration_index)
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::IntegrityFailed,
                            "retention migration backup candidate is missing",
                        )
                    })?;
                let (applied_rows, applied_bytes) =
                    apply_migration_backup_retention_candidate(&self.root, plan, migration_index)?;
                (
                    applied_rows,
                    applied_bytes,
                    candidate.candidate_id.clone(),
                    true,
                )
            };
        next.committed_batch_count = next.committed_batch_count.saturating_add(1);
        next.applied_rows = next.applied_rows.checked_add(applied_rows).ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "retention applied row count overflowed",
            )
        })?;
        next.applied_bytes = next
            .applied_bytes
            .checked_add(applied_bytes)
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "retention applied byte count overflowed",
                )
            })?;
        if candidate_complete {
            if let Some(candidate) = plan.candidates.get(candidate_index) {
                let candidate_rows = next
                    .active_candidate_applied_rows
                    .checked_add(applied_rows)
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::QuotaExceeded,
                            "retention candidate row count overflowed",
                        )
                    })?;
                let candidate_bytes = next
                    .active_candidate_applied_bytes
                    .checked_add(applied_bytes)
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::QuotaExceeded,
                            "retention candidate byte count overflowed",
                        )
                    })?;
                if candidate_rows != candidate.estimated_rows
                    || candidate_bytes != candidate.estimated_bytes
                {
                    return Err(repository_error(
                        RepositoryErrorCategory::IntegrityFailed,
                        "retention candidate committed totals do not match the approved plan",
                    ));
                }
            }
            next.next_candidate_index = next.next_candidate_index.saturating_add(1);
            next.last_candidate_id = Some(candidate_id);
            next.active_candidate_id = None;
            next.active_candidate_applied_rows = 0;
            next.active_candidate_applied_bytes = 0;
        } else {
            next.active_candidate_id = Some(candidate_id);
            next.active_candidate_applied_rows = next
                .active_candidate_applied_rows
                .checked_add(applied_rows)
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention active candidate row count overflowed",
                    )
                })?;
            next.active_candidate_applied_bytes = next
                .active_candidate_applied_bytes
                .checked_add(applied_bytes)
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention active candidate byte count overflowed",
                    )
                })?;
        }
        next.complete = next.next_candidate_index == plan.total_candidate_count() as u64;
        next.updated_at = Utc::now();
        self.save_retention_checkpoint(&next)?;
        Ok(RetentionBatchResultV1 {
            processed_candidates: u64::from(candidate_complete),
            applied_rows,
            applied_bytes,
            checkpoint: next,
        })
    }

    fn save_retention_checkpoint(
        &self,
        checkpoint: &RetentionCheckpointV1,
    ) -> Result<(), RepositoryError> {
        let plan_path = self.root.join(RETENTION_EXECUTION_PLAN_FILENAME);
        let plan: RetentionPlanV2 = read_strict_json_file(&plan_path)?;
        validate_retention_plan_v2(&plan)?;
        validate_retention_checkpoint(&plan, checkpoint)?;
        let bytes = serde_json::to_vec_pretty(checkpoint).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "retention checkpoint serialization failed",
            )
        })?;
        write_private_atomic(&self.root.join(RETENTION_CHECKPOINT_FILENAME), &bytes)
    }

    fn clear_retention_execution(
        &self,
        plan_fingerprint: &Sha256Hash,
    ) -> Result<(), RepositoryError> {
        let plan_path = self.root.join(RETENTION_EXECUTION_PLAN_FILENAME);
        let checkpoint_path = self.root.join(RETENTION_CHECKPOINT_FILENAME);
        if !plan_path.try_exists().map_err(map_io)?
            && !checkpoint_path.try_exists().map_err(map_io)?
        {
            return Ok(());
        }
        let plan: RetentionPlanV2 = read_strict_json_file(&plan_path)?;
        let checkpoint: RetentionCheckpointV1 = read_strict_json_file(&checkpoint_path)?;
        if &plan.plan_fingerprint != plan_fingerprint || !checkpoint.complete {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention execution is not complete",
            ));
        }
        // Completed plan/checkpoint documents are retained as the crash-recovery receipt.
        Ok(())
    }

    fn plan_compaction(
        &self,
        store_id: &ManagementStoreId,
        backup_root: Option<&Path>,
        backup_result: Option<&BackupApplyResult>,
    ) -> Result<ManagementCompactionPlanV1, RepositoryError> {
        if backup_root.is_some() != backup_result.is_some() {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "management compaction backup root and result must be supplied together",
            ));
        }
        let (path, status) = self.store_path_and_status(store_id)?;
        let active_set = self.active_set()?;
        let (database_bytes, estimated_reclaimable_bytes, required_free_bytes) =
            compaction_space_snapshot(&path)?;
        let available_free_bytes_at_plan = available_disk_bytes(&path)?;
        let store_fingerprint =
            versioned_fingerprint("star.management-compaction-store", 1, &status).map_err(
                |_| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "management compaction store fingerprint failed",
                    )
                },
            )?;
        let source_fingerprint = versioned_fingerprint(
            "star.management-compaction-source",
            1,
            &serde_json::json!({
                "active_set_fingerprint":active_set.manifest_fingerprint,
                "store_fingerprint":store_fingerprint,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management compaction source fingerprint failed",
            )
        })?;
        let backup_binding = match (backup_root, backup_result) {
            (Some(backup_root), Some(backup_result)) => Some(verified_compaction_backup_binding(
                &self.root,
                backup_root,
                backup_result,
                &status,
                &active_set,
            )?),
            (None, None) => None,
            _ => unreachable!("paired compaction backup inputs were checked"),
        };
        let plan_fingerprint = versioned_fingerprint(
            "star.management-compaction-plan",
            MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION,
            &serde_json::json!({
                "store_id":status.store_id,
                "store_scope":status.store_scope,
                "store_revision":status.store_revision,
                "source_fingerprint":source_fingerprint,
                "store_fingerprint":store_fingerprint,
                "database_bytes":database_bytes,
                "estimated_reclaimable_bytes":estimated_reclaimable_bytes,
                "required_free_bytes":required_free_bytes,
                "available_free_bytes_at_plan":available_free_bytes_at_plan,
                "backup_binding":backup_binding,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management compaction plan fingerprint failed",
            )
        })?;
        let plan = ManagementCompactionPlanV1 {
            schema_id: MANAGEMENT_COMPACTION_PLAN_SCHEMA_ID.to_owned(),
            schema_version: MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION,
            store_id: status.store_id,
            store_scope: status.store_scope,
            store_revision: status.store_revision,
            source_fingerprint,
            store_fingerprint,
            database_bytes,
            estimated_reclaimable_bytes,
            required_free_bytes,
            available_free_bytes_at_plan,
            backup_binding,
            created_at: Utc::now(),
            plan_fingerprint,
        };
        validate_compaction_plan(&plan)?;
        Ok(plan)
    }

    fn apply_compaction(
        &self,
        plan: &ManagementCompactionPlanV1,
        approved_plan_fingerprint: &str,
        backup_root: &Path,
        backup_result: &BackupApplyResult,
    ) -> Result<ManagementCompactionApplyResultV1, RepositoryError> {
        validate_compaction_plan(plan)?;
        if plan.plan_fingerprint.as_str() != approved_plan_fingerprint
            || plan.backup_binding.is_none()
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management compaction approval or backup binding is missing",
            ));
        }
        let (path, status) = self.store_path_and_status(&plan.store_id)?;
        if status.store_scope != plan.store_scope || status.store_revision != plan.store_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management compaction source revision is stale",
            ));
        }
        let active_set = self.active_set()?;
        let current_backup_binding = verified_compaction_backup_binding(
            &self.root,
            backup_root,
            backup_result,
            &status,
            &active_set,
        )?;
        if plan.backup_binding.as_ref() != Some(&current_backup_binding) {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management compaction backup binding is stale",
            ));
        }
        let current_store_fingerprint =
            versioned_fingerprint("star.management-compaction-store", 1, &status).map_err(
                |_| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "management compaction store fingerprint failed",
                    )
                },
            )?;
        let current_source_fingerprint = versioned_fingerprint(
            "star.management-compaction-source",
            1,
            &serde_json::json!({
                "active_set_fingerprint":active_set.manifest_fingerprint,
                "store_fingerprint":current_store_fingerprint,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management compaction source fingerprint failed",
            )
        })?;
        let before_bytes = fs::metadata(&path).map_err(map_io)?.len();
        if current_store_fingerprint != plan.store_fingerprint
            || current_source_fingerprint != plan.source_fingerprint
            || before_bytes != plan.database_bytes
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "management compaction source fingerprint is stale",
            ));
        }
        if available_disk_bytes(&path)? < plan.required_free_bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "management compaction has insufficient free space",
            ));
        }
        match &plan.store_scope {
            StoreScope::Global => {
                let connection = self.global.connection.lock().map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Unavailable,
                        "global store lock is unavailable",
                    )
                })?;
                connection
                    .execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
                    .map_err(map_sql)?;
                verify_connection(&connection)?;
            }
            StoreScope::Project { project_id } => {
                let repository = self.project_repository(project_id)?;
                let connection = repository.connection.lock().map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Unavailable,
                        "project store lock is unavailable",
                    )
                })?;
                connection
                    .execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
                    .map_err(map_sql)?;
                verify_connection(&connection)?;
            }
        }
        let after_bytes = fs::metadata(&path).map_err(map_io)?.len();
        Ok(ManagementCompactionApplyResultV1 {
            plan_fingerprint: plan.plan_fingerprint.clone(),
            before_bytes,
            after_bytes,
            completed_at: Utc::now(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreFaultPoint {
    AfterFirstStore,
    BeforeActivation,
    AfterActivation,
}

impl ManagementRecovery for SqliteManagementRecovery {
    fn status(&self) -> Result<RecoveryStatus, RepositoryError> {
        let mut stores = Vec::new();
        let active_set = match read_active_set(&self.root) {
            Ok(Some(parsed)) => Some(parsed.manifest),
            Ok(None) => None,
            Err(_) => {
                stores.push(StoreRecoveryStatus {
                    scope: None,
                    relative_locator: None,
                    inspection: RecoveryInspection::ActiveSetMismatch,
                    diagnostic_code: "RECOVERY_ACTIVE_SET_INVALID".to_owned(),
                });
                None
            }
        };
        if let Some(manifest) = &active_set {
            for entry in &manifest.entries {
                let inspection = inspect_store_read_only(&active_store_file(&self.root, entry));
                stores.push(StoreRecoveryStatus {
                    scope: Some(entry.scope.clone()),
                    relative_locator: Some(entry.relative_locator.clone()),
                    inspection,
                    diagnostic_code: recovery_diagnostic_code(inspection).to_owned(),
                });
            }
            if validate_active_set_materialization(&self.root, manifest).is_err() {
                stores.push(StoreRecoveryStatus {
                    scope: None,
                    relative_locator: None,
                    inspection: RecoveryInspection::ActiveSetMismatch,
                    diagnostic_code: "RECOVERY_ACTIVE_SET_MATERIALIZATION_MISMATCH".to_owned(),
                });
            }
        } else if stores.is_empty() {
            stores.push(StoreRecoveryStatus {
                scope: None,
                relative_locator: None,
                inspection: RecoveryInspection::Missing,
                diagnostic_code: "RECOVERY_ACTIVE_SET_MISSING".to_owned(),
            });
        }
        let healthy = active_set.is_some()
            && stores
                .iter()
                .all(|store| store.inspection == RecoveryInspection::Healthy);
        let mode = if healthy {
            ControllerRecoveryMode::Normal
        } else {
            ControllerRecoveryMode::RecoveryOnly
        };
        let allowed_operations = vec![
            RecoveryOperation::Status,
            RecoveryOperation::RestorePlan,
            RecoveryOperation::RestoreApply,
            RecoveryOperation::RebuildPlan,
            RecoveryOperation::RebuildApply,
            RecoveryOperation::LocalStateExportPlan,
            RecoveryOperation::LocalStateExportApply,
        ];
        let status_fingerprint = versioned_fingerprint(
            "star.management-recovery-status",
            1,
            &serde_json::json!({
                "mode":mode,
                "active_set":active_set,
                "stores":stores,
                "allowed_operations":allowed_operations,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "recovery status fingerprint failed",
            )
        })?;
        Ok(RecoveryStatus {
            schema_id: RECOVERY_STATUS_SCHEMA_ID.to_owned(),
            schema_version: 1,
            mode,
            active_set,
            stores,
            allowed_operations,
            status_fingerprint,
        })
    }

    fn plan_restore(&self, backup_root: &Path) -> Result<RestorePlan, RepositoryError> {
        let backup = read_verified_backup_set(backup_root)?;
        let current = read_active_set(&self.root)
            .ok()
            .flatten()
            .map(|parsed| parsed.manifest);
        let recovery_plan_id = RecoveryPlanId::new();
        let mut stores = Vec::with_capacity(backup.entries.len());
        let mut candidate_entries = Vec::with_capacity(backup.entries.len());
        for source in &backup.entries {
            let current_generation = current
                .as_ref()
                .and_then(|manifest| {
                    manifest
                        .entries
                        .iter()
                        .find(|entry| entry.scope == source.scope)
                        .map(|entry| entry.generation)
                })
                .unwrap_or(0);
            let candidate_generation = source
                .generation
                .max(current_generation)
                .checked_add(1)
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "restore generation overflowed",
                    )
                })?;
            let candidate_relative_locator =
                restore_candidate_locator(&recovery_plan_id, &source.scope);
            stores.push(RestoreStoreTarget {
                scope: source.scope.clone(),
                store_id: source.store_id.clone(),
                source_generation: source.generation,
                candidate_generation,
                management_store_version: source.management_store_version,
                source_byte_sha256: source.byte_sha256.clone(),
                candidate_relative_locator: candidate_relative_locator.clone(),
            });
            candidate_entries.push(ActiveStoreGeneration {
                scope: source.scope.clone(),
                store_id: source.store_id.clone(),
                generation: candidate_generation,
                management_store_version: source.management_store_version,
                relative_locator: candidate_relative_locator,
                header_fingerprint: Sha256Hash::digest(b"unsealed-restore-candidate"),
            });
        }
        let candidate_active_set = seal_active_set(candidate_entries).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "restore candidate active set could not be sealed",
            )
        })?;
        let created_at = Utc::now();
        let mut plan = RestorePlan {
            schema_id: RESTORE_PLAN_SCHEMA_ID.to_owned(),
            schema_version: 1,
            recovery_plan_id,
            backup_set_id: backup.backup_set_id.clone(),
            created_at,
            backup_set_fingerprint: backup.set_fingerprint.clone(),
            expected_active_set_fingerprint: active_set_file_fingerprint(&self.root)?,
            stores,
            candidate_active_set,
            plan_fingerprint: Sha256Hash::digest(b"unsealed-restore-plan"),
        };
        plan.plan_fingerprint = restore_plan_fingerprint(&plan).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "restore plan fingerprint failed",
            )
        })?;
        validate_restore_plan(&plan, &backup).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "restore plan is inconsistent with the backup set",
            )
        })?;
        Ok(plan)
    }

    fn apply_restore(
        &self,
        backup_root: &Path,
        plan: &RestorePlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RestoreApplyResult, RepositoryError> {
        require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "restore approval fingerprint is stale",
                )
            },
        )?;
        if let Some(completed) = read_recovery_receipt::<RestoreApplyResult>(
            &self.root,
            "restore",
            &plan.plan_fingerprint,
        )? {
            let expected = restore_apply_result(plan, completed.applied_at)?;
            if completed != expected {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "completed restore receipt is invalid",
                ));
            }
            return Ok(completed);
        }
        if read_active_set(&self.root)
            .ok()
            .flatten()
            .is_some_and(|active| active.manifest == plan.candidate_active_set)
            && validate_active_set_materialization(&self.root, &plan.candidate_active_set).is_ok()
        {
            let result = restore_apply_result(plan, Utc::now())?;
            write_recovery_receipt(&self.root, "restore", &plan.plan_fingerprint, &result)?;
            return Ok(result);
        }
        let result =
            self.apply_restore_with_fault(backup_root, plan, approved_plan_fingerprint, None)?;
        write_recovery_receipt(&self.root, "restore", &plan.plan_fingerprint, &result)?;
        Ok(result)
    }

    fn plan_rebuild(
        &self,
        projects: Vec<RebuildProjectInput>,
        predicted_losses: Vec<RecoveryLossItem>,
    ) -> Result<RebuildPlan, RepositoryError> {
        seal_rebuild_plan(
            RecoveryPlanId::new(),
            Utc::now(),
            active_set_file_fingerprint(&self.root)?,
            next_rebuild_generation(&self.root)?,
            projects,
            predicted_losses,
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "source rebuild plan could not be sealed",
            )
        })
    }

    fn completed_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<Option<RebuildApplyResult>, RepositoryError> {
        require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "source rebuild approval fingerprint is stale",
                )
            },
        )?;
        validate_rebuild_plan(plan).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "source rebuild plan is invalid",
            )
        })?;
        let Some(completed) = read_recovery_receipt::<RebuildApplyResult>(
            &self.root,
            "rebuild",
            &plan.plan_fingerprint,
        )?
        else {
            if let Some(reconciled) = reconcile_rebuild_completion(&self.root, plan)? {
                write_recovery_receipt(&self.root, "rebuild", &plan.plan_fingerprint, &reconciled)?;
                return Ok(Some(reconciled));
            }
            return Ok(None);
        };
        let expected = rebuild_apply_result(
            plan,
            completed.rebuilt_projects.clone(),
            completed.activated_set.clone(),
            completed.applied_at,
        )?;
        if completed != expected {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "completed source rebuild receipt is invalid",
            ));
        }
        Ok(Some(completed))
    }

    fn begin_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<Arc<dyn ManagementRepositorySet>, RepositoryError> {
        validate_rebuild_precondition(&self.root, plan, approved_plan_fingerprint)?;
        for project in &plan.projects {
            let destination = self.root.join(rebuild_target_locator(
                plan.candidate_generation,
                &StoreScope::Project {
                    project_id: project.project_id.clone(),
                },
            ));
            if destination.exists() {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "source rebuild candidate generation already exists",
                ));
            }
        }
        if self
            .root
            .join(rebuild_target_locator(
                plan.candidate_generation,
                &StoreScope::Global,
            ))
            .exists()
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "source rebuild candidate generation already exists",
            ));
        }
        let candidate_root = rebuild_candidate_root(&self.root, &plan.recovery_plan_id);
        if candidate_root.exists() {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "source rebuild staging root already exists",
            ));
        }
        Ok(Arc::new(SqliteManagementRepositorySet::open(
            candidate_root,
            &self.product_version,
        )?))
    }

    fn apply_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
        rebuilt_projects: Vec<RebuiltProjectSummary>,
    ) -> Result<RebuildApplyResult, RepositoryError> {
        let result = self.apply_rebuild_inner(plan, approved_plan_fingerprint, rebuilt_projects);
        if result.is_err() && rebuild_destination_exists(&self.root, plan) {
            let _ = write_rebuild_quarantine(&self.root, plan, "REBUILD_CANDIDATE_NOT_CONFIRMED");
        }
        let result = result?;
        write_recovery_receipt(&self.root, "rebuild", &plan.plan_fingerprint, &result)?;
        Ok(result)
    }

    fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, RepositoryError> {
        let repository = recovery_project_repository(&self.root, project_id)?;
        plan_local_state_export_for_repository(repository.as_ref(), project_id, destination)
    }

    fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, RepositoryError> {
        let repository = recovery_project_repository(&self.root, &plan.project_id)?;
        apply_local_state_export_for_repository(
            &self.root,
            repository.as_ref(),
            destination,
            plan,
            approved_plan_fingerprint,
        )
    }
}

impl SqliteManagementRecovery {
    fn apply_restore_with_fault(
        &self,
        backup_root: &Path,
        plan: &RestorePlan,
        approved_plan_fingerprint: &str,
        fault: Option<RestoreFaultPoint>,
    ) -> Result<RestoreApplyResult, RepositoryError> {
        let result = self.apply_restore_inner(backup_root, plan, approved_plan_fingerprint, fault);
        if result.is_err()
            && plan.stores.iter().any(|store| {
                self.root
                    .join(&store.candidate_relative_locator)
                    .join(STORE_FILENAME)
                    .exists()
            })
        {
            let activated = read_active_set(&self.root)
                .ok()
                .flatten()
                .is_some_and(|active| {
                    active.manifest.manifest_fingerprint
                        == plan.candidate_active_set.manifest_fingerprint
                });
            let reason = if activated {
                "RESTORE_ACTIVATED_OUTCOME_REQUIRES_RECONCILE"
            } else {
                "RESTORE_CANDIDATE_NOT_ACTIVATED"
            };
            let _ = write_restore_quarantine(&self.root, plan, reason);
        }
        result
    }

    fn apply_restore_inner(
        &self,
        backup_root: &Path,
        plan: &RestorePlan,
        approved_plan_fingerprint: &str,
        fault: Option<RestoreFaultPoint>,
    ) -> Result<RestoreApplyResult, RepositoryError> {
        require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "restore approval fingerprint is stale",
                )
            },
        )?;
        let backup = read_verified_backup_set(backup_root)?;
        validate_restore_plan(plan, &backup).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "restore plan no longer matches the verified backup set",
            )
        })?;
        if active_set_file_fingerprint(&self.root)? != plan.expected_active_set_fingerprint {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "active set changed after restore planning",
            ));
        }
        for target in &plan.stores {
            let destination = self
                .root
                .join(&target.candidate_relative_locator)
                .join(STORE_FILENAME);
            if destination.exists() {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "restore candidate generation already exists",
                ));
            }
        }

        for (index, (target, source)) in plan.stores.iter().zip(&backup.entries).enumerate() {
            let source_path = backup_root.join(&source.relative_locator);
            let destination = self
                .root
                .join(&target.candidate_relative_locator)
                .join(STORE_FILENAME);
            copy_restore_candidate(&source_path, &destination, target, &self.product_version)?;
            if index == 0 && fault == Some(RestoreFaultPoint::AfterFirstStore) {
                return Err(repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "simulated restore interruption after the first store",
                ));
            }
        }
        validate_active_set_materialization(&self.root, &plan.candidate_active_set)?;
        if fault == Some(RestoreFaultPoint::BeforeActivation) {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "simulated restore interruption before activation",
            ));
        }
        write_active_set_document(&self.root, &plan.candidate_active_set)?;
        if fault == Some(RestoreFaultPoint::AfterActivation) {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "simulated restore interruption after activation",
            ));
        }
        restore_apply_result(plan, Utc::now())
    }

    fn apply_rebuild_inner(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
        mut rebuilt_projects: Vec<RebuiltProjectSummary>,
    ) -> Result<RebuildApplyResult, RepositoryError> {
        validate_rebuild_precondition(&self.root, plan, approved_plan_fingerprint)?;
        rebuilt_projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        let planned_projects = plan
            .projects
            .iter()
            .map(|project| &project.project_id)
            .collect::<Vec<_>>();
        let rebuilt_ids = rebuilt_projects
            .iter()
            .map(|project| &project.project_id)
            .collect::<Vec<_>>();
        if planned_projects != rebuilt_ids
            || rebuilt_projects.iter().any(|project| {
                project.scan_run_id.as_str().is_empty()
                    || project.workspace_snapshot_id.as_str().is_empty()
            })
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "source rebuild result does not cover the exact planned project set",
            ));
        }

        let candidate_root = rebuild_candidate_root(&self.root, &plan.recovery_plan_id);
        let candidate = SqliteManagementRepositorySet::open(&candidate_root, &self.product_version)
            .map_err(|error| {
                repository_error(
                    error.category,
                    "source rebuild staging repository could not be reopened",
                )
            })?;
        let _ = candidate.verify_all().map_err(|error| {
            repository_error(
                error.category,
                "source rebuild staging repository failed verification",
            )
        })?;
        let candidate_active_set = candidate.active_set().map_err(|error| {
            repository_error(
                error.category,
                "source rebuild staging active set is unavailable",
            )
        })?;
        let candidate_projects = candidate_active_set
            .entries
            .iter()
            .filter_map(|entry| match &entry.scope {
                StoreScope::Global => None,
                StoreScope::Project { project_id } => Some(project_id),
            })
            .collect::<Vec<_>>();
        if candidate_projects != planned_projects {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "source rebuild staging stores do not match the planned project set",
            ));
        }
        drop(candidate);

        for entry in &candidate_active_set.entries {
            let destination = self
                .root
                .join(rebuild_target_locator(
                    plan.candidate_generation,
                    &entry.scope,
                ))
                .join(STORE_FILENAME);
            if destination.exists() {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "source rebuild destination changed after planning",
                ));
            }
        }

        let mut activated_entries = Vec::with_capacity(candidate_active_set.entries.len());
        for entry in &candidate_active_set.entries {
            let source = active_store_file(&candidate_root, entry);
            let relative_locator = rebuild_target_locator(plan.candidate_generation, &entry.scope);
            let destination = self.root.join(&relative_locator).join(STORE_FILENAME);
            let status = copy_rebuild_candidate(
                &source,
                &destination,
                &entry.scope,
                plan.candidate_generation,
                &self.product_version,
                &plan.plan_fingerprint,
            )
            .map_err(|error| {
                repository_error(
                    error.category,
                    "source rebuild staging store could not be materialized",
                )
            })?;
            activated_entries.push(ActiveStoreGeneration {
                scope: status.store_scope,
                store_id: status.store_id,
                generation: status.generation,
                management_store_version: status.management_store_version,
                relative_locator,
                header_fingerprint: Sha256Hash::digest(b"unsealed"),
            });
        }
        let activated_set = seal_active_set(activated_entries).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "source rebuild active set could not be sealed",
            )
        })?;
        validate_active_set_materialization(&self.root, &activated_set).map_err(|error| {
            repository_error(
                error.category,
                "source rebuild candidate store set failed relationship verification",
            )
        })?;
        write_active_set_document(&self.root, &activated_set).map_err(|error| {
            repository_error(
                error.category,
                "source rebuild active set activation failed",
            )
        })?;

        rebuild_apply_result(plan, rebuilt_projects, activated_set, Utc::now())
    }
}

fn recovery_project_repository(
    root: &Path,
    project_id: &ProjectId,
) -> Result<Arc<SqliteProjectRepository>, RepositoryError> {
    let active_set = read_active_set(root)?
        .map(|parsed| parsed.manifest)
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::NotFound,
                "recovery-only local state export has no active set",
            )
        })?;
    validate_active_set(&active_set).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery-only local state export active set is invalid",
        )
    })?;
    let entry = active_set
        .entries
        .iter()
        .find(|entry| {
            matches!(
                &entry.scope,
                StoreScope::Project {
                    project_id: active_project_id
                } if active_project_id == project_id
            )
        })
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::NotFound,
                "recovery-only local state project store is not active",
            )
        })?;
    let path = active_store_file(root, entry);
    if inspect_store_read_only(&path) != RecoveryInspection::Healthy {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "recovery-only local state project store is unhealthy",
        ));
    }
    Ok(Arc::new(SqliteProjectRepository::open_read_only(
        &path, project_id,
    )?))
}

fn recovery_diagnostic_code(inspection: RecoveryInspection) -> &'static str {
    match inspection {
        RecoveryInspection::Missing => "RECOVERY_STORE_MISSING",
        RecoveryInspection::Healthy => "RECOVERY_STORE_HEALTHY",
        RecoveryInspection::MigrationRequired => "RECOVERY_STORE_MIGRATION_REQUIRED",
        RecoveryInspection::FutureVersion => "RECOVERY_STORE_FUTURE_VERSION",
        RecoveryInspection::Corrupt => "RECOVERY_STORE_CORRUPT",
        RecoveryInspection::ActiveSetMismatch => "RECOVERY_ACTIVE_SET_MISMATCH",
    }
}

fn read_verified_backup_set(root: &Path) -> Result<BackupSetManifest, RepositoryError> {
    let input = fs::read_to_string(root.join("backup-set.json")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            repository_error(
                RepositoryErrorCategory::NotFound,
                "backup set manifest is missing",
            )
        } else {
            map_io(error)
        }
    })?;
    let manifest =
        star_contracts::management::decode_current_management_document::<BackupSetManifest>(
            &input,
            star_contracts::recovery::BACKUP_SET_MANIFEST_SCHEMA_ID,
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "backup set manifest is invalid or unsupported",
            )
        })?;
    validate_backup_set_relationships(root, &manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "backup set bytes, headers, or project relationships are invalid",
        )
    })?;
    Ok(manifest)
}

fn verified_compaction_backup_binding(
    management_root: &Path,
    backup_root: &Path,
    result: &BackupApplyResult,
    status: &ManagementStoreStatus,
    active_set: &ActiveSetManifest,
) -> Result<ManagementCompactionBackupBindingV1, RepositoryError> {
    let expected_result_fingerprint = backup_apply_result_fingerprint(
        &result.backup_set_id,
        result.applied_at,
        &result.approved_plan_fingerprint,
        &result.manifest,
    )?;
    if result.schema_id != BACKUP_APPLY_RESULT_SCHEMA_ID
        || result.schema_version != 1
        || result.backup_set_id != result.manifest.backup_set_id
        || result.result_fingerprint != expected_result_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "management compaction backup result is invalid",
        ));
    }
    let completed = read_recovery_receipt::<BackupApplyResult>(
        management_root,
        "backup",
        &result.approved_plan_fingerprint,
    )?
    .ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::NotFound,
            "management compaction backup receipt is missing",
        )
    })?;
    let manifest = read_verified_backup_set(backup_root)?;
    if completed != *result
        || manifest != result.manifest
        || manifest.source_active_set_fingerprint != active_set.manifest_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "management compaction backup is not bound to the current active set",
        ));
    }
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.store_id == status.store_id && entry.scope == status.store_scope)
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::NotFound,
                "management compaction backup omits the target store",
            )
        })?;
    if entry.generation != status.generation
        || entry.management_store_version != status.management_store_version
        || entry.store_revision != status.store_revision
    {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "management compaction backup target is stale",
        ));
    }
    Ok(ManagementCompactionBackupBindingV1 {
        backup_set_id: result.backup_set_id.clone(),
        approved_backup_plan_fingerprint: result.approved_plan_fingerprint.clone(),
        backup_result_fingerprint: result.result_fingerprint.clone(),
        backup_set_fingerprint: manifest.set_fingerprint,
        backup_destination_fingerprint: backup_destination_fingerprint(
            management_root,
            backup_root,
        )?,
        source_active_set_fingerprint: manifest.source_active_set_fingerprint,
        store_id: entry.store_id.clone(),
        store_revision: entry.store_revision,
        store_size_bytes: entry.size_bytes,
        store_byte_sha256: entry.byte_sha256.clone(),
    })
}

fn active_set_file_fingerprint(root: &Path) -> Result<Option<Sha256Hash>, RepositoryError> {
    let path = root.join(ACTIVE_SET_FILENAME);
    match fs::File::open(path) {
        Ok(file) => Sha256Hash::digest_reader(file).map(Some).map_err(map_io),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(map_io(error)),
    }
}

fn restore_candidate_locator(plan_id: &RecoveryPlanId, scope: &StoreScope) -> String {
    match scope {
        StoreScope::Global => format!("global/generations/{}", plan_id.as_str()),
        StoreScope::Project { project_id } => format!(
            "projects/{}/generations/{}",
            project_id.as_str(),
            plan_id.as_str()
        ),
    }
}

fn rebuild_candidate_root(root: &Path, plan_id: &RecoveryPlanId) -> PathBuf {
    let token = Sha256Hash::digest(plan_id.as_str().as_bytes());
    root.join("rc")
        .join(&token.as_str().trim_start_matches("sha256:")[..20])
}

fn rebuild_target_locator(generation: u64, scope: &StoreScope) -> String {
    match scope {
        StoreScope::Global => format!("global/generations/{generation:020}"),
        StoreScope::Project { project_id } => format!(
            "projects/{}/generations/{generation:020}",
            project_id.as_str()
        ),
    }
}

fn validate_rebuild_precondition(
    root: &Path,
    plan: &RebuildPlan,
    approved_plan_fingerprint: &str,
) -> Result<(), RepositoryError> {
    require_exact_approval(&plan.plan_fingerprint, approved_plan_fingerprint).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "source rebuild approval fingerprint is stale",
        )
    })?;
    validate_rebuild_plan(plan).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "source rebuild plan is invalid",
        )
    })?;
    if active_set_file_fingerprint(root)? != plan.expected_active_set_fingerprint {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "active set changed after source rebuild planning",
        ));
    }
    Ok(())
}

fn next_rebuild_generation(root: &Path) -> Result<u64, RepositoryError> {
    let mut maximum = read_active_set(root)
        .ok()
        .flatten()
        .and_then(|active| {
            active
                .manifest
                .entries
                .iter()
                .map(|entry| entry.generation)
                .max()
        })
        .unwrap_or(1);
    inspect_generation_directory(&root.join("global").join("generations"), &mut maximum)?;
    let projects_root = root.join("projects");
    if projects_root.is_dir() {
        for entry in fs::read_dir(&projects_root).map_err(map_io)? {
            let entry = entry.map_err(map_io)?;
            if entry.file_type().map_err(map_io)?.is_dir() {
                inspect_generation_directory(&entry.path().join("generations"), &mut maximum)?;
            }
        }
    }
    maximum.checked_add(1).ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "source rebuild generation space is exhausted",
        )
    })
}

fn inspect_generation_directory(
    generations: &Path,
    maximum: &mut u64,
) -> Result<(), RepositoryError> {
    if !generations.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(generations).map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        if !entry.file_type().map_err(map_io)?.is_dir() {
            continue;
        }
        if let Some(value) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse().ok())
        {
            *maximum = (*maximum).max(value);
        }
        let store = entry.path().join(STORE_FILENAME);
        if store.is_file()
            && let Ok(status) = read_store_status_read_only(&store)
        {
            *maximum = (*maximum).max(status.generation);
        }
    }
    Ok(())
}

fn rebuild_destination_exists(root: &Path, plan: &RebuildPlan) -> bool {
    std::iter::once(StoreScope::Global)
        .chain(plan.projects.iter().map(|project| StoreScope::Project {
            project_id: project.project_id.clone(),
        }))
        .any(|scope| {
            root.join(rebuild_target_locator(plan.candidate_generation, &scope))
                .join(STORE_FILENAME)
                .exists()
        })
}

fn reconcile_rebuild_completion(
    root: &Path,
    plan: &RebuildPlan,
) -> Result<Option<RebuildApplyResult>, RepositoryError> {
    let Some(active_set) = read_active_set(root)
        .ok()
        .flatten()
        .map(|active| active.manifest)
    else {
        return Ok(None);
    };
    let active_projects = active_set
        .entries
        .iter()
        .filter_map(|entry| match &entry.scope {
            StoreScope::Global => None,
            StoreScope::Project { project_id } => Some(project_id),
        })
        .collect::<Vec<_>>();
    let planned_projects = plan
        .projects
        .iter()
        .map(|project| &project.project_id)
        .collect::<Vec<_>>();
    if active_set.entries.len() != plan.projects.len() + 1
        || active_projects != planned_projects
        || active_set
            .entries
            .iter()
            .any(|entry| entry.generation != plan.candidate_generation)
    {
        return Ok(None);
    }
    validate_active_set_materialization(root, &active_set)?;
    for entry in &active_set.entries {
        let connection = Connection::open_with_flags(
            active_store_file(root, entry),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        connection
            .execute_batch("PRAGMA query_only=ON;")
            .map_err(map_sql)?;
        if get_meta_optional(&connection, "rebuild_plan_fingerprint")
            .map_err(map_sql)?
            .as_deref()
            != Some(plan.plan_fingerprint.as_str())
        {
            return Ok(None);
        }
    }

    let mut rebuilt_projects = Vec::with_capacity(plan.projects.len());
    for input in &plan.projects {
        let repository = recovery_project_repository(root, &input.project_id)?;
        let scan = repository.latest_scan()?.ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "activated source rebuild store has no completed scan",
            )
        })?;
        if scan.status != ScanStatus::Succeeded
            || scan.project_revision_id != input.source_revision_id
            || scan.effective_config_fingerprint != input.effective_config_fingerprint
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "activated source rebuild scan does not match the approved input",
            ));
        }
        let artifact_count =
            u64::try_from(repository.list_artifact_refs()?.len()).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "activated source rebuild artifact count exceeds its supported range",
                )
            })?;
        if artifact_count < input.verified_artifact_count {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "activated source rebuild is missing verified artifact references",
            ));
        }
        rebuilt_projects.push(RebuiltProjectSummary {
            project_id: input.project_id.clone(),
            project_revision_id: scan.project_revision_id,
            workspace_snapshot_id: scan.workspace_snapshot_id,
            scan_run_id: scan.scan_run_id,
            canonical_source_count: scan.counts.get("source").copied().unwrap_or(0),
            symbol_count: scan.counts.get("symbol").copied().unwrap_or(0),
            finding_count: u64::try_from(repository.list_findings()?.len()).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "activated source rebuild finding count exceeds its supported range",
                )
            })?,
            reindexed_artifact_count: input.verified_artifact_count,
            rejected_artifact_count: input.rejected_artifact_count,
        });
    }
    rebuild_apply_result(plan, rebuilt_projects, active_set, Utc::now()).map(Some)
}

fn copy_rebuild_candidate(
    source: &Path,
    destination: &Path,
    expected_scope: &StoreScope,
    candidate_generation: u64,
    product_version: &str,
    plan_fingerprint: &Sha256Hash,
) -> Result<ManagementStoreStatus, RepositoryError> {
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    source_connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    verify_connection(&source_connection)?;
    let before = status_from_connection(&source_connection)?;
    if &before.store_scope != expected_scope
        || before.management_store_version != MANAGEMENT_STORE_VERSION
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "source rebuild staging store header is invalid",
        ));
    }
    backup_connection(&source_connection, destination)?;
    drop(source_connection);

    let mut destination_connection = Connection::open(destination).map_err(map_sql)?;
    verify_connection(&destination_connection)?;
    let transaction = destination_connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql)?;
    set_meta(
        &transaction,
        "generation",
        &candidate_generation.to_string(),
    )
    .map_err(map_sql)?;
    set_meta(
        &transaction,
        "last_opened_by_product_version",
        product_version,
    )
    .map_err(map_sql)?;
    set_meta(
        &transaction,
        "rebuild_plan_fingerprint",
        plan_fingerprint.as_str(),
    )
    .map_err(map_sql)?;
    set_meta(&transaction, "last_clean_shutdown", "true").map_err(map_sql)?;
    transaction.commit().map_err(map_sql)?;
    destination_connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA synchronous=FULL;")
        .map_err(map_sql)?;
    verify_connection(&destination_connection)?;
    let after = status_from_connection(&destination_connection)?;
    if after.store_scope != *expected_scope
        || after.store_id != before.store_id
        || after.generation != candidate_generation
        || after.store_revision != before.store_revision
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "source rebuild candidate store header is invalid",
        ));
    }
    drop(destination_connection);
    apply_owner_system_dacl(destination)?;
    if inspect_store_read_only(destination) != RecoveryInspection::Healthy {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "source rebuild candidate failed final read-only inspection",
        ));
    }
    Ok(after)
}

fn write_rebuild_quarantine(
    root: &Path,
    plan: &RebuildPlan,
    reason_code: &str,
) -> Result<(), RepositoryError> {
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_id":"star.management-rebuild-quarantine",
        "schema_version":1,
        "recovery_plan_id":plan.recovery_plan_id,
        "candidate_generation":plan.candidate_generation,
        "reason_code":reason_code,
        "recorded_at":Utc::now(),
        "plan_fingerprint":plan.plan_fingerprint,
    }))
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "source rebuild quarantine serialization failed",
        )
    })?;
    write_private_atomic(
        &root
            .join("quarantine")
            .join(format!("{}.json", plan.recovery_plan_id.as_str())),
        &bytes,
    )
}

fn copy_restore_candidate(
    source: &Path,
    destination: &Path,
    target: &RestoreStoreTarget,
    product_version: &str,
) -> Result<(), RepositoryError> {
    let source_sha256 =
        Sha256Hash::digest_reader(fs::File::open(source).map_err(map_io)?).map_err(map_io)?;
    if source_sha256 != target.source_byte_sha256
        || inspect_store_read_only(source) != RecoveryInspection::Healthy
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restore source bytes do not match the verified backup",
        ));
    }
    if let Some(parent) = destination.parent() {
        create_private_dir(parent)?;
    }
    fs::copy(source, destination).map_err(map_io)?;
    let copied_sha256 =
        Sha256Hash::digest_reader(fs::File::open(destination).map_err(map_io)?).map_err(map_io)?;
    if copied_sha256 != source_sha256 {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restore candidate copy hash does not match the backup",
        ));
    }
    let mut connection = Connection::open(destination).map_err(map_sql)?;
    verify_connection(&connection)?;
    let before = status_from_connection(&connection)?;
    if before.store_scope != target.scope
        || before.store_id != target.store_id
        || before.generation != target.source_generation
        || before.management_store_version != target.management_store_version
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restore source store header does not match the plan",
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql)?;
    set_meta(
        &transaction,
        "generation",
        &target.candidate_generation.to_string(),
    )
    .map_err(map_sql)?;
    set_meta(
        &transaction,
        "last_opened_by_product_version",
        product_version,
    )
    .map_err(map_sql)?;
    set_meta(&transaction, "last_clean_shutdown", "true").map_err(map_sql)?;
    transaction.commit().map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA synchronous=FULL;")
        .map_err(map_sql)?;
    verify_connection(&connection)?;
    let after = status_from_connection(&connection)?;
    if after.store_scope != target.scope
        || after.store_id != target.store_id
        || after.generation != target.candidate_generation
        || after.management_store_version != target.management_store_version
        || after.store_revision != before.store_revision
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restore candidate store header is invalid",
        ));
    }
    drop(connection);
    apply_owner_system_dacl(destination)?;
    if inspect_store_read_only(destination) != RecoveryInspection::Healthy {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "restore candidate failed final read-only inspection",
        ));
    }
    Ok(())
}

fn write_restore_quarantine(
    root: &Path,
    plan: &RestorePlan,
    reason_code: &str,
) -> Result<(), RepositoryError> {
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_id":"star.management-restore-quarantine",
        "schema_version":1,
        "recovery_plan_id":plan.recovery_plan_id,
        "backup_set_id":plan.backup_set_id,
        "reason_code":reason_code,
        "recorded_at":Utc::now(),
        "candidate_locators":plan.stores.iter().map(|store| &store.candidate_relative_locator).collect::<Vec<_>>(),
        "plan_fingerprint":plan.plan_fingerprint,
    }))
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "restore quarantine serialization failed",
        )
    })?;
    write_private_atomic(
        &root
            .join("quarantine")
            .join(format!("{}.json", plan.recovery_plan_id.as_str())),
        &bytes,
    )
}

struct SqliteGlobalRepository {
    connection: Mutex<Connection>,
}

impl SqliteGlobalRepository {
    fn open(path: &Path, product_version: &str) -> Result<Self, RepositoryError> {
        let connection = open_store(path, StoreScope::Global, product_version, GLOBAL_SCHEMA)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn verify_integrity(&self) -> Result<ManagementStoreStatus, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        verify_connection(&connection)?;
        set_meta(&connection, "last_verified_at", &Utc::now().to_rfc3339()).map_err(map_sql)?;
        status_from_connection(&connection)
    }

    fn backup(&self, destination: &Path) -> Result<(), RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        backup_connection(&connection, destination)
    }
}

fn analyze_project_retention_v2(
    connection: &Connection,
    project_id: &ProjectId,
    policy: &RetentionPolicyV2,
    created_at: DateTime<Utc>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(Vec<RetentionCandidateV2>, Vec<RetentionProtectedTargetV1>), RepositoryError> {
    policy.validate().map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "retention policy is invalid",
        )
    })?;
    let current_generation = get_meta_optional(connection, "current_generation")
        .map_err(map_sql)?
        .unwrap_or_default();
    let (protected_snapshots, unscoped_protection) = protected_snapshot_reasons(connection)?;
    let active_suppressions_v2 = query_documents::<SuppressionV2, _>(
        connection,
        "SELECT document_json FROM suppressions_v2",
        [],
    )?
    .into_iter()
    .filter(|suppression| suppression.status == SuppressionStatus::Active)
    .collect::<Vec<_>>();
    let hold_artifact_ids = query_documents::<ArtifactRef, _>(
        connection,
        "SELECT document_json FROM artifact_refs",
        [],
    )?
    .into_iter()
    .filter(|artifact| artifact.retention_class == EvidenceRetentionClass::Hold)
    .map(|artifact| artifact.artifact_id.as_str().to_owned())
    .collect::<BTreeSet<_>>();
    let hold_finding_ids =
        query_documents::<Occurrence, _>(connection, "SELECT document_json FROM occurrences", [])?
            .into_iter()
            .filter(|occurrence| {
                occurrence.evidence_refs.iter().any(|artifact| {
                    artifact.retention_class == EvidenceRetentionClass::Hold
                        || hold_artifact_ids.contains(artifact.artifact_id.as_str())
                })
            })
            .map(|occurrence| occurrence.finding_id.as_str().to_owned())
            .collect::<BTreeSet<_>>();

    let incomplete_cutoff = created_at
        - chrono::Duration::days(
            policy
                .incomplete_staging_retention_days
                .min(i64::MAX as u64) as i64,
        );
    let scan_detail_cutoff = created_at
        - chrono::Duration::days(policy.scan_detail_retention_days.min(i64::MAX as u64) as i64);
    let resolved_cutoff = created_at
        - chrono::Duration::days(policy.resolved_finding_retention_days.min(i64::MAX as u64) as i64);
    let local_decision_cutoff = created_at
        - chrono::Duration::days(policy.local_decision_retention_days.min(i64::MAX as u64) as i64);

    let scan_documents: Vec<(String, String)> = {
        let mut statement = connection
            .prepare("SELECT generation_id, document_json FROM scan_runs ORDER BY generation_id")
            .map_err(map_sql)?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(map_sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sql)?
    };
    let mut runs = Vec::with_capacity(scan_documents.len());
    let mut run_locations = BTreeMap::new();
    for (generation_id, document) in scan_documents {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        let run: ScanRun = serde_json::from_str(&document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored ScanRun is invalid",
            )
        })?;
        let finished_at = run.finished_at.unwrap_or(run.started_at);
        run_locations.insert(
            run.scan_run_id.as_str().to_owned(),
            (
                generation_id.clone(),
                finished_at,
                run.workspace_snapshot_id.clone(),
            ),
        );
        runs.push((generation_id, finished_at, run));
    }
    let mut generation_inventory = None;

    let mut candidates = Vec::new();
    let mut protected_targets = Vec::new();
    let mut generation_candidates = BTreeSet::new();
    let embedded_artifact_hold = |run: &ScanRun| {
        run.artifact_refs.iter().any(|artifact| {
            artifact.retention_class == EvidenceRetentionClass::Hold
                || hold_artifact_ids.contains(artifact.artifact_id.as_str())
        })
    };

    for (generation_id, finished_at, run) in runs
        .iter()
        .filter(|(_, _, run)| run.status != ScanStatus::Succeeded)
    {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        if *finished_at >= incomplete_cutoff {
            continue;
        }
        let aggregate = retention_generation_aggregate_for_plan(
            connection,
            is_cancelled,
            &mut generation_inventory,
            generation_id,
        )?;
        let estimated_rows = aggregate.rows;
        let estimated_bytes = aggregate.bytes;
        let max_document_bytes = aggregate.max_document_bytes;
        let reason_code = "INCOMPLETE_STAGING_EXPIRED";
        let candidate = RetentionCandidateV2 {
            candidate_id: retention_candidate_id(
                project_id,
                RetentionTargetKindV2::ScanGeneration,
                Some(generation_id),
                None,
                None,
                reason_code,
            )?,
            project_id: project_id.clone(),
            target_kind: RetentionTargetKindV2::ScanGeneration,
            generation_id: Some(generation_id.clone()),
            entity_id: None,
            entity_revision: None,
            scan_run_id: Some(run.scan_run_id.clone()),
            retention_class: "incomplete_staging".to_owned(),
            reason_code: reason_code.to_owned(),
            estimated_rows,
            estimated_bytes,
        };
        let protection = if *generation_id == current_generation {
            Some("CURRENT_GENERATION")
        } else if let Some(reason) = protected_snapshots.get(run.workspace_snapshot_id.as_str()) {
            Some(reason.as_str())
        } else if embedded_artifact_hold(run) {
            Some("ARTIFACT_RETENTION_HOLD")
        } else if max_document_bytes > policy.max_batch_bytes {
            Some("RETENTION_ROW_EXCEEDS_BATCH_LIMIT")
        } else {
            unscoped_protection.as_deref()
        };
        if protection.is_none() {
            generation_candidates.insert(generation_id.clone());
        }
        push_retention_candidate_or_protection(
            &mut candidates,
            &mut protected_targets,
            policy,
            candidate,
            protection,
        );
    }

    let mut successful = runs
        .iter()
        .filter(|(_, _, run)| run.status == ScanStatus::Succeeded)
        .collect::<Vec<_>>();
    successful.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));
    for (index, (generation_id, finished_at, run)) in successful.into_iter().enumerate() {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        if *finished_at >= scan_detail_cutoff {
            continue;
        }
        let aggregate = retention_generation_aggregate_for_plan(
            connection,
            is_cancelled,
            &mut generation_inventory,
            generation_id,
        )?;
        let estimated_rows = aggregate.rows;
        let estimated_bytes = aggregate.bytes;
        let max_document_bytes = aggregate.max_document_bytes;
        let reason_code = "SUCCESSFUL_SCAN_DETAIL_EXPIRED";
        let candidate = RetentionCandidateV2 {
            candidate_id: retention_candidate_id(
                project_id,
                RetentionTargetKindV2::ScanGeneration,
                Some(generation_id),
                None,
                None,
                reason_code,
            )?,
            project_id: project_id.clone(),
            target_kind: RetentionTargetKindV2::ScanGeneration,
            generation_id: Some(generation_id.clone()),
            entity_id: None,
            entity_revision: None,
            scan_run_id: Some(run.scan_run_id.clone()),
            retention_class: "successful_scan_detail".to_owned(),
            reason_code: reason_code.to_owned(),
            estimated_rows,
            estimated_bytes,
        };
        let protection = if *generation_id == current_generation {
            Some("CURRENT_GENERATION")
        } else if index < usize::try_from(policy.keep_latest_successful_scans).unwrap_or(usize::MAX)
        {
            Some("LATEST_SUCCESSFUL_SCAN_FLOOR")
        } else if let Some(reason) = protected_snapshots.get(run.workspace_snapshot_id.as_str()) {
            Some(reason.as_str())
        } else if embedded_artifact_hold(run) {
            Some("ARTIFACT_RETENTION_HOLD")
        } else if max_document_bytes > policy.max_batch_bytes {
            Some("RETENTION_ROW_EXCEEDS_BATCH_LIMIT")
        } else {
            unscoped_protection.as_deref()
        };
        if protection.is_none() {
            generation_candidates.insert(generation_id.clone());
        }
        push_retention_candidate_or_protection(
            &mut candidates,
            &mut protected_targets,
            policy,
            candidate,
            protection,
        );
    }

    let finding_documents: Vec<(String, String, String)> = {
        let mut statement = connection
            .prepare("SELECT entity_id, generation_id, document_json FROM findings")
            .map_err(map_sql)?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(map_sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sql)?
    };
    for (entity_id, generation_id, document) in finding_documents {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        let finding: Finding = serde_json::from_str(&document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored Finding is invalid",
            )
        })?;
        let Some((_, finished_at, workspace_snapshot_id)) =
            run_locations.get(finding.last_observed_scan_id.as_str())
        else {
            continue;
        };
        if finding.lifecycle != FindingLifecycle::Resolved
            || *finished_at >= resolved_cutoff
            || generation_id == current_generation
            || generation_candidates.contains(&generation_id)
        {
            continue;
        }
        let finding_parameter = finding.finding_id.as_str();
        let (finding_rows, finding_bytes) = retention_document_stats(
            connection,
            "findings",
            "entity_id=?1 AND generation_id=?2",
            &[&entity_id, &generation_id],
        )?;
        let (occurrence_rows, occurrence_bytes) = retention_document_stats(
            connection,
            "occurrences",
            "generation_id=?1 AND json_extract(document_json, '$.finding_id')=?2",
            &[&generation_id, &finding_parameter],
        )?;
        let estimated_rows = finding_rows.saturating_add(occurrence_rows);
        let estimated_bytes = finding_bytes.saturating_add(occurrence_bytes);
        let max_document_bytes = retention_document_max_bytes(
            connection,
            "findings",
            "entity_id=?1 AND generation_id=?2",
            &[&entity_id, &generation_id],
        )?
        .max(retention_document_max_bytes(
            connection,
            "occurrences",
            "generation_id=?1 AND json_extract(document_json, '$.finding_id')=?2",
            &[&generation_id, &finding_parameter],
        )?);
        let reason_code = "RESOLVED_FINDING_EXPIRED";
        let candidate = RetentionCandidateV2 {
            candidate_id: retention_candidate_id(
                project_id,
                RetentionTargetKindV2::ResolvedFinding,
                Some(&generation_id),
                Some(&entity_id),
                None,
                reason_code,
            )?,
            project_id: project_id.clone(),
            target_kind: RetentionTargetKindV2::ResolvedFinding,
            generation_id: Some(generation_id),
            entity_id: Some(entity_id),
            entity_revision: None,
            scan_run_id: Some(finding.last_observed_scan_id),
            retention_class: "resolved_finding".to_owned(),
            reason_code: reason_code.to_owned(),
            estimated_rows,
            estimated_bytes,
        };
        let active_v2_decision = active_suppressions_v2.iter().any(|suppression| {
            if suppression.subject_binding_constraint.is_some() {
                return false;
            }
            let exact_subject = suppression.subject_kind == DecisionSubjectKindV2::Finding
                && suppression
                    .subject_fingerprint
                    .as_ref()
                    .is_some_and(|fingerprint| fingerprint == &finding.finding_fingerprint);
            let rule_subject = suppression
                .rule_ref_constraint
                .as_ref()
                .is_some_and(|rule| {
                    rule.rule_id == finding.rule_id && rule.rule_version == finding.rule_version
                });
            let evidence_subject = suppression.review_evidence_refs.iter().any(|reference| {
                matches!(
                    reference,
                    DiagnosticEvidenceRefV2::Finding {
                        finding_id,
                        finding_fingerprint,
                    } if finding_id == &finding.finding_id
                        && finding_fingerprint == &finding.finding_fingerprint
                )
            });
            exact_subject || rule_subject || evidence_subject
        });
        let protection = if !finding.active_suppression_ids.is_empty()
            || finding.active_disposition_id.is_some()
            || active_v2_decision
        {
            Some("ACTIVE_DECISION_HOLD")
        } else if let Some(reason) = protected_snapshots.get(workspace_snapshot_id.as_str()) {
            Some(reason.as_str())
        } else if hold_finding_ids.contains(finding.finding_id.as_str()) {
            Some("ARTIFACT_RETENTION_HOLD")
        } else if max_document_bytes > policy.max_batch_bytes {
            Some("RETENTION_ROW_EXCEEDS_BATCH_LIMIT")
        } else {
            unscoped_protection.as_deref()
        };
        push_retention_candidate_or_protection(
            &mut candidates,
            &mut protected_targets,
            policy,
            candidate,
            protection,
        );
    }

    let legacy_suppressions: Vec<(String, i64, String, i64)> = {
        let mut statement = connection
            .prepare(
                "SELECT current.entity_id, current.revision, current.document_json,
                        (SELECT MAX(candidate.revision)
                         FROM suppressions AS candidate
                         WHERE candidate.entity_id = current.entity_id)
                 FROM suppressions AS current",
            )
            .map_err(map_sql)?;
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(map_sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sql)?
    };
    for (entity_id, revision, document, latest_revision) in legacy_suppressions {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        let revision = u64::try_from(revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored Suppression revision is invalid",
            )
        })?;
        let latest_revision = u64::try_from(latest_revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "latest Suppression revision is invalid",
            )
        })?;
        let suppression: Suppression = serde_json::from_str(&document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored Suppression is invalid",
            )
        })?;
        if suppression.scope_kind != SuppressionScope::Local
            || suppression.created_at >= local_decision_cutoff
        {
            continue;
        }
        let candidate = local_decision_candidate(
            project_id,
            RetentionTargetKindV2::LocalSuppression,
            &entity_id,
            revision,
            "local_suppression_v1",
            "LOCAL_SUPPRESSION_EXPIRED",
            document.len() as u64,
        )?;
        let protection = (revision == latest_revision
            && suppression.status == SuppressionStatus::Active)
            .then_some("ACTIVE_LOCAL_DECISION");
        push_retention_candidate_or_protection(
            &mut candidates,
            &mut protected_targets,
            policy,
            candidate,
            protection,
        );
    }

    let legacy_dispositions: Vec<(String, i64, String, i64)> = {
        let mut statement = connection
            .prepare(
                "SELECT current.entity_id, current.revision, current.document_json,
                        (SELECT MAX(candidate.revision)
                         FROM dispositions AS candidate
                         WHERE candidate.entity_id = current.entity_id)
                 FROM dispositions AS current",
            )
            .map_err(map_sql)?;
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(map_sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sql)?
    };
    for (entity_id, revision, document, latest_revision) in legacy_dispositions {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        let revision = u64::try_from(revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored Disposition revision is invalid",
            )
        })?;
        let latest_revision = u64::try_from(latest_revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "latest Disposition revision is invalid",
            )
        })?;
        let disposition: Disposition = serde_json::from_str(&document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored Disposition is invalid",
            )
        })?;
        if disposition.decided_at >= local_decision_cutoff {
            continue;
        }
        let candidate = local_decision_candidate(
            project_id,
            RetentionTargetKindV2::LocalDisposition,
            &entity_id,
            revision,
            "local_disposition_v1",
            "LOCAL_DISPOSITION_EXPIRED",
            document.len() as u64,
        )?;
        let protection = (revision == latest_revision
            && disposition.status == DispositionStatus::Active)
            .then_some("ACTIVE_LOCAL_DECISION");
        push_retention_candidate_or_protection(
            &mut candidates,
            &mut protected_targets,
            policy,
            candidate,
            protection,
        );
    }

    for (table, target_kind, retention_class, reason_code) in [
        (
            "suppressions_v2",
            RetentionTargetKindV2::LocalSuppression,
            "local_suppression_v2",
            "LOCAL_SUPPRESSION_EXPIRED",
        ),
        (
            "dispositions_v2",
            RetentionTargetKindV2::LocalDisposition,
            "local_disposition_v2",
            "LOCAL_DISPOSITION_EXPIRED",
        ),
    ] {
        let documents: Vec<(String, String)> = {
            let mut statement = connection
                .prepare(&format!("SELECT entity_id, document_json FROM {table}"))
                .map_err(map_sql)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(map_sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_sql)?
        };
        for (entity_id, document) in documents {
            if is_cancelled() {
                return Err(repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "retention planning was cancelled",
                ));
            }
            let (revision, decided_at, active) = if table == "suppressions_v2" {
                let value: SuppressionV2 = serde_json::from_str(&document).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored SuppressionV2 is invalid",
                    )
                })?;
                (
                    value.revision,
                    value.created_at,
                    value.status == SuppressionStatus::Active,
                )
            } else {
                let value: DispositionV2 = serde_json::from_str(&document).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored DispositionV2 is invalid",
                    )
                })?;
                (
                    value.revision,
                    value.decided_at,
                    value.status == DispositionStatus::Active,
                )
            };
            if decided_at >= local_decision_cutoff {
                continue;
            }
            let candidate = local_decision_candidate(
                project_id,
                target_kind,
                &entity_id,
                revision,
                retention_class,
                reason_code,
                document.len() as u64,
            )?;
            push_retention_candidate_or_protection(
                &mut candidates,
                &mut protected_targets,
                policy,
                candidate,
                active.then_some("ACTIVE_LOCAL_DECISION"),
            );
        }
    }

    Ok((candidates, protected_targets))
}

impl SqliteGlobalRepository {
    fn project_ids_page(
        &self,
        after_project_id: Option<&ProjectId>,
        limit: usize,
    ) -> Result<(u64, Vec<ProjectId>), RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let total: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .map_err(map_sql)?;
        let total = u64::try_from(total).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "global store contains an invalid project count",
            )
        })?;
        let limit = i64::try_from(limit).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management status page limit is invalid",
            )
        })?;
        let raw_ids = if let Some(after_project_id) = after_project_id {
            let mut statement = connection
                .prepare(
                    "SELECT project_id FROM projects WHERE project_id > ?1 ORDER BY project_id LIMIT ?2",
                )
                .map_err(map_sql)?;
            statement
                .query_map(params![after_project_id.as_str(), limit], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(map_sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_sql)?
        } else {
            let mut statement = connection
                .prepare("SELECT project_id FROM projects ORDER BY project_id LIMIT ?1")
                .map_err(map_sql)?;
            statement
                .query_map(params![limit], |row| row.get::<_, String>(0))
                .map_err(map_sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_sql)?
        };
        let project_ids = raw_ids
            .into_iter()
            .map(|value| {
                ProjectId::parse(value).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "global store contains an invalid project identifier",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((total, project_ids))
    }
}

impl Drop for SqliteGlobalRepository {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.lock() {
            let _ = set_meta(&connection, "last_clean_shutdown", "true");
        }
    }
}

impl GlobalManagementRepository for SqliteGlobalRepository {
    fn status(&self) -> Result<ManagementStoreStatus, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        status_from_connection(&connection)
    }

    fn register_project(
        &self,
        project: &Project,
        checkout: &ProjectCheckout,
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Project, RepositoryError> {
        if checkout.project_id != project.project_id
            || !project
                .attached_checkout_ids
                .contains(&checkout.checkout_id)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "project and checkout identity do not match",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        if let Some(result) =
            idempotency_result(&transaction, idempotency_key, payload_fingerprint)?
        {
            return serde_json::from_str(&result).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored idempotency result is invalid",
                )
            });
        }
        let document = serde_json::to_string(project).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "project serialization failed",
            )
        })?;
        let checkout_document = serde_json::to_string(checkout).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "project checkout serialization failed",
            )
        })?;
        let existing_checkout: Option<(String, Option<String>)> = transaction
            .query_row(
                "SELECT project_id, root_binding_id FROM project_checkouts WHERE checkout_id=?1",
                params![checkout.checkout_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sql)?;
        if existing_checkout
            .as_ref()
            .is_some_and(|(project_id, root_binding_id)| {
                project_id != project.project_id.as_str()
                    || root_binding_id.as_deref()
                        != checkout
                            .root_binding_id
                            .as_ref()
                            .map(|value| value.as_str())
            })
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "CheckoutId is already bound to another immutable identity",
            ));
        }
        let identity_scope = serialized_enum_label(&project.identity_scope)?;
        transaction
            .execute(
                "INSERT INTO projects(project_id, identity_scope, document_json, updated_at)
                 VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id) DO UPDATE SET
                    identity_scope=excluded.identity_scope,
                    document_json=excluded.document_json,
                    updated_at=excluded.updated_at",
                params![
                    project.project_id.as_str(),
                    identity_scope,
                    document,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO project_checkouts(
                    checkout_id, project_id, root_binding_id, document_json, updated_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(checkout_id) DO UPDATE SET
                    document_json=excluded.document_json,
                    updated_at=excluded.updated_at",
                params![
                    checkout.checkout_id.as_str(),
                    checkout.project_id.as_str(),
                    checkout.root_binding_id.as_ref().map(|id| id.as_str()),
                    checkout_document,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(map_sql)?;
        append_event(
            &transaction,
            "project.registered",
            Some(&project.project_id),
            payload_fingerprint,
        )?;
        bump_revision(&transaction)?;
        store_idempotency(
            &transaction,
            idempotency_key,
            payload_fingerprint,
            &document,
        )?;
        transaction.commit().map_err(map_sql)?;
        Ok(project.clone())
    }

    fn get_project(&self, project_id: &ProjectId) -> Result<Option<Project>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM projects WHERE project_id=?1",
            project_id.as_str(),
        )
    }

    fn list_projects(&self) -> Result<Vec<Project>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            "SELECT document_json FROM projects ORDER BY project_id",
            [],
        )
    }

    fn get_project_checkout(
        &self,
        checkout_id: &CheckoutId,
    ) -> Result<Option<ProjectCheckout>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM project_checkouts WHERE checkout_id=?1",
            checkout_id.as_str(),
        )
    }

    fn list_project_checkouts(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ProjectCheckout>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            "SELECT document_json FROM project_checkouts WHERE project_id=?1 ORDER BY checkout_id",
            params![project_id.as_str()],
        )
    }

    fn put_project_catalog_snapshot(
        &self,
        snapshot: &ProjectCatalogSnapshot,
    ) -> Result<(), RepositoryError> {
        let document = serde_json::to_string(snapshot).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "project catalog snapshot serialization failed",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT document_json FROM project_catalog_snapshots WHERE entity_id=?1",
                [snapshot.project_catalog_snapshot_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some(existing) = existing {
            let existing: ProjectCatalogSnapshot =
                serde_json::from_str(&existing).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored project catalog snapshot is invalid",
                    )
                })?;
            if existing.content_fingerprint != snapshot.content_fingerprint {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "project catalog snapshot identity conflicts with stored content",
                ));
            }
            return Ok(());
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO project_catalog_snapshots(entity_id, document_json)
                 VALUES(?1, ?2)
                 ON CONFLICT(entity_id) DO UPDATE SET document_json=excluded.document_json
                 WHERE project_catalog_snapshots.document_json=excluded.document_json",
                params![snapshot.project_catalog_snapshot_id.as_str(), document],
            )
            .map_err(map_sql)?;
        if snapshot.completeness == star_contracts::management::Completeness::Complete {
            set_meta(
                &transaction,
                "current_project_catalog_snapshot",
                snapshot.project_catalog_snapshot_id.as_str(),
            )
            .map_err(map_sql)?;
        }
        append_event(
            &transaction,
            if snapshot.completeness == star_contracts::management::Completeness::Complete {
                "project_catalog.published"
            } else {
                "project_catalog.incomplete"
            },
            None,
            &snapshot.content_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn latest_project_catalog_snapshot(
        &self,
    ) -> Result<Option<ProjectCatalogSnapshot>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let Some(snapshot_id) =
            get_meta_optional(&connection, "current_project_catalog_snapshot").map_err(map_sql)?
        else {
            return Ok(None);
        };
        query_document(
            &connection,
            "SELECT document_json FROM project_catalog_snapshots WHERE entity_id=?1",
            &snapshot_id,
        )
    }

    fn put_planning_bundle(
        &self,
        bundle: &PlanningBundle,
        idempotency_key: &str,
        input_fingerprint: &Sha256Hash,
    ) -> Result<PlanningBundle, RepositoryError> {
        if idempotency_key.trim().is_empty() || bundle.clone().seal().as_ref() != Ok(bundle) {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "planning bundle invariant failed",
            ));
        }
        let document = serde_json::to_string(bundle).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "planning bundle serialization failed",
            )
        })?;
        let bundle_revision = bundle
            .task_spec
            .revision
            .max(bundle.scope_revision.revision)
            .max(bundle.impact_analysis.revision)
            .max(bundle.validation_plan.revision);
        let bundle_revision = bundle
            .change_plans
            .iter()
            .fold(bundle_revision, |revision, plan| {
                revision.max(plan.revision)
            });
        let sql_bundle_revision = i64::try_from(bundle_revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "planning bundle revision exceeds the SQLite integer range",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let replay: Option<(String, String, String)> = connection
            .query_row(
                "SELECT input_fingerprint, bundle_fingerprint, document_json
                 FROM planning_bundle_revisions
                 WHERE idempotency_key=?1",
                [idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some((stored_input, stored_fingerprint, stored_document)) = replay {
            if stored_input != input_fingerprint.as_str()
                || stored_fingerprint != bundle.bundle_fingerprint.as_str()
            {
                return Err(repository_error(
                    RepositoryErrorCategory::IdempotencyConflict,
                    "planning idempotency key conflicts with stored input or result",
                ));
            }
            let stored: PlanningBundle = serde_json::from_str(&stored_document).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored planning bundle is invalid",
                )
            })?;
            return stored.migrate_v1_to_v2().map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored planning bundle migration failed",
                )
            });
        }
        let current_revision: Option<i64> = connection
            .query_row(
                "SELECT bundle_revision
                 FROM planning_bundle_revisions
                 WHERE task_spec_id=?1
                 ORDER BY bundle_revision DESC
                 LIMIT 1",
                [bundle.task_spec.task_spec_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        let current_revision = current_revision
            .map(u64::try_from)
            .transpose()
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored planning bundle revision is invalid",
                )
            })?;
        match current_revision {
            None if bundle_revision != 1 => {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "initial planning bundle revision must be one",
                ));
            }
            Some(current) if current.checked_add(1) != Some(bundle_revision) => {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "planning bundle revision is not the next immutable revision",
                ));
            }
            _ => {}
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO planning_bundle_revisions(
                    task_spec_id, bundle_revision, idempotency_key,
                    input_fingerprint, bundle_fingerprint, document_json
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    bundle.task_spec.task_spec_id.as_str(),
                    sql_bundle_revision,
                    idempotency_key,
                    input_fingerprint.as_str(),
                    bundle.bundle_fingerprint.as_str(),
                    document,
                ],
            )
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO planning_bundles(
                    task_spec_id, idempotency_key, input_fingerprint,
                    bundle_fingerprint, document_json
                 ) VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(task_spec_id) DO UPDATE SET
                    idempotency_key=excluded.idempotency_key,
                    input_fingerprint=excluded.input_fingerprint,
                    bundle_fingerprint=excluded.bundle_fingerprint,
                    document_json=excluded.document_json",
                params![
                    bundle.task_spec.task_spec_id.as_str(),
                    idempotency_key,
                    input_fingerprint.as_str(),
                    bundle.bundle_fingerprint.as_str(),
                    document,
                ],
            )
            .map_err(map_sql)?;
        append_event(
            &transaction,
            if current_revision.is_some() {
                "planning.bundle.revised"
            } else {
                "planning.bundle.created"
            },
            None,
            &bundle.bundle_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)?;
        Ok(bundle.clone())
    }

    fn get_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<Option<PlanningBundle>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let stored: Option<PlanningBundle> = query_document(
            &connection,
            "SELECT document_json FROM planning_bundles WHERE task_spec_id=?1",
            task_spec_id.as_str(),
        )?;
        stored
            .map(PlanningBundle::migrate_v1_to_v2)
            .transpose()
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored planning bundle migration failed",
                )
            })
    }

    fn get_planning_bundle_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<(PlanningBundle, Sha256Hash)>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let stored: Option<(String, String)> = connection
            .query_row(
                "SELECT document_json, input_fingerprint
                 FROM planning_bundle_revisions WHERE idempotency_key=?1",
                [idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sql)?;
        stored
            .map(|(document, fingerprint)| {
                let bundle: PlanningBundle = serde_json::from_str(&document).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored planning bundle is invalid",
                    )
                })?;
                let bundle = bundle.migrate_v1_to_v2().map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored planning bundle migration failed",
                    )
                })?;
                let fingerprint = Sha256Hash::from_str(&fingerprint).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored planning input fingerprint is invalid",
                    )
                })?;
                Ok((bundle, fingerprint))
            })
            .transpose()
    }

    fn list_planning_bundle_revisions(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<Vec<PlanningBundle>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let mut statement = connection
            .prepare(
                "SELECT document_json
                 FROM planning_bundle_revisions
                 WHERE task_spec_id=?1
                 ORDER BY bundle_revision ASC",
            )
            .map_err(map_sql)?;
        let documents = statement
            .query_map([task_spec_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(map_sql)?;
        let mut bundles = Vec::new();
        for document in documents {
            let bundle: PlanningBundle = serde_json::from_str(&document.map_err(map_sql)?)
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored planning bundle revision is invalid",
                    )
                })?;
            bundles.push(bundle.migrate_v1_to_v2().map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored planning bundle revision migration failed",
                )
            })?);
        }
        Ok(bundles)
    }

    fn put_coordination(&self, operation: &CoordinatedOperation) -> Result<(), RepositoryError> {
        validate_coordination(operation).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "coordination invariant failed",
            )
        })?;
        let document = serde_json::to_string(operation).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "coordination serialization failed",
            )
        })?;
        let state = serialized_enum_label(&operation.state)?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO coordinated_operations(operation_id, idempotency_key, state, input_fingerprint, document_json, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(operation_id) DO UPDATE SET
                    idempotency_key=excluded.idempotency_key,
                    state=excluded.state,
                    document_json=excluded.document_json,
                    updated_at=excluded.updated_at
                 WHERE coordinated_operations.input_fingerprint=excluded.input_fingerprint
                   AND coordinated_operations.idempotency_key=excluded.idempotency_key",
                params![
                    operation.coordinated_operation_id.as_str(),
                    operation.idempotency_key,
                    state,
                    operation.input_fingerprint.as_str(),
                    document,
                    operation.updated_at.to_rfc3339(),
                ],
            )
            .map_err(map_sql)?;
        if transaction.changes() == 0 {
            return Err(repository_error(
                RepositoryErrorCategory::IdempotencyConflict,
                "coordination ID was reused with a different input",
            ));
        }
        append_event(
            &transaction,
            match operation.state {
                star_contracts::management::CoordinationState::Prepared => {
                    "management.coordination_prepared"
                }
                star_contracts::management::CoordinationState::Completed => {
                    "management.coordination_completed"
                }
                star_contracts::management::CoordinationState::Blocked => {
                    "management.coordination_blocked"
                }
                star_contracts::management::CoordinationState::OutcomeUnknown => {
                    "management.outcome_unknown"
                }
                star_contracts::management::CoordinationState::Applying => {
                    "management.coordination_applying"
                }
            },
            None,
            &operation.input_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn get_coordination(
        &self,
        operation_id: &CoordinatedOperationId,
    ) -> Result<Option<CoordinatedOperation>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM coordinated_operations WHERE operation_id=?1",
            operation_id.as_str(),
        )
    }

    fn get_coordination_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CoordinatedOperation>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM coordinated_operations WHERE idempotency_key=?1",
            idempotency_key,
        )
    }

    fn list_incomplete_coordination(&self) -> Result<Vec<CoordinatedOperation>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            "SELECT document_json FROM coordinated_operations
             WHERE state IN ('prepared','applying','outcome_unknown') ORDER BY operation_id",
            [],
        )
    }

    fn put_development_record(&self, record: &DevelopmentRecord) -> Result<(), RepositoryError> {
        validate_development_record(record)?;
        let revision = i64::try_from(record.revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "development record revision exceeds storage range",
            )
        })?;
        let document = serde_json::to_string(record).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "development record serialization failed",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT document_json FROM development_records_v1
                 WHERE record_kind=?1 AND record_id=?2 AND revision=?3",
                params![record.record_kind, record.record_id, revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some(existing) = existing {
            let existing: DevelopmentRecord = serde_json::from_str(&existing).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored development record is invalid",
                )
            })?;
            if existing == *record {
                return Ok(());
            }
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "development record revision conflicts with immutable stored content",
            ));
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        transaction
            .execute(
                "INSERT INTO development_records_v1(
                    record_kind, record_id, revision, project_id, state, document_json
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    record.record_kind,
                    record.record_id,
                    revision,
                    record.project_id.as_ref().map(ProjectId::as_str),
                    record.state,
                    document,
                ],
            )
            .map_err(map_sql)?;
        append_event(
            &transaction,
            "management.development_record_published",
            record.project_id.as_ref(),
            &record.document_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn get_development_record(
        &self,
        record_kind: &str,
        record_id: &str,
        revision: Option<u64>,
    ) -> Result<Option<DevelopmentRecord>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        let document: Option<String> = match revision {
            Some(revision) => {
                let revision = i64::try_from(revision).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "development record revision exceeds storage range",
                    )
                })?;
                connection
                    .query_row(
                        "SELECT document_json FROM development_records_v1
                     WHERE record_kind=?1 AND record_id=?2 AND revision=?3",
                        params![record_kind, record_id, revision],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(map_sql)?
            }
            None => connection
                .query_row(
                    "SELECT document_json FROM development_records_v1
                     WHERE record_kind=?1 AND record_id=?2
                     ORDER BY revision DESC LIMIT 1",
                    params![record_kind, record_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_sql)?,
        };
        document
            .map(|document| {
                serde_json::from_str::<DevelopmentRecord>(&document).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored development record is invalid",
                    )
                })
            })
            .transpose()
    }

    fn list_development_records(
        &self,
        record_kind: &str,
        project_id: Option<&ProjectId>,
    ) -> Result<Vec<DevelopmentRecord>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "global store lock is unavailable",
            )
        })?;
        match project_id {
            Some(project_id) => query_documents(
                &connection,
                "SELECT document_json FROM development_records_v1
                 WHERE record_kind=?1 AND project_id=?2
                 ORDER BY record_id, revision",
                params![record_kind, project_id.as_str()],
            ),
            None => query_documents(
                &connection,
                "SELECT document_json FROM development_records_v1
                 WHERE record_kind=?1 ORDER BY record_id, revision",
                params![record_kind],
            ),
        }
    }
}

struct SqliteProjectRepository {
    project_id: ProjectId,
    connection: Mutex<Connection>,
}

impl SqliteProjectRepository {
    fn open(
        path: &Path,
        project_id: &ProjectId,
        product_version: &str,
    ) -> Result<Self, RepositoryError> {
        let connection = open_store(
            path,
            StoreScope::Project {
                project_id: project_id.clone(),
            },
            product_version,
            PROJECT_SCHEMA,
        )?;
        Ok(Self {
            project_id: project_id.clone(),
            connection: Mutex::new(connection),
        })
    }

    fn open_read_only(path: &Path, project_id: &ProjectId) -> Result<Self, RepositoryError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        connection
            .execute_batch("PRAGMA query_only=ON;")
            .map_err(map_sql)?;
        verify_connection(&connection)?;
        let status = status_from_connection(&connection)?;
        if status.store_scope
            != (StoreScope::Project {
                project_id: project_id.clone(),
            })
            || status.management_store_version != MANAGEMENT_STORE_VERSION
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "recovery-only project store header does not match",
            ));
        }
        Ok(Self {
            project_id: project_id.clone(),
            connection: Mutex::new(connection),
        })
    }

    fn verify_integrity(&self) -> Result<ManagementStoreStatus, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        verify_connection(&connection)?;
        let stored_project_id: Option<String> = connection
            .query_row(
                "SELECT project_id FROM project_document WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        if stored_project_id
            .as_deref()
            .is_some_and(|id| id != self.project_id.as_str())
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "project store partition does not match its ProjectId",
            ));
        }
        verify_project_relations(&connection, &self.project_id)?;
        set_meta(&connection, "last_verified_at", &Utc::now().to_rfc3339()).map_err(map_sql)?;
        status_from_connection(&connection)
    }

    fn backup(&self, destination: &Path) -> Result<(), RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        backup_connection(&connection, destination)
    }

    fn retention_analysis_v2(
        &self,
        policy: &RetentionPolicyV2,
        created_at: DateTime<Utc>,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Vec<RetentionCandidateV2>, Vec<RetentionProtectedTargetV1>), RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        analyze_project_retention_v2(
            &connection,
            &self.project_id,
            policy,
            created_at,
            is_cancelled,
        )
    }

    fn retention_candidates(
        &self,
        incomplete_cutoff: DateTime<Utc>,
        scan_detail_cutoff: DateTime<Utc>,
        keep_latest_successful_scans: usize,
    ) -> Result<Vec<RetentionCandidate>, RepositoryError> {
        if keep_latest_successful_scans == 0 {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "successful scan retention floor is invalid",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let current = get_meta_optional(&connection, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        let mut statement = connection
            .prepare("SELECT generation_id, document_json FROM scan_runs ORDER BY generation_id")
            .map_err(map_sql)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(map_sql)?;
        let protected_snapshots = protected_snapshot_ids(&connection)?;
        let mut candidates = Vec::new();
        let mut successful = Vec::new();
        for row in rows {
            let (generation_id, document) = row.map_err(map_sql)?;
            let run: ScanRun = serde_json::from_str(&document).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored ScanRun is invalid",
                )
            })?;
            let finished = run.finished_at.unwrap_or(run.started_at);
            if generation_id != current
                && run.status != star_contracts::management::ScanStatus::Succeeded
                && finished < incomplete_cutoff
            {
                candidates.push(RetentionCandidate {
                    project_id: self.project_id.clone(),
                    generation_id,
                    scan_run_id: run.scan_run_id,
                    retention_class: "incomplete_staging".to_owned(),
                    reason_code: "INCOMPLETE_STAGING_EXPIRED".to_owned(),
                });
            } else if run.status == star_contracts::management::ScanStatus::Succeeded {
                successful.push((finished, generation_id, run));
            }
        }
        successful.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        for (index, (finished, generation_id, run)) in successful.into_iter().enumerate() {
            if index >= keep_latest_successful_scans
                && generation_id != current
                && finished < scan_detail_cutoff
                && !protected_snapshots.contains(run.workspace_snapshot_id.as_str())
            {
                candidates.push(RetentionCandidate {
                    project_id: self.project_id.clone(),
                    generation_id,
                    scan_run_id: run.scan_run_id,
                    retention_class: "successful_scan_detail".to_owned(),
                    reason_code: "SUCCESSFUL_SCAN_DETAIL_EXPIRED".to_owned(),
                });
            }
        }
        Ok(candidates)
    }

    fn apply_retention(
        &self,
        expected_revision: u64,
        candidates: &[&RetentionCandidate],
        plan_fingerprint: &Sha256Hash,
    ) -> Result<usize, RepositoryError> {
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let current_revision: u64 =
            get_meta(&transaction, "store_revision")?
                .parse()
                .map_err(|_| {
                    repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid")
                })?;
        if current_revision != expected_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention project revision is stale",
            ));
        }
        let current_generation = get_meta_optional(&transaction, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        let mut applied = 0;
        for candidate in candidates {
            if candidate.project_id != self.project_id
                || candidate.generation_id == current_generation
                || !matches!(
                    candidate.retention_class.as_str(),
                    "incomplete_staging" | "successful_scan_detail"
                )
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "retention candidate is not safely removable",
                ));
            }
            for table in [
                "canonical_sources",
                "symbols",
                "symbol_references",
                "findings",
                "occurrences",
                "scan_runs",
            ] {
                transaction
                    .execute(
                        &format!("DELETE FROM {table} WHERE generation_id=?1"),
                        [&candidate.generation_id],
                    )
                    .map_err(map_sql)?;
            }
            applied += 1;
        }
        if applied > 0 {
            append_event(
                &transaction,
                "retention.applied",
                Some(&self.project_id),
                plan_fingerprint,
            )?;
            bump_revision(&transaction)?;
        }
        transaction.commit().map_err(map_sql)?;
        Ok(applied)
    }

    fn apply_retention_candidate_v2(
        &self,
        expected_revision: u64,
        candidate: &RetentionCandidateV2,
        plan_fingerprint: &Sha256Hash,
        limits: RetentionBatchLimits,
        progress: RetentionCandidateProgress,
    ) -> Result<(u64, u64, u64, bool), RepositoryError> {
        if candidate.project_id != self.project_id || candidate.estimated_rows == 0 {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "retention candidate is invalid",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let current_revision: u64 =
            get_meta(&transaction, "store_revision")?
                .parse()
                .map_err(|_| {
                    repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid")
                })?;
        let last_candidate =
            get_meta_optional(&transaction, "retention_last_candidate_id").map_err(map_sql)?;
        let last_plan =
            get_meta_optional(&transaction, "retention_last_plan_fingerprint").map_err(map_sql)?;
        if current_revision == expected_revision.saturating_add(1)
            && last_candidate.as_deref() == Some(candidate.candidate_id.as_str())
            && last_plan.as_deref() == Some(plan_fingerprint.as_str())
        {
            let rows = get_meta(&transaction, "retention_last_applied_rows")?
                .parse()
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "retention replay row count is invalid",
                    )
                })?;
            let bytes = get_meta(&transaction, "retention_last_applied_bytes")?
                .parse()
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "retention replay byte count is invalid",
                    )
                })?;
            let candidate_complete = get_meta(&transaction, "retention_last_candidate_complete")?
                .parse()
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "retention replay completion state is invalid",
                    )
                })?;
            return Ok((rows, bytes, current_revision, candidate_complete));
        }
        if current_revision != expected_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention project revision is stale",
            ));
        }
        let current_generation = get_meta_optional(&transaction, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        if candidate
            .generation_id
            .as_deref()
            .is_some_and(|generation| generation == current_generation)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "retention candidate targets the current generation",
            ));
        }
        let remaining = retention_candidate_remaining_stats(&transaction, candidate)?;
        if progress.rows.checked_add(remaining.0) != Some(candidate.estimated_rows)
            || progress.bytes.checked_add(remaining.1) != Some(candidate.estimated_bytes)
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "retention candidate estimate no longer matches source",
            ));
        }
        let (applied_rows, applied_bytes, candidate_complete) = match candidate.target_kind {
            RetentionTargetKindV2::ScanGeneration => {
                let generation_id = candidate.generation_id.as_deref().ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "scan retention candidate omits generation",
                    )
                })?;
                let mut selected = RetentionRowSelection::default();
                for table in RETENTION_GENERATION_TABLES {
                    if select_retention_rows_for_predicate(
                        &transaction,
                        table,
                        "generation_id=?1",
                        &[&generation_id],
                        limits,
                        &mut selected,
                    )? {
                        break;
                    }
                }
                let applied_rows = delete_selected_retention_rows(&transaction, &selected.rows)?;
                (
                    applied_rows,
                    selected.bytes,
                    retention_generation_stats(&transaction, generation_id)?.0 == 0,
                )
            }
            RetentionTargetKindV2::ResolvedFinding => {
                let generation_id = candidate.generation_id.as_deref().ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "resolved finding candidate omits generation",
                    )
                })?;
                let entity_id = candidate.entity_id.as_deref().ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "resolved finding candidate omits identity",
                    )
                })?;
                let mut selected = RetentionRowSelection::default();
                let full = select_retention_rows_for_predicate(
                    &transaction,
                    "occurrences",
                    "generation_id=?1 AND json_extract(document_json, '$.finding_id')=?2",
                    &[&generation_id, &entity_id],
                    limits,
                    &mut selected,
                )?;
                if !full {
                    let _ = select_retention_rows_for_predicate(
                        &transaction,
                        "findings",
                        "generation_id=?1 AND entity_id=?2",
                        &[&generation_id, &entity_id],
                        limits,
                        &mut selected,
                    )?;
                }
                let applied_rows = delete_selected_retention_rows(&transaction, &selected.rows)?;
                let finding_rows = retention_document_stats(
                    &transaction,
                    "findings",
                    "generation_id=?1 AND entity_id=?2",
                    &[&generation_id, &entity_id],
                )?
                .0;
                let occurrence_rows = retention_document_stats(
                    &transaction,
                    "occurrences",
                    "generation_id=?1 AND json_extract(document_json, '$.finding_id')=?2",
                    &[&generation_id, &entity_id],
                )?
                .0;
                (
                    applied_rows,
                    selected.bytes,
                    finding_rows == 0 && occurrence_rows == 0,
                )
            }
            RetentionTargetKindV2::LocalSuppression | RetentionTargetKindV2::LocalDisposition => {
                let entity_id = candidate.entity_id.as_deref().ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "local decision candidate omits identity",
                    )
                })?;
                let entity_revision = candidate
                    .entity_revision
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|_| {
                        repository_error(
                            RepositoryErrorCategory::Invalid,
                            "local decision revision is invalid",
                        )
                    })?;
                let changed = match candidate.retention_class.as_str() {
                    "local_suppression_v1" => transaction
                        .execute(
                            "DELETE FROM suppressions WHERE entity_id=?1 AND revision=?2",
                            params![entity_id, entity_revision],
                        )
                        .map_err(map_sql)?,
                    "local_suppression_v2" => transaction
                        .execute(
                            "DELETE FROM suppressions_v2 WHERE entity_id=?1",
                            [entity_id],
                        )
                        .map_err(map_sql)?,
                    "local_disposition_v1" => transaction
                        .execute(
                            "DELETE FROM dispositions WHERE entity_id=?1 AND revision=?2",
                            params![entity_id, entity_revision],
                        )
                        .map_err(map_sql)?,
                    "local_disposition_v2" => transaction
                        .execute(
                            "DELETE FROM dispositions_v2 WHERE entity_id=?1",
                            [entity_id],
                        )
                        .map_err(map_sql)?,
                    _ => {
                        return Err(repository_error(
                            RepositoryErrorCategory::Invalid,
                            "local decision retention class is invalid",
                        ));
                    }
                };
                if changed != 1 {
                    return Err(repository_error(
                        RepositoryErrorCategory::RevisionConflict,
                        "local decision retention candidate no longer matches source",
                    ));
                }
                (changed as u64, candidate.estimated_bytes, true)
            }
        };
        if applied_rows == 0 || applied_rows > limits.rows || applied_bytes > limits.bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "retention batch exceeds its configured transaction limit",
            ));
        }
        set_meta(
            &transaction,
            "retention_last_candidate_id",
            &candidate.candidate_id,
        )
        .map_err(map_sql)?;
        set_meta(
            &transaction,
            "retention_last_plan_fingerprint",
            plan_fingerprint.as_str(),
        )
        .map_err(map_sql)?;
        set_meta(
            &transaction,
            "retention_last_applied_rows",
            &applied_rows.to_string(),
        )
        .map_err(map_sql)?;
        set_meta(
            &transaction,
            "retention_last_applied_bytes",
            &applied_bytes.to_string(),
        )
        .map_err(map_sql)?;
        set_meta(
            &transaction,
            "retention_last_candidate_complete",
            &candidate_complete.to_string(),
        )
        .map_err(map_sql)?;
        append_event(
            &transaction,
            "retention.applied",
            Some(&self.project_id),
            plan_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)?;
        Ok((
            applied_rows,
            applied_bytes,
            current_revision.saturating_add(1),
            candidate_complete,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn put_versioned_decision<T: Serialize>(
        &self,
        table: &str,
        entity_id: &str,
        revision: u64,
        expected_revision: u64,
        value: &T,
        event_type: &str,
        event_fingerprint: &Sha256Hash,
    ) -> Result<(), RepositoryError> {
        if revision
            != expected_revision.checked_add(1).ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "decision revision overflowed",
                )
            })?
        {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "decision revision does not follow the expected revision",
            ));
        }
        let document = serde_json::to_string(value).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "decision serialization failed",
            )
        })?;
        let sql_revision = i64::try_from(revision).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "decision revision exceeds the backend integer range",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let existing_same_revision: Option<String> = transaction
            .query_row(
                &format!("SELECT document_json FROM {table} WHERE entity_id=?1 AND revision=?2"),
                params![entity_id, sql_revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some(existing) = existing_same_revision {
            if existing == document {
                return Ok(());
            }
            return Err(repository_error(
                RepositoryErrorCategory::IdempotencyConflict,
                "decision identity and revision already contain another document",
            ));
        }
        let current_revision: i64 = transaction
            .query_row(
                &format!("SELECT COALESCE(MAX(revision), 0) FROM {table} WHERE entity_id=?1"),
                [entity_id],
                |row| row.get(0),
            )
            .map_err(map_sql)?;
        if current_revision < 0 || current_revision as u64 != expected_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "decision expected revision is stale",
            ));
        }
        transaction
            .execute(
                &format!(
                    "INSERT INTO {table}(entity_id, revision, document_json) VALUES(?1, ?2, ?3)"
                ),
                params![entity_id, sql_revision, document],
            )
            .map_err(map_sql)?;
        append_event(
            &transaction,
            event_type,
            Some(&self.project_id),
            event_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn list_latest_decisions<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
    ) -> Result<Vec<T>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            &format!(
                "SELECT current.document_json
                 FROM {table} AS current
                 JOIN (
                    SELECT entity_id, MAX(revision) AS revision
                    FROM {table}
                    GROUP BY entity_id
                 ) AS latest
                   ON latest.entity_id=current.entity_id AND latest.revision=current.revision
                 ORDER BY current.entity_id"
            ),
            [],
        )
    }

    fn list_projection_documents<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
    ) -> Result<Vec<T>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            &format!("SELECT document_json FROM {table} ORDER BY entity_id"),
            [],
        )
    }

    fn list_m3_documents<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
    ) -> Result<Vec<T>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(&connection, sql, [])
    }

    fn list_latest_m3_decision_documents<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        logical_id_path: &str,
    ) -> Result<Vec<T>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            &format!(
                "SELECT current.document_json
                 FROM {table} AS current
                 JOIN (
                    SELECT json_extract(document_json, '{logical_id_path}') AS logical_id,
                           MAX(CAST(json_extract(document_json, '$.revision') AS INTEGER)) AS revision
                    FROM {table}
                    GROUP BY json_extract(document_json, '{logical_id_path}')
                 ) AS latest
                   ON json_extract(current.document_json, '{logical_id_path}')=latest.logical_id
                  AND CAST(json_extract(current.document_json, '$.revision') AS INTEGER)=latest.revision
                 ORDER BY latest.logical_id"
            ),
            [],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn put_m3_decision_document<T: Serialize>(
        &self,
        table: &str,
        entity_id: &str,
        value: &T,
        valid: bool,
        event_type: &str,
        event_fingerprint: &Sha256Hash,
    ) -> Result<(), RepositoryError> {
        let json = serde_json::to_value(value).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "M3 decision serialization failed",
            )
        })?;
        let revision = json
            .get("revision")
            .and_then(serde_json::Value::as_u64)
            .filter(|revision| *revision > 0)
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "M3 decision revision is invalid",
                )
            })?;
        let project_matches = json.get("project_id").and_then(serde_json::Value::as_str)
            == Some(self.project_id.as_str());
        if !valid || !project_matches {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "M3 decision invariant or Project partition failed",
            ));
        }
        let key = format!("{entity_id}:{revision}");
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let existing: Option<String> = transaction
            .query_row(
                &format!("SELECT document_json FROM {table} WHERE entity_id=?1"),
                [&key],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some(existing) = existing {
            if existing
                == serde_json::to_string(value).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "M3 decision serialization failed",
                    )
                })?
            {
                return Ok(());
            }
            return Err(repository_error(
                RepositoryErrorCategory::IdempotencyConflict,
                "M3 decision identity and revision already contain another document",
            ));
        }
        let current_revision: i64 = transaction
            .query_row(
                &format!(
                    "SELECT COALESCE(MAX(CAST(json_extract(document_json, '$.revision') AS INTEGER)), 0)
                     FROM {table}
                     WHERE substr(entity_id, 1, length(?1) + 1)=?1 || ':'"
                ),
                [entity_id],
                |row| row.get(0),
            )
            .map_err(map_sql)?;
        if current_revision < 0 || current_revision as u64 != revision.saturating_sub(1) {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "M3 decision revision is stale or non-contiguous",
            ));
        }
        insert_immutable_document(&transaction, table, &key, value)?;
        append_event(
            &transaction,
            event_type,
            Some(&self.project_id),
            event_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }
}

impl Drop for SqliteProjectRepository {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.lock() {
            let _ = set_meta(&connection, "last_clean_shutdown", "true");
        }
    }
}

fn validate_development_record(record: &DevelopmentRecord) -> Result<(), RepositoryError> {
    let valid_key = |value: &str, max: usize| {
        !value.is_empty()
            && value.len() <= max
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':')
            })
    };
    let document_schema_id = record
        .document
        .get("schema_id")
        .and_then(serde_json::Value::as_str);
    let document_schema_version = record
        .document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64);
    if record.schema_version != 1
        || !valid_key(&record.record_kind, 128)
        || !valid_key(&record.record_id, 256)
        || record.revision == 0
        || record.state.trim().is_empty()
        || record.state.len() > 128
        || !record.document.is_object()
        || document_schema_id != Some(record.document_schema_id.as_str())
        || document_schema_version != Some(u64::from(record.document_schema_version))
        || DateTime::parse_from_rfc3339(&record.created_at).is_err()
        || canonical_sha256(&record.document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "development record fingerprint calculation failed",
            )
        })? != record.document_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "development record invariant failed",
        ));
    }
    Ok(())
}

impl ProjectManagementRepository for SqliteProjectRepository {
    fn status(&self) -> Result<ManagementStoreStatus, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        status_from_connection(&connection)
    }

    fn commit_registration_participant(
        &self,
        project: &Project,
        operation_id: &CoordinatedOperationId,
        payload_fingerprint: &Sha256Hash,
        result_fingerprint: &Sha256Hash,
    ) -> Result<ParticipantReceipt, RepositoryError> {
        if project.project_id != self.project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "project cannot be written to another project partition",
            ));
        }
        let document = serde_json::to_string(project).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "project serialization failed",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT payload_fingerprint, document_json FROM participant_receipts
                 WHERE operation_id=?1",
                [operation_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sql)?;
        if let Some((existing_payload, existing_document)) = existing {
            if existing_payload != payload_fingerprint.as_str() {
                return Err(repository_error(
                    RepositoryErrorCategory::IdempotencyConflict,
                    "participant operation ID was reused with another payload",
                ));
            }
            return serde_json::from_str(&existing_document).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored participant receipt is invalid",
                )
            });
        }
        let current_revision: u64 =
            get_meta(&transaction, "store_revision")?
                .parse()
                .map_err(|_| {
                    repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid")
                })?;
        let committed_store_revision = current_revision.checked_add(1).ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "store revision overflowed",
            )
        })?;
        let receipt = ParticipantReceipt {
            project_id: project.project_id.clone(),
            operation_id: operation_id.clone(),
            payload_fingerprint: payload_fingerprint.clone(),
            result_fingerprint: result_fingerprint.clone(),
            committed_store_revision,
            local_event_ref: "management.participant_committed".to_owned(),
        };
        let receipt_document = serde_json::to_string(&receipt).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "participant receipt serialization failed",
            )
        })?;
        transaction
            .execute(
                "INSERT INTO project_document(singleton, project_id, document_json)
                 VALUES(1, ?1, ?2)
                 ON CONFLICT(singleton) DO UPDATE SET document_json=excluded.document_json
                 WHERE project_document.project_id=excluded.project_id",
                params![project.project_id.as_str(), document],
            )
            .map_err(map_sql)?;
        if transaction.changes() == 0 {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "project store already belongs to a different ProjectId",
            ));
        }
        append_event(
            &transaction,
            "management.participant_committed",
            Some(&project.project_id),
            result_fingerprint,
        )?;
        transaction
            .execute(
                "INSERT INTO participant_receipts(operation_id, payload_fingerprint, document_json)
                 VALUES(?1, ?2, ?3)",
                params![
                    operation_id.as_str(),
                    payload_fingerprint.as_str(),
                    receipt_document,
                ],
            )
            .map_err(map_sql)?;
        set_meta(
            &transaction,
            "store_revision",
            &committed_store_revision.to_string(),
        )
        .map_err(map_sql)?;
        transaction.commit().map_err(map_sql)?;
        Ok(receipt)
    }

    fn get_project(&self) -> Result<Option<Project>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let json: Option<String> = connection
            .query_row(
                "SELECT document_json FROM project_document WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        deserialize_optional(json)
    }

    fn replay_scan(
        &self,
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Option<ScanRun>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT payload_fingerprint, result_json FROM idempotency
                 WHERE idempotency_key=?1",
                [idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sql)?;
        match existing {
            Some((fingerprint, result)) if fingerprint == payload_fingerprint.as_str() => {
                serde_json::from_str(&result).map(Some).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored scan idempotency result is invalid",
                    )
                })
            }
            Some(_) => Err(repository_error(
                RepositoryErrorCategory::IdempotencyConflict,
                "scan idempotency key was reused with a different input",
            )),
            None => Ok(None),
        }
    }

    fn commit_scan(&self, commit: &ScanCommit) -> Result<ScanRun, RepositoryError> {
        let mut indexed_artifacts = BTreeMap::new();
        for artifact in commit
            .run
            .artifact_refs
            .iter()
            .chain(std::iter::once(&commit.snapshot.entries_manifest_ref))
            .chain(
                commit
                    .code_index
                    .iter()
                    .flat_map(|index| index.artifact_refs.iter()),
            )
        {
            artifact.validate().map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "scan artifact reference invariant is invalid",
                )
            })?;
            if artifact.project_id.as_ref() != Some(&self.project_id) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "scan artifact reference crosses a ProjectId partition",
                ));
            }
            indexed_artifacts.insert(artifact.artifact_id.as_str().to_owned(), artifact.clone());
        }
        if commit.project.project_id != self.project_id
            || commit.run.project_id != self.project_id
            || commit
                .sources
                .iter()
                .any(|item| item.project_id != self.project_id)
            || commit
                .findings
                .iter()
                .any(|item| item.project_id != self.project_id)
            || commit
                .source_entries
                .iter()
                .any(|item| item.owner_project_id != self.project_id)
            || commit
                .code_index
                .as_ref()
                .is_some_and(|item| item.project_id != self.project_id)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "scan commit crosses a ProjectId partition",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        if let Some(result) = idempotency_result(
            &transaction,
            &commit.idempotency_key,
            &commit.payload_fingerprint,
        )? {
            return serde_json::from_str(&result).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "stored scan idempotency result is invalid",
                )
            });
        }
        let project_document = serde_json::to_string(&commit.project).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "project serialization failed",
            )
        })?;
        transaction
            .execute(
                "INSERT INTO project_document(singleton, project_id, document_json)
                 VALUES(1, ?1, ?2)
                 ON CONFLICT(singleton) DO UPDATE SET document_json=excluded.document_json
                 WHERE project_document.project_id=excluded.project_id",
                params![commit.project.project_id.as_str(), project_document],
            )
            .map_err(map_sql)?;
        insert_first_observation(
            &transaction,
            "project_revisions",
            commit.revision.project_revision_id.as_str(),
            &commit.revision,
        )?;
        insert_first_observation(
            &transaction,
            "workspace_snapshots",
            commit.snapshot.workspace_snapshot_id.as_str(),
            &commit.snapshot,
        )?;
        let generation = commit.run.generation_id.as_str();
        insert_generation_document(
            &transaction,
            "scan_runs",
            "scan_run_id",
            commit.run.scan_run_id.as_str(),
            generation,
            &commit.run,
        )?;
        for artifact in indexed_artifacts.values() {
            insert_immutable_document(
                &transaction,
                "artifact_refs",
                artifact.artifact_id.as_str(),
                artifact,
            )?;
        }
        for value in &commit.sources {
            insert_generation_document(
                &transaction,
                "canonical_sources",
                "canonical_source_id",
                value.canonical_source_id.as_str(),
                generation,
                value,
            )?;
        }
        for value in &commit.symbols {
            insert_generation_document(
                &transaction,
                "symbols",
                "symbol_id",
                value.symbol_id.as_str(),
                generation,
                value,
            )?;
        }
        for value in &commit.references {
            insert_generation_document(
                &transaction,
                "symbol_references",
                "symbol_reference_id",
                value.symbol_reference_id.as_str(),
                generation,
                value,
            )?;
        }
        if let Some(value) = &commit.code_index {
            insert_generation_document(
                &transaction,
                "code_index_snapshots",
                "code_index_snapshot_id",
                value.code_index_snapshot_id.as_str(),
                generation,
                value,
            )?;
        }
        for value in &commit.source_entries {
            insert_generation_document(
                &transaction,
                "source_entries",
                "canonical_source_id",
                value.canonical_source_id.as_str(),
                generation,
                value,
            )?;
        }
        for value in &commit.index_entities {
            insert_generation_document(
                &transaction,
                "index_entities",
                "entity_key",
                &value.entity_key,
                generation,
                value,
            )?;
        }
        for value in &commit.index_edges {
            insert_generation_document(
                &transaction,
                "index_edges",
                "edge_key",
                &value.edge_key,
                generation,
                value,
            )?;
        }
        for value in &commit.findings {
            insert_generation_document(
                &transaction,
                "findings",
                "finding_id",
                value.finding_id.as_str(),
                generation,
                value,
            )?;
        }
        for value in &commit.occurrences {
            insert_generation_document(
                &transaction,
                "occurrences",
                "occurrence_id",
                value.occurrence_id.as_str(),
                generation,
                value,
            )?;
        }
        if commit.run.status == star_contracts::management::ScanStatus::Succeeded {
            set_meta(&transaction, "current_generation", generation).map_err(map_sql)?;
        }
        let result = serde_json::to_string(&commit.run).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "scan result serialization failed",
            )
        })?;
        append_event(
            &transaction,
            if commit.run.status == star_contracts::management::ScanStatus::Succeeded {
                "scan.finished"
            } else {
                "scan.incomplete"
            },
            Some(&self.project_id),
            &commit.payload_fingerprint,
        )?;
        bump_revision(&transaction)?;
        store_idempotency(
            &transaction,
            &commit.idempotency_key,
            &commit.payload_fingerprint,
            &result,
        )?;
        transaction.commit().map_err(map_sql)?;
        Ok(commit.run.clone())
    }

    fn latest_scan(&self) -> Result<Option<ScanRun>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let generation = get_meta_optional(&connection, "current_generation").map_err(map_sql)?;
        let Some(generation) = generation else {
            return Ok(None);
        };
        query_document(
            &connection,
            "SELECT document_json FROM scan_runs WHERE generation_id=?1 ORDER BY entity_id DESC LIMIT 1",
            &generation,
        )
    }

    fn latest_code_index_projection(
        &self,
    ) -> Result<Option<StoredCodeIndexProjection>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let Some(generation) =
            get_meta_optional(&connection, "current_generation").map_err(map_sql)?
        else {
            return Ok(None);
        };
        let Some(snapshot) = query_document::<CodeIndexSnapshot>(
            &connection,
            "SELECT document_json FROM code_index_snapshots WHERE generation_id=?1 ORDER BY entity_id DESC LIMIT 1",
            &generation,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(StoredCodeIndexProjection {
            snapshot,
            source_entries: query_documents(
                &connection,
                "SELECT document_json FROM source_entries WHERE generation_id=?1 ORDER BY entity_id",
                [&generation],
            )?,
            entities: query_documents(
                &connection,
                "SELECT document_json FROM index_entities WHERE generation_id=?1 ORDER BY entity_id",
                [&generation],
            )?,
            edges: query_documents(
                &connection,
                "SELECT document_json FROM index_edges WHERE generation_id=?1 ORDER BY entity_id",
                [&generation],
            )?,
            symbols: query_documents(
                &connection,
                "SELECT document_json FROM symbols WHERE generation_id=?1 ORDER BY entity_id",
                [&generation],
            )?,
            references: query_documents(
                &connection,
                "SELECT document_json FROM symbol_references WHERE generation_id=?1 ORDER BY entity_id",
                [&generation],
            )?,
        }))
    }

    fn get_code_index_snapshot(
        &self,
        snapshot_id: &CodeIndexSnapshotId,
    ) -> Result<Option<CodeIndexSnapshot>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM code_index_snapshots WHERE entity_id=?1 ORDER BY generation_id DESC LIMIT 1",
            snapshot_id.as_str(),
        )
    }

    fn get_workspace_snapshot(
        &self,
        workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    ) -> Result<Option<star_contracts::management::WorkspaceSnapshot>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM workspace_snapshots WHERE entity_id=?1",
            workspace_snapshot_id.as_str(),
        )
    }

    fn list_findings(&self) -> Result<Vec<Finding>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let generation = get_meta_optional(&connection, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        query_documents(
            &connection,
            "SELECT document_json FROM findings WHERE generation_id=?1 ORDER BY entity_id",
            [generation],
        )
    }

    fn get_finding(&self, finding_id: &FindingId) -> Result<Option<Finding>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let generation = get_meta_optional(&connection, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        let json: Option<String> = connection
            .query_row(
                "SELECT document_json FROM findings WHERE entity_id=?1 AND generation_id=?2",
                params![finding_id.as_str(), generation],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql)?;
        deserialize_optional(json)
    }

    fn occurrences_for_finding(
        &self,
        finding_id: &FindingId,
    ) -> Result<Vec<Occurrence>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let generation = get_meta_optional(&connection, "current_generation")
            .map_err(map_sql)?
            .unwrap_or_default();
        let mut statement = connection
            .prepare(
                "SELECT document_json FROM occurrences WHERE generation_id=?1 ORDER BY entity_id",
            )
            .map_err(map_sql)?;
        let documents = statement
            .query_map([generation], |row| row.get::<_, String>(0))
            .map_err(map_sql)?;
        let mut matches = Vec::new();
        for document in documents {
            let occurrence: Occurrence = serde_json::from_str(&document.map_err(map_sql)?)
                .map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "stored occurrence is invalid",
                    )
                })?;
            if &occurrence.finding_id == finding_id {
                matches.push(occurrence);
            }
        }
        Ok(matches)
    }

    fn put_suppression(
        &self,
        suppression: &Suppression,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        if suppression.project_id != self.project_id
            || suppression.scope_kind != star_contracts::management::SuppressionScope::Local
            || suppression.reason.trim().is_empty()
            || validate_suppression(suppression).is_err()
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "Suppression invariant failed",
            ));
        }
        validate_decision_strings([
            suppression.selector.as_str(),
            suppression.reason_code.as_str(),
            suppression.reason.as_str(),
            suppression.provenance.as_str(),
        ])?;
        if let Some(justification) = suppression.justification.as_deref() {
            validate_decision_strings([justification])?;
        }
        let fingerprint = versioned_fingerprint("star.suppression-revision", 1, suppression)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "Suppression fingerprint failed",
                )
            })?;
        self.put_versioned_decision(
            "suppressions",
            suppression.suppression_id.as_str(),
            suppression.revision,
            expected_revision,
            suppression,
            "suppression.changed",
            &fingerprint,
        )
    }

    fn sync_shared_decisions(
        &self,
        baselines: &[Baseline],
        suppressions: &[Suppression],
        source_fingerprint: &Sha256Hash,
    ) -> Result<(), RepositoryError> {
        for baseline in baselines {
            if baseline.project_id != self.project_id
                || baseline.scope_kind != star_contracts::management::BaselineScope::Shared
                || baseline.reason.trim().is_empty()
                || validate_baseline(baseline).is_err()
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Baseline invariant failed",
                ));
            }
            validate_decision_strings([baseline.reason.as_str()])?;
        }
        for suppression in suppressions {
            if suppression.project_id != self.project_id
                || suppression.scope_kind != star_contracts::management::SuppressionScope::Shared
                || suppression.reason.trim().is_empty()
                || validate_suppression(suppression).is_err()
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Suppression invariant failed",
                ));
            }
            validate_decision_strings([
                suppression.selector.as_str(),
                suppression.reason_code.as_str(),
                suppression.reason.as_str(),
                suppression.provenance.as_str(),
            ])?;
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        if get_meta_optional(&transaction, "shared_decision_source_fingerprint")
            .map_err(map_sql)?
            .as_deref()
            == Some(source_fingerprint.as_str())
        {
            return Ok(());
        }
        transaction
            .execute("DELETE FROM shared_baselines", [])
            .map_err(map_sql)?;
        transaction
            .execute("DELETE FROM shared_suppressions", [])
            .map_err(map_sql)?;
        for baseline in baselines {
            let revision = i64::try_from(baseline.revision).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Baseline revision exceeds the backend range",
                )
            })?;
            let document = serde_json::to_string(baseline).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Baseline serialization failed",
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO shared_baselines(entity_id, revision, document_json)
                     VALUES(?1, ?2, ?3)",
                    params![baseline.baseline_id.as_str(), revision, document],
                )
                .map_err(map_sql)?;
        }
        for suppression in suppressions {
            let revision = i64::try_from(suppression.revision).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Suppression revision exceeds the backend range",
                )
            })?;
            let document = serde_json::to_string(suppression).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "shared Suppression serialization failed",
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO shared_suppressions(entity_id, revision, document_json)
                     VALUES(?1, ?2, ?3)",
                    params![suppression.suppression_id.as_str(), revision, document],
                )
                .map_err(map_sql)?;
        }
        set_meta(
            &transaction,
            "shared_decision_source_fingerprint",
            source_fingerprint.as_str(),
        )
        .map_err(map_sql)?;
        append_event(
            &transaction,
            "shared_decisions.projected",
            Some(&self.project_id),
            source_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn list_suppressions(&self) -> Result<Vec<Suppression>, RepositoryError> {
        let mut values: Vec<Suppression> = self.list_latest_decisions("suppressions")?;
        values.retain(|value| {
            value.scope_kind == star_contracts::management::SuppressionScope::Local
        });
        values.extend(self.list_projection_documents("shared_suppressions")?);
        values.sort_by(|left, right| left.suppression_id.cmp(&right.suppression_id));
        Ok(values)
    }

    fn put_baseline(
        &self,
        baseline: &Baseline,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        if baseline.project_id != self.project_id
            || baseline.scope_kind != star_contracts::management::BaselineScope::Local
            || baseline.reason.trim().is_empty()
            || validate_baseline(baseline).is_err()
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "Baseline invariant failed",
            ));
        }
        validate_decision_strings([baseline.reason.as_str()])?;
        let fingerprint =
            versioned_fingerprint("star.baseline-revision", 1, baseline).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "Baseline fingerprint failed",
                )
            })?;
        self.put_versioned_decision(
            "baselines",
            baseline.baseline_id.as_str(),
            baseline.revision,
            expected_revision,
            baseline,
            "baseline.changed",
            &fingerprint,
        )
    }

    fn list_baselines(&self) -> Result<Vec<Baseline>, RepositoryError> {
        let mut values: Vec<Baseline> = self.list_latest_decisions("baselines")?;
        values.retain(|value| value.scope_kind == star_contracts::management::BaselineScope::Local);
        values.extend(self.list_projection_documents("shared_baselines")?);
        values.sort_by(|left, right| left.baseline_id.cmp(&right.baseline_id));
        Ok(values)
    }

    fn put_disposition(
        &self,
        disposition: &Disposition,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let duplicate_shape = matches!(
            disposition.decision,
            star_contracts::management::DispositionDecision::Duplicate
        ) == disposition.duplicate_of_finding_id.is_some();
        if disposition.reason.trim().is_empty() || !duplicate_shape {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "Disposition invariant failed",
            ));
        }
        validate_decision_strings([
            disposition.reason_code.as_str(),
            disposition.reason.as_str(),
            disposition.provenance.as_str(),
        ])?;
        let fingerprint = versioned_fingerprint("star.disposition-revision", 1, disposition)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "Disposition fingerprint failed",
                )
            })?;
        self.put_versioned_decision(
            "dispositions",
            disposition.disposition_id.as_str(),
            disposition.revision,
            expected_revision,
            disposition,
            "disposition.changed",
            &fingerprint,
        )
    }

    fn list_dispositions(&self) -> Result<Vec<Disposition>, RepositoryError> {
        self.list_latest_decisions("dispositions")
    }

    fn save_patch_set(&self, patch_set: &PatchSet) -> Result<(), RepositoryError> {
        if patch_set.project_id != self.project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "PatchSet crosses a ProjectId partition",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        insert_document(
            &transaction,
            "patch_sets",
            "patch_set_id",
            patch_set.patch_set_id.as_str(),
            patch_set,
        )?;
        append_event(
            &transaction,
            match patch_set.status {
                star_contracts::management::PatchSetStatus::Proposed => "patch.prepared",
                star_contracts::management::PatchSetStatus::Applied => "patch.applied",
                star_contracts::management::PatchSetStatus::PartiallyApplied => {
                    "patch.partially_applied"
                }
                star_contracts::management::PatchSetStatus::Failed => "patch.failed",
                star_contracts::management::PatchSetStatus::Reverted => "patch.reverted",
            },
            Some(&self.project_id),
            &patch_set.patch_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn save_change_plan(&self, change_plan: &ChangePlan) -> Result<(), RepositoryError> {
        if change_plan.project_id != self.project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "ChangePlan crosses a ProjectId partition",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        insert_document(
            &transaction,
            "change_plans",
            "change_plan_id",
            change_plan.change_plan_id.as_str(),
            change_plan,
        )?;
        let change_fingerprint = versioned_fingerprint("star.change-plan-event", 1, change_plan)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "ChangePlan event fingerprint failed",
                )
            })?;
        append_event(
            &transaction,
            "change_plan.created",
            Some(&self.project_id),
            &change_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn list_change_plans(&self) -> Result<Vec<ChangePlan>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            "SELECT document_json FROM change_plans ORDER BY entity_id",
            [],
        )
    }

    fn import_local_state(
        &self,
        bundle: &LocalStateBundle,
        expected_store_revision: u64,
    ) -> Result<(), RepositoryError> {
        validate_local_state_bundle(bundle, &PersistenceRedactor::for_current_user()).map_err(
            |_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "local state bundle invariant failed",
                )
            },
        )?;
        if bundle.project_id != self.project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "local state bundle crosses a ProjectId partition",
            ));
        }
        for value in &bundle.local_suppressions {
            validate_suppression(value).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported Suppression invariant failed",
                )
            })?;
            validate_decision_strings([
                value.selector.as_str(),
                value.reason_code.as_str(),
                value.reason.as_str(),
                value.provenance.as_str(),
            ])?;
        }
        for value in &bundle.local_baselines {
            validate_baseline(value).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported Baseline invariant failed",
                )
            })?;
            validate_decision_strings([value.reason.as_str()])?;
        }
        for value in &bundle.local_dispositions {
            let duplicate_shape = matches!(
                value.decision,
                star_contracts::management::DispositionDecision::Duplicate
            ) == value.duplicate_of_finding_id.is_some();
            if value.reason.trim().is_empty() || !duplicate_shape {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported Disposition invariant failed",
                ));
            }
            validate_decision_strings([
                value.reason_code.as_str(),
                value.reason.as_str(),
                value.provenance.as_str(),
            ])?;
        }
        for value in &bundle.local_suppressions_v2 {
            if value.project_id != self.project_id || value.clone().seal().as_ref() != Ok(value) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported SuppressionV2 invariant failed",
                ));
            }
        }
        for value in &bundle.local_baselines_v2 {
            if value.project_id != self.project_id || value.clone().seal().as_ref() != Ok(value) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported BaselineV2 invariant failed",
                ));
            }
        }
        for value in &bundle.local_dispositions_v2 {
            if value.project_id != self.project_id || value.clone().seal().as_ref() != Ok(value) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "imported DispositionV2 invariant failed",
                ));
            }
        }

        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        let current_revision = get_meta(&transaction, "store_revision")?
            .parse::<u64>()
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "project store revision is invalid",
                )
            })?;
        if current_revision != expected_store_revision {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "local state import store revision is stale",
            ));
        }
        let generation = get_meta_optional(&transaction, "current_generation")
            .map_err(map_sql)?
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::NotFound,
                    "local state import target has no current scan",
                )
            })?;
        let current_findings: Vec<Finding> = query_documents(
            &transaction,
            "SELECT document_json FROM findings WHERE generation_id=?1 ORDER BY entity_id",
            [generation.as_str()],
        )?;
        let current_diagnostics: Vec<DiagnosticV2> = query_documents(
            &transaction,
            "SELECT document_json FROM diagnostics_v2 ORDER BY entity_id",
            [],
        )?;
        let current_validation_runs: Vec<ValidationRunV2> = query_documents(
            &transaction,
            "SELECT document_json FROM validation_runs_v2 ORDER BY entity_id",
            [],
        )?;
        for (table, entity_id) in bundle
            .local_suppressions
            .iter()
            .map(|value| ("suppressions", value.suppression_id.as_str()))
            .chain(
                bundle
                    .local_baselines
                    .iter()
                    .map(|value| ("baselines", value.baseline_id.as_str())),
            )
            .chain(
                bundle
                    .local_dispositions
                    .iter()
                    .map(|value| ("dispositions", value.disposition_id.as_str())),
            )
            .chain(
                bundle
                    .active_change_plans
                    .iter()
                    .map(|value| ("change_plans", value.change_plan_id.as_str())),
            )
        {
            if local_state_entity_exists(&transaction, table, entity_id)? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local state import entity already exists",
                ));
            }
        }
        for (table, entity_id, revision) in bundle
            .local_suppressions_v2
            .iter()
            .map(|value| {
                (
                    "suppressions_v2",
                    value.suppression_id.as_str(),
                    value.revision,
                )
            })
            .chain(
                bundle
                    .local_baselines_v2
                    .iter()
                    .map(|value| ("baselines_v2", value.baseline_id.as_str(), value.revision)),
            )
            .chain(bundle.local_dispositions_v2.iter().map(|value| {
                (
                    "dispositions_v2",
                    value.disposition_id.as_str(),
                    value.revision,
                )
            }))
        {
            let storage_key = format!("{entity_id}:{revision}");
            if local_state_entity_exists(&transaction, table, &storage_key)? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local state import M3 entity already exists",
                ));
            }
        }
        for disposition in &bundle.local_dispositions {
            if !generation_entity_exists(
                &transaction,
                "findings",
                &generation,
                disposition.finding_id.as_str(),
            )? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local Disposition target Finding is not current",
                ));
            }
            if let Some(duplicate_id) = disposition.duplicate_of_finding_id.as_ref()
                && !generation_entity_exists(
                    &transaction,
                    "findings",
                    &generation,
                    duplicate_id.as_str(),
                )?
            {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local Disposition duplicate target Finding is not current",
                ));
            }
        }
        for suppression in &bundle.local_suppressions_v2 {
            if !suppression_v2_subject_matches(
                suppression,
                &current_findings,
                &current_diagnostics,
                &current_validation_runs,
                &bundle.source_revision_id,
                &bundle.effective_config_fingerprint,
            ) || suppression.review_evidence_refs.iter().any(|reference| {
                !diagnostic_evidence_ref_matches(
                    reference,
                    &current_findings,
                    &current_diagnostics,
                    &current_validation_runs,
                    &bundle.project_id,
                    &bundle.source_revision_id,
                    &bundle.effective_config_fingerprint,
                )
            }) {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local SuppressionV2 subject or evidence is not current",
                ));
            }
        }
        for baseline in &bundle.local_baselines_v2 {
            if !baseline_v2_subject_matches(
                baseline,
                &current_validation_runs,
                &bundle.source_revision_id,
                &bundle.effective_config_fingerprint,
            ) {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local BaselineV2 subject or rule set is not current",
                ));
            }
        }
        for disposition in &bundle.local_dispositions_v2 {
            if !disposition_v2_subject_matches(
                disposition,
                &current_findings,
                &current_diagnostics,
                &current_validation_runs,
                &bundle.source_revision_id,
                &bundle.effective_config_fingerprint,
            ) {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local DispositionV2 target subject is not current",
                ));
            }
            if let Some(duplicate_id) = disposition.duplicate_of_finding_id.as_ref()
                && !generation_entity_exists(
                    &transaction,
                    "findings",
                    &generation,
                    duplicate_id.as_str(),
                )?
            {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local DispositionV2 duplicate target Finding is not current",
                ));
            }
            for reference in &disposition.review_evidence_refs {
                if !diagnostic_evidence_ref_matches(
                    reference,
                    &current_findings,
                    &current_diagnostics,
                    &current_validation_runs,
                    &bundle.project_id,
                    &bundle.source_revision_id,
                    &bundle.effective_config_fingerprint,
                ) {
                    return Err(repository_error(
                        RepositoryErrorCategory::RevisionConflict,
                        "local DispositionV2 evidence target is not current",
                    ));
                }
            }
        }
        for baseline in &bundle.local_baselines {
            if !generation_entity_exists(
                &transaction,
                "workspace_snapshots",
                &generation,
                baseline.workspace_snapshot_id.as_str(),
            )? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "local Baseline target snapshot is not current",
                ));
            }
        }
        for plan in &bundle.active_change_plans {
            if !generation_entity_exists(
                &transaction,
                "workspace_snapshots",
                &generation,
                plan.target_workspace_snapshot_id.as_str(),
            )? {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "active ChangePlan target is not current",
                ));
            }
            for finding_id in &plan.finding_refs {
                if !generation_entity_exists(
                    &transaction,
                    "findings",
                    &generation,
                    finding_id.as_str(),
                )? {
                    return Err(repository_error(
                        RepositoryErrorCategory::RevisionConflict,
                        "active ChangePlan target is not current",
                    ));
                }
            }
        }

        for value in &bundle.local_suppressions {
            insert_local_state_revision(
                &transaction,
                "suppressions",
                value.suppression_id.as_str(),
                value.revision,
                value,
            )?;
        }
        for value in &bundle.local_baselines {
            insert_local_state_revision(
                &transaction,
                "baselines",
                value.baseline_id.as_str(),
                value.revision,
                value,
            )?;
        }
        for value in &bundle.local_dispositions {
            insert_local_state_revision(
                &transaction,
                "dispositions",
                value.disposition_id.as_str(),
                value.revision,
                value,
            )?;
        }
        for value in &bundle.local_suppressions_v2 {
            insert_immutable_document(
                &transaction,
                "suppressions_v2",
                &format!("{}:{}", value.suppression_id.as_str(), value.revision),
                value,
            )?;
        }
        for value in &bundle.local_baselines_v2 {
            insert_immutable_document(
                &transaction,
                "baselines_v2",
                &format!("{}:{}", value.baseline_id.as_str(), value.revision),
                value,
            )?;
        }
        for value in &bundle.local_dispositions_v2 {
            insert_immutable_document(
                &transaction,
                "dispositions_v2",
                &format!("{}:{}", value.disposition_id.as_str(), value.revision),
                value,
            )?;
        }
        for value in &bundle.active_change_plans {
            insert_document(
                &transaction,
                "change_plans",
                "change_plan_id",
                value.change_plan_id.as_str(),
                value,
            )?;
        }
        append_event(
            &transaction,
            "local_state.imported",
            Some(&self.project_id),
            &bundle.content_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn get_patch_set(
        &self,
        patch_set_id: &PatchSetId,
    ) -> Result<Option<PatchSet>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM patch_sets WHERE entity_id=?1",
            patch_set_id.as_str(),
        )
    }

    fn save_validation(
        &self,
        result: &ValidationResult,
        decision: &GateDecision,
    ) -> Result<(), RepositoryError> {
        if result.project_id != self.project_id
            || gate_project_id(decision) != Some(&self.project_id)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "validation crosses a ProjectId partition",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        insert_document(
            &transaction,
            "validation_results",
            "validation_result_id",
            result.validation_result_id.as_str(),
            result,
        )?;
        insert_document(
            &transaction,
            "gate_decisions",
            "gate_id",
            decision.gate_id.as_str(),
            decision,
        )?;
        append_event(
            &transaction,
            "validation_result.recorded",
            Some(&self.project_id),
            &result.result_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn save_check_graph_evidence(
        &self,
        evidence: CheckGraphEvidenceTransaction<'_>,
    ) -> Result<(), RepositoryError> {
        let CheckGraphEvidenceTransaction {
            runs,
            results,
            diagnostics,
            decision,
            bundle,
            review_pack,
            rework_directive,
            source_bound_findings,
        } = evidence;
        if runs.is_empty()
            || runs.iter().any(|run| {
                run.project_id != self.project_id || run.clone().seal().as_ref() != Ok(run)
            })
            || diagnostics.iter().any(|diagnostic| {
                diagnostic.project_id != self.project_id
                    || diagnostic.clone().seal().as_ref() != Ok(diagnostic)
            })
            || results.iter().any(|result| {
                result.project_id != self.project_id
                    || result.clone().seal(runs).as_ref() != Ok(result)
            })
            || decision.clone().seal(runs, diagnostics, results).as_ref() != Ok(decision)
            || bundle
                .clone()
                .seal(runs, results, diagnostics, decision)
                .as_ref()
                != Ok(bundle)
            || review_pack.clone().seal(bundle, decision).as_ref() != Ok(review_pack)
            || rework_directive
                .is_some_and(|directive| directive.clone().seal(decision).as_ref() != Ok(directive))
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "M3 evidence invariant or Project partition failed",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        if let Some(import) = source_bound_findings {
            let generation = get_meta_optional(&transaction, "current_generation")
                .map_err(map_sql)?
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::Invalid,
                        "source-bound finding import has no current generation",
                    )
                })?;
            let scan: Option<ScanRun> = query_documents(
                &transaction,
                "SELECT document_json FROM scan_runs WHERE entity_id=?1 AND generation_id=?2",
                params![import.scan_run_id.as_str(), generation],
            )?
            .into_iter()
            .next();
            let Some(scan) = scan else {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "source-bound finding import scan is stale",
                ));
            };
            let index: Option<CodeIndexSnapshot> = query_documents(
                &transaction,
                "SELECT document_json FROM code_index_snapshots WHERE entity_id=?1 AND generation_id=?2",
                params![import.code_index_snapshot_id.as_str(), generation],
            )?.into_iter().next();
            let Some(index) = index else {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "source-bound finding import CodeIndex is stale",
                ));
            };
            if scan.project_id != self.project_id
                || scan.project_revision_id != *import.project_revision_id
                || scan.workspace_snapshot_id != *import.workspace_snapshot_id
                || index.project_id != self.project_id
                || index.scan_run_id != *import.scan_run_id
                || index.project_revision_id != *import.project_revision_id
                || index.workspace_snapshot_id != *import.workspace_snapshot_id
                || import.findings.iter().any(|finding| {
                    finding.project_id != self.project_id
                        || finding.last_observed_scan_id != *import.scan_run_id
                })
                || import.occurrences.iter().any(|occurrence| {
                    occurrence.scan_run_id != *import.scan_run_id
                        || occurrence.project_revision_id != *import.project_revision_id
                        || occurrence.workspace_snapshot_id != *import.workspace_snapshot_id
                })
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "source-bound finding import does not match current source generation",
                ));
            }
            for finding in import.findings {
                insert_generation_document(
                    &transaction,
                    "findings",
                    "finding_id",
                    finding.finding_id.as_str(),
                    &generation,
                    finding,
                )?;
            }
            for occurrence in import.occurrences {
                insert_generation_document(
                    &transaction,
                    "occurrences",
                    "occurrence_id",
                    occurrence.occurrence_id.as_str(),
                    &generation,
                    occurrence,
                )?;
            }
            for report in import.reports {
                if !report.is_current_schema()
                    || report.project_id != self.project_id
                    || report.project_revision_ref != import.project_revision_id.as_str()
                    || report.workspace_snapshot_ref != import.workspace_snapshot_id.as_str()
                    || report.code_index_snapshot_ref != import.code_index_snapshot_id.as_str()
                {
                    return Err(repository_error(
                        RepositoryErrorCategory::Invalid,
                        "static-analysis import report is not source-bound",
                    ));
                }
                insert_immutable_document(
                    &transaction,
                    "static_analysis_import_reports",
                    &report.report_id,
                    report,
                )?;
            }
        }
        for run in runs {
            insert_immutable_document(
                &transaction,
                "validation_runs_v2",
                run.validation_run_id.as_str(),
                run,
            )?;
        }
        for diagnostic in diagnostics {
            insert_immutable_document(
                &transaction,
                "diagnostics_v2",
                diagnostic.diagnostic_id.as_str(),
                diagnostic,
            )?;
        }
        for result in results {
            insert_immutable_document(
                &transaction,
                "validation_results_v2",
                result.validation_result_id.as_str(),
                result,
            )?;
        }
        insert_immutable_document(
            &transaction,
            "gate_decisions_v2",
            decision.gate_id.as_str(),
            decision,
        )?;
        insert_immutable_document(
            &transaction,
            "evidence_bundles_v2",
            bundle.evidence_bundle_id.as_str(),
            bundle,
        )?;
        if let Some(directive) = rework_directive {
            insert_immutable_document(
                &transaction,
                "rework_directives_v1",
                directive.rework_directive_id.as_str(),
                directive,
            )?;
        }
        insert_immutable_document(
            &transaction,
            "review_packs_v1",
            review_pack.review_pack_id.as_str(),
            review_pack,
        )?;
        append_event(
            &transaction,
            "validation.evidence_bundle.recorded",
            Some(&self.project_id),
            &bundle.bundle_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn get_validation_run_v2(
        &self,
        validation_run_id: &ValidationRunId,
    ) -> Result<Option<ValidationRunV2>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM validation_runs_v2 WHERE entity_id=?1",
            validation_run_id.as_str(),
        )
    }

    fn list_validation_runs_v2(&self) -> Result<Vec<ValidationRunV2>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM validation_runs_v2 ORDER BY json_extract(document_json, '$.started_at') ASC, entity_id ASC",
        )
    }

    fn get_validation_result_v2(
        &self,
        validation_result_id: &ValidationResultId,
    ) -> Result<Option<ValidationResultV2>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM validation_results_v2 WHERE entity_id=?1",
            validation_result_id.as_str(),
        )
    }

    fn list_validation_results_v2(&self) -> Result<Vec<ValidationResultV2>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM validation_results_v2 ORDER BY json_extract(document_json, '$.created_at') ASC, entity_id ASC",
        )
    }

    fn get_diagnostic_v2(
        &self,
        diagnostic_id: &DiagnosticId,
    ) -> Result<Option<DiagnosticV2>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM diagnostics_v2 WHERE entity_id=?1",
            diagnostic_id.as_str(),
        )
    }

    fn list_diagnostics_v2(&self) -> Result<Vec<DiagnosticV2>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM diagnostics_v2 ORDER BY json_extract(document_json, '$.sequence') ASC, entity_id ASC",
        )
    }

    fn list_static_analysis_import_reports(
        &self,
    ) -> Result<Vec<StaticAnalysisImportReport>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM static_analysis_import_reports ORDER BY entity_id",
        )
    }

    fn get_gate_decision_v2(
        &self,
        gate_id: &GateId,
    ) -> Result<Option<GateDecisionV2>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM gate_decisions_v2 WHERE entity_id=?1",
            gate_id.as_str(),
        )
    }

    fn list_gate_decisions_v2(&self) -> Result<Vec<GateDecisionV2>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM gate_decisions_v2 ORDER BY json_extract(document_json, '$.decided_at') ASC, entity_id ASC",
        )
    }

    fn get_evidence_bundle_v2(
        &self,
        evidence_bundle_id: &EvidenceBundleId,
    ) -> Result<Option<EvidenceBundleV2>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM evidence_bundles_v2 WHERE entity_id=?1",
            evidence_bundle_id.as_str(),
        )
    }

    fn list_evidence_bundles_v2(&self) -> Result<Vec<EvidenceBundleV2>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM evidence_bundles_v2 ORDER BY json_extract(document_json, '$.created_at') ASC, entity_id ASC",
        )
    }

    fn get_review_pack_v1(
        &self,
        review_pack_id: &ReviewPackId,
    ) -> Result<Option<ReviewPackV1>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM review_packs_v1 WHERE entity_id=?1",
            review_pack_id.as_str(),
        )
    }

    fn list_review_packs_v1(&self) -> Result<Vec<ReviewPackV1>, RepositoryError> {
        self.list_m3_documents(
            "SELECT document_json FROM review_packs_v1 ORDER BY json_extract(document_json, '$.created_at') ASC, entity_id ASC",
        )
    }

    fn put_baseline_v2(&self, baseline: &BaselineV2) -> Result<(), RepositoryError> {
        self.put_m3_decision_document(
            "baselines_v2",
            baseline.baseline_id.as_str(),
            baseline,
            baseline.clone().seal().as_ref() == Ok(baseline),
            "validation.baseline_v2.recorded",
            &baseline.set_fingerprint,
        )
    }

    fn list_baselines_v2(&self) -> Result<Vec<BaselineV2>, RepositoryError> {
        self.list_latest_m3_decision_documents("baselines_v2", "$.baseline_id")
    }

    fn put_suppression_v2(&self, suppression: &SuppressionV2) -> Result<(), RepositoryError> {
        self.put_m3_decision_document(
            "suppressions_v2",
            suppression.suppression_id.as_str(),
            suppression,
            suppression.clone().seal().as_ref() == Ok(suppression),
            "validation.suppression_v2.recorded",
            &suppression.content_fingerprint,
        )
    }

    fn get_suppression_v2(
        &self,
        suppression_id: &star_contracts::ids::SuppressionId,
        revision: u64,
    ) -> Result<Option<SuppressionV2>, RepositoryError> {
        if revision == 0 {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "suppression revision must be positive",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let key = format!("{}:{revision}", suppression_id.as_str());
        query_document(
            &connection,
            "SELECT document_json FROM suppressions_v2 WHERE entity_id=?1",
            key.as_str(),
        )
    }

    fn list_suppressions_v2(&self) -> Result<Vec<SuppressionV2>, RepositoryError> {
        self.list_latest_m3_decision_documents("suppressions_v2", "$.suppression_id")
    }

    fn put_disposition_v2(&self, disposition: &DispositionV2) -> Result<(), RepositoryError> {
        self.put_m3_decision_document(
            "dispositions_v2",
            disposition.disposition_id.as_str(),
            disposition,
            disposition.clone().seal().as_ref() == Ok(disposition),
            "validation.disposition_v2.recorded",
            &disposition.content_fingerprint,
        )
    }

    fn list_dispositions_v2(&self) -> Result<Vec<DispositionV2>, RepositoryError> {
        self.list_latest_m3_decision_documents("dispositions_v2", "$.disposition_id")
    }

    fn save_managed_registry_resolution(
        &self,
        snapshot: &ManagedRegistrySnapshot,
        consistency_records: &[RegistryConsistencyRecord],
    ) -> Result<(), RepositoryError> {
        let sealed_snapshot = snapshot.clone().seal().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "managed registry snapshot invariant failed",
            )
        })?;
        let snapshot_ref = sealed_snapshot.reference().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "managed registry snapshot reference failed",
            )
        })?;
        if sealed_snapshot != *snapshot || snapshot.owner_project_id != self.project_id {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "managed registry snapshot Project partition failed",
            ));
        }
        for record in consistency_records {
            if record.clone().seal().as_ref() != Ok(record)
                || record.registry_snapshot_ref != snapshot_ref
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "registry consistency record invariant failed",
                ));
            }
        }
        let mut canonical_records = consistency_records.to_vec();
        canonical_records.sort_by(|left, right| {
            left.registry_consistency_record_id
                .cmp(&right.registry_consistency_record_id)
        });
        if canonical_records != consistency_records
            || canonical_records.windows(2).any(|pair| {
                pair[0].registry_consistency_record_id == pair[1].registry_consistency_record_id
            })
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "registry consistency records are not canonical",
            ));
        }
        if let Some(existing) =
            self.get_managed_registry_snapshot(&snapshot.managed_registry_snapshot_id)?
        {
            if existing != *snapshot {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "managed registry snapshot identity conflict",
                ));
            }
            let mut existing_records =
                self.list_registry_consistency_records(&snapshot.managed_registry_snapshot_id)?;
            existing_records.sort_by(|left, right| {
                left.registry_consistency_record_id
                    .cmp(&right.registry_consistency_record_id)
            });
            if existing_records == canonical_records {
                return Ok(());
            }
        }
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        insert_immutable_document(
            &transaction,
            "managed_registry_snapshots_v2",
            snapshot.managed_registry_snapshot_id.as_str(),
            snapshot,
        )?;
        for record in consistency_records {
            insert_immutable_document(
                &transaction,
                "registry_consistency_records_v1",
                record.registry_consistency_record_id.as_str(),
                record,
            )?;
        }
        append_event(
            &transaction,
            "managed_registry.snapshot.projected",
            Some(&self.project_id),
            &snapshot.content_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn latest_managed_registry_snapshot(
        &self,
    ) -> Result<Option<ManagedRegistrySnapshot>, RepositoryError> {
        let mut snapshots = self.list_m3_documents(
            "SELECT document_json FROM managed_registry_snapshots_v2 ORDER BY rowid ASC",
        )?;
        Ok(snapshots.pop())
    }

    fn get_managed_registry_snapshot(
        &self,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Option<ManagedRegistrySnapshot>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_document(
            &connection,
            "SELECT document_json FROM managed_registry_snapshots_v2 WHERE entity_id=?1",
            snapshot_id.as_str(),
        )
    }

    fn list_registry_consistency_records(
        &self,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Vec<RegistryConsistencyRecord>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let mut statement = connection
            .prepare(
                "SELECT document_json FROM registry_consistency_records_v1
                 WHERE json_extract(document_json, '$.registry_snapshot_ref.document_id')=?1
                 ORDER BY entity_id ASC",
            )
            .map_err(map_sql)?;
        let rows = statement
            .query_map(params![snapshot_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(map_sql)?;
        rows.map(|row| {
            let document = row.map_err(map_sql)?;
            serde_json::from_str(&document).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "registry consistency record JSON is corrupt",
                )
            })
        })
        .collect()
    }

    fn artifact_refs_for_scan(
        &self,
        scan_run_id: &ScanRunId,
    ) -> Result<Vec<ArtifactRef>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let run: Option<ScanRun> = query_document(
            &connection,
            "SELECT document_json FROM scan_runs WHERE entity_id=?1 ORDER BY generation_id DESC LIMIT 1",
            scan_run_id.as_str(),
        )?;
        Ok(run.map(|run| run.artifact_refs).unwrap_or_default())
    }

    fn reindex_artifact_refs(&self, artifact_refs: &[ArtifactRef]) -> Result<(), RepositoryError> {
        let mut ordered = BTreeMap::new();
        for artifact in artifact_refs {
            artifact.validate().map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "artifact reference invariant is invalid",
                )
            })?;
            if artifact.project_id.as_ref() != Some(&self.project_id) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "artifact reference crosses a ProjectId partition",
                ));
            }
            if let Some(existing) =
                ordered.insert(artifact.artifact_id.as_str().to_owned(), artifact.clone())
                && existing != *artifact
            {
                return Err(repository_error(
                    RepositoryErrorCategory::RevisionConflict,
                    "artifact reference identity resolves to conflicting content",
                ));
            }
        }
        let payload_fingerprint = versioned_fingerprint(
            "star.artifact-reindex",
            1,
            &ordered.values().collect::<Vec<_>>(),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "artifact reindex fingerprint failed",
            )
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql)?;
        for artifact in ordered.values() {
            insert_immutable_document(
                &transaction,
                "artifact_refs",
                artifact.artifact_id.as_str(),
                artifact,
            )?;
        }
        append_event(
            &transaction,
            "artifact.index.rebuilt",
            Some(&self.project_id),
            &payload_fingerprint,
        )?;
        bump_revision(&transaction)?;
        transaction.commit().map_err(map_sql)
    }

    fn list_artifact_refs(&self) -> Result<Vec<ArtifactRef>, RepositoryError> {
        let connection = self.connection.lock().map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "project store lock is unavailable",
            )
        })?;
        query_documents(
            &connection,
            "SELECT document_json FROM artifact_refs ORDER BY entity_id",
            [],
        )
    }
}

fn open_store(
    path: &Path,
    scope: StoreScope,
    product_version: &str,
    schema: &str,
) -> Result<Connection, RepositoryError> {
    let parent = path.parent().ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::Unavailable,
            "management store has no parent directory",
        )
    })?;
    create_private_dir(parent)?;
    let connection = Connection::open(path).map_err(map_sql)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(map_sql)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, MANAGEMENT_DOCUMENT_MAX_BYTES)
        .map_err(map_sql)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024)
        .map_err(map_sql)?;
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_sql)?;
    if version > MANAGEMENT_STORE_VERSION {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "management store was written by a future product version",
        ));
    }
    if version > 0 && version < MANAGEMENT_STORE_VERSION {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "management store requires an explicit offline migration",
        ));
    }
    connection
        .execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA trusted_schema=OFF;
             PRAGMA temp_store=MEMORY;",
        )
        .map_err(map_sql)?;
    if version == 0 {
        connection.execute_batch(schema).map_err(map_sql)?;
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .map_err(map_sql)?;
        connection
            .pragma_update(None, "user_version", MANAGEMENT_STORE_VERSION)
            .map_err(map_sql)?;
        set_meta(&connection, "store_id", ManagementStoreId::new().as_str()).map_err(map_sql)?;
        set_meta(
            &connection,
            "store_scope",
            &serde_json::to_string(&scope).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "store scope serialization failed",
                )
            })?,
        )
        .map_err(map_sql)?;
        set_meta(&connection, "store_revision", "0").map_err(map_sql)?;
        set_meta(&connection, "generation", "1").map_err(map_sql)?;
        set_meta(&connection, "created_by_product_version", product_version).map_err(map_sql)?;
        set_meta(&connection, "last_verified_at", "").map_err(map_sql)?;
    } else if version == MANAGEMENT_STORE_VERSION {
        ensure_current_store_shape(&connection, &scope)?;
    }
    let application_id: i32 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(map_sql)?;
    if application_id != APPLICATION_ID {
        return Err(repository_error(
            RepositoryErrorCategory::Corrupt,
            "file is not a Star-Control management store",
        ));
    }
    let previous_clean = get_meta_optional(&connection, "last_clean_shutdown")
        .map_err(map_sql)?
        .as_deref()
        == Some("true");
    if !previous_clean {
        verify_connection(&connection)?;
    }
    set_meta(&connection, "last_clean_shutdown", "false").map_err(map_sql)?;
    set_meta(
        &connection,
        "last_opened_by_product_version",
        product_version,
    )
    .map_err(map_sql)?;
    apply_owner_system_dacl(path)?;
    for extension in ["db-wal", "db-shm"] {
        let auxiliary = path.with_extension(extension);
        if auxiliary.exists() {
            apply_owner_system_dacl(&auxiliary)?;
        }
    }
    Ok(connection)
}

fn ensure_current_store_shape(
    connection: &Connection,
    scope: &StoreScope,
) -> Result<(), RepositoryError> {
    let event_project_column = match scope {
        StoreScope::Global => "project_id TEXT",
        StoreScope::Project { .. } => "project_id TEXT NOT NULL",
    };
    connection
        .execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS events(
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                {event_project_column},
                payload_fingerprint TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                store_revision INTEGER NOT NULL CHECK(store_revision > 0),
                previous_event_hash TEXT,
                event_hash TEXT NOT NULL UNIQUE
            ) STRICT;"
        ))
        .map_err(map_sql)?;
    ensure_event_hash_columns(connection)?;
    if matches!(scope, StoreScope::Global) {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS project_catalog_snapshots(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS planning_bundles(
                    task_spec_id TEXT PRIMARY KEY,
                    idempotency_key TEXT NOT NULL UNIQUE,
                    input_fingerprint TEXT NOT NULL,
                    bundle_fingerprint TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS planning_bundle_revisions(
                    task_spec_id TEXT NOT NULL,
                    bundle_revision INTEGER NOT NULL CHECK(bundle_revision > 0),
                    idempotency_key TEXT NOT NULL UNIQUE,
                    input_fingerprint TEXT NOT NULL,
                    bundle_fingerprint TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(task_spec_id, bundle_revision)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS development_records_v1(
                    record_kind TEXT NOT NULL,
                    record_id TEXT NOT NULL,
                    revision INTEGER NOT NULL CHECK(revision > 0),
                    project_id TEXT,
                    state TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(record_kind, record_id, revision)
                 ) STRICT;",
            )
            .map_err(map_sql)?;
        connection
            .execute_batch(
                "INSERT OR IGNORE INTO planning_bundle_revisions(
                    task_spec_id, bundle_revision, idempotency_key,
                    input_fingerprint, bundle_fingerprint, document_json
                 )
                 SELECT task_spec_id,
                        MAX(
                            CAST(json_extract(document_json, '$.task_spec.revision') AS INTEGER),
                            CAST(json_extract(document_json, '$.scope_revision.revision') AS INTEGER),
                            CAST(json_extract(document_json, '$.impact_analysis.revision') AS INTEGER),
                            CAST(json_extract(document_json, '$.validation_plan.revision') AS INTEGER)
                        ),
                        idempotency_key, input_fingerprint,
                        bundle_fingerprint, document_json
                 FROM planning_bundles;",
            )
            .map_err(map_sql)?;
        let has_idempotency_key = connection
            .prepare("PRAGMA table_info(coordinated_operations)")
            .and_then(|mut statement| {
                let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
                for column in columns {
                    if column.is_ok_and(|column| column == "idempotency_key") {
                        return Ok(true);
                    }
                }
                Ok(false)
            })
            .map_err(map_sql)?;
        if !has_idempotency_key {
            connection
                .execute_batch(
                    "ALTER TABLE coordinated_operations ADD COLUMN idempotency_key TEXT;
                     UPDATE coordinated_operations
                     SET idempotency_key='legacy-' || operation_id
                     WHERE idempotency_key IS NULL;",
                )
                .map_err(map_sql)?;
        }
        connection
            .execute_batch(
                "CREATE UNIQUE INDEX IF NOT EXISTS coordinated_operations_idempotency_key
                 ON coordinated_operations(idempotency_key);",
            )
            .map_err(map_sql)?;
    } else {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS shared_suppressions(
                    entity_id TEXT PRIMARY KEY,
                    revision INTEGER NOT NULL CHECK(revision > 0),
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS shared_baselines(
                    entity_id TEXT PRIMARY KEY,
                    revision INTEGER NOT NULL CHECK(revision > 0),
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS code_index_snapshots(
                    entity_id TEXT NOT NULL,
                    generation_id TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(entity_id, generation_id)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS source_entries(
                    entity_id TEXT NOT NULL,
                    generation_id TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(entity_id, generation_id)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS index_entities(
                    entity_id TEXT NOT NULL,
                    generation_id TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(entity_id, generation_id)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS index_edges(
                    entity_id TEXT NOT NULL,
                    generation_id TEXT NOT NULL,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                    PRIMARY KEY(entity_id, generation_id)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS validation_runs_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS validation_results_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS diagnostics_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS gate_decisions_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS evidence_bundles_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS review_packs_v1(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS rework_directives_v1(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS baselines_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS suppressions_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS dispositions_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS managed_registry_snapshots_v2(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS registry_consistency_records_v1(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS static_analysis_import_reports(
                    entity_id TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL CHECK(json_valid(document_json))
                 ) STRICT;",
            )
            .map_err(map_sql)?;
        for table in ["suppressions", "baselines", "dispositions"] {
            ensure_versioned_decision_table(connection, table)?;
        }
    }
    Ok(())
}

fn ensure_versioned_decision_table(
    connection: &Connection,
    table: &str,
) -> Result<(), RepositoryError> {
    let has_revision = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut statement| {
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            for column in columns {
                if column.is_ok_and(|column| column == "revision") {
                    return Ok(true);
                }
            }
            Ok(false)
        })
        .map_err(map_sql)?;
    if has_revision {
        return Ok(());
    }
    let legacy = format!("{table}_legacy_single_revision");
    connection
        .execute_batch(&format!(
            "BEGIN IMMEDIATE;
             ALTER TABLE {table} RENAME TO {legacy};
             CREATE TABLE {table}(
                entity_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision > 0),
                document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                PRIMARY KEY(entity_id, revision)
             ) STRICT;
             INSERT INTO {table}(entity_id, revision, document_json)
             SELECT entity_id, CAST(json_extract(document_json, '$.revision') AS INTEGER), document_json
             FROM {legacy};
             DROP TABLE {legacy};
             COMMIT;"
        ))
        .map_err(map_sql)
}

fn ensure_event_hash_columns(connection: &Connection) -> Result<(), RepositoryError> {
    let mut columns = std::collections::BTreeSet::new();
    let mut statement = connection
        .prepare("PRAGMA table_info(events)")
        .map_err(map_sql)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(map_sql)?;
    for row in rows {
        columns.insert(row.map_err(map_sql)?);
    }
    drop(statement);
    if !columns.contains("event_hash") {
        connection
            .execute_batch(
                "ALTER TABLE events ADD COLUMN store_revision INTEGER;
                 ALTER TABLE events ADD COLUMN previous_event_hash TEXT;
                 ALTER TABLE events ADD COLUMN event_hash TEXT;",
            )
            .map_err(map_sql)?;
        let mut select = connection
            .prepare(
                "SELECT sequence, event_id, event_type, project_id, payload_fingerprint, occurred_at
                 FROM events ORDER BY sequence",
            )
            .map_err(map_sql)?;
        let rows = select
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(map_sql)?;
        let mut existing = Vec::new();
        for row in rows {
            existing.push(row.map_err(map_sql)?);
        }
        drop(select);
        let transaction = connection.unchecked_transaction().map_err(map_sql)?;
        let mut previous: Option<Sha256Hash> = None;
        for (sequence, event_id, event_type, project_id, payload, occurred_at) in existing {
            let hash = management_event_hash(
                sequence,
                &event_id,
                &event_type,
                project_id.as_deref(),
                &payload,
                &occurred_at,
                sequence,
                previous.as_ref().map(Sha256Hash::as_str),
            )?;
            transaction
                .execute(
                    "UPDATE events
                     SET store_revision=?2, previous_event_hash=?3, event_hash=?4
                     WHERE sequence=?1",
                    params![
                        sequence,
                        sequence,
                        previous.as_ref().map(Sha256Hash::as_str),
                        hash.as_str(),
                    ],
                )
                .map_err(map_sql)?;
            previous = Some(hash);
        }
        transaction.commit().map_err(map_sql)?;
    }
    connection
        .execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS events_event_hash ON events(event_hash);")
        .map_err(map_sql)
}

fn status_from_connection(
    connection: &Connection,
) -> Result<ManagementStoreStatus, RepositoryError> {
    let scope: StoreScope =
        serde_json::from_str(&get_meta(connection, "store_scope")?).map_err(|_| {
            repository_error(RepositoryErrorCategory::Corrupt, "store scope is invalid")
        })?;
    let last_verified = get_meta_optional(connection, "last_verified_at")
        .map_err(map_sql)?
        .filter(|value| !value.is_empty())
        .map(|value| DateTime::parse_from_rfc3339(&value).map(|value| value.with_timezone(&Utc)))
        .transpose()
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "last verified timestamp is invalid",
            )
        })?;
    Ok(ManagementStoreStatus {
        schema_id: "star.management-store-status".to_owned(),
        schema_version: 1,
        store_id: ManagementStoreId::parse(get_meta(connection, "store_id")?).map_err(|_| {
            repository_error(RepositoryErrorCategory::Corrupt, "store ID is invalid")
        })?,
        store_scope: scope,
        management_store_version: MANAGEMENT_STORE_VERSION,
        min_reader_version: MANAGEMENT_STORE_VERSION,
        writer_version: MANAGEMENT_STORE_VERSION,
        store_revision: get_meta(connection, "store_revision")?
            .parse()
            .map_err(|_| {
                repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid")
            })?,
        generation: get_meta(connection, "generation")?.parse().map_err(|_| {
            repository_error(RepositoryErrorCategory::Corrupt, "generation is invalid")
        })?,
        created_by_product_version: get_meta(connection, "created_by_product_version")?,
        last_opened_by_product_version: get_meta(connection, "last_opened_by_product_version")?,
        last_clean_shutdown: get_meta_optional(connection, "last_clean_shutdown")
            .map_err(map_sql)?
            .as_deref()
            == Some("true"),
        integrity_state: IntegrityState::Healthy,
        open_mode: StoreOpenMode::ReadWrite,
        last_verified_at: last_verified,
        last_backup_ref: None,
        redaction_contract_version: REDACTION_CONTRACT_VERSION,
    })
}

#[cfg(test)]
thread_local! {
    static VERIFY_CONNECTION_CALLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_verify_connection_calls() {
    VERIFY_CONNECTION_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
fn verify_connection_calls() -> u64 {
    VERIFY_CONNECTION_CALLS.with(std::cell::Cell::get)
}

fn verify_connection(connection: &Connection) -> Result<(), RepositoryError> {
    #[cfg(test)]
    VERIFY_CONNECTION_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(map_sql)?;
    if result != "ok" {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "management store integrity check failed",
        ));
    }
    verify_event_chain(connection)
}

fn verify_event_chain(connection: &Connection) -> Result<(), RepositoryError> {
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, project_id, payload_fingerprint, occurred_at,
                    store_revision, previous_event_hash, event_hash
             FROM events ORDER BY sequence",
        )
        .map_err(map_sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(map_sql)?;
    let current_store_revision: i64 = get_meta(connection, "store_revision")?
        .parse::<u64>()
        .ok()
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid"))?;
    let mut previous: Option<Sha256Hash> = None;
    for (expected_sequence, row) in (1_i64..).zip(rows) {
        let (
            sequence,
            event_id,
            event_type,
            project_id,
            payload,
            occurred_at,
            store_revision,
            previous_hash,
            event_hash,
        ) = row.map_err(map_sql)?;
        let valid_shape = sequence == expected_sequence
            && EventId::parse(&event_id).is_ok()
            && payload.parse::<Sha256Hash>().is_ok()
            && DateTime::parse_from_rfc3339(&occurred_at).is_ok()
            && store_revision > 0
            && store_revision <= current_store_revision
            && previous_hash.as_deref() == previous.as_ref().map(Sha256Hash::as_str);
        let expected_hash = management_event_hash(
            sequence,
            &event_id,
            &event_type,
            project_id.as_deref(),
            &payload,
            &occurred_at,
            store_revision,
            previous_hash.as_deref(),
        )?;
        if !valid_shape || expected_hash.as_str() != event_hash {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "management event sequence or hash chain is invalid",
            ));
        }
        previous = Some(expected_hash);
    }
    Ok(())
}

fn backup_connection(connection: &Connection, destination: &Path) -> Result<(), RepositoryError> {
    if let Some(parent) = destination.parent() {
        create_private_dir(parent)?;
    }
    let mut destination_connection = Connection::open(destination).map_err(map_sql)?;
    let backup = Backup::new(connection, &mut destination_connection).map_err(map_sql)?;
    backup
        .run_to_completion(64, Duration::from_millis(5), None)
        .map_err(map_sql)?;
    drop(backup);
    destination_connection
        .execute_batch("PRAGMA synchronous=FULL;")
        .map_err(map_sql)?;
    apply_owner_system_dacl(destination)
}

fn get_meta(connection: &Connection, key: &str) -> Result<String, RepositoryError> {
    get_meta_optional(connection, key)
        .map_err(map_sql)?
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "store metadata is missing",
            )
        })
}

fn get_meta_optional(connection: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    connection
        .query_row("SELECT value FROM metadata WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
}

fn set_meta(connection: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn bump_revision(transaction: &Transaction<'_>) -> Result<u64, RepositoryError> {
    let current: u64 = get_meta(transaction, "store_revision")?
        .parse()
        .map_err(|_| repository_error(RepositoryErrorCategory::Corrupt, "revision is invalid"))?;
    let next = current.checked_add(1).ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "store revision overflowed",
        )
    })?;
    set_meta(transaction, "store_revision", &next.to_string()).map_err(map_sql)?;
    Ok(next)
}

fn idempotency_result(
    transaction: &Transaction<'_>,
    key: &str,
    payload_fingerprint: &Sha256Hash,
) -> Result<Option<String>, RepositoryError> {
    let existing: Option<(String, String)> = transaction
        .query_row(
            "SELECT payload_fingerprint, result_json FROM idempotency WHERE idempotency_key=?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_sql)?;
    match existing {
        Some((fingerprint, result)) if fingerprint == payload_fingerprint.as_str() => {
            Ok(Some(result))
        }
        Some(_) => Err(repository_error(
            RepositoryErrorCategory::IdempotencyConflict,
            "idempotency key was reused with a different payload",
        )),
        None => Ok(None),
    }
}

fn store_idempotency(
    transaction: &Transaction<'_>,
    key: &str,
    payload_fingerprint: &Sha256Hash,
    result: &str,
) -> Result<(), RepositoryError> {
    transaction
        .execute(
            "INSERT INTO idempotency(idempotency_key, payload_fingerprint, result_json, created_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![
                key,
                payload_fingerprint.as_str(),
                result,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(map_sql)?;
    Ok(())
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: &str,
    project_id: Option<&ProjectId>,
    payload_fingerprint: &Sha256Hash,
) -> Result<(), RepositoryError> {
    let sequence: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events",
            [],
            |row| row.get(0),
        )
        .map_err(map_sql)?;
    let previous_event_hash: Option<String> = transaction
        .query_row(
            "SELECT event_hash FROM events ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sql)?;
    let store_revision: i64 = get_meta(transaction, "store_revision")?
        .parse::<u64>()
        .ok()
        .and_then(|revision| revision.checked_add(1))
        .and_then(|revision| i64::try_from(revision).ok())
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "event store revision is invalid",
            )
        })?;
    let event_id = EventId::new();
    let occurred_at = Utc::now().to_rfc3339();
    let event_hash = management_event_hash(
        sequence,
        event_id.as_str(),
        event_type,
        project_id.map(ProjectId::as_str),
        payload_fingerprint.as_str(),
        &occurred_at,
        store_revision,
        previous_event_hash.as_deref(),
    )?;
    transaction
        .execute(
            "INSERT INTO events(
                sequence, event_id, event_type, project_id, payload_fingerprint, occurred_at,
                store_revision, previous_event_hash, event_hash
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                sequence,
                event_id.as_str(),
                event_type,
                project_id.map(ProjectId::as_str),
                payload_fingerprint.as_str(),
                occurred_at,
                store_revision,
                previous_event_hash,
                event_hash.as_str(),
            ],
        )
        .map_err(map_sql)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn management_event_hash(
    sequence: i64,
    event_id: &str,
    event_type: &str,
    project_id: Option<&str>,
    payload_fingerprint: &str,
    occurred_at: &str,
    store_revision: i64,
    previous_event_hash: Option<&str>,
) -> Result<Sha256Hash, RepositoryError> {
    canonical_sha256(&serde_json::json!({
        "schema_id":"star.management-event",
        "schema_version":1,
        "sequence":sequence,
        "event_id":event_id,
        "event_type":event_type,
        "project_id":project_id,
        "payload_fingerprint":payload_fingerprint,
        "occurred_at":occurred_at,
        "store_revision":store_revision,
        "previous_event_hash":previous_event_hash,
    }))
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management event hash generation failed",
        )
    })
}

fn local_state_entity_exists(
    connection: &Connection,
    table: &str,
    entity_id: &str,
) -> Result<bool, RepositoryError> {
    connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE entity_id=?1)"),
            [entity_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sql)
}

fn generation_entity_exists(
    connection: &Connection,
    table: &str,
    generation: &str,
    entity_id: &str,
) -> Result<bool, RepositoryError> {
    connection
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {table} WHERE generation_id=?1 AND entity_id=?2)"
            ),
            params![generation, entity_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sql)
}

fn insert_local_state_revision<T: Serialize>(
    transaction: &Transaction<'_>,
    table: &str,
    entity_id: &str,
    revision: u64,
    value: &T,
) -> Result<(), RepositoryError> {
    let revision = i64::try_from(revision).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "local state revision exceeds the storage range",
        )
    })?;
    let document = serde_json::to_string(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "local state document serialization failed",
        )
    })?;
    transaction
        .execute(
            &format!("INSERT INTO {table}(entity_id, revision, document_json) VALUES(?1, ?2, ?3)"),
            params![entity_id, revision, document],
        )
        .map_err(map_sql)?;
    Ok(())
}

fn insert_document<T: Serialize>(
    transaction: &Transaction<'_>,
    table: &str,
    _id_column: &str,
    id: &str,
    value: &T,
) -> Result<(), RepositoryError> {
    let document = serde_json::to_string(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "management document serialization failed",
        )
    })?;
    let sql = format!(
        "INSERT INTO {table}(entity_id, document_json) VALUES(?1, ?2)
         ON CONFLICT(entity_id) DO UPDATE SET document_json=excluded.document_json"
    );
    transaction
        .execute(&sql, params![id, document])
        .map_err(map_sql)?;
    Ok(())
}

fn insert_first_observation<T: Serialize>(
    transaction: &Transaction<'_>,
    table: &str,
    id: &str,
    value: &T,
) -> Result<(), RepositoryError> {
    let document = serde_json::to_string(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "immutable observation serialization failed",
        )
    })?;
    transaction
        .execute(
            &format!(
                "INSERT INTO {table}(entity_id, document_json) VALUES(?1, ?2)
                 ON CONFLICT(entity_id) DO NOTHING"
            ),
            params![id, document],
        )
        .map_err(map_sql)?;
    Ok(())
}

fn insert_immutable_document<T: Serialize>(
    transaction: &Transaction<'_>,
    table: &str,
    id: &str,
    value: &T,
) -> Result<(), RepositoryError> {
    let document = serde_json::to_string(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "immutable management document serialization failed",
        )
    })?;
    let existing: Option<String> = transaction
        .query_row(
            &format!("SELECT document_json FROM {table} WHERE entity_id=?1"),
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sql)?;
    if let Some(existing) = existing {
        if existing != document {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "immutable management document identity conflict",
            ));
        }
        return Ok(());
    }
    transaction
        .execute(
            &format!("INSERT INTO {table}(entity_id, document_json) VALUES(?1, ?2)"),
            params![id, document],
        )
        .map_err(map_sql)?;
    Ok(())
}

fn insert_generation_document<T: Serialize>(
    transaction: &Transaction<'_>,
    table: &str,
    _id_column: &str,
    id: &str,
    generation: &str,
    value: &T,
) -> Result<(), RepositoryError> {
    let document = serde_json::to_string(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "scan document serialization failed",
        )
    })?;
    let sql = format!(
        "INSERT INTO {table}(entity_id, generation_id, document_json) VALUES(?1, ?2, ?3)
         ON CONFLICT(entity_id, generation_id) DO UPDATE SET document_json=excluded.document_json"
    );
    transaction
        .execute(&sql, params![id, generation, document])
        .map_err(map_sql)?;
    Ok(())
}

fn query_document<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    parameter: &str,
) -> Result<Option<T>, RepositoryError> {
    let json: Option<String> = connection
        .query_row(sql, [parameter], |row| row.get(0))
        .optional()
        .map_err(map_sql)?;
    deserialize_optional(json)
}

fn deserialize_optional<T: serde::de::DeserializeOwned>(
    json: Option<String>,
) -> Result<Option<T>, RepositoryError> {
    json.map(|json| {
        serde_json::from_str(&json).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored management document is invalid",
            )
        })
    })
    .transpose()
}

fn protected_snapshot_reasons(
    connection: &Connection,
) -> Result<(BTreeMap<String, String>, Option<String>), RepositoryError> {
    let mut protected = BTreeMap::new();
    let mut baselines: Vec<Baseline> = query_documents(
        connection,
        "SELECT document_json FROM baselines UNION ALL SELECT document_json FROM shared_baselines",
        [],
    )?;
    for baseline in baselines.drain(..) {
        protected.insert(
            baseline.workspace_snapshot_id.as_str().to_owned(),
            "BASELINE_HOLD".to_owned(),
        );
    }
    for plan in
        query_documents::<ChangePlan, _>(connection, "SELECT document_json FROM change_plans", [])?
    {
        protected.insert(
            plan.target_workspace_snapshot_id.as_str().to_owned(),
            "CHANGE_EVIDENCE_HOLD".to_owned(),
        );
    }
    for patch in
        query_documents::<PatchSet, _>(connection, "SELECT document_json FROM patch_sets", [])?
    {
        protected.insert(
            patch.base_workspace_snapshot_id.as_str().to_owned(),
            "CHANGE_EVIDENCE_HOLD".to_owned(),
        );
        if let Some(snapshot_id) = patch.applied_workspace_snapshot_id {
            protected.insert(
                snapshot_id.as_str().to_owned(),
                "CHANGE_EVIDENCE_HOLD".to_owned(),
            );
        }
    }
    for result in query_documents::<ValidationResult, _>(
        connection,
        "SELECT document_json FROM validation_results",
        [],
    )? {
        protected.insert(
            result.workspace_snapshot_id.as_str().to_owned(),
            "VALIDATION_RESULT_HOLD".to_owned(),
        );
    }
    for decision in query_documents::<GateDecision, _>(
        connection,
        "SELECT document_json FROM gate_decisions",
        [],
    )? {
        protected.insert(
            gate_workspace_snapshot_id(&decision)?.as_str().to_owned(),
            "GATE_DECISION_HOLD".to_owned(),
        );
    }

    let runs: Vec<ValidationRunV2> = query_documents(
        connection,
        "SELECT document_json FROM validation_runs_v2",
        [],
    )?;
    let results: Vec<ValidationResultV2> = query_documents(
        connection,
        "SELECT document_json FROM validation_results_v2",
        [],
    )?;
    let gates: Vec<GateDecisionV2> = query_documents(
        connection,
        "SELECT document_json FROM gate_decisions_v2",
        [],
    )?;
    let review_packs: Vec<ReviewPackV1> =
        query_documents(connection, "SELECT document_json FROM review_packs_v1", [])?;
    let reviewed_gates = review_packs
        .iter()
        .map(|pack| {
            (
                pack.authoritative_gate_decision_ref.gate_id.clone(),
                pack.authoritative_gate_decision_ref.revision,
            )
        })
        .collect::<BTreeSet<_>>();
    if reviewed_gates.iter().any(|(gate_id, revision)| {
        !gates
            .iter()
            .any(|gate| &gate.gate_id == gate_id && gate.revision == *revision)
    }) {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "ReviewPack references a missing GateDecisionV2",
        ));
    }

    let mut binding_snapshots = BTreeMap::new();
    for binding in runs
        .iter()
        .map(|run| &run.subject_binding)
        .chain(results.iter().map(|result| &result.subject_binding))
    {
        if let Some(existing) = binding_snapshots.insert(
            binding.binding_fingerprint.clone(),
            binding.workspace_snapshot_id.as_str().to_owned(),
        ) && existing != binding.workspace_snapshot_id.as_str()
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "subject binding fingerprint maps to conflicting workspace snapshots",
            ));
        }
    }

    let mut unscoped_reason = None;
    for gate in &gates {
        let reviewed = reviewed_gates.contains(&(gate.gate_id.clone(), gate.revision));
        let reason = if reviewed {
            "REVIEW_PACK_HOLD"
        } else {
            "GATE_DECISION_HOLD"
        };
        let mut matched = false;
        for reference in gate
            .required_run_refs
            .iter()
            .chain(gate.satisfied_run_refs.iter())
        {
            let run = runs
                .iter()
                .find(|run| {
                    run.validation_run_id == reference.validation_run_id
                        && run.revision == reference.revision
                })
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::IntegrityFailed,
                        "GateDecisionV2 references a missing ValidationRunV2",
                    )
                })?;
            protected.insert(
                run.subject_binding
                    .workspace_snapshot_id
                    .as_str()
                    .to_owned(),
                reason.to_owned(),
            );
            matched = true;
        }
        for reference in &gate.validation_result_refs {
            let result = results
                .iter()
                .find(|result| {
                    result.validation_result_id.as_str() == reference.document_id
                        && result.revision == reference.revision
                })
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::IntegrityFailed,
                        "GateDecisionV2 references a missing ValidationResultV2",
                    )
                })?;
            protected.insert(
                result
                    .subject_binding
                    .workspace_snapshot_id
                    .as_str()
                    .to_owned(),
                reason.to_owned(),
            );
            matched = true;
        }
        if !matched {
            unscoped_reason = Some(reason.to_owned());
        }
    }

    for baseline in
        query_documents::<BaselineV2, _>(connection, "SELECT document_json FROM baselines_v2", [])?
    {
        let reason = if baseline.active {
            "ACTIVE_BASELINE_HOLD"
        } else {
            "BASELINE_HOLD"
        };
        if let Some(snapshot_id) = binding_snapshots.get(&baseline.subject_binding_fingerprint) {
            protected.insert(snapshot_id.clone(), reason.to_owned());
        } else {
            unscoped_reason = Some(reason.to_owned());
        }
    }
    for suppression in query_documents::<SuppressionV2, _>(
        connection,
        "SELECT document_json FROM suppressions_v2",
        [],
    )?
    .into_iter()
    .filter(|suppression| suppression.status == SuppressionStatus::Active)
    {
        if let Some(binding) = suppression.subject_binding_constraint.as_ref() {
            if let Some(snapshot_id) = binding_snapshots.get(binding) {
                protected.insert(snapshot_id.clone(), "ACTIVE_SUPPRESSION_HOLD".to_owned());
            } else {
                unscoped_reason = Some("ACTIVE_SUPPRESSION_HOLD".to_owned());
            }
        } else {
            // An unbound active rule/finding suppression can apply across scan
            // generations. Keep the protection unscoped instead of guessing a
            // narrower generation and deleting evidence that still supports it.
            unscoped_reason = Some("ACTIVE_SUPPRESSION_HOLD".to_owned());
        }
    }
    let active_legacy_suppression = query_documents::<Suppression, _>(
        connection,
        "SELECT current.document_json
         FROM suppressions AS current
         WHERE current.revision = (
             SELECT MAX(candidate.revision)
             FROM suppressions AS candidate
             WHERE candidate.entity_id = current.entity_id
         )
         UNION ALL
         SELECT document_json FROM shared_suppressions",
        [],
    )?
    .into_iter()
    .any(|suppression| suppression.status == SuppressionStatus::Active);
    let active_disposition = query_documents::<Disposition, _>(
        connection,
        "SELECT current.document_json
         FROM dispositions AS current
         WHERE current.revision = (
             SELECT MAX(candidate.revision)
             FROM dispositions AS candidate
             WHERE candidate.entity_id = current.entity_id
         )",
        [],
    )?
    .into_iter()
    .any(|disposition| disposition.status == DispositionStatus::Active)
        || query_documents::<DispositionV2, _>(
            connection,
            "SELECT document_json FROM dispositions_v2",
            [],
        )?
        .into_iter()
        .any(|disposition| disposition.status == DispositionStatus::Active);
    if active_legacy_suppression {
        unscoped_reason = Some("ACTIVE_SUPPRESSION_HOLD".to_owned());
    }
    if active_disposition {
        unscoped_reason = Some("ACTIVE_DISPOSITION_HOLD".to_owned());
    }
    Ok((protected, unscoped_reason))
}

fn protected_snapshot_ids(connection: &Connection) -> Result<BTreeSet<String>, RepositoryError> {
    Ok(protected_snapshot_reasons(connection)?
        .0
        .into_keys()
        .collect())
}

const RETENTION_GENERATION_TABLES: [&str; 10] = [
    "canonical_sources",
    "symbols",
    "symbol_references",
    "code_index_snapshots",
    "source_entries",
    "index_entities",
    "index_edges",
    "findings",
    "occurrences",
    "scan_runs",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RetentionGenerationAggregate {
    rows: u64,
    bytes: u64,
    max_document_bytes: u64,
}

fn retention_generation_inventory(
    connection: &Connection,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<BTreeMap<String, RetentionGenerationAggregate>, RepositoryError> {
    let mut inventory = BTreeMap::<String, RetentionGenerationAggregate>::new();
    for table in RETENTION_GENERATION_TABLES {
        if is_cancelled() {
            return Err(repository_error(
                RepositoryErrorCategory::Unavailable,
                "retention planning was cancelled",
            ));
        }
        let sql = format!(
            "SELECT generation_id, COUNT(*), COALESCE(SUM(length(document_json)), 0), \
                    COALESCE(MAX(length(document_json)), 0) \
             FROM {table} GROUP BY generation_id"
        );
        let mut statement = connection.prepare(&sql).map_err(map_sql)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(map_sql)?;
        for row in rows {
            if is_cancelled() {
                return Err(repository_error(
                    RepositoryErrorCategory::Unavailable,
                    "retention planning was cancelled",
                ));
            }
            let (generation_id, table_rows, table_bytes, table_max_document_bytes) =
                row.map_err(map_sql)?;
            let table_rows = u64::try_from(table_rows).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "retention row estimate is invalid",
                )
            })?;
            let table_bytes = u64::try_from(table_bytes).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "retention byte estimate is invalid",
                )
            })?;
            let table_max_document_bytes =
                u64::try_from(table_max_document_bytes).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "retention maximum document size is invalid",
                    )
                })?;
            let aggregate = inventory.entry(generation_id).or_default();
            aggregate.rows = aggregate.rows.checked_add(table_rows).ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "retention row estimate overflowed",
                )
            })?;
            aggregate.bytes = aggregate.bytes.checked_add(table_bytes).ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "retention byte estimate overflowed",
                )
            })?;
            aggregate.max_document_bytes =
                aggregate.max_document_bytes.max(table_max_document_bytes);
        }
    }
    Ok(inventory)
}

fn retention_generation_aggregate_for_plan(
    connection: &Connection,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    inventory: &mut Option<BTreeMap<String, RetentionGenerationAggregate>>,
    generation_id: &str,
) -> Result<RetentionGenerationAggregate, RepositoryError> {
    if inventory.is_none() {
        *inventory = Some(retention_generation_inventory(connection, is_cancelled)?);
    }
    Ok(inventory
        .as_ref()
        .and_then(|inventory| inventory.get(generation_id))
        .copied()
        .unwrap_or_default())
}

fn retention_document_stats(
    connection: &Connection,
    table: &str,
    predicate: &str,
    parameters: &[&dyn rusqlite::ToSql],
) -> Result<(u64, u64), RepositoryError> {
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(length(document_json)), 0) FROM {table} WHERE {predicate}"
    );
    let (rows, bytes): (i64, i64) = connection
        .query_row(&sql, parameters, |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(map_sql)?;
    Ok((
        u64::try_from(rows).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "retention row estimate is invalid",
            )
        })?,
        u64::try_from(bytes).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "retention byte estimate is invalid",
            )
        })?,
    ))
}

fn retention_document_max_bytes(
    connection: &Connection,
    table: &str,
    predicate: &str,
    parameters: &[&dyn rusqlite::ToSql],
) -> Result<u64, RepositoryError> {
    let sql =
        format!("SELECT COALESCE(MAX(length(document_json)), 0) FROM {table} WHERE {predicate}");
    let bytes: i64 = connection
        .query_row(&sql, parameters, |row| row.get(0))
        .map_err(map_sql)?;
    u64::try_from(bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "retention maximum document size is invalid",
        )
    })
}

fn retention_generation_stats(
    connection: &Connection,
    generation_id: &str,
) -> Result<(u64, u64), RepositoryError> {
    RETENTION_GENERATION_TABLES
        .iter()
        .try_fold((0_u64, 0_u64), |(rows, bytes), table| {
            let (table_rows, table_bytes) =
                retention_document_stats(connection, table, "generation_id=?1", &[&generation_id])?;
            Ok((
                rows.checked_add(table_rows).ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention row estimate overflowed",
                    )
                })?,
                bytes.checked_add(table_bytes).ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention byte estimate overflowed",
                    )
                })?,
            ))
        })
}

#[derive(Clone, Copy)]
struct RetentionBatchLimits {
    rows: u64,
    bytes: u64,
}

#[derive(Clone, Copy)]
struct RetentionCandidateProgress {
    rows: u64,
    bytes: u64,
}

#[derive(Default)]
struct RetentionRowSelection {
    rows: Vec<(String, i64)>,
    bytes: u64,
}

fn select_retention_rows_for_predicate(
    transaction: &Transaction<'_>,
    table: &str,
    predicate: &str,
    parameters: &[&dyn rusqlite::ToSql],
    limits: RetentionBatchLimits,
    selected: &mut RetentionRowSelection,
) -> Result<bool, RepositoryError> {
    let sql = format!(
        "SELECT rowid, length(document_json) FROM {table} WHERE {predicate} ORDER BY rowid"
    );
    let mut statement = transaction.prepare(&sql).map_err(map_sql)?;
    let mut rows = statement.query(parameters).map_err(map_sql)?;
    while let Some(row) = rows.next().map_err(map_sql)? {
        let row_id: i64 = row.get(0).map_err(map_sql)?;
        let document_bytes =
            u64::try_from(row.get::<_, i64>(1).map_err(map_sql)?).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "retention document size is invalid",
                )
            })?;
        if document_bytes > limits.bytes {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "one retention row exceeds the configured byte limit",
            ));
        }
        let next_bytes = selected.bytes.checked_add(document_bytes).ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "retention batch byte count overflowed",
            )
        })?;
        if selected.rows.len() as u64 >= limits.rows || next_bytes > limits.bytes {
            return Ok(true);
        }
        selected.rows.push((table.to_owned(), row_id));
        selected.bytes = next_bytes;
        if selected.rows.len() as u64 == limits.rows || selected.bytes == limits.bytes {
            return Ok(true);
        }
    }
    Ok(false)
}

fn delete_selected_retention_rows(
    transaction: &Transaction<'_>,
    selected: &[(String, i64)],
) -> Result<u64, RepositoryError> {
    let mut by_table = BTreeMap::<&str, Vec<i64>>::new();
    for (table, row_id) in selected {
        by_table.entry(table.as_str()).or_default().push(*row_id);
    }
    let mut changed = 0_u64;
    for (table, row_ids) in by_table {
        let sql = format!("DELETE FROM {table} WHERE rowid=?1");
        let mut statement = transaction.prepare(&sql).map_err(map_sql)?;
        for row_id in row_ids {
            changed = changed
                .checked_add(statement.execute([row_id]).map_err(map_sql)? as u64)
                .ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention deleted row count overflowed",
                    )
                })?;
        }
    }
    if changed != selected.len() as u64 {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "retention row selection no longer matches source",
        ));
    }
    Ok(changed)
}

fn retention_candidate_remaining_stats(
    connection: &Connection,
    candidate: &RetentionCandidateV2,
) -> Result<(u64, u64), RepositoryError> {
    match candidate.target_kind {
        RetentionTargetKindV2::ScanGeneration => retention_generation_stats(
            connection,
            candidate.generation_id.as_deref().ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "scan retention candidate omits generation",
                )
            })?,
        ),
        RetentionTargetKindV2::ResolvedFinding => {
            let generation_id = candidate.generation_id.as_deref().ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "resolved finding candidate omits generation",
                )
            })?;
            let entity_id = candidate.entity_id.as_deref().ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "resolved finding candidate omits identity",
                )
            })?;
            let finding = retention_document_stats(
                connection,
                "findings",
                "generation_id=?1 AND entity_id=?2",
                &[&generation_id, &entity_id],
            )?;
            let occurrence = retention_document_stats(
                connection,
                "occurrences",
                "generation_id=?1 AND json_extract(document_json, '$.finding_id')=?2",
                &[&generation_id, &entity_id],
            )?;
            Ok((
                finding.0.checked_add(occurrence.0).ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention remaining row count overflowed",
                    )
                })?,
                finding.1.checked_add(occurrence.1).ok_or_else(|| {
                    repository_error(
                        RepositoryErrorCategory::QuotaExceeded,
                        "retention remaining byte count overflowed",
                    )
                })?,
            ))
        }
        RetentionTargetKindV2::LocalSuppression | RetentionTargetKindV2::LocalDisposition => {
            let entity_id = candidate.entity_id.as_deref().ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "local decision candidate omits identity",
                )
            })?;
            let entity_revision = i64::try_from(candidate.entity_revision.ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "local decision candidate omits revision",
                )
            })?)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "local decision candidate revision is invalid",
                )
            })?;
            match candidate.retention_class.as_str() {
                "local_suppression_v1" => retention_document_stats(
                    connection,
                    "suppressions",
                    "entity_id=?1 AND revision=?2",
                    &[&entity_id, &entity_revision],
                ),
                "local_suppression_v2" => retention_document_stats(
                    connection,
                    "suppressions_v2",
                    "entity_id=?1",
                    &[&entity_id],
                ),
                "local_disposition_v1" => retention_document_stats(
                    connection,
                    "dispositions",
                    "entity_id=?1 AND revision=?2",
                    &[&entity_id, &entity_revision],
                ),
                "local_disposition_v2" => retention_document_stats(
                    connection,
                    "dispositions_v2",
                    "entity_id=?1",
                    &[&entity_id],
                ),
                _ => Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "local decision retention class is invalid",
                )),
            }
        }
    }
}

fn retention_candidate_id(
    project_id: &ProjectId,
    target_kind: RetentionTargetKindV2,
    generation_id: Option<&str>,
    entity_id: Option<&str>,
    entity_revision: Option<u64>,
    reason_code: &str,
) -> Result<String, RepositoryError> {
    versioned_fingerprint(
        "star.management-retention-candidate",
        2,
        &serde_json::json!({
            "project_id":project_id,
            "target_kind":target_kind,
            "generation_id":generation_id,
            "entity_id":entity_id,
            "entity_revision":entity_revision,
            "reason_code":reason_code,
        }),
    )
    .map(|fingerprint| fingerprint.as_str().to_owned())
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "retention candidate fingerprint failed",
        )
    })
}

fn local_decision_candidate(
    project_id: &ProjectId,
    target_kind: RetentionTargetKindV2,
    entity_id: &str,
    entity_revision: u64,
    retention_class: &str,
    reason_code: &str,
    estimated_bytes: u64,
) -> Result<RetentionCandidateV2, RepositoryError> {
    Ok(RetentionCandidateV2 {
        candidate_id: retention_candidate_id(
            project_id,
            target_kind,
            None,
            Some(entity_id),
            Some(entity_revision),
            reason_code,
        )?,
        project_id: project_id.clone(),
        target_kind,
        generation_id: None,
        entity_id: Some(entity_id.to_owned()),
        entity_revision: Some(entity_revision),
        scan_run_id: None,
        retention_class: retention_class.to_owned(),
        reason_code: reason_code.to_owned(),
        estimated_rows: 1,
        estimated_bytes,
    })
}

fn push_retention_candidate_or_protection(
    candidates: &mut Vec<RetentionCandidateV2>,
    protected_targets: &mut Vec<RetentionProtectedTargetV1>,
    policy: &RetentionPolicyV2,
    candidate: RetentionCandidateV2,
    protection_reason: Option<&str>,
) {
    let limit_reason = (matches!(
        candidate.target_kind,
        RetentionTargetKindV2::LocalSuppression | RetentionTargetKindV2::LocalDisposition
    ) && candidate.estimated_bytes > policy.max_batch_bytes)
        .then_some("RETENTION_ROW_EXCEEDS_BATCH_LIMIT");
    if let Some(reason_code) = protection_reason.or(limit_reason) {
        protected_targets.push(RetentionProtectedTargetV1 {
            project_id: candidate.project_id,
            target_kind: candidate.target_kind,
            target_ref: candidate.candidate_id,
            reason_code: reason_code.to_owned(),
            estimated_rows: candidate.estimated_rows,
            estimated_bytes: candidate.estimated_bytes,
        });
    } else {
        candidates.push(candidate);
    }
}

fn verify_project_relations(
    connection: &Connection,
    project_id: &ProjectId,
) -> Result<(), RepositoryError> {
    let revisions: Vec<ProjectRevision> = query_documents(
        connection,
        "SELECT document_json FROM project_revisions",
        [],
    )?;
    let revision_ids: std::collections::BTreeSet<_> = revisions
        .iter()
        .filter(|revision| revision.project_id == *project_id)
        .map(|revision| revision.project_revision_id.as_str().to_owned())
        .collect();
    if revisions.len() != revision_ids.len() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "project revision partition or identity relation is invalid",
        ));
    }
    let snapshots: Vec<WorkspaceSnapshot> = query_documents(
        connection,
        "SELECT document_json FROM workspace_snapshots",
        [],
    )?;
    let snapshot_ids: std::collections::BTreeSet<_> = snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.project_id == *project_id
                && revision_ids.contains(snapshot.project_revision_id.as_str())
        })
        .map(|snapshot| snapshot.workspace_snapshot_id.as_str().to_owned())
        .collect();
    if snapshots.len() != snapshot_ids.len() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "workspace snapshot partition or revision relation is invalid",
        ));
    }
    let artifact_refs: Vec<ArtifactRef> = query_documents(
        connection,
        "SELECT document_json FROM artifact_refs ORDER BY entity_id",
        [],
    )?;
    if artifact_refs.iter().any(|artifact| {
        artifact.project_id.as_ref() != Some(project_id) || artifact.validate().is_err()
    }) {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "artifact reference partition or invariant is invalid",
        ));
    }
    let Some(generation) = get_meta_optional(connection, "current_generation").map_err(map_sql)?
    else {
        let _ = protected_snapshot_ids(connection)?;
        return Ok(());
    };
    let runs: Vec<ScanRun> = query_documents(
        connection,
        "SELECT document_json FROM scan_runs WHERE generation_id=?1",
        [&generation],
    )?;
    if runs.len() != 1
        || runs[0].project_id != *project_id
        || !revision_ids.contains(runs[0].project_revision_id.as_str())
        || !snapshot_ids.contains(runs[0].workspace_snapshot_id.as_str())
        || runs[0].status != star_contracts::management::ScanStatus::Succeeded
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "current scan generation header is invalid",
        ));
    }
    let sources: Vec<CanonicalSource> = query_documents(
        connection,
        "SELECT document_json FROM canonical_sources WHERE generation_id=?1",
        [&generation],
    )?;
    let source_ids: std::collections::BTreeSet<_> = sources
        .iter()
        .filter(|source| {
            source.project_id == *project_id
                && source
                    .project_revision_id
                    .as_ref()
                    .is_none_or(|id| revision_ids.contains(id.as_str()))
                && source
                    .workspace_snapshot_id
                    .as_ref()
                    .is_none_or(|id| snapshot_ids.contains(id.as_str()))
        })
        .map(|source| source.canonical_source_id.as_str().to_owned())
        .collect();
    if sources.len() != source_ids.len()
        || sources.iter().any(|source| {
            source
                .generated_from_refs
                .iter()
                .any(|id| !source_ids.contains(id.as_str()))
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "canonical source partition or relation is invalid",
        ));
    }
    let code_indexes: Vec<CodeIndexSnapshot> = query_documents(
        connection,
        "SELECT document_json FROM code_index_snapshots WHERE generation_id=?1",
        [&generation],
    )?;
    if code_indexes.len() > 1
        || code_indexes.first().is_some_and(|index| {
            index.project_id != *project_id
                || index.scan_run_id != runs[0].scan_run_id
                || index.generation_id.as_str() != generation
                || !revision_ids.contains(index.project_revision_id.as_str())
                || !snapshot_ids.contains(index.workspace_snapshot_id.as_str())
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "code index generation header is invalid",
        ));
    }
    let source_entries: Vec<SourceEntry> = query_documents(
        connection,
        "SELECT document_json FROM source_entries WHERE generation_id=?1",
        [&generation],
    )?;
    if source_entries.iter().any(|entry| {
        entry.owner_project_id != *project_id
            || !source_ids.contains(entry.canonical_source_id.as_str())
    }) || code_indexes
        .first()
        .is_some_and(|index| index.counts.sources != source_entries.len() as u64)
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "code index source-entry partition is invalid",
        ));
    }
    let symbols: Vec<Symbol> = query_documents(
        connection,
        "SELECT document_json FROM symbols WHERE generation_id=?1",
        [&generation],
    )?;
    let symbol_ids: std::collections::BTreeSet<_> = symbols
        .iter()
        .filter(|symbol| {
            symbol.project_id == *project_id
                && source_ids.contains(symbol.canonical_source_id.as_str())
                && snapshot_ids.contains(symbol.workspace_snapshot_id.as_str())
                && symbol.scan_run_id == runs[0].scan_run_id
        })
        .map(|symbol| symbol.symbol_id.as_str().to_owned())
        .collect();
    if symbols.len() != symbol_ids.len() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "symbol partition or relation is invalid",
        ));
    }
    let index_entities: Vec<IndexEntity> = query_documents(
        connection,
        "SELECT document_json FROM index_entities WHERE generation_id=?1",
        [&generation],
    )?;
    let entity_keys: BTreeSet<_> = index_entities
        .iter()
        .map(|entity| entity.entity_key.as_str())
        .collect();
    if entity_keys.len() != index_entities.len()
        || index_entities.iter().any(|entity| {
            entity
                .canonical_source_id
                .as_ref()
                .is_some_and(|id| !source_ids.contains(id.as_str()))
                || entity
                    .symbol_id
                    .as_ref()
                    .is_some_and(|id| !symbol_ids.contains(id.as_str()))
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "code index entity partition is invalid",
        ));
    }
    let index_edges: Vec<IndexEdge> = query_documents(
        connection,
        "SELECT document_json FROM index_edges WHERE generation_id=?1",
        [&generation],
    )?;
    let edge_keys: BTreeSet<_> = index_edges
        .iter()
        .map(|edge| edge.edge_key.as_str())
        .collect();
    if edge_keys.len() != index_edges.len()
        || index_edges.iter().any(|edge| {
            !source_ids.contains(edge.evidence_source_id.as_str())
                || edge
                    .to_entity_key
                    .as_ref()
                    .is_some_and(|key| !entity_keys.contains(key.as_str()))
                || (!entity_keys.contains(edge.from_entity_key.as_str())
                    && !edge.from_entity_key.starts_with("source:"))
        })
        || code_indexes
            .first()
            .is_some_and(|index| index.counts.graph_edges != index_edges.len() as u64)
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "code index edge partition is invalid",
        ));
    }
    let references: Vec<SymbolReference> = query_documents(
        connection,
        "SELECT document_json FROM symbol_references WHERE generation_id=?1",
        [&generation],
    )?;
    if references.iter().any(|reference| {
        reference.project_id != *project_id
            || !source_ids.contains(reference.from_source_id.as_str())
            || reference
                .from_symbol_id
                .as_ref()
                .is_some_and(|id| !symbol_ids.contains(id.as_str()))
            || reference
                .to_symbol_id
                .as_ref()
                .is_some_and(|id| !symbol_ids.contains(id.as_str()))
            || reference.scan_run_id != runs[0].scan_run_id
            || !snapshot_ids.contains(reference.workspace_snapshot_id.as_str())
    }) || code_indexes
        .first()
        .is_some_and(|index| index.counts.references != references.len() as u64)
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "symbol reference partition or relation is invalid",
        ));
    }
    let findings: Vec<Finding> = query_documents(
        connection,
        "SELECT document_json FROM findings WHERE generation_id=?1",
        [&generation],
    )?;
    let finding_ids: std::collections::BTreeSet<_> = findings
        .iter()
        .filter(|finding| finding.project_id == *project_id)
        .map(|finding| finding.finding_id.as_str().to_owned())
        .collect();
    if findings.len() != finding_ids.len() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "finding partition is invalid",
        ));
    }
    let occurrences: Vec<Occurrence> = query_documents(
        connection,
        "SELECT document_json FROM occurrences WHERE generation_id=?1",
        [&generation],
    )?;
    if occurrences.iter().any(|occurrence| {
        !finding_ids.contains(occurrence.finding_id.as_str())
            || !source_ids.contains(occurrence.canonical_source_id.as_str())
            || occurrence
                .symbol_id
                .as_ref()
                .is_some_and(|id| !symbol_ids.contains(id.as_str()))
            || occurrence.scan_run_id != runs[0].scan_run_id
            || !revision_ids.contains(occurrence.project_revision_id.as_str())
            || !snapshot_ids.contains(occurrence.workspace_snapshot_id.as_str())
    }) {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "occurrence relation is invalid",
        ));
    }
    let _ = protected_snapshot_ids(connection)?;
    Ok(())
}

fn query_documents<T, P>(
    connection: &Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<T>, RepositoryError>
where
    T: serde::de::DeserializeOwned,
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(map_sql)?;
    let rows = statement
        .query_map(parameters, |row| row.get::<_, String>(0))
        .map_err(map_sql)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(serde_json::from_str(&row.map_err(map_sql)?).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "stored management document is invalid",
            )
        })?);
    }
    Ok(values)
}

const GLOBAL_SCHEMA: &str = r#"
CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE projects(
    project_id TEXT PRIMARY KEY,
    identity_scope TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE project_checkouts(
    checkout_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    root_binding_id TEXT UNIQUE,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
) STRICT;
CREATE INDEX project_checkouts_by_project ON project_checkouts(project_id, checkout_id);
CREATE TABLE project_catalog_snapshots(
    entity_id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL CHECK(json_valid(document_json))
) STRICT;
CREATE TABLE planning_bundles(
    task_spec_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    input_fingerprint TEXT NOT NULL,
    bundle_fingerprint TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json))
) STRICT;
CREATE TABLE planning_bundle_revisions(
    task_spec_id TEXT NOT NULL,
    bundle_revision INTEGER NOT NULL CHECK(bundle_revision > 0),
    idempotency_key TEXT NOT NULL UNIQUE,
    input_fingerprint TEXT NOT NULL,
    bundle_fingerprint TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    PRIMARY KEY(task_spec_id, bundle_revision)
) STRICT;
CREATE TABLE development_records_v1(
    record_kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    project_id TEXT,
    state TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    PRIMARY KEY(record_kind, record_id, revision)
) STRICT;
CREATE TABLE coordinated_operations(
    operation_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL,
    input_fingerprint TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE idempotency(
    idempotency_key TEXT PRIMARY KEY,
    payload_fingerprint TEXT NOT NULL,
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE events(
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    project_id TEXT,
    payload_fingerprint TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    store_revision INTEGER NOT NULL CHECK(store_revision > 0),
    previous_event_hash TEXT,
    event_hash TEXT NOT NULL UNIQUE
) STRICT;
"#;

const PROJECT_SCHEMA: &str = r#"
CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE project_document(
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    project_id TEXT NOT NULL UNIQUE,
    document_json TEXT NOT NULL CHECK(json_valid(document_json))
) STRICT;
CREATE TABLE project_revisions(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE workspace_snapshots(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE scan_runs(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE canonical_sources(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE symbols(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE symbol_references(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE code_index_snapshots(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE source_entries(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE index_entities(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE index_edges(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE findings(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE occurrences(entity_id TEXT NOT NULL, generation_id TEXT NOT NULL, document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, generation_id)) STRICT;
CREATE TABLE suppressions(entity_id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0), document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, revision)) STRICT;
CREATE TABLE baselines(entity_id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0), document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, revision)) STRICT;
CREATE TABLE dispositions(entity_id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0), document_json TEXT NOT NULL CHECK(json_valid(document_json)), PRIMARY KEY(entity_id, revision)) STRICT;
CREATE TABLE shared_suppressions(entity_id TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision > 0), document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE shared_baselines(entity_id TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision > 0), document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE change_plans(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE patch_sets(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE validation_results(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE gate_decisions(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE artifact_refs(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE validation_runs_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE validation_results_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE diagnostics_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE gate_decisions_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE evidence_bundles_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE review_packs_v1(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE rework_directives_v1(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE baselines_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE suppressions_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE dispositions_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE managed_registry_snapshots_v2(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE registry_consistency_records_v1(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE static_analysis_import_reports(entity_id TEXT PRIMARY KEY, document_json TEXT NOT NULL CHECK(json_valid(document_json))) STRICT;
CREATE TABLE participant_receipts(
    operation_id TEXT PRIMARY KEY,
    payload_fingerprint TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json))
) STRICT;
CREATE TABLE idempotency(
    idempotency_key TEXT PRIMARY KEY,
    payload_fingerprint TEXT NOT NULL,
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE events(
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    project_id TEXT NOT NULL,
    payload_fingerprint TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    store_revision INTEGER NOT NULL CHECK(store_revision > 0),
    previous_event_hash TEXT,
    event_hash TEXT NOT NULL UNIQUE
) STRICT;
"#;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootBindingEnvelope {
    schema_version: u32,
    root_binding_id: RootBindingId,
    project_id: ProjectId,
    checkout_id: CheckoutId,
    protection_kind: String,
    ciphertext: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootLocator {
    locator_format_version: u32,
    absolute_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootBindingEnvelopeV1 {
    schema_version: u32,
    root_binding_id: RootBindingId,
    project_id: ProjectId,
    protection_kind: String,
    ciphertext: String,
    created_at: DateTime<Utc>,
}

fn management_global_path(root: &Path) -> PathBuf {
    root.join("global").join("active").join(STORE_FILENAME)
}

fn management_project_path(root: &Path, project_id: &ProjectId) -> PathBuf {
    root.join("projects")
        .join(project_id.as_str())
        .join("active")
        .join(STORE_FILENAME)
}

fn root_binding_path(root: &Path, binding_id: &RootBindingId) -> PathBuf {
    root.join(format!("{}.binding", binding_id.as_str()))
}

fn checked_store_version(connection: &Connection) -> Result<u32, RepositoryError> {
    let application_id: i32 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(map_sql)?;
    if application_id != APPLICATION_ID {
        return Err(repository_error(
            RepositoryErrorCategory::Corrupt,
            "file is not a Star-Control management store",
        ));
    }
    let quick_check: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(map_sql)?;
    if quick_check != "ok" {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "management store failed quick_check",
        ));
    }
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_sql)
}

fn read_v1_root_binding(
    binding_root: &Path,
    binding_id: &RootBindingId,
) -> Result<(RootBindingEnvelopeV1, PathBuf), RepositoryError> {
    let path = root_binding_path(binding_root, binding_id);
    let bytes = fs::read(&path).map_err(map_io)?;
    let envelope: RootBindingEnvelopeV1 = serde_json::from_slice(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "v1 root binding envelope is invalid",
        )
    })?;
    if envelope.schema_version != 1
        || envelope.root_binding_id != *binding_id
        || envelope.protection_kind != "windows_current_user"
    {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "v1 root binding identity or version is incompatible",
        ));
    }
    let ciphertext = BASE64.decode(&envelope.ciphertext).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "v1 root binding ciphertext is invalid",
        )
    })?;
    let plaintext = unprotect_current_user(&ciphertext, &binding_entropy(binding_id))?;
    let locator: RootLocator = serde_json::from_slice(&plaintext).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "v1 root locator is invalid",
        )
    })?;
    if locator.locator_format_version != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "v1 root locator version is incompatible",
        ));
    }
    Ok((envelope, PathBuf::from(locator.absolute_path)))
}

fn migration_checkout(
    source: &ProjectV1,
    checkout_id: CheckoutId,
    binding_id: RootBindingId,
    root: &Path,
) -> Result<ProjectCheckout, RepositoryError> {
    let attachment_state = if root.is_dir() {
        CheckoutAttachmentState::Attached
    } else {
        CheckoutAttachmentState::Missing
    };
    let checkout_kind = match source.repository_kind {
        RepositoryKind::Git if root.join(".git").is_file() => CheckoutKind::LinkedWorktree,
        RepositoryKind::Git => CheckoutKind::MainWorktree,
        RepositoryKind::None => CheckoutKind::FilesystemRoot,
    };
    let limitations = vec!["runtime_observation_deferred_to_m1_scan".to_owned()];
    let content_fingerprint = versioned_fingerprint(
        "star.identity.project-checkout",
        1,
        &serde_json::json!({
            "identity_contract_version":1,
            "checkout_id":checkout_id,
            "project_id":source.project_id,
            "root_binding_id":binding_id,
            "repository_kind":source.repository_kind,
            "checkout_kind":checkout_kind,
            "repository_binding_id":null,
            "worktree_binding_id":null,
            "object_format":null,
            "head_state":"unavailable",
            "head_ref":null,
            "head_commit_id":null,
            "head_tree_id":null,
            "upstream_ref":null,
            "default_branch_hint":null,
            "remote_identity":null,
            "attachment_state":attachment_state,
            "limitations":limitations,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration checkout fingerprint failed",
        )
    })?;
    Ok(ProjectCheckout {
        schema_id: "star.project-checkout".to_owned(),
        schema_version: 1,
        checkout_id,
        project_id: source.project_id.clone(),
        root_binding_id: Some(binding_id),
        repository_kind: source.repository_kind,
        checkout_kind,
        repository_binding_id: None,
        worktree_binding_id: None,
        object_format: None,
        head_state: CheckoutHeadState::Unavailable,
        head_ref: None,
        head_commit_id: None,
        head_tree_id: None,
        upstream_ref: None,
        default_branch_hint: None,
        remote_identity: None,
        attachment_state,
        last_observed_at: Utc::now(),
        limitations,
        content_fingerprint,
    })
}

fn migration_plan_fingerprint(
    entries: &[ProjectV1ToV2MigrationEntry],
    limitations: &[String],
) -> Result<Sha256Hash, RepositoryError> {
    versioned_fingerprint(
        "star.management.project-v1-to-v2-migration-plan",
        1,
        &serde_json::json!({
            "source_store_version":1,
            "target_store_version":MANAGEMENT_STORE_VERSION,
            "entries":entries,
            "limitations":limitations,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration plan fingerprint failed",
        )
    })
}

fn validate_migration_plan(plan: &ProjectV1ToV2MigrationPlan) -> Result<(), RepositoryError> {
    if plan.schema_id != "star.management.project-v1-to-v2-migration-plan"
        || plan.schema_version != 1
        || plan.source_store_version != 1
        || plan.target_store_version != MANAGEMENT_STORE_VERSION
        || plan
            .entries
            .windows(2)
            .any(|pair| pair[0].project_id >= pair[1].project_id)
        || migration_plan_fingerprint(&plan.entries, &plan.limitations)? != plan.plan_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "migration plan invariant or fingerprint failed",
        ));
    }
    Ok(())
}

pub fn plan_project_v1_to_v2(
    management_root: &Path,
    binding_root: &Path,
) -> Result<ProjectV1ToV2MigrationPlan, RepositoryError> {
    let global_path = management_global_path(management_root);
    let connection = Connection::open_with_flags(
        &global_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA query_only=ON;")
        .map_err(map_sql)?;
    if checked_store_version(&connection)? != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "migration source global store is not version 1",
        ));
    }
    let incomplete: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM coordinated_operations WHERE state <> 'completed'",
            [],
            |row| row.get(0),
        )
        .map_err(map_sql)?;
    if incomplete != 0 {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "v1 registration coordination must be complete before migration",
        ));
    }
    let mut statement = connection
        .prepare(
            "SELECT project_id, root_binding_id, document_json, updated_at
             FROM projects ORDER BY project_id",
        )
        .map_err(map_sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(map_sql)?;
    let mut entries = Vec::new();
    let mut referenced_bindings = BTreeSet::new();
    for row in rows {
        let (project_id_text, column_binding_id, document, updated_at) = row.map_err(map_sql)?;
        let source: ProjectV1 = serde_json::from_str(&document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "v1 project document is invalid",
            )
        })?;
        if source.schema_id != "star.project"
            || source.schema_version != 1
            || source.project_id.as_str() != project_id_text
            || source.root_binding_id.as_ref().map(|value| value.as_str())
                != column_binding_id.as_deref()
        {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "v1 project identity columns and document conflict",
            ));
        }
        let project_path = management_project_path(management_root, &source.project_id);
        let project_connection = Connection::open_with_flags(
            &project_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        project_connection
            .execute_batch("PRAGMA query_only=ON;")
            .map_err(map_sql)?;
        if checked_store_version(&project_connection)? != 1 {
            return Err(repository_error(
                RepositoryErrorCategory::IncompatibleVersion,
                "migration source project store is not version 1",
            ));
        }
        let local_document: String = project_connection
            .query_row(
                "SELECT document_json FROM project_document WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .map_err(map_sql)?;
        let local_source: ProjectV1 = serde_json::from_str(&local_document).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "v1 project partition document is invalid",
            )
        })?;
        if local_source != source {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "global and project v1 declarations conflict",
            ));
        }
        let source_project_fingerprint = versioned_fingerprint("star.project", 1, &source)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "v1 project fingerprint failed",
                )
            })?;
        let checkout = if let Some(binding_id) = source.root_binding_id.clone() {
            if !referenced_bindings.insert(binding_id.clone()) {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "multiple v1 projects reference one root binding",
                ));
            }
            let (binding, root) = read_v1_root_binding(binding_root, &binding_id)?;
            if binding.project_id != source.project_id {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "v1 project and root binding identities conflict",
                ));
            }
            Some(migration_checkout(
                &source,
                CheckoutId::new(),
                binding_id,
                &root,
            )?)
        } else {
            None
        };
        let derived_registration_state = match (&checkout, source.registration_state) {
            (_, RegistrationState::Invalid) => RegistrationState::Invalid,
            (Some(checkout), _)
                if checkout.attachment_state == CheckoutAttachmentState::Attached =>
            {
                RegistrationState::Attached
            }
            _ => RegistrationState::Detached,
        };
        let project = Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id: source.project_id.clone(),
            identity_scope: source.identity_scope,
            display_name: source.display_name.clone(),
            repository_kind: source.repository_kind,
            source_of_truth: source.source_of_truth.clone(),
            declaration_fingerprint: source.declaration_fingerprint.clone(),
            registration_state: derived_registration_state,
            attached_checkout_ids: checkout
                .as_ref()
                .map(|value| vec![value.checkout_id.clone()])
                .unwrap_or_default(),
            latest_revision_id: source.latest_revision_id.clone(),
            latest_workspace_snapshot_id: source.latest_workspace_snapshot_id.clone(),
        };
        entries.push(ProjectV1ToV2MigrationEntry {
            project_id: source.project_id.clone(),
            source_root_binding_id: source.root_binding_id.clone(),
            source_project: source,
            source_project_fingerprint,
            source_updated_at: updated_at,
            project,
            checkout,
        });
    }
    drop(statement);
    drop(connection);
    let mut binding_files = BTreeSet::new();
    if binding_root.is_dir() {
        for entry in fs::read_dir(binding_root).map_err(map_io)? {
            let path = entry.map_err(map_io)?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "binding")
            {
                let stem = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| {
                        repository_error(
                            RepositoryErrorCategory::Corrupt,
                            "root binding filename is invalid",
                        )
                    })?;
                binding_files.insert(RootBindingId::parse(stem).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "root binding filename identity is invalid",
                    )
                })?);
            }
        }
    }
    if binding_files != referenced_bindings {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "v1 root binding set has orphaned or missing identities",
        ));
    }
    let limitations = vec!["runtime_observation_deferred_to_m1_scan".to_owned()];
    let plan_fingerprint = migration_plan_fingerprint(&entries, &limitations)?;
    Ok(ProjectV1ToV2MigrationPlan {
        schema_id: "star.management.project-v1-to-v2-migration-plan".to_owned(),
        schema_version: 1,
        source_store_version: 1,
        target_store_version: MANAGEMENT_STORE_VERSION,
        entries,
        limitations,
        plan_fingerprint,
    })
}

fn migration_file_sha256(path: &Path) -> Result<Sha256Hash, RepositoryError> {
    let file = fs::File::open(path).map_err(map_io)?;
    Sha256Hash::digest_reader(file).map_err(map_io)
}

fn migration_backup_fingerprint(
    plan_fingerprint: &Sha256Hash,
    files: &[MigrationBackupFile],
) -> Result<Sha256Hash, RepositoryError> {
    versioned_fingerprint(
        "star.management.project-v1-to-v2-backup",
        1,
        &serde_json::json!({
            "plan_fingerprint":plan_fingerprint,
            "files":files,
        }),
    )
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration backup fingerprint failed",
        )
    })
}

fn backup_kind_key(kind: &MigrationBackupKind) -> String {
    match kind {
        MigrationBackupKind::Global => "global".to_owned(),
        MigrationBackupKind::Project { project_id } => {
            format!("project:{}", project_id.as_str())
        }
        MigrationBackupKind::RootBinding { root_binding_id } => {
            format!("binding:{}", root_binding_id.as_str())
        }
    }
}

fn expected_backup_keys(plan: &ProjectV1ToV2MigrationPlan) -> BTreeSet<String> {
    let mut keys = BTreeSet::from(["global".to_owned()]);
    for entry in &plan.entries {
        keys.insert(format!("project:{}", entry.project_id.as_str()));
        if let Some(binding_id) = &entry.source_root_binding_id {
            keys.insert(format!("binding:{}", binding_id.as_str()));
        }
    }
    keys
}

fn safe_backup_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VerifiedMigrationBackupObservation {
    backup_locator: String,
    backup_fingerprint: Sha256Hash,
    manifest_fingerprint: Sha256Hash,
    last_modified_at: DateTime<Utc>,
    estimated_rows: u64,
    estimated_bytes: u64,
}

fn migration_backups_root(management_root: &Path) -> Result<PathBuf, RepositoryError> {
    management_root
        .parent()
        .map(|parent| parent.join(MIGRATION_BACKUPS_DIRECTORY))
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "management root has no migration backup parent",
            )
        })
}

fn backup_relative_locator(root: &Path, path: &Path) -> Result<String, RepositoryError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup file escapes its backup root",
        )
    })?;
    let mut components = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup file locator is invalid",
            ));
        };
        components.push(component.to_str().ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup file locator is not UTF-8",
            )
        })?);
    }
    if components.is_empty() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup file locator is empty",
        ));
    }
    Ok(components.join("/"))
}

fn collect_migration_backup_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut BTreeSet<String>,
) -> Result<(), RepositoryError> {
    if depth > 16 {
        return Err(repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "migration backup directory nesting is too deep",
        ));
    }
    for entry in fs::read_dir(directory).map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(map_io)?;
        if metadata.file_type().is_symlink() {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup contains a symbolic link",
            ));
        }
        if metadata.is_dir() {
            collect_migration_backup_files(root, &path, depth.saturating_add(1), files)?;
        } else if metadata.is_file() {
            if files.len() >= RETENTION_BACKUP_FILE_SCAN_LIMIT
                || !files.insert(backup_relative_locator(root, &path)?)
            {
                return Err(repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "migration backup file inventory exceeds its bound",
                ));
            }
        } else {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup contains an unsupported filesystem entry",
            ));
        }
    }
    Ok(())
}

fn verify_migration_backup_for_retention(
    backup_root: &Path,
) -> Result<VerifiedMigrationBackupObservation, RepositoryError> {
    let root_metadata = fs::symlink_metadata(backup_root).map_err(map_io)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup root is not a regular directory",
        ));
    }
    let backup_locator = backup_root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup locator is invalid",
            )
        })?
        .to_owned();
    let manifest_path = backup_root.join(MIGRATION_BACKUP_MANIFEST_FILENAME);
    let manifest_bytes = fs::read(&manifest_path).map_err(map_io)?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MANAGEMENT_DOCUMENT_MAX_BYTES as usize {
        return Err(repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "migration backup manifest exceeds its size bound",
        ));
    }
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "migration backup manifest is not UTF-8",
        )
    })?;
    let manifest_value = star_contracts::parse_no_duplicate_keys(manifest_text).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "migration backup manifest is invalid",
        )
    })?;
    let manifest: MigrationBackupManifest =
        serde_json::from_value(manifest_value).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "migration backup manifest shape is invalid",
            )
        })?;
    let keys: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| backup_kind_key(&file.kind))
        .collect();
    let relative_paths: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();
    let expected_locator = format!(
        "{MIGRATION_BACKUP_PREFIX}{}",
        manifest
            .plan_fingerprint
            .as_str()
            .trim_start_matches("sha256:")
    );
    if manifest.schema_id != "star.management.project-v1-to-v2-backup"
        || manifest.schema_version != 1
        || backup_locator != expected_locator
        || manifest.files.is_empty()
        || keys.len() != manifest.files.len()
        || !keys.contains("global")
        || relative_paths.len() != manifest.files.len()
        || manifest
            .files
            .iter()
            .any(|file| !safe_backup_relative_path(&file.relative_path))
        || migration_backup_fingerprint(&manifest.plan_fingerprint, &manifest.files)?
            != manifest.backup_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup manifest invariant failed",
        ));
    }
    let mut estimated_bytes = u64::try_from(manifest_bytes.len()).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::QuotaExceeded,
            "migration backup manifest size overflowed",
        )
    })?;
    for file in &manifest.files {
        let path = backup_root.join(&file.relative_path);
        let metadata = fs::symlink_metadata(&path).map_err(map_io)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || migration_file_sha256(&path)? != file.content_sha256
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup file invariant failed",
            ));
        }
        estimated_bytes = estimated_bytes.checked_add(metadata.len()).ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "migration backup size overflowed",
            )
        })?;
    }
    let mut actual_files = BTreeSet::new();
    collect_migration_backup_files(backup_root, backup_root, 0, &mut actual_files)?;
    let mut expected_files = relative_paths
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    expected_files.insert(MIGRATION_BACKUP_MANIFEST_FILENAME.to_owned());
    if actual_files != expected_files {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup contains unmanifested files",
        ));
    }
    let last_modified_at = fs::symlink_metadata(&manifest_path)
        .map_err(map_io)?
        .modified()
        .map(DateTime::<Utc>::from)
        .map_err(map_io)?;
    Ok(VerifiedMigrationBackupObservation {
        backup_locator,
        backup_fingerprint: manifest.backup_fingerprint,
        manifest_fingerprint: Sha256Hash::digest(&manifest_bytes),
        last_modified_at,
        estimated_rows: u64::try_from(manifest.files.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "migration backup row count overflowed",
                )
            })?,
        estimated_bytes,
    })
}

fn retention_migration_backup_inventory(
    management_root: &Path,
    policy: &RetentionPolicyV2,
) -> Result<
    (
        Vec<RetentionMigrationBackupCandidateV1>,
        Vec<RetentionProtectedMigrationBackupV1>,
    ),
    RepositoryError,
> {
    let backups_root = migration_backups_root(management_root)?;
    if !backups_root.try_exists().map_err(map_io)? {
        return Ok((Vec::new(), Vec::new()));
    }
    let root_metadata = fs::symlink_metadata(&backups_root).map_err(map_io)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup parent is not a regular directory",
        ));
    }
    let mut observations = Vec::new();
    let mut protected = Vec::new();
    for (index, entry) in fs::read_dir(&backups_root).map_err(map_io)?.enumerate() {
        if index >= RETENTION_BACKUP_DIRECTORY_LIMIT {
            return Err(repository_error(
                RepositoryErrorCategory::QuotaExceeded,
                "migration backup directory inventory exceeds its bound",
            ));
        }
        let entry = entry.map_err(map_io)?;
        let locator = entry
            .file_name()
            .to_str()
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 255
                    && !value.contains(['/', '\\', '\0'])
                    && *value != "."
                    && *value != ".."
            })
            .ok_or_else(|| {
                repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "migration backup directory has an invalid entry name",
                )
            })?
            .to_owned();
        if locator.starts_with(RETENTION_DELETE_TOMBSTONE_PREFIX) {
            protected.push(RetentionProtectedMigrationBackupV1 {
                backup_locator: locator,
                backup_fingerprint: None,
                manifest_fingerprint: None,
                last_modified_at: None,
                reason_code: "MIGRATION_BACKUP_RECOVERY_HOLD".to_owned(),
                estimated_rows: 0,
                estimated_bytes: 0,
            });
            continue;
        }
        match verify_migration_backup_for_retention(&entry.path()) {
            Ok(observation) => observations.push(observation),
            Err(_) => protected.push(RetentionProtectedMigrationBackupV1 {
                backup_locator: locator,
                backup_fingerprint: None,
                manifest_fingerprint: None,
                last_modified_at: None,
                reason_code: "MIGRATION_BACKUP_UNVERIFIED".to_owned(),
                estimated_rows: 0,
                estimated_bytes: 0,
            }),
        }
    }
    observations.sort_by(|left, right| {
        right
            .last_modified_at
            .cmp(&left.last_modified_at)
            .then_with(|| left.backup_locator.cmp(&right.backup_locator))
    });
    let protected_floor = policy
        .migration_backup_min_count
        .saturating_add(1)
        .try_into()
        .unwrap_or(usize::MAX);
    let mut candidates = Vec::new();
    for (index, observation) in observations.into_iter().enumerate() {
        let protected_reason = if index == 0 {
            Some("MIGRATION_BACKUP_LATEST_KNOWN_GOOD")
        } else if index < protected_floor {
            Some("MIGRATION_BACKUP_FLOOR")
        } else if observation.estimated_rows > policy.max_batch_rows
            || observation.estimated_bytes > policy.max_batch_bytes
        {
            Some("MIGRATION_BACKUP_EXCEEDS_BATCH_LIMIT")
        } else {
            None
        };
        if let Some(reason_code) = protected_reason {
            protected.push(RetentionProtectedMigrationBackupV1 {
                backup_locator: observation.backup_locator,
                backup_fingerprint: Some(observation.backup_fingerprint),
                manifest_fingerprint: Some(observation.manifest_fingerprint),
                last_modified_at: Some(observation.last_modified_at),
                reason_code: reason_code.to_owned(),
                estimated_rows: observation.estimated_rows,
                estimated_bytes: observation.estimated_bytes,
            });
            continue;
        }
        let candidate_id = versioned_fingerprint(
            "star.management-retention-migration-backup-candidate",
            1,
            &serde_json::json!({
                "backup_locator":observation.backup_locator,
                "backup_fingerprint":observation.backup_fingerprint,
                "manifest_fingerprint":observation.manifest_fingerprint,
                "last_modified_at":observation.last_modified_at,
                "estimated_rows":observation.estimated_rows,
                "estimated_bytes":observation.estimated_bytes,
            }),
        )
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "migration backup retention candidate fingerprint failed",
            )
        })?
        .to_string();
        candidates.push(RetentionMigrationBackupCandidateV1 {
            candidate_id,
            backup_locator: observation.backup_locator,
            backup_fingerprint: observation.backup_fingerprint,
            manifest_fingerprint: observation.manifest_fingerprint,
            last_modified_at: observation.last_modified_at,
            reason_code: "MIGRATION_BACKUP_SUPERSEDED".to_owned(),
            estimated_rows: observation.estimated_rows,
            estimated_bytes: observation.estimated_bytes,
        });
    }
    protected.sort_by(|left, right| left.backup_locator.cmp(&right.backup_locator));
    Ok((candidates, protected))
}

fn migration_backup_inventory_matches(
    management_root: &Path,
    plan: &RetentionPlanV2,
    processed_candidates: usize,
) -> Result<bool, RepositoryError> {
    if processed_candidates > plan.migration_backup_candidates.len() {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup retention cursor is invalid",
        ));
    }
    let (candidates, protected) =
        retention_migration_backup_inventory(management_root, &plan.policy)?;
    Ok(
        candidates == plan.migration_backup_candidates[processed_candidates..]
            && protected == plan.protected_migration_backups,
    )
}

fn retention_delete_tombstone(
    backups_root: &Path,
    candidate: &RetentionMigrationBackupCandidateV1,
) -> PathBuf {
    backups_root.join(format!(
        "{RETENTION_DELETE_TOMBSTONE_PREFIX}{}",
        candidate.candidate_id.trim_start_matches("sha256:")
    ))
}

fn apply_migration_backup_retention_candidate(
    management_root: &Path,
    plan: &RetentionPlanV2,
    candidate_index: usize,
) -> Result<(u64, u64), RepositoryError> {
    let candidate = plan
        .migration_backup_candidates
        .get(candidate_index)
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup retention candidate is missing",
            )
        })?;
    let backups_root = migration_backups_root(management_root)?;
    let candidate_root = backups_root.join(&candidate.backup_locator);
    let tombstone = retention_delete_tombstone(&backups_root, candidate);
    let candidate_exists = candidate_root.try_exists().map_err(map_io)?;
    let tombstone_exists = tombstone.try_exists().map_err(map_io)?;
    if candidate_exists && tombstone_exists {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration backup retention candidate and tombstone both exist",
        ));
    }
    if tombstone_exists {
        let metadata = fs::symlink_metadata(&tombstone).map_err(map_io)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup retention tombstone is not a regular directory",
            ));
        }
        fs::remove_dir_all(&tombstone).map_err(map_io)?;
        if !migration_backup_inventory_matches(management_root, plan, candidate_index + 1)? {
            return Err(repository_error(
                RepositoryErrorCategory::RevisionConflict,
                "migration backup inventory changed while resuming retention",
            ));
        }
        return Ok((candidate.estimated_rows, candidate.estimated_bytes));
    }
    if !candidate_exists {
        if migration_backup_inventory_matches(management_root, plan, candidate_index + 1)? {
            return Ok((candidate.estimated_rows, candidate.estimated_bytes));
        }
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration backup retention candidate disappeared",
        ));
    }
    if !migration_backup_inventory_matches(management_root, plan, candidate_index)? {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration backup inventory changed after retention planning",
        ));
    }
    let observed = verify_migration_backup_for_retention(&candidate_root)?;
    if observed.backup_fingerprint != candidate.backup_fingerprint
        || observed.manifest_fingerprint != candidate.manifest_fingerprint
        || observed.last_modified_at != candidate.last_modified_at
        || observed.estimated_rows != candidate.estimated_rows
        || observed.estimated_bytes != candidate.estimated_bytes
    {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration backup retention candidate fingerprint is stale",
        ));
    }
    fs::rename(&candidate_root, &tombstone).map_err(map_io)?;
    fs::remove_dir_all(&tombstone).map_err(map_io)?;
    if !migration_backup_inventory_matches(management_root, plan, candidate_index + 1)? {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration backup inventory changed during retention apply",
        ));
    }
    Ok((candidate.estimated_rows, candidate.estimated_bytes))
}

fn verify_migration_backup(
    backup_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
) -> Result<MigrationBackupManifest, RepositoryError> {
    let bytes = fs::read(backup_root.join(MIGRATION_BACKUP_MANIFEST_FILENAME)).map_err(map_io)?;
    let input = std::str::from_utf8(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "migration backup manifest is invalid",
        )
    })?;
    let value = star_contracts::parse_no_duplicate_keys(input).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "migration backup manifest is invalid",
        )
    })?;
    let manifest: MigrationBackupManifest = serde_json::from_value(value).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "migration backup manifest is invalid",
        )
    })?;
    let keys: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| backup_kind_key(&file.kind))
        .collect();
    if manifest.schema_id != "star.management.project-v1-to-v2-backup"
        || manifest.schema_version != 1
        || manifest.plan_fingerprint != plan.plan_fingerprint
        || keys.len() != manifest.files.len()
        || keys != expected_backup_keys(plan)
        || manifest
            .files
            .iter()
            .any(|file| !safe_backup_relative_path(&file.relative_path))
        || migration_backup_fingerprint(&manifest.plan_fingerprint, &manifest.files)?
            != manifest.backup_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migration backup manifest invariant failed",
        ));
    }
    for file in &manifest.files {
        let path = backup_root.join(&file.relative_path);
        if migration_file_sha256(&path)? != file.content_sha256 {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "migration backup file digest mismatch",
            ));
        }
    }
    Ok(manifest)
}

fn create_or_verify_migration_backup(
    management_root: &Path,
    binding_root: &Path,
    backup_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
) -> Result<MigrationBackupManifest, RepositoryError> {
    if !backup_root.is_absolute()
        || backup_root.starts_with(management_root)
        || backup_root.starts_with(binding_root)
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "migration backup root must be an independent absolute directory",
        ));
    }
    let manifest_path = backup_root.join("migration-backup.json");
    if manifest_path.exists() {
        return verify_migration_backup(backup_root, plan);
    }
    if backup_root.exists() && fs::read_dir(backup_root).map_err(map_io)?.next().is_some() {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "new migration backup root is not empty",
        ));
    }
    create_private_dir(backup_root)?;
    let mut files = Vec::new();
    let global_source = management_global_path(management_root);
    let global_relative = "global.db".to_owned();
    let global_connection = Connection::open_with_flags(
        &global_source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    backup_connection(&global_connection, &backup_root.join(&global_relative))?;
    drop(global_connection);
    files.push(MigrationBackupFile {
        kind: MigrationBackupKind::Global,
        relative_path: global_relative.clone(),
        content_sha256: migration_file_sha256(&backup_root.join(global_relative))?,
    });
    for entry in &plan.entries {
        let source = management_project_path(management_root, &entry.project_id);
        let relative = format!("projects/{}.db", entry.project_id.as_str());
        let connection = Connection::open_with_flags(
            &source,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        backup_connection(&connection, &backup_root.join(&relative))?;
        drop(connection);
        files.push(MigrationBackupFile {
            kind: MigrationBackupKind::Project {
                project_id: entry.project_id.clone(),
            },
            relative_path: relative.clone(),
            content_sha256: migration_file_sha256(&backup_root.join(relative))?,
        });
        if let Some(binding_id) = &entry.source_root_binding_id {
            let relative = format!("bindings/{}.binding", binding_id.as_str());
            let destination = backup_root.join(&relative);
            if let Some(parent) = destination.parent() {
                create_private_dir(parent)?;
            }
            fs::copy(root_binding_path(binding_root, binding_id), &destination).map_err(map_io)?;
            apply_owner_system_dacl(&destination)?;
            files.push(MigrationBackupFile {
                kind: MigrationBackupKind::RootBinding {
                    root_binding_id: binding_id.clone(),
                },
                relative_path: relative,
                content_sha256: migration_file_sha256(&destination)?,
            });
        }
    }
    files.sort_by_key(|file| backup_kind_key(&file.kind));
    let backup_fingerprint = migration_backup_fingerprint(&plan.plan_fingerprint, &files)?;
    let manifest = MigrationBackupManifest {
        schema_id: "star.management.project-v1-to-v2-backup".to_owned(),
        schema_version: 1,
        plan_fingerprint: plan.plan_fingerprint.clone(),
        files,
        backup_fingerprint,
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration backup manifest serialization failed",
        )
    })?;
    write_private_atomic(&manifest_path, &bytes)?;
    verify_migration_backup(backup_root, plan)
}

fn migrate_project_partition(
    management_root: &Path,
    entry: &ProjectV1ToV2MigrationEntry,
    plan_fingerprint: &Sha256Hash,
) -> Result<(), RepositoryError> {
    let path = management_project_path(management_root, &entry.project_id);
    let mut connection = Connection::open(&path).map_err(map_sql)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(map_sql)?;
    let version = checked_store_version(&connection)?;
    if version == MANAGEMENT_STORE_VERSION {
        let stored: Project = connection
            .query_row(
                "SELECT document_json FROM project_document WHERE singleton=1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(map_sql)
            .and_then(|document| {
                serde_json::from_str(&document).map_err(|_| {
                    repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "migrated project partition document is invalid",
                    )
                })
            })?;
        if stored != entry.project {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "migrated project partition conflicts with the approved plan",
            ));
        }
        return Ok(());
    }
    if version != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "project partition version cannot be migrated",
        ));
    }
    let stored: ProjectV1 = connection
        .query_row(
            "SELECT document_json FROM project_document WHERE singleton=1",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(map_sql)
        .and_then(|document| {
            serde_json::from_str(&document).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "v1 project partition document is invalid",
                )
            })
        })?;
    if stored != entry.source_project
        || versioned_fingerprint("star.project", 1, &stored).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "v1 project fingerprint failed",
            )
        })? != entry.source_project_fingerprint
    {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "v1 project partition changed after migration planning",
        ));
    }
    let document = serde_json::to_string(&entry.project).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "v2 project serialization failed",
        )
    })?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql)?;
    set_meta(
        &transaction,
        "last_opened_by_product_version",
        "project-v1-to-v2-migration",
    )
    .map_err(map_sql)?;
    transaction
        .execute(
            "UPDATE project_document SET document_json=?1 WHERE singleton=1 AND project_id=?2",
            params![document, entry.project_id.as_str()],
        )
        .map_err(map_sql)?;
    append_event(
        &transaction,
        "project.migrated.v1-to-v2",
        Some(&entry.project_id),
        plan_fingerprint,
    )?;
    bump_revision(&transaction)?;
    transaction
        .pragma_update(None, "user_version", MANAGEMENT_STORE_VERSION)
        .map_err(map_sql)?;
    transaction.commit().map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(map_sql)?;
    Ok(())
}

fn migrate_root_binding(
    binding_root: &Path,
    entry: &ProjectV1ToV2MigrationEntry,
) -> Result<(), RepositoryError> {
    let Some(checkout) = &entry.checkout else {
        if entry.source_root_binding_id.is_some() {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "migration entry lost its checkout allocation",
            ));
        }
        return Ok(());
    };
    let binding_id = entry.source_root_binding_id.as_ref().ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "migration checkout has no source root binding",
        )
    })?;
    let path = root_binding_path(binding_root, binding_id);
    let bytes = fs::read(&path).map_err(map_io)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "root binding envelope is invalid",
        )
    })?;
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(2) => {
            let envelope: RootBindingEnvelope = serde_json::from_value(value).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Corrupt,
                    "migrated root binding envelope is invalid",
                )
            })?;
            if envelope.root_binding_id != *binding_id
                || envelope.project_id != entry.project_id
                || envelope.checkout_id != checkout.checkout_id
            {
                return Err(repository_error(
                    RepositoryErrorCategory::Invalid,
                    "migrated root binding conflicts with the approved plan",
                ));
            }
            return Ok(());
        }
        Some(1) => {}
        _ => {
            return Err(repository_error(
                RepositoryErrorCategory::IncompatibleVersion,
                "root binding version cannot be migrated",
            ));
        }
    }
    let (source, _) = read_v1_root_binding(binding_root, binding_id)?;
    if source.project_id != entry.project_id {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "v1 root binding changed after migration planning",
        ));
    }
    let migrated = RootBindingEnvelope {
        schema_version: 2,
        root_binding_id: source.root_binding_id,
        project_id: source.project_id,
        checkout_id: checkout.checkout_id.clone(),
        protection_kind: source.protection_kind,
        ciphertext: source.ciphertext,
        created_at: source.created_at,
    };
    let bytes = serde_json::to_vec_pretty(&migrated).map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Invalid,
            "v2 root binding serialization failed",
        )
    })?;
    write_private_atomic(&path, &bytes)
}

fn validate_migrated_global(
    connection: &Connection,
    plan: &ProjectV1ToV2MigrationPlan,
) -> Result<(), RepositoryError> {
    let projects: Vec<Project> = query_documents(
        connection,
        "SELECT document_json FROM projects ORDER BY project_id",
        [],
    )?;
    let expected_projects: Vec<_> = plan
        .entries
        .iter()
        .map(|entry| entry.project.clone())
        .collect();
    if projects != expected_projects {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "migrated global projects conflict with the approved plan",
        ));
    }
    let checkouts: Vec<ProjectCheckout> = query_documents(
        connection,
        "SELECT document_json FROM project_checkouts ORDER BY checkout_id",
        [],
    )?;
    let mut expected_checkouts: Vec<_> = plan
        .entries
        .iter()
        .filter_map(|entry| entry.checkout.clone())
        .collect();
    expected_checkouts.sort_by(|left, right| left.checkout_id.cmp(&right.checkout_id));
    if checkouts != expected_checkouts {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "migrated global checkouts conflict with the approved plan",
        ));
    }
    Ok(())
}

fn migrate_global_store(
    management_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
) -> Result<(), RepositoryError> {
    let path = management_global_path(management_root);
    let mut connection = Connection::open(&path).map_err(map_sql)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(map_sql)?;
    let version = checked_store_version(&connection)?;
    if version == MANAGEMENT_STORE_VERSION {
        return validate_migrated_global(&connection, plan);
    }
    if version != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IncompatibleVersion,
            "global store version cannot be migrated",
        ));
    }
    let current: Vec<ProjectV1> = query_documents(
        &connection,
        "SELECT document_json FROM projects ORDER BY project_id",
        [],
    )?;
    let expected: Vec<_> = plan
        .entries
        .iter()
        .map(|entry| entry.source_project.clone())
        .collect();
    if current != expected {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "v1 global projects changed after migration planning",
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql)?;
    set_meta(
        &transaction,
        "last_opened_by_product_version",
        "project-v1-to-v2-migration",
    )
    .map_err(map_sql)?;
    transaction
        .execute_batch(
            "ALTER TABLE projects RENAME TO projects_v1;
             CREATE TABLE projects(
                project_id TEXT PRIMARY KEY,
                identity_scope TEXT NOT NULL,
                document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                updated_at TEXT NOT NULL
             ) STRICT;
             CREATE TABLE project_checkouts(
                checkout_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                root_binding_id TEXT UNIQUE,
                document_json TEXT NOT NULL CHECK(json_valid(document_json)),
                updated_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(project_id)
             ) STRICT;
             CREATE INDEX project_checkouts_by_project
                ON project_checkouts(project_id, checkout_id);",
        )
        .map_err(map_sql)?;
    for entry in &plan.entries {
        let project_document = serde_json::to_string(&entry.project).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "v2 project serialization failed",
            )
        })?;
        transaction
            .execute(
                "INSERT INTO projects(project_id, identity_scope, document_json, updated_at)
                 VALUES(?1, ?2, ?3, ?4)",
                params![
                    entry.project_id.as_str(),
                    serialized_enum_label(&entry.project.identity_scope)?,
                    project_document,
                    entry.source_updated_at,
                ],
            )
            .map_err(map_sql)?;
        if let Some(checkout) = &entry.checkout {
            let checkout_document = serde_json::to_string(checkout).map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "v2 checkout serialization failed",
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO project_checkouts(
                        checkout_id, project_id, root_binding_id, document_json, updated_at
                     ) VALUES(?1, ?2, ?3, ?4, ?5)",
                    params![
                        checkout.checkout_id.as_str(),
                        checkout.project_id.as_str(),
                        checkout
                            .root_binding_id
                            .as_ref()
                            .map(|value| value.as_str()),
                        checkout_document,
                        checkout.last_observed_at.to_rfc3339(),
                    ],
                )
                .map_err(map_sql)?;
        }
    }
    transaction
        .execute_batch("DROP TABLE projects_v1;")
        .map_err(map_sql)?;
    append_event(
        &transaction,
        "management.migrated.project-v1-to-v2",
        None,
        &plan.plan_fingerprint,
    )?;
    bump_revision(&transaction)?;
    transaction
        .pragma_update(None, "user_version", MANAGEMENT_STORE_VERSION)
        .map_err(map_sql)?;
    transaction.commit().map_err(map_sql)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(map_sql)?;
    validate_migrated_global(&connection, plan)
}

fn interrupted_migration_result(
    completed_steps: usize,
    total_steps: usize,
    plan: &ProjectV1ToV2MigrationPlan,
    backup: &MigrationBackupManifest,
) -> ProjectV1ToV2MigrationResult {
    ProjectV1ToV2MigrationResult {
        schema_id: "star.management.project-v1-to-v2-migration-result".to_owned(),
        schema_version: 1,
        state: MigrationApplyState::Interrupted,
        completed_steps,
        total_steps,
        plan_fingerprint: plan.plan_fingerprint.clone(),
        backup_fingerprint: backup.backup_fingerprint.clone(),
    }
}

fn apply_project_v1_to_v2_with_step_limit(
    management_root: &Path,
    binding_root: &Path,
    backup_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
    approved_plan_fingerprint: &str,
    stop_after_steps: Option<usize>,
) -> Result<ProjectV1ToV2MigrationResult, RepositoryError> {
    validate_migration_plan(plan)?;
    if approved_plan_fingerprint != plan.plan_fingerprint.as_str() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration approval fingerprint is stale",
        ));
    }
    let backup =
        create_or_verify_migration_backup(management_root, binding_root, backup_root, plan)?;
    let total_steps = plan.entries.len()
        + plan
            .entries
            .iter()
            .filter(|entry| entry.checkout.is_some())
            .count()
        + 1;
    let mut completed_steps = 0;
    if stop_after_steps.is_some_and(|limit| completed_steps >= limit) {
        return Ok(interrupted_migration_result(
            completed_steps,
            total_steps,
            plan,
            &backup,
        ));
    }
    for entry in &plan.entries {
        migrate_project_partition(management_root, entry, &plan.plan_fingerprint)?;
        completed_steps += 1;
        if stop_after_steps.is_some_and(|limit| completed_steps >= limit) {
            return Ok(interrupted_migration_result(
                completed_steps,
                total_steps,
                plan,
                &backup,
            ));
        }
    }
    for entry in plan.entries.iter().filter(|entry| entry.checkout.is_some()) {
        migrate_root_binding(binding_root, entry)?;
        completed_steps += 1;
        if stop_after_steps.is_some_and(|limit| completed_steps >= limit) {
            return Ok(interrupted_migration_result(
                completed_steps,
                total_steps,
                plan,
                &backup,
            ));
        }
    }
    migrate_global_store(management_root, plan)?;
    completed_steps += 1;
    refresh_migration_active_set(management_root)?;
    if inspect_store_read_only(&management_global_path(management_root))
        != RecoveryInspection::Healthy
        || plan.entries.iter().any(|entry| {
            inspect_store_read_only(&management_project_path(management_root, &entry.project_id))
                != RecoveryInspection::Healthy
        })
    {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "migrated management stores failed final inspection",
        ));
    }
    Ok(ProjectV1ToV2MigrationResult {
        schema_id: "star.management.project-v1-to-v2-migration-result".to_owned(),
        schema_version: 1,
        state: MigrationApplyState::Completed,
        completed_steps,
        total_steps,
        plan_fingerprint: plan.plan_fingerprint.clone(),
        backup_fingerprint: backup.backup_fingerprint,
    })
}

pub fn apply_project_v1_to_v2(
    management_root: &Path,
    binding_root: &Path,
    backup_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
    approved_plan_fingerprint: &str,
) -> Result<ProjectV1ToV2MigrationResult, RepositoryError> {
    apply_project_v1_to_v2_with_step_limit(
        management_root,
        binding_root,
        backup_root,
        plan,
        approved_plan_fingerprint,
        None,
    )
}

fn checkpoint_if_store_exists(path: &Path) -> Result<(), RepositoryError> {
    if path.exists() {
        let connection = Connection::open(path).map_err(map_sql)?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(map_sql)?;
    }
    Ok(())
}

pub fn rollback_project_v1_to_v2(
    management_root: &Path,
    binding_root: &Path,
    backup_root: &Path,
    plan: &ProjectV1ToV2MigrationPlan,
    approved_backup_fingerprint: &str,
) -> Result<Sha256Hash, RepositoryError> {
    validate_migration_plan(plan)?;
    let backup = verify_migration_backup(backup_root, plan)?;
    if approved_backup_fingerprint != backup.backup_fingerprint.as_str() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "migration rollback approval fingerprint is stale",
        ));
    }
    for file in &backup.files {
        let source = backup_root.join(&file.relative_path);
        let destination = match &file.kind {
            MigrationBackupKind::Global => management_global_path(management_root),
            MigrationBackupKind::Project { project_id } => {
                management_project_path(management_root, project_id)
            }
            MigrationBackupKind::RootBinding { root_binding_id } => {
                root_binding_path(binding_root, root_binding_id)
            }
        };
        if !matches!(file.kind, MigrationBackupKind::RootBinding { .. }) {
            checkpoint_if_store_exists(&destination)?;
        }
        if let Some(parent) = destination.parent() {
            create_private_dir(parent)?;
        }
        fs::copy(&source, &destination).map_err(map_io)?;
        apply_owner_system_dacl(&destination)?;
    }
    let global = Connection::open_with_flags(
        management_global_path(management_root),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    if checked_store_version(&global)? != 1 {
        return Err(repository_error(
            RepositoryErrorCategory::IntegrityFailed,
            "rolled back global store is not version 1",
        ));
    }
    for entry in &plan.entries {
        let connection = Connection::open_with_flags(
            management_project_path(management_root, &entry.project_id),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sql)?;
        if checked_store_version(&connection)? != 1 {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "rolled back project store is not version 1",
            ));
        }
        if let Some(binding_id) = &entry.source_root_binding_id {
            let (binding, _) = read_v1_root_binding(binding_root, binding_id)?;
            if binding.project_id != entry.project_id {
                return Err(repository_error(
                    RepositoryErrorCategory::IntegrityFailed,
                    "rolled back root binding identity is invalid",
                ));
            }
        }
    }
    refresh_migration_active_set(management_root)?;
    Ok(backup.backup_fingerprint)
}

pub struct WindowsProjectRootBindingStore {
    root: PathBuf,
}

impl WindowsProjectRootBindingStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RepositoryError> {
        let root = root.into();
        create_private_dir(&root)?;
        Ok(Self { root })
    }

    fn path(&self, binding_id: &RootBindingId) -> PathBuf {
        self.root.join(format!("{}.binding", binding_id.as_str()))
    }

    fn binding_paths(&self) -> Result<Vec<PathBuf>, RepositoryError> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(map_io)? {
            let path = entry.map_err(map_io)?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "binding")
            {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn decode_binding(
        &self,
        path: &Path,
    ) -> Result<(ProjectRootAttachment, PathBuf), RepositoryError> {
        let bytes = fs::read(path).map_err(map_io)?;
        let envelope: RootBindingEnvelope = serde_json::from_slice(&bytes).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "root binding envelope is invalid",
            )
        })?;
        if envelope.schema_version != 2 || envelope.protection_kind != "windows_current_user" {
            return Err(repository_error(
                RepositoryErrorCategory::IncompatibleVersion,
                "root binding version is incompatible",
            ));
        }
        if self.path(&envelope.root_binding_id) != path {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "root binding filename does not match its identity",
            ));
        }
        let ciphertext = BASE64.decode(envelope.ciphertext).map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Corrupt,
                "root binding ciphertext is invalid",
            )
        })?;
        let plaintext =
            unprotect_current_user(&ciphertext, &binding_entropy(&envelope.root_binding_id))?;
        let locator: RootLocator = serde_json::from_slice(&plaintext).map_err(|_| {
            repository_error(RepositoryErrorCategory::Corrupt, "root locator is invalid")
        })?;
        if locator.locator_format_version != 1 {
            return Err(repository_error(
                RepositoryErrorCategory::IncompatibleVersion,
                "root locator version is incompatible",
            ));
        }
        Ok((
            ProjectRootAttachment {
                project_id: envelope.project_id,
                checkout_id: envelope.checkout_id,
                root_binding_id: envelope.root_binding_id,
            },
            PathBuf::from(locator.absolute_path),
        ))
    }
}

impl ProjectRootBindingStore for WindowsProjectRootBindingStore {
    fn list_attachments(&self) -> Result<Vec<ProjectRootAttachment>, RepositoryError> {
        let mut attachments = Vec::new();
        for path in self.binding_paths()? {
            attachments.push(self.decode_binding(&path)?.0);
        }
        attachments.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.checkout_id.cmp(&right.checkout_id))
        });
        if attachments
            .windows(2)
            .any(|pair| pair[0].checkout_id == pair[1].checkout_id)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "a CheckoutId has multiple active root bindings",
            ));
        }
        Ok(attachments)
    }

    fn find_by_root(&self, root: &Path) -> Result<Option<ProjectRootAttachment>, RepositoryError> {
        let canonical = canonical_project_root(root)?;
        let mut found = None;
        for path in self.binding_paths()? {
            let (attachment, attached_root) = self.decode_binding(&path)?;
            if same_windows_path(&canonical, &attached_root) {
                if found.is_some() {
                    return Err(repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "multiple root bindings target the same project root",
                    ));
                }
                found = Some(attachment);
            }
        }
        Ok(found)
    }

    fn find_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<ProjectRootAttachment>, RepositoryError> {
        let mut found = None;
        for path in self.binding_paths()? {
            let (attachment, _) = self.decode_binding(&path)?;
            if attachment.project_id == *project_id {
                if found.is_some() {
                    return Err(repository_error(
                        RepositoryErrorCategory::Invalid,
                        "ProjectId has multiple checkouts; a CheckoutId is required",
                    ));
                }
                found = Some(attachment);
            }
        }
        Ok(found)
    }

    fn find_by_checkout(
        &self,
        checkout_id: &CheckoutId,
    ) -> Result<Option<ProjectRootAttachment>, RepositoryError> {
        let mut found = None;
        for path in self.binding_paths()? {
            let (attachment, _) = self.decode_binding(&path)?;
            if attachment.checkout_id == *checkout_id {
                if found.is_some() {
                    return Err(repository_error(
                        RepositoryErrorCategory::Corrupt,
                        "multiple root bindings target the same CheckoutId",
                    ));
                }
                found = Some(attachment);
            }
        }
        Ok(found)
    }

    fn attach(
        &self,
        project_id: &ProjectId,
        checkout_id: &CheckoutId,
        root: &Path,
    ) -> Result<RootBindingId, RepositoryError> {
        let canonical = canonical_project_root(root)?;
        if let Some(existing) = self.find_by_root(&canonical)? {
            if existing.project_id == *project_id && existing.checkout_id == *checkout_id {
                return Ok(existing.root_binding_id);
            }
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "project root is already attached to another immutable checkout identity",
            ));
        }
        if self.find_by_checkout(checkout_id)?.is_some() {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "CheckoutId is already attached to another root",
            ));
        }
        let binding_id = RootBindingId::new();
        let locator = serde_json::to_vec(&RootLocator {
            locator_format_version: 1,
            absolute_path: canonical.to_string_lossy().into_owned(),
        })
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "root locator serialization failed",
            )
        })?;
        let entropy = binding_entropy(&binding_id);
        let ciphertext = protect_current_user(&locator, &entropy)?;
        let envelope = serde_json::to_vec_pretty(&RootBindingEnvelope {
            schema_version: 2,
            root_binding_id: binding_id.clone(),
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            protection_kind: "windows_current_user".to_owned(),
            ciphertext: BASE64.encode(ciphertext),
            created_at: Utc::now(),
        })
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "root binding envelope serialization failed",
            )
        })?;
        write_private_atomic(&self.path(&binding_id), &envelope)?;
        Ok(binding_id)
    }

    fn resolve(&self, binding_id: &RootBindingId) -> Result<PathBuf, RepositoryError> {
        let (attachment, path) = self.decode_binding(&self.path(binding_id))?;
        if attachment.root_binding_id != *binding_id {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "root binding identity does not match",
            ));
        }
        let canonical = path.canonicalize().map_err(map_io)?;
        if !same_windows_path(&canonical, &path)
            || !canonical.is_dir()
            || is_network_or_reparse_root(&canonical)?
        {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "project root binding is detached",
            ));
        }
        Ok(canonical)
    }
}

fn canonical_project_root(root: &Path) -> Result<PathBuf, RepositoryError> {
    let canonical = root.canonicalize().map_err(map_io)?;
    if !canonical.is_dir() || is_network_or_reparse_root(&canonical)? {
        return Err(repository_error(
            RepositoryErrorCategory::Invalid,
            "project root must be a fixed local non-reparse directory",
        ));
    }
    Ok(canonical)
}

fn same_windows_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn binding_entropy(binding_id: &RootBindingId) -> Vec<u8> {
    format!("Star-Control/root-binding/v1/{}", binding_id.as_str()).into_bytes()
}

fn crypt_blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

fn take_crypt_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(blob.pbData.cast())));
    }
    bytes
}

fn protect_current_user(plaintext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, RepositoryError> {
    let input = crypt_blob(plaintext);
    let entropy = crypt_blob(entropy);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Unavailable,
            "current-user data protection failed",
        )
    })?;
    Ok(take_crypt_blob(output))
}

fn unprotect_current_user(ciphertext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, RepositoryError> {
    let input = crypt_blob(ciphertext);
    let entropy = crypt_blob(entropy);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Corrupt,
            "current-user data unprotection failed",
        )
    })?;
    Ok(take_crypt_blob(output))
}

fn is_network_or_reparse_root(path: &Path) -> Result<bool, RepositoryError> {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let text = path.as_os_str().to_string_lossy();
    let network =
        text.starts_with(r"\\?\UNC\") || text.starts_with(r"\\") && !text.starts_with(r"\\?\");
    let reparse =
        fs::metadata(path).map_err(map_io)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    Ok(network || reparse)
}

fn create_private_dir(path: &Path) -> Result<(), RepositoryError> {
    fs::create_dir_all(path).map_err(map_io)?;
    apply_owner_system_dacl(path)
}

fn write_private_new(path: &Path, bytes: &[u8], plan_token: &str) -> Result<(), RepositoryError> {
    let parent = path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "local state export parent directory is unavailable",
            )
        })?;
    if path.exists() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state export destination already exists",
        ));
    }
    let token = Sha256Hash::digest(plan_token.as_bytes());
    let temporary = parent.join(format!(
        ".star-local-state-{}.tmp",
        &token.as_str().trim_start_matches("sha256:")[..20]
    ));
    if temporary.exists() {
        let metadata = fs::symlink_metadata(&temporary).map_err(map_io)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || fs::read(&temporary).map_err(map_io)? != bytes
        {
            return Err(repository_error(
                RepositoryErrorCategory::IntegrityFailed,
                "local state export temporary file conflicts with the approved plan",
            ));
        }
    } else {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(map_io)?;
        file.write_all(bytes).map_err(map_io)?;
        file.sync_all().map_err(map_io)?;
        drop(file);
    }
    apply_owner_system_dacl(&temporary)?;
    if path.exists() {
        return Err(repository_error(
            RepositoryErrorCategory::RevisionConflict,
            "local state export destination appeared during apply",
        ));
    }
    fs::rename(&temporary, path).map_err(map_io)?;
    apply_owner_system_dacl(path)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), RepositoryError> {
    let parent = path.parent().ok_or_else(|| {
        repository_error(
            RepositoryErrorCategory::Unavailable,
            "private state file has no parent directory",
        )
    })?;
    create_private_dir(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        RootBindingId::new()
    ));
    fs::write(&temporary, bytes).map_err(map_io)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .map_err(map_io)?;
    file.sync_all().map_err(map_io)?;
    drop(file);
    apply_owner_system_dacl(&temporary)?;
    if path.exists() {
        let target = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
        let replacement = HSTRING::from(temporary.as_os_str().to_string_lossy().as_ref());
        unsafe {
            ReplaceFileW(
                &target,
                &replacement,
                PCWSTR::null(),
                REPLACEFILE_WRITE_THROUGH,
                None,
                None,
            )
        }
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "private state atomic replace failed",
            )
        })?;
    } else {
        fs::rename(&temporary, path).map_err(map_io)?;
    }
    apply_owner_system_dacl(path)
}

fn apply_owner_system_dacl(path: &Path) -> Result<(), RepositoryError> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let path = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;GA;;;OW)(A;;GA;;;SY)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Unavailable,
            "private state DACL construction failed",
        )
    })?;
    let result = unsafe {
        SetFileSecurityW(
            &path,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(descriptor.0.cast())));
    }
    result.ok().map_err(|_| {
        repository_error(
            RepositoryErrorCategory::Unavailable,
            "private state DACL application failed",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        ids::{GenerationId, ProjectRevisionId, WorkspaceSnapshotId},
        management::{
            CheckoutAttachmentState, CheckoutHeadState, CheckoutKind, IdentityScope,
            ProjectPathRef, RegistrationState, RepositoryKind, ScanStatus,
        },
    };

    #[test]
    fn management_document_limit_accepts_aggregate_snapshots_and_classifies_overflow() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .set_limit(Limit::SQLITE_LIMIT_LENGTH, MANAGEMENT_DOCUMENT_MAX_BYTES)
            .unwrap();
        connection
            .execute("CREATE TABLE documents(value TEXT NOT NULL)", [])
            .unwrap();

        let above_legacy_limit = "x".repeat(16 * 1024 * 1024 + 1024);
        connection
            .execute(
                "INSERT INTO documents(value) VALUES(?1)",
                [&above_legacy_limit],
            )
            .unwrap();

        connection
            .set_limit(Limit::SQLITE_LIMIT_LENGTH, 1024)
            .unwrap();
        let error = connection
            .execute(
                "INSERT INTO documents(value) VALUES(?1)",
                ["x".repeat(2048)],
            )
            .unwrap_err();
        assert_eq!(
            map_sql(error).category,
            RepositoryErrorCategory::QuotaExceeded
        );
    }

    #[test]
    fn retention_generation_inventory_aggregates_each_table_scan_by_generation() {
        let connection = Connection::open_in_memory().unwrap();
        for table in RETENTION_GENERATION_TABLES {
            connection
                .execute(
                    &format!(
                        "CREATE TABLE {table}(generation_id TEXT NOT NULL, document_json TEXT NOT NULL)"
                    ),
                    [],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO canonical_sources(generation_id, document_json) VALUES(?1, ?2)",
                params!["gen-a", "abc"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO symbols(generation_id, document_json) VALUES(?1, ?2), (?1, ?3)",
                params!["gen-a", "12345", "xy"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO scan_runs(generation_id, document_json) VALUES(?1, ?2)",
                params!["gen-b", "z"],
            )
            .unwrap();

        let inventory = retention_generation_inventory(&connection, &|| false).unwrap();

        assert_eq!(
            inventory.get("gen-a"),
            Some(&RetentionGenerationAggregate {
                rows: 3,
                bytes: 10,
                max_document_bytes: 5,
            })
        );
        assert_eq!(
            inventory.get("gen-b"),
            Some(&RetentionGenerationAggregate {
                rows: 1,
                bytes: 1,
                max_document_bytes: 1,
            })
        );
        assert!(!inventory.contains_key("gen-missing"));
    }

    fn project(project_id: ProjectId, checkout_id: CheckoutId) -> Project {
        Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id,
            identity_scope: IdentityScope::Local,
            display_name: "local-project".to_owned(),
            repository_kind: RepositoryKind::None,
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: Sha256Hash::digest(b"project"),
            registration_state: RegistrationState::Attached,
            attached_checkout_ids: vec![checkout_id],
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        }
    }

    fn checkout(
        project_id: ProjectId,
        checkout_id: CheckoutId,
        binding_id: RootBindingId,
    ) -> ProjectCheckout {
        ProjectCheckout {
            schema_id: "star.project-checkout".to_owned(),
            schema_version: 1,
            checkout_id,
            project_id,
            root_binding_id: Some(binding_id),
            repository_kind: RepositoryKind::None,
            checkout_kind: CheckoutKind::FilesystemRoot,
            repository_binding_id: None,
            worktree_binding_id: None,
            object_format: None,
            head_state: CheckoutHeadState::Unavailable,
            head_ref: None,
            head_commit_id: None,
            head_tree_id: None,
            upstream_ref: None,
            default_branch_hint: None,
            remote_identity: None,
            attachment_state: CheckoutAttachmentState::Attached,
            last_observed_at: Utc::now(),
            limitations: vec![],
            content_fingerprint: Sha256Hash::digest(b"checkout"),
        }
    }

    fn create_retention_migration_backup(management_root: &Path, seed: &str) -> PathBuf {
        let plan_fingerprint = Sha256Hash::digest(format!("plan-{seed}").as_bytes());
        let backup_root = migration_backups_root(management_root)
            .unwrap()
            .join(format!(
                "{MIGRATION_BACKUP_PREFIX}{}",
                plan_fingerprint.as_str().trim_start_matches("sha256:")
            ));
        fs::create_dir_all(&backup_root).unwrap();
        let relative_path = "global.db".to_owned();
        let payload = format!("migration-backup-{seed}").into_bytes();
        fs::write(backup_root.join(&relative_path), &payload).unwrap();
        let files = vec![MigrationBackupFile {
            kind: MigrationBackupKind::Global,
            relative_path,
            content_sha256: Sha256Hash::digest(&payload),
        }];
        let backup_fingerprint = migration_backup_fingerprint(&plan_fingerprint, &files).unwrap();
        let manifest = MigrationBackupManifest {
            schema_id: "star.management.project-v1-to-v2-backup".to_owned(),
            schema_version: 1,
            plan_fingerprint,
            files,
            backup_fingerprint,
        };
        fs::write(
            backup_root.join(MIGRATION_BACKUP_MANIFEST_FILENAME),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        backup_root
    }

    struct RecoveryFixture {
        root: PathBuf,
        management_root: PathBuf,
        backup_root: PathBuf,
        project_id: ProjectId,
        old_global_path: PathBuf,
        backup_manifest: BackupSetManifest,
    }

    fn recovery_fixture(label: &str) -> RecoveryFixture {
        let root = std::env::temp_dir().join(format!(
            "star-recovery-{label}-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let management_root = root.join("management");
        let backup_root = root.join("backup");
        let repositories =
            SqliteManagementRepositorySet::open(&management_root, "recovery-test").unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let binding_id = RootBindingId::new();
        let project = project(project_id.clone(), checkout_id.clone());
        let checkout = checkout(project_id.clone(), checkout_id, binding_id);
        let fingerprint = versioned_fingerprint("star.recovery.fixture", 1, &project).unwrap();
        repositories
            .global()
            .register_project(&project, &checkout, label, &fingerprint)
            .unwrap();
        let project_repository = repositories.project(&project_id).unwrap();
        project_repository
            .commit_registration_participant(
                &project,
                &CoordinatedOperationId::new(),
                &fingerprint,
                &fingerprint,
            )
            .unwrap();
        repositories.verify_all().unwrap();
        let backup_plan = repositories.plan_backup(&backup_root).unwrap();
        let backup = repositories
            .apply_backup(
                &backup_root,
                &backup_plan,
                backup_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        let active_set = repositories.active_set().unwrap();
        let old_global_path = active_store_file(
            &management_root,
            active_set
                .entries
                .iter()
                .find(|entry| matches!(entry.scope, StoreScope::Global))
                .unwrap(),
        );
        drop(project_repository);
        drop(repositories);
        RecoveryFixture {
            root,
            management_root,
            backup_root,
            project_id,
            old_global_path,
            backup_manifest: backup.manifest,
        }
    }

    #[test]
    fn status_all_reads_bounded_snapshots_without_verifying_or_resealing_active_set() {
        let fixture = recovery_fixture("bounded-status");
        let repositories =
            SqliteManagementRepositorySet::open(&fixture.management_root, "status-test").unwrap();
        let active_set_path = fixture.management_root.join(ACTIVE_SET_FILENAME);
        let active_set_before = fs::read(&active_set_path).unwrap();
        let global_before = repositories.global().status().unwrap();
        let project_before =
            read_store_status_snapshot(&repositories.project_path(&fixture.project_id).unwrap())
                .unwrap();

        let statuses = repositories.status_all().unwrap();

        assert_eq!(
            statuses,
            vec![global_before.clone(), project_before.clone()]
        );
        assert_eq!(
            repositories.global().status().unwrap().last_verified_at,
            global_before.last_verified_at
        );
        assert_eq!(
            read_store_status_snapshot(&repositories.project_path(&fixture.project_id).unwrap())
                .unwrap()
                .last_verified_at,
            project_before.last_verified_at
        );
        assert_eq!(fs::read(&active_set_path).unwrap(), active_set_before);
    }

    #[test]
    fn clean_startup_uses_bounded_snapshots_while_unclean_startup_verifies_deeply() {
        let fixture = recovery_fixture("bounded-startup");

        reset_verify_connection_calls();
        assert_eq!(
            inspect_management_root_for_startup(&fixture.management_root),
            Some(RecoveryInspection::Healthy)
        );
        assert_eq!(verify_connection_calls(), 0);

        reset_verify_connection_calls();
        let repositories =
            SqliteManagementRepositorySet::open(&fixture.management_root, "startup-test").unwrap();
        assert_eq!(verify_connection_calls(), 0);
        drop(repositories);

        let connection = Connection::open(&fixture.old_global_path).unwrap();
        set_meta(&connection, "last_clean_shutdown", "false").unwrap();
        drop(connection);

        reset_verify_connection_calls();
        assert_eq!(
            inspect_management_root_for_startup(&fixture.management_root),
            Some(RecoveryInspection::Healthy)
        );
        assert!(verify_connection_calls() > 0);

        reset_verify_connection_calls();
        let repositories =
            SqliteManagementRepositorySet::open(&fixture.management_root, "startup-test").unwrap();
        assert!(verify_connection_calls() > 0);
        drop(repositories);

        let connection = Connection::open(&fixture.old_global_path).unwrap();
        connection
            .execute(
                "UPDATE events SET event_hash=?1 WHERE sequence=(SELECT MIN(sequence) FROM events)",
                [Sha256Hash::digest(b"tampered-event").as_str()],
            )
            .unwrap();
        set_meta(&connection, "last_clean_shutdown", "false").unwrap();
        drop(connection);

        reset_verify_connection_calls();
        assert_eq!(
            inspect_management_root_for_startup(&fixture.management_root),
            Some(RecoveryInspection::Corrupt)
        );
        assert!(verify_connection_calls() > 0);
        let open_error = match SqliteManagementRepositorySet::open(
            &fixture.management_root,
            "tampered-startup-test",
        ) {
            Err(error) => error,
            Ok(_) => panic!("unclean tampered event chain must not enter normal mode"),
        };
        assert_eq!(
            open_error.category,
            RepositoryErrorCategory::IntegrityFailed
        );
    }

    #[test]
    fn management_status_pages_are_revision_bound_transport_bounded_and_complete() {
        let root = std::env::temp_dir().join(format!(
            "star-management-status-page-{}",
            RootBindingId::new().as_str()
        ));
        let repositories = SqliteManagementRepositorySet::open(&root, "status-page-test").unwrap();
        for index in 0_u32..1 {
            let project_id = ProjectId::from_stable_bytes(format!("project-{index:03}").as_bytes());
            let checkout_id =
                CheckoutId::from_stable_bytes(format!("checkout-{index:03}").as_bytes());
            let project = project(project_id.clone(), checkout_id.clone());
            let checkout = checkout(project_id.clone(), checkout_id, RootBindingId::new());
            repositories
                .global()
                .register_project(
                    &project,
                    &checkout,
                    &format!("status-page-{index}"),
                    &Sha256Hash::digest(format!("status-page-{index}").as_bytes()),
                )
                .unwrap();
            repositories.project(&project_id).unwrap();
        }

        let mut query = ManagementStatusQueryV1 {
            cursor: None,
            max_items: 1,
            max_bytes: 65_536,
        };
        let mut store_ids = BTreeSet::new();
        let mut first_cursor = None;
        loop {
            let page = repositories.status_page(&query).unwrap();
            page.validate().unwrap();
            assert!(serde_json::to_vec(&page).unwrap().len() <= 65_536);
            assert!(page.items.len() <= 1);
            assert_eq!(page.total_count, 2);
            for status in page.items {
                assert!(store_ids.insert(status.store_id));
            }
            if first_cursor.is_none() {
                first_cursor = page.next_cursor.clone();
            }
            let Some(cursor) = page.next_cursor else {
                break;
            };
            query.cursor = Some(cursor);
        }
        assert_eq!(store_ids.len(), 2);

        // Legacy status must fail closed before opening every project store once the
        // default transport item cap would be exceeded.
        for index in 1_u32..32 {
            let project_id = ProjectId::from_stable_bytes(format!("project-{index:03}").as_bytes());
            let checkout_id =
                CheckoutId::from_stable_bytes(format!("checkout-{index:03}").as_bytes());
            repositories
                .global()
                .register_project(
                    &project(project_id.clone(), checkout_id.clone()),
                    &checkout(project_id, checkout_id, RootBindingId::new()),
                    &format!("status-page-{index}"),
                    &Sha256Hash::digest(format!("status-page-{index}").as_bytes()),
                )
                .unwrap();
        }
        assert_eq!(
            repositories.status_all().unwrap_err().category,
            RepositoryErrorCategory::QuotaExceeded
        );

        let stale = repositories
            .status_page(&ManagementStatusQueryV1 {
                cursor: first_cursor,
                max_items: 1,
                max_bytes: 65_536,
            })
            .unwrap_err();
        assert_eq!(stale.category, RepositoryErrorCategory::RevisionConflict);
    }

    #[test]
    fn management_status_cursor_rejects_noncanonical_and_oversized_inputs() {
        let cursor = ManagementStatusCursorV1 {
            schema_version: 1,
            global_store_id: ManagementStoreId::new(),
            snapshot_revision: 1,
            global_returned: true,
            after_project_id: Some(ProjectId::new()),
        };
        let encoded = encode_management_status_cursor(&cursor).unwrap();
        assert_eq!(decode_management_status_cursor(&encoded).unwrap(), cursor);
        let pretty = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec_pretty(&serde_json::to_value(&cursor).unwrap()).unwrap());
        assert!(decode_management_status_cursor(&pretty).is_err());
        assert!(decode_management_status_cursor(&"a".repeat(1_025)).is_err());
    }

    #[test]
    fn management_status_legacy_path_fails_closed_at_cli_equivalent_cardinality() {
        const PROJECT_COUNT: u32 = 4_096;
        let root = std::env::temp_dir().join(format!(
            "star-management-status-cardinality-{}",
            RootBindingId::new().as_str()
        ));
        let repositories =
            SqliteManagementRepositorySet::open(&root, "status-cardinality-test").unwrap();

        // Seed one transaction so this remains a bounded transport/cardinality
        // fixture rather than a 4,096-event registration performance test.
        let mut connection = repositories.global.connection.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..PROJECT_COUNT {
            let project_id =
                ProjectId::from_stable_bytes(format!("cardinality-project-{index:04}").as_bytes());
            let document = serde_json::to_string(&project(
                project_id.clone(),
                CheckoutId::from_stable_bytes(
                    format!("cardinality-checkout-{index:04}").as_bytes(),
                ),
            ))
            .unwrap();
            transaction
                .execute(
                    "INSERT INTO projects(project_id, identity_scope, document_json, updated_at) VALUES(?1, 'local', ?2, ?3)",
                    params![project_id.as_str(), document, Utc::now().to_rfc3339()],
                )
                .unwrap();
        }
        transaction.commit().unwrap();
        drop(connection);

        let (total, ids) = repositories
            .global
            .project_ids_page(None, PROJECT_COUNT as usize + 1)
            .unwrap();
        assert_eq!(total, u64::from(PROJECT_COUNT));
        assert_eq!(ids.len(), PROJECT_COUNT as usize);
        assert_eq!(
            repositories.status_all().unwrap_err().category,
            RepositoryErrorCategory::QuotaExceeded
        );
    }

    fn scan_commit(mut project: Project, status: ScanStatus, seed: &str) -> ScanCommit {
        let mut revision: ProjectRevision = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/project-revision/minimal.json"
        ))
        .unwrap();
        revision.project_id = project.project_id.clone();
        revision.project_revision_id = ProjectRevisionId::new();
        revision.revision_kind = star_contracts::management::RevisionKind::FilesystemManifest;
        let mut snapshot: WorkspaceSnapshot = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/workspace-snapshot/minimal.json"
        ))
        .unwrap();
        snapshot.project_id = project.project_id.clone();
        snapshot.project_revision_id = revision.project_revision_id.clone();
        snapshot.workspace_snapshot_id = WorkspaceSnapshotId::new();
        snapshot.entries_manifest_ref.project_id = Some(project.project_id.clone());
        let mut run: ScanRun = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/scan-run/minimal.json"
        ))
        .unwrap();
        run.scan_run_id = ScanRunId::new();
        run.project_id = project.project_id.clone();
        run.project_revision_id = revision.project_revision_id.clone();
        run.workspace_snapshot_id = snapshot.workspace_snapshot_id.clone();
        run.generation_id = GenerationId::new();
        run.status = status;
        run.finished_at = Some(Utc::now());
        run.input_fingerprint = Sha256Hash::digest(seed.as_bytes());
        project.latest_revision_id = Some(revision.project_revision_id.clone());
        project.latest_workspace_snapshot_id = Some(snapshot.workspace_snapshot_id.clone());
        ScanCommit {
            project,
            revision,
            snapshot,
            run,
            sources: Vec::new(),
            symbols: Vec::new(),
            references: Vec::new(),
            findings: Vec::new(),
            occurrences: Vec::new(),
            code_index: None,
            source_entries: Vec::new(),
            index_entities: Vec::new(),
            index_edges: Vec::new(),
            idempotency_key: format!("scan-{seed}"),
            payload_fingerprint: Sha256Hash::digest(seed.as_bytes()),
        }
    }

    const GLOBAL_SCHEMA_V1: &str = r#"
CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE projects(
    project_id TEXT PRIMARY KEY,
    identity_scope TEXT NOT NULL,
    root_binding_id TEXT,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE coordinated_operations(
    operation_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL,
    input_fingerprint TEXT NOT NULL,
    document_json TEXT NOT NULL CHECK(json_valid(document_json)),
    updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE idempotency(
    idempotency_key TEXT PRIMARY KEY,
    payload_fingerprint TEXT NOT NULL,
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE events(
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    project_id TEXT,
    payload_fingerprint TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    store_revision INTEGER NOT NULL CHECK(store_revision > 0),
    previous_event_hash TEXT,
    event_hash TEXT NOT NULL UNIQUE
) STRICT;
"#;

    struct V1Fixture {
        root: PathBuf,
        management_root: PathBuf,
        binding_root: PathBuf,
        project_root: PathBuf,
        project_id: ProjectId,
        binding_id: RootBindingId,
    }

    fn seed_v1_metadata(connection: &Connection, scope: &StoreScope) -> ManagementStoreId {
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        let store_id = ManagementStoreId::new();
        set_meta(connection, "store_id", store_id.as_str()).unwrap();
        set_meta(
            connection,
            "store_scope",
            &serde_json::to_string(scope).unwrap(),
        )
        .unwrap();
        set_meta(connection, "store_revision", "0").unwrap();
        set_meta(connection, "generation", "1").unwrap();
        set_meta(connection, "created_by_product_version", "v1-test").unwrap();
        set_meta(connection, "last_verified_at", "").unwrap();
        set_meta(connection, "last_clean_shutdown", "true").unwrap();
        store_id
    }

    struct EmptyV1Fixture {
        root: PathBuf,
        management_root: PathBuf,
        binding_root: PathBuf,
    }

    fn create_empty_v1_fixture() -> EmptyV1Fixture {
        let root = std::env::temp_dir().join(format!(
            "star-state-empty-v1-v2-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let management_root = root.join("management");
        let binding_root = root.join("bindings");
        create_private_dir(&binding_root).unwrap();
        let global_path = management_global_path(&management_root);
        fs::create_dir_all(global_path.parent().unwrap()).unwrap();
        let global = Connection::open(global_path).unwrap();
        global.execute_batch(GLOBAL_SCHEMA_V1).unwrap();
        let store_id = seed_v1_metadata(&global, &StoreScope::Global);
        drop(global);
        let active_set = seal_active_set(vec![ActiveStoreGeneration {
            scope: StoreScope::Global,
            store_id,
            generation: 1,
            management_store_version: 1,
            relative_locator: "global/active".to_owned(),
            header_fingerprint: Sha256Hash::digest(b"unsealed-v1-test-header"),
        }])
        .unwrap();
        write_active_set_document(&management_root, &active_set).unwrap();
        EmptyV1Fixture {
            root,
            management_root,
            binding_root,
        }
    }

    fn create_v1_fixture(binding_identity_conflict: bool) -> V1Fixture {
        let root = std::env::temp_dir().join(format!(
            "star-state-v1-v2-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let management_root = root.join("management");
        let binding_root = root.join("bindings");
        let project_root = root.join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join("source.txt"), b"v1\n").unwrap();
        let project_root = project_root.canonicalize().unwrap();
        create_private_dir(&binding_root).unwrap();
        let project_id = ProjectId::new();
        let binding_id = RootBindingId::new();
        let source = ProjectV1 {
            schema_id: "star.project".to_owned(),
            schema_version: 1,
            project_id: project_id.clone(),
            identity_scope: IdentityScope::Local,
            display_name: "v1-project".to_owned(),
            repository_kind: RepositoryKind::None,
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: Sha256Hash::digest(b"v1-project"),
            registration_state: RegistrationState::Attached,
            root_binding_id: Some(binding_id.clone()),
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        };
        let global_path = management_global_path(&management_root);
        fs::create_dir_all(global_path.parent().unwrap()).unwrap();
        let global = Connection::open(&global_path).unwrap();
        global.execute_batch(GLOBAL_SCHEMA_V1).unwrap();
        seed_v1_metadata(&global, &StoreScope::Global);
        global
            .execute(
                "INSERT INTO projects(project_id, identity_scope, root_binding_id, document_json, updated_at)
                 VALUES(?1, 'local', ?2, ?3, ?4)",
                params![
                    project_id.as_str(),
                    binding_id.as_str(),
                    serde_json::to_string(&source).unwrap(),
                    Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
        drop(global);
        let project_path = management_project_path(&management_root, &project_id);
        fs::create_dir_all(project_path.parent().unwrap()).unwrap();
        let project_connection = Connection::open(&project_path).unwrap();
        project_connection.execute_batch(PROJECT_SCHEMA).unwrap();
        seed_v1_metadata(
            &project_connection,
            &StoreScope::Project {
                project_id: project_id.clone(),
            },
        );
        project_connection
            .execute(
                "INSERT INTO project_document(singleton, project_id, document_json)
                 VALUES(1, ?1, ?2)",
                params![project_id.as_str(), serde_json::to_string(&source).unwrap()],
            )
            .unwrap();
        drop(project_connection);
        let locator = serde_json::to_vec(&RootLocator {
            locator_format_version: 1,
            absolute_path: project_root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let ciphertext = protect_current_user(&locator, &binding_entropy(&binding_id)).unwrap();
        let binding = RootBindingEnvelopeV1 {
            schema_version: 1,
            root_binding_id: binding_id.clone(),
            project_id: if binding_identity_conflict {
                ProjectId::new()
            } else {
                project_id.clone()
            },
            protection_kind: "windows_current_user".to_owned(),
            ciphertext: BASE64.encode(ciphertext),
            created_at: Utc::now(),
        };
        write_private_atomic(
            &root_binding_path(&binding_root, &binding_id),
            &serde_json::to_vec_pretty(&binding).unwrap(),
        )
        .unwrap();
        V1Fixture {
            root,
            management_root,
            binding_root,
            project_root,
            project_id,
            binding_id,
        }
    }

    #[test]
    fn project_v1_to_v2_dry_run_resume_idempotency_and_rollback_are_verified() {
        let fixture = create_v1_fixture(false);
        let global_path = management_global_path(&fixture.management_root);
        let binding_path = root_binding_path(&fixture.binding_root, &fixture.binding_id);
        let global_before = migration_file_sha256(&global_path).unwrap();
        let binding_before = migration_file_sha256(&binding_path).unwrap();
        let open_error =
            match SqliteManagementRepositorySet::open(&fixture.management_root, "v2-test") {
                Err(error) => error,
                Ok(_) => panic!("v1 store must require explicit migration"),
            };
        assert_eq!(
            open_error.category,
            RepositoryErrorCategory::IncompatibleVersion
        );
        assert_eq!(migration_file_sha256(&global_path).unwrap(), global_before);
        let plan = plan_project_v1_to_v2(&fixture.management_root, &fixture.binding_root).unwrap();
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].project_id, fixture.project_id);
        assert_eq!(plan.entries[0].project.schema_version, 2);
        assert_eq!(plan.entries[0].project.attached_checkout_ids.len(), 1);
        assert_eq!(migration_file_sha256(&global_path).unwrap(), global_before);
        assert_eq!(
            migration_file_sha256(&binding_path).unwrap(),
            binding_before
        );
        assert!(!fixture.root.join("backup").exists());

        let stale = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            Sha256Hash::digest(b"stale").as_str(),
        )
        .unwrap_err();
        assert_eq!(stale.category, RepositoryErrorCategory::RevisionConflict);
        assert!(!fixture.root.join("backup").exists());

        let interrupted = apply_project_v1_to_v2_with_step_limit(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            plan.plan_fingerprint.as_str(),
            Some(1),
        )
        .unwrap();
        assert_eq!(interrupted.state, MigrationApplyState::Interrupted);
        assert_eq!(interrupted.completed_steps, 1);
        assert_eq!(
            inspect_store_read_only(&global_path),
            RecoveryInspection::MigrationRequired
        );
        read_store_status_read_only(&management_project_path(
            &fixture.management_root,
            &fixture.project_id,
        ))
        .unwrap();
        assert_eq!(
            inspect_store_read_only(&management_project_path(
                &fixture.management_root,
                &fixture.project_id
            )),
            RecoveryInspection::Healthy
        );

        let completed = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            plan.plan_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(completed.state, MigrationApplyState::Completed);
        assert_eq!(completed.completed_steps, completed.total_steps);
        let second = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            plan.plan_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(second.state, MigrationApplyState::Completed);
        assert_eq!(second.backup_fingerprint, completed.backup_fingerprint);

        let repositories =
            SqliteManagementRepositorySet::open(&fixture.management_root, "v2-test").unwrap();
        assert_eq!(repositories.global().list_projects().unwrap().len(), 1);
        assert_eq!(
            repositories
                .global()
                .list_project_checkouts(&fixture.project_id)
                .unwrap()
                .len(),
            1
        );
        drop(repositories);
        let bindings = WindowsProjectRootBindingStore::open(&fixture.binding_root).unwrap();
        let attachment = bindings
            .find_by_checkout(&plan.entries[0].checkout.as_ref().unwrap().checkout_id)
            .unwrap()
            .unwrap();
        assert_eq!(attachment.project_id, fixture.project_id);
        assert_eq!(
            bindings.resolve(&attachment.root_binding_id).unwrap(),
            fixture.project_root
        );

        let stale_rollback = rollback_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            Sha256Hash::digest(b"stale-backup").as_str(),
        )
        .unwrap_err();
        assert_eq!(
            stale_rollback.category,
            RepositoryErrorCategory::RevisionConflict
        );
        rollback_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            completed.backup_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(
            inspect_store_read_only(&global_path),
            RecoveryInspection::MigrationRequired
        );
        let restored: RootBindingEnvelopeV1 =
            serde_json::from_slice(&fs::read(binding_path).unwrap()).unwrap();
        assert_eq!(restored.schema_version, 1);
        assert_eq!(restored.project_id, fixture.project_id);
    }

    #[test]
    fn empty_project_v1_store_migrates_idempotently_and_rolls_back() {
        let fixture = create_empty_v1_fixture();
        let global_path = management_global_path(&fixture.management_root);
        let plan = plan_project_v1_to_v2(&fixture.management_root, &fixture.binding_root).unwrap();
        assert!(plan.entries.is_empty());

        let completed = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            plan.plan_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(completed.state, MigrationApplyState::Completed);
        assert_eq!(completed.completed_steps, 1);
        assert_eq!(completed.total_steps, 1);
        assert_eq!(
            inspect_management_root(&fixture.management_root),
            Some(RecoveryInspection::Healthy)
        );
        assert_eq!(
            read_active_set(&fixture.management_root)
                .unwrap()
                .unwrap()
                .manifest
                .entries[0]
                .management_store_version,
            MANAGEMENT_STORE_VERSION
        );
        let repositories =
            SqliteManagementRepositorySet::open(&fixture.management_root, "v2-test").unwrap();
        assert!(repositories.global().list_projects().unwrap().is_empty());
        drop(repositories);

        let second = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            plan.plan_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(second, completed);
        rollback_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &fixture.root.join("backup"),
            &plan,
            completed.backup_fingerprint.as_str(),
        )
        .unwrap();
        assert_eq!(
            inspect_management_root(&fixture.management_root),
            Some(RecoveryInspection::MigrationRequired)
        );
        assert_eq!(
            read_active_set(&fixture.management_root)
                .unwrap()
                .unwrap()
                .manifest
                .entries[0]
                .management_store_version,
            1
        );
        let restored = Connection::open_with_flags(
            &global_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        assert_eq!(checked_store_version(&restored).unwrap(), 1);
        assert_eq!(
            restored
                .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn project_v1_to_v2_blocks_root_binding_identity_conflicts() {
        let fixture = create_v1_fixture(true);
        let error =
            plan_project_v1_to_v2(&fixture.management_root, &fixture.binding_root).unwrap_err();
        assert_eq!(error.category, RepositoryErrorCategory::Invalid);
    }

    #[test]
    fn project_v1_to_v2_rejects_a_tampered_verified_backup() {
        let fixture = create_v1_fixture(false);
        let plan = plan_project_v1_to_v2(&fixture.management_root, &fixture.binding_root).unwrap();
        let backup_root = fixture.root.join("backup");
        let interrupted = apply_project_v1_to_v2_with_step_limit(
            &fixture.management_root,
            &fixture.binding_root,
            &backup_root,
            &plan,
            plan.plan_fingerprint.as_str(),
            Some(0),
        )
        .unwrap();
        assert_eq!(interrupted.state, MigrationApplyState::Interrupted);
        let manifest: MigrationBackupManifest =
            serde_json::from_slice(&fs::read(backup_root.join("migration-backup.json")).unwrap())
                .unwrap();
        let global_backup = manifest
            .files
            .iter()
            .find(|file| matches!(file.kind, MigrationBackupKind::Global))
            .unwrap();
        let mut bytes = fs::read(backup_root.join(&global_backup.relative_path)).unwrap();
        bytes.push(0);
        fs::write(backup_root.join(&global_backup.relative_path), bytes).unwrap();
        let error = apply_project_v1_to_v2(
            &fixture.management_root,
            &fixture.binding_root,
            &backup_root,
            &plan,
            plan.plan_fingerprint.as_str(),
        )
        .unwrap_err();
        assert_eq!(error.category, RepositoryErrorCategory::IntegrityFailed);
    }

    #[test]
    fn writer_lease_project_partition_backup_and_root_binding_are_real() {
        let root = std::env::temp_dir().join(format!(
            "star-state-p0-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("file.txt"), b"value\n").unwrap();
        let bindings = WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let binding_id = bindings.attach(&project_id, &checkout_id, &source).unwrap();
        assert_eq!(
            bindings.resolve(&binding_id).unwrap(),
            source.canonicalize().unwrap()
        );

        let repositories =
            SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap();
        match SqliteManagementRepositorySet::open(root.join("management"), "test") {
            Err(error) => assert_eq!(error.category, RepositoryErrorCategory::Busy),
            Ok(_) => panic!("a second writer must not acquire the lease"),
        }
        let project = project(project_id.clone(), checkout_id.clone());
        let checkout = checkout(project_id.clone(), checkout_id, binding_id);
        let fingerprint =
            star_domain::versioned_fingerprint("star.project-register", 1, &project).unwrap();
        repositories
            .global()
            .register_project(&project, &checkout, "register-1", &fingerprint)
            .unwrap();
        let project_repository = repositories.project(&project_id).unwrap();
        let operation_id = CoordinatedOperationId::new();
        project_repository
            .commit_registration_participant(&project, &operation_id, &fingerprint, &fingerprint)
            .unwrap();
        assert_eq!(
            repositories.global().list_projects().unwrap(),
            vec![project]
        );
        assert_eq!(repositories.verify_all().unwrap().len(), 2);
        assert_eq!(
            repositories
                .plan_backup(&root.join("management/unsafe-backup"))
                .unwrap_err()
                .category,
            RepositoryErrorCategory::Invalid
        );
        let backup_plan = repositories.plan_backup(&root.join("backup")).unwrap();
        let backup = repositories
            .apply_backup(
                &root.join("backup"),
                &backup_plan,
                backup_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(
            repositories
                .apply_backup(
                    &root.join("backup"),
                    &backup_plan,
                    backup_plan.plan_fingerprint.as_str(),
                )
                .unwrap(),
            backup
        );
        assert_eq!(backup.manifest.entries.len(), 2);
        let global_backup = root.join("backup").join(
            &backup
                .manifest
                .entries
                .iter()
                .find(|entry| matches!(entry.scope, StoreScope::Global))
                .unwrap()
                .relative_locator,
        );
        assert_eq!(
            inspect_store_read_only(&global_backup),
            RecoveryInspection::Healthy
        );
        assert_eq!(
            restore_verified_backup_side_by_side(
                &global_backup,
                &root.join("restored/global-generation.db")
            )
            .unwrap(),
            RecoveryInspection::Healthy
        );
        let global_status = repositories.global().status().unwrap();
        let unbound_compaction_plan = repositories
            .plan_compaction(&global_status.store_id, None, None)
            .unwrap();
        assert_eq!(
            unbound_compaction_plan.store_revision,
            global_status.store_revision
        );
        assert!(
            unbound_compaction_plan.required_free_bytes >= unbound_compaction_plan.database_bytes
        );
        assert_eq!(
            repositories
                .apply_compaction(
                    &unbound_compaction_plan,
                    unbound_compaction_plan.plan_fingerprint.as_str(),
                    &root.join("backup"),
                    &backup,
                )
                .unwrap_err()
                .category,
            RepositoryErrorCategory::RevisionConflict
        );
        let compaction_plan = repositories
            .plan_compaction(
                &global_status.store_id,
                Some(&root.join("backup")),
                Some(&backup),
            )
            .unwrap();
        let mut tampered_backup = backup.clone();
        tampered_backup.result_fingerprint = Sha256Hash::digest(b"tampered-backup-result");
        assert_eq!(
            repositories
                .apply_compaction(
                    &compaction_plan,
                    compaction_plan.plan_fingerprint.as_str(),
                    &root.join("backup"),
                    &tampered_backup,
                )
                .unwrap_err()
                .category,
            RepositoryErrorCategory::IntegrityFailed
        );
        let compacted = repositories
            .apply_compaction(
                &compaction_plan,
                compaction_plan.plan_fingerprint.as_str(),
                &root.join("backup"),
                &backup,
            )
            .unwrap();
        assert_eq!(compacted.plan_fingerprint, compaction_plan.plan_fingerprint);
        assert!(compacted.before_bytes > 0 && compacted.after_bytes > 0);
        let future = root.join("future.db");
        fs::copy(&global_backup, &future).unwrap();
        let future_connection = Connection::open(&future).unwrap();
        future_connection
            .pragma_update(None, "user_version", MANAGEMENT_STORE_VERSION + 1)
            .unwrap();
        drop(future_connection);
        assert_eq!(
            inspect_store_read_only(&future),
            RecoveryInspection::FutureVersion
        );
        let corrupt = root.join("corrupt.db");
        fs::write(&corrupt, b"not a database").unwrap();
        assert_eq!(
            inspect_store_read_only(&corrupt),
            RecoveryInspection::Corrupt
        );

        let concrete_project = repositories.project_repository(&project_id).unwrap();
        let old_run = ScanRun {
            schema_id: "star.scan-run".to_owned(),
            schema_version: 1,
            scan_run_id: ScanRunId::new(),
            project_id: project_id.clone(),
            project_revision_id: ProjectRevisionId::new(),
            workspace_snapshot_id: WorkspaceSnapshotId::new(),
            effective_config_fingerprint: Sha256Hash::digest(b"config"),
            scan_config_fingerprint: Sha256Hash::digest(b"scan-config"),
            rule_set_fingerprint: Sha256Hash::digest(b"rules"),
            input_fingerprint: Sha256Hash::digest(b"input"),
            status: ScanStatus::Incomplete,
            generation_id: GenerationId::new(),
            started_at: Utc::now() - chrono::Duration::days(8),
            finished_at: Some(Utc::now() - chrono::Duration::days(8)),
            reused_from_scan_run_id: None,
            counts: BTreeMap::new(),
            limitations: vec!["fixture".to_owned()],
            artifact_refs: vec![],
        };
        let mut held_run = old_run.clone();
        held_run.scan_run_id = ScanRunId::new();
        held_run.project_revision_id = ProjectRevisionId::new();
        held_run.workspace_snapshot_id = WorkspaceSnapshotId::new();
        held_run.generation_id = GenerationId::new();
        held_run.input_fingerprint = Sha256Hash::digest(b"held-input");
        let mut hold_artifact: ArtifactRef = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/artifact-ref/full.json"
        ))
        .unwrap();
        hold_artifact.project_id = Some(project_id.clone());
        hold_artifact.retention_class = EvidenceRetentionClass::Hold;
        held_run.artifact_refs = vec![hold_artifact];
        {
            let mut connection = concrete_project.connection.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            insert_generation_document(
                &transaction,
                "scan_runs",
                "scan_run_id",
                old_run.scan_run_id.as_str(),
                old_run.generation_id.as_str(),
                &old_run,
            )
            .unwrap();
            insert_generation_document(
                &transaction,
                "scan_runs",
                "scan_run_id",
                held_run.scan_run_id.as_str(),
                held_run.generation_id.as_str(),
                &held_run,
            )
            .unwrap();
            transaction.commit().unwrap();
        }
        let mut active_suppression: Suppression = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/suppression/full.json"
        ))
        .unwrap();
        active_suppression.revision = 1;
        active_suppression.scope_kind = SuppressionScope::Local;
        active_suppression.project_id = project_id.clone();
        active_suppression.selector = "rule:retention-fixture".to_owned();
        active_suppression.created_at = Utc::now() - chrono::Duration::days(1);
        active_suppression.expires_at = Some(Utc::now() + chrono::Duration::days(30));
        active_suppression.source_revision_constraint = None;
        active_suppression.config_fingerprint_constraint = None;
        concrete_project
            .put_suppression(&active_suppression, 0)
            .unwrap();
        let active_hold_plan = repositories
            .plan_retention_v2(&RetentionPolicyV2::default())
            .unwrap();
        assert!(active_hold_plan.candidates.is_empty());
        assert!(
            active_hold_plan
                .protected_targets
                .iter()
                .any(|target| target.reason_code == "ACTIVE_SUPPRESSION_HOLD")
        );

        active_suppression.revision = 2;
        active_suppression.status = SuppressionStatus::Revoked;
        concrete_project
            .put_suppression(&active_suppression, 1)
            .unwrap();
        let plan = repositories
            .plan_retention_v2(&RetentionPolicyV2::default())
            .unwrap();
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.protected_targets.len(), 1);
        assert_eq!(
            plan.protected_targets[0].reason_code,
            "ARTIFACT_RETENTION_HOLD"
        );
        assert_eq!(plan.estimated_rows, 1);
        assert!(plan.estimated_bytes > 0);
        let checkpoint = RetentionCheckpointV1::start(
            star_contracts::ids::OperationId::new(),
            plan.plan_fingerprint.clone(),
            plan.expected_store_revisions.clone(),
        );
        let mut unsafe_plan = plan.clone();
        unsafe_plan.candidates[0].retention_class = "local_disposition_v2".to_owned();
        assert!(unsafe_plan.validate().is_err());
        let mut tampered_checkpoint = checkpoint.clone();
        tampered_checkpoint.committed_batch_count = 1;
        assert!(validate_retention_checkpoint(&plan, &tampered_checkpoint).is_err());
        let mut tampered_checkpoint = checkpoint.clone();
        tampered_checkpoint
            .expected_store_revisions
            .remove("global");
        assert!(validate_retention_checkpoint(&plan, &tampered_checkpoint).is_err());
        repositories
            .start_retention_execution(&plan, &checkpoint)
            .unwrap();
        let applied = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert_eq!(applied.processed_candidates, 1);
        assert_eq!(applied.applied_rows, 1);
        assert!(applied.checkpoint.complete);
        // Replaying a batch after the project transaction committed but before an
        // external checkpoint observation is idempotent and advances the same cursor.
        let replay = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert_eq!(
            replay.checkpoint.next_candidate_index,
            applied.checkpoint.next_candidate_index
        );
        assert_eq!(
            replay.checkpoint.applied_rows,
            applied.checkpoint.applied_rows
        );
        assert_eq!(
            replay.checkpoint.expected_store_revisions,
            applied.checkpoint.expected_store_revisions
        );
        repositories
            .clear_retention_execution(&plan.plan_fingerprint)
            .unwrap();
        assert!(repositories.load_retention_execution().unwrap().is_none());
        let remaining = repositories
            .plan_retention_v2(&RetentionPolicyV2::default())
            .unwrap();
        assert!(remaining.candidates.is_empty());
        assert_eq!(remaining.protected_targets.len(), 1);
        assert_eq!(
            remaining.protected_targets[0].reason_code,
            "ARTIFACT_RETENTION_HOLD"
        );

        let active_set = read_active_set(&root.join("management"))
            .unwrap()
            .unwrap()
            .manifest;
        let global = active_set
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, StoreScope::Global))
            .unwrap();
        let database = fs::read(active_store_file(&root.join("management"), global)).unwrap();
        assert!(
            !String::from_utf8_lossy(&database).contains(&source.to_string_lossy().to_string())
        );
    }

    #[test]
    fn migration_backup_retention_preserves_floor_and_resumes_tombstone_delete() {
        let state_root = std::env::temp_dir().join(format!(
            "star-migration-backup-retention-{}",
            RootBindingId::new().as_str()
        ));
        let management_root = state_root.join("management");
        let repositories =
            SqliteManagementRepositorySet::open(&management_root, "retention-backup-test").unwrap();
        for seed in ["oldest", "older", "newer", "newest"] {
            create_retention_migration_backup(&management_root, seed);
            std::thread::sleep(Duration::from_millis(20));
        }
        let invalid_locator = format!(
            "{MIGRATION_BACKUP_PREFIX}{}",
            Sha256Hash::digest(b"invalid")
                .as_str()
                .trim_start_matches("sha256:")
        );
        let invalid_root = migration_backups_root(&management_root)
            .unwrap()
            .join(invalid_locator);
        fs::create_dir_all(&invalid_root).unwrap();
        fs::write(invalid_root.join(MIGRATION_BACKUP_MANIFEST_FILENAME), b"{}").unwrap();

        let plan = repositories
            .plan_retention_v2(&RetentionPolicyV2::default())
            .unwrap();
        plan.validate().unwrap();
        assert_eq!(plan.migration_backup_candidates.len(), 1);
        assert_eq!(plan.protected_migration_backups.len(), 4);
        assert_eq!(
            plan.protected_migration_backups
                .iter()
                .filter(|backup| backup.reason_code == "MIGRATION_BACKUP_LATEST_KNOWN_GOOD")
                .count(),
            1
        );
        assert_eq!(
            plan.protected_migration_backups
                .iter()
                .filter(|backup| backup.reason_code == "MIGRATION_BACKUP_FLOOR")
                .count(),
            2
        );
        assert_eq!(
            plan.protected_migration_backups
                .iter()
                .filter(|backup| backup.reason_code == "MIGRATION_BACKUP_UNVERIFIED")
                .count(),
            1
        );
        let candidate = plan.migration_backup_candidates[0].clone();
        assert_eq!(plan.estimated_rows, candidate.estimated_rows);
        assert_eq!(plan.estimated_bytes, candidate.estimated_bytes);
        let checkpoint = RetentionCheckpointV1::start(
            star_contracts::ids::OperationId::new(),
            plan.plan_fingerprint.clone(),
            plan.expected_store_revisions.clone(),
        );
        repositories
            .start_retention_execution(&plan, &checkpoint)
            .unwrap();

        // Model a process stop after the atomic quarantine rename and before the
        // tombstone removal/checkpoint write. The same approved batch resumes it.
        let backups_root = migration_backups_root(&management_root).unwrap();
        let candidate_root = backups_root.join(&candidate.backup_locator);
        let tombstone = retention_delete_tombstone(&backups_root, &candidate);
        fs::rename(&candidate_root, &tombstone).unwrap();
        let applied = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert!(applied.checkpoint.complete);
        assert_eq!(applied.applied_rows, candidate.estimated_rows);
        assert_eq!(applied.applied_bytes, candidate.estimated_bytes);
        assert!(!candidate_root.exists());
        assert!(!tombstone.exists());

        let replay = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert_eq!(
            replay.checkpoint.next_candidate_index,
            applied.checkpoint.next_candidate_index
        );
        assert_eq!(
            replay.checkpoint.applied_rows,
            applied.checkpoint.applied_rows
        );
        assert_eq!(
            replay.checkpoint.applied_bytes,
            applied.checkpoint.applied_bytes
        );
        repositories
            .clear_retention_execution(&plan.plan_fingerprint)
            .unwrap();
        let remaining = repositories
            .plan_retention_v2(&RetentionPolicyV2::default())
            .unwrap();
        assert!(remaining.migration_backup_candidates.is_empty());
        assert_eq!(remaining.protected_migration_backups.len(), 4);
    }

    #[test]
    fn retention_splits_large_generation_at_row_limit_and_resumes_after_restart() {
        let root = std::env::temp_dir().join(format!(
            "star-retention-cardinality-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let management_root = root.join("management");
        let repositories =
            SqliteManagementRepositorySet::open(&management_root, "retention-cardinality-test")
                .unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let project = project(project_id.clone(), checkout_id.clone());
        let checkout = checkout(project_id.clone(), checkout_id, RootBindingId::new());
        let fingerprint =
            versioned_fingerprint("star.retention.cardinality.fixture", 1, &project).unwrap();
        repositories
            .global()
            .register_project(&project, &checkout, "retention-cardinality", &fingerprint)
            .unwrap();
        repositories
            .project(&project_id)
            .unwrap()
            .commit_registration_participant(
                &project,
                &CoordinatedOperationId::new(),
                &fingerprint,
                &fingerprint,
            )
            .unwrap();

        let concrete_project = repositories.project_repository(&project_id).unwrap();
        let old_run = ScanRun {
            schema_id: "star.scan-run".to_owned(),
            schema_version: 1,
            scan_run_id: ScanRunId::new(),
            project_id: project_id.clone(),
            project_revision_id: ProjectRevisionId::new(),
            workspace_snapshot_id: WorkspaceSnapshotId::new(),
            effective_config_fingerprint: Sha256Hash::digest(b"config"),
            scan_config_fingerprint: Sha256Hash::digest(b"scan-config"),
            rule_set_fingerprint: Sha256Hash::digest(b"rules"),
            input_fingerprint: Sha256Hash::digest(b"input"),
            status: ScanStatus::Incomplete,
            generation_id: GenerationId::new(),
            started_at: Utc::now() - chrono::Duration::days(8),
            finished_at: Some(Utc::now() - chrono::Duration::days(8)),
            reused_from_scan_run_id: None,
            counts: BTreeMap::new(),
            limitations: vec!["cardinality fixture".to_owned()],
            artifact_refs: Vec::new(),
        };
        {
            let mut connection = concrete_project.connection.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            insert_generation_document(
                &transaction,
                "scan_runs",
                "scan_run_id",
                old_run.scan_run_id.as_str(),
                old_run.generation_id.as_str(),
                &old_run,
            )
            .unwrap();
            let mut insert = transaction
                .prepare(
                    "INSERT INTO source_entries(entity_id, generation_id, document_json) VALUES(?1, ?2, ?3)",
                )
                .unwrap();
            for index in 0..10_004_u64 {
                insert
                    .execute(params![
                        format!("entry-{index:05}"),
                        old_run.generation_id.as_str(),
                        r#"{"fixture":true}"#,
                    ])
                    .unwrap();
            }
            drop(insert);
            transaction.commit().unwrap();
        }

        let policy = RetentionPolicyV2::default();
        let plan = repositories.plan_retention_v2(&policy).unwrap();
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].estimated_rows, 10_005);
        assert!(plan.candidates[0].estimated_rows > policy.max_batch_rows);
        let checkpoint = RetentionCheckpointV1::start(
            star_contracts::ids::OperationId::new(),
            plan.plan_fingerprint.clone(),
            plan.expected_store_revisions.clone(),
        );
        repositories
            .start_retention_execution(&plan, &checkpoint)
            .unwrap();
        let first = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert_eq!(first.applied_rows, policy.max_batch_rows);
        assert_eq!(first.processed_candidates, 0);
        assert_eq!(first.checkpoint.next_candidate_index, 0);
        assert_eq!(
            first.checkpoint.active_candidate_id.as_deref(),
            Some(plan.candidates[0].candidate_id.as_str())
        );
        assert!(!first.checkpoint.complete);

        let replay = repositories
            .apply_retention_batch_v2(&plan, &checkpoint)
            .unwrap();
        assert_eq!(
            replay.checkpoint.next_candidate_index,
            first.checkpoint.next_candidate_index
        );
        assert_eq!(
            replay.checkpoint.committed_batch_count,
            first.checkpoint.committed_batch_count
        );
        assert_eq!(
            replay.checkpoint.applied_rows,
            first.checkpoint.applied_rows
        );
        assert_eq!(
            replay.checkpoint.active_candidate_id,
            first.checkpoint.active_candidate_id
        );
        drop(concrete_project);
        drop(repositories);

        let reopened =
            SqliteManagementRepositorySet::open(&management_root, "retention-cardinality-test")
                .unwrap();
        let (resumed_plan, resumed_checkpoint) = reopened
            .load_retention_execution()
            .unwrap()
            .expect("durable partial retention checkpoint");
        assert_eq!(resumed_plan.plan_fingerprint, plan.plan_fingerprint);
        assert_eq!(resumed_checkpoint, replay.checkpoint);
        let second = reopened
            .apply_retention_batch_v2(&resumed_plan, &resumed_checkpoint)
            .unwrap();
        assert_eq!(second.applied_rows, 5);
        assert_eq!(second.processed_candidates, 1);
        assert_eq!(second.checkpoint.committed_batch_count, 2);
        assert_eq!(second.checkpoint.applied_rows, 10_005);
        assert!(second.checkpoint.active_candidate_id.is_none());
        assert!(second.checkpoint.complete);
        reopened
            .clear_retention_execution(&plan.plan_fingerprint)
            .unwrap();
    }

    #[test]
    fn backup_plan_restore_preserves_corrupt_generation_and_rejects_tamper() {
        let fixture = recovery_fixture("restore");
        let manifest_json = serde_json::to_string(&fixture.backup_manifest).unwrap();
        assert!(!manifest_json.contains(fixture.root.to_string_lossy().as_ref()));
        if let Ok(username) = std::env::var("USERNAME")
            && username.len() >= 3
        {
            assert!(
                !manifest_json
                    .to_ascii_lowercase()
                    .contains(&username.to_ascii_lowercase())
            );
        }

        let corrupt_bytes = b"corrupt-active-generation";
        fs::write(&fixture.old_global_path, corrupt_bytes).unwrap();
        let open_error = match SqliteManagementRepositorySet::open(
            &fixture.management_root,
            "must-enter-recovery",
        ) {
            Err(error) => error,
            Ok(_) => panic!("corrupt active generation must not enter normal mode"),
        };
        assert_eq!(
            open_error.category,
            RepositoryErrorCategory::IntegrityFailed
        );

        let recovery =
            SqliteManagementRecovery::open(&fixture.management_root, "recovery-test").unwrap();
        assert_eq!(
            recovery.status().unwrap().mode,
            ControllerRecoveryMode::RecoveryOnly
        );
        let global_backup = fixture
            .backup_manifest
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, StoreScope::Global))
            .unwrap();
        let global_backup_path = fixture.backup_root.join(&global_backup.relative_locator);
        let original_backup = fs::read(&global_backup_path).unwrap();

        let future_connection = Connection::open(&global_backup_path).unwrap();
        future_connection
            .pragma_update(None, "user_version", MANAGEMENT_STORE_VERSION + 1)
            .unwrap();
        drop(future_connection);
        assert_eq!(
            inspect_store_read_only(&global_backup_path),
            RecoveryInspection::FutureVersion
        );
        assert_eq!(
            recovery
                .plan_restore(&fixture.backup_root)
                .unwrap_err()
                .category,
            RepositoryErrorCategory::IntegrityFailed
        );
        fs::write(&global_backup_path, &original_backup).unwrap();

        let project_backup = fixture
            .backup_manifest
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, StoreScope::Project { .. }))
            .unwrap();
        let project_backup_path = fixture.backup_root.join(&project_backup.relative_locator);
        let missing_path = project_backup_path.with_extension("missing-fixture");
        fs::rename(&project_backup_path, &missing_path).unwrap();
        assert_eq!(
            recovery
                .plan_restore(&fixture.backup_root)
                .unwrap_err()
                .category,
            RepositoryErrorCategory::IntegrityFailed
        );
        fs::rename(&missing_path, &project_backup_path).unwrap();

        fs::write(&global_backup_path, b"tampered-backup").unwrap();
        assert_eq!(
            recovery
                .plan_restore(&fixture.backup_root)
                .unwrap_err()
                .category,
            RepositoryErrorCategory::IntegrityFailed
        );
        fs::write(&global_backup_path, original_backup).unwrap();

        let plan = recovery.plan_restore(&fixture.backup_root).unwrap();
        assert_eq!(
            recovery
                .apply_restore(
                    &fixture.backup_root,
                    &plan,
                    Sha256Hash::digest(b"wrong approval").as_str(),
                )
                .unwrap_err()
                .category,
            RepositoryErrorCategory::RevisionConflict
        );
        assert!(plan.stores.iter().all(|store| {
            !fixture
                .management_root
                .join(&store.candidate_relative_locator)
                .join(STORE_FILENAME)
                .exists()
        }));

        let restored = recovery
            .apply_restore(&fixture.backup_root, &plan, plan.plan_fingerprint.as_str())
            .unwrap();
        assert_eq!(
            recovery
                .apply_restore(&fixture.backup_root, &plan, plan.plan_fingerprint.as_str(),)
                .unwrap(),
            restored
        );
        assert_eq!(
            restored.activated_set.manifest_fingerprint,
            plan.candidate_active_set.manifest_fingerprint
        );
        assert_eq!(fs::read(&fixture.old_global_path).unwrap(), corrupt_bytes);
        assert_eq!(
            recovery.status().unwrap().mode,
            ControllerRecoveryMode::Normal
        );
        drop(recovery);

        let reopened =
            SqliteManagementRepositorySet::open(&fixture.management_root, "restored-test").unwrap();
        assert!(
            reopened
                .global()
                .get_project(&fixture.project_id)
                .unwrap()
                .is_some()
        );
        assert_eq!(reopened.verify_all().unwrap().len(), 2);
    }

    #[test]
    fn active_set_header_mismatch_enters_recovery_only_even_when_manifest_is_resealed() {
        let fixture = recovery_fixture("active-header-mismatch");
        let mut entries = read_active_set(&fixture.management_root)
            .unwrap()
            .unwrap()
            .manifest
            .entries;
        entries[0].generation = entries[0].generation.checked_add(1).unwrap();
        let mismatched = seal_active_set(entries).unwrap();
        write_active_set_document(&fixture.management_root, &mismatched).unwrap();

        let normal = SqliteManagementRepositorySet::open(&fixture.management_root, "test");
        assert!(matches!(
            normal,
            Err(RepositoryError {
                category: RepositoryErrorCategory::IntegrityFailed,
                ..
            })
        ));
        let recovery =
            SqliteManagementRecovery::open(&fixture.management_root, "recovery-test").unwrap();
        let status = recovery.status().unwrap();
        assert_eq!(status.mode, ControllerRecoveryMode::RecoveryOnly);
        assert!(status.stores.iter().any(|store| {
            store.inspection == RecoveryInspection::ActiveSetMismatch
                && store.diagnostic_code == "RECOVERY_ACTIVE_SET_MATERIALIZATION_MISMATCH"
        }));
    }

    #[test]
    fn restore_activation_is_all_old_or_all_new_across_crash_points() {
        let during_write = recovery_fixture("crash-during-write");
        fs::write(
            &during_write.old_global_path,
            b"corrupt-during-candidate-write",
        )
        .unwrap();
        let recovery =
            SqliteManagementRecovery::open(&during_write.management_root, "recovery-test").unwrap();
        let plan = recovery.plan_restore(&during_write.backup_root).unwrap();
        let old_active_fingerprint =
            active_set_file_fingerprint(&during_write.management_root).unwrap();
        assert!(
            recovery
                .apply_restore_with_fault(
                    &during_write.backup_root,
                    &plan,
                    plan.plan_fingerprint.as_str(),
                    Some(RestoreFaultPoint::AfterFirstStore),
                )
                .is_err()
        );
        assert_eq!(
            active_set_file_fingerprint(&during_write.management_root).unwrap(),
            old_active_fingerprint
        );
        assert_eq!(
            inspect_management_root(&during_write.management_root),
            Some(RecoveryInspection::Corrupt)
        );
        assert_eq!(
            plan.stores
                .iter()
                .filter(|store| during_write
                    .management_root
                    .join(&store.candidate_relative_locator)
                    .join(STORE_FILENAME)
                    .is_file())
                .count(),
            1
        );
        assert!(
            during_write
                .management_root
                .join("quarantine")
                .join(format!("{}.json", plan.recovery_plan_id.as_str()))
                .is_file()
        );
        drop(recovery);

        let before = recovery_fixture("crash-before");
        fs::write(&before.old_global_path, b"corrupt-before-restore").unwrap();
        let recovery =
            SqliteManagementRecovery::open(&before.management_root, "recovery-test").unwrap();
        let plan = recovery.plan_restore(&before.backup_root).unwrap();
        let old_active_fingerprint = active_set_file_fingerprint(&before.management_root).unwrap();
        assert!(
            recovery
                .apply_restore_with_fault(
                    &before.backup_root,
                    &plan,
                    plan.plan_fingerprint.as_str(),
                    Some(RestoreFaultPoint::BeforeActivation),
                )
                .is_err()
        );
        assert_eq!(
            active_set_file_fingerprint(&before.management_root).unwrap(),
            old_active_fingerprint
        );
        assert_eq!(
            inspect_management_root(&before.management_root),
            Some(RecoveryInspection::Corrupt)
        );
        assert!(
            before
                .management_root
                .join("quarantine")
                .join(format!("{}.json", plan.recovery_plan_id.as_str()))
                .is_file()
        );
        drop(recovery);

        let after = recovery_fixture("crash-after");
        fs::write(&after.old_global_path, b"corrupt-after-restore").unwrap();
        let recovery =
            SqliteManagementRecovery::open(&after.management_root, "recovery-test").unwrap();
        let plan = recovery.plan_restore(&after.backup_root).unwrap();
        assert!(
            recovery
                .apply_restore_with_fault(
                    &after.backup_root,
                    &plan,
                    plan.plan_fingerprint.as_str(),
                    Some(RestoreFaultPoint::AfterActivation),
                )
                .is_err()
        );
        let active = read_active_set(&after.management_root)
            .unwrap()
            .unwrap()
            .manifest;
        assert_eq!(
            active.manifest_fingerprint,
            plan.candidate_active_set.manifest_fingerprint
        );
        assert_eq!(
            inspect_management_root(&after.management_root),
            Some(RecoveryInspection::Healthy)
        );
        let quarantine = fs::read_to_string(
            after
                .management_root
                .join("quarantine")
                .join(format!("{}.json", plan.recovery_plan_id.as_str())),
        )
        .unwrap();
        assert!(quarantine.contains("RESTORE_ACTIVATED_OUTCOME_REQUIRES_RECONCILE"));
        let reconciled = recovery
            .apply_restore(&after.backup_root, &plan, plan.plan_fingerprint.as_str())
            .unwrap();
        assert_eq!(reconciled.activated_set, plan.candidate_active_set);
        assert_eq!(
            recovery
                .apply_restore(&after.backup_root, &plan, plan.plan_fingerprint.as_str())
                .unwrap(),
            reconciled
        );
        drop(recovery);
        SqliteManagementRepositorySet::open(&after.management_root, "restored-after-crash")
            .unwrap();
    }

    #[test]
    fn code_index_cache_is_content_addressed_bounded_and_never_current_truth() {
        let root = std::env::temp_dir().join(format!(
            "star-index-cache-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let snapshot: CodeIndexSnapshot = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/code-index-snapshot/minimal.json"
        ))
        .unwrap();
        let project_id = snapshot.project_id.clone();
        let projection = StoredCodeIndexProjection {
            snapshot,
            source_entries: vec![],
            entities: vec![],
            edges: vec![],
            symbols: vec![],
            references: vec![],
        };
        let default_cache = FileCodeIndexCache::open(root.join("default-limits")).unwrap();
        assert_eq!(default_cache.max_entries_per_project, 8);
        assert_eq!(default_cache.max_entry_bytes, 256 * 1024 * 1024);
        assert_eq!(default_cache.max_project_bytes, 512 * 1024 * 1024);
        let integrity_cache = FileCodeIndexCache::open_with_limits(
            root.join("integrity"),
            3,
            1024 * 1024,
            3 * 1024 * 1024,
        )
        .unwrap();
        let key = Sha256Hash::digest(b"cache-key");
        integrity_cache
            .store(&project_id, &key, &projection)
            .unwrap();
        assert_eq!(
            integrity_cache
                .load(&project_id, &key)
                .unwrap()
                .unwrap()
                .snapshot
                .code_index_snapshot_id,
            projection.snapshot.code_index_snapshot_id
        );
        let entry = integrity_cache.entry_path(&project_id, &key);
        let protected_entry = String::from_utf8_lossy(&fs::read(&entry).unwrap()).to_string();
        assert!(!protected_entry.contains(&root.to_string_lossy().to_string()));
        assert!(!protected_entry.contains(project_id.as_str()));
        assert!(!protected_entry.contains("star.code-index-snapshot"));
        assert!(!protected_entry.contains(projection.snapshot.content_fingerprint.as_str()));
        fs::write(&entry, b"{}").unwrap();
        assert_eq!(
            integrity_cache
                .load(&project_id, &key)
                .unwrap_err()
                .category,
            RepositoryErrorCategory::Corrupt
        );
        integrity_cache
            .store(&project_id, &key, &projection)
            .unwrap();
        assert!(integrity_cache.load(&project_id, &key).unwrap().is_some());

        let eviction_cache = FileCodeIndexCache::open_with_limits(
            root.join("eviction"),
            2,
            1024 * 1024,
            2 * 1024 * 1024,
        )
        .unwrap();
        for seed in [b"one".as_slice(), b"two".as_slice(), b"three".as_slice()] {
            eviction_cache
                .store(&project_id, &Sha256Hash::digest(seed), &projection)
                .unwrap();
        }
        assert_eq!(
            fs::read_dir(eviction_cache.project_root(&project_id))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "json"))
                .count(),
            2
        );
    }

    #[test]
    fn cancelled_and_crashed_scan_never_replace_current_generation_after_restart() {
        let root = std::env::temp_dir().join(format!(
            "star-scan-recovery-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let management_root = root.join("management");
        let repositories = SqliteManagementRepositorySet::open(&management_root, "test").unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let binding_id = RootBindingId::new();
        let registered_project = project(project_id.clone(), checkout_id.clone());
        let registered_checkout = checkout(project_id.clone(), checkout_id, binding_id);
        let registration_fingerprint =
            versioned_fingerprint("star.test.registration", 1, &registered_project).unwrap();
        repositories
            .global()
            .register_project(
                &registered_project,
                &registered_checkout,
                "register-recovery",
                &registration_fingerprint,
            )
            .unwrap();
        let repository = repositories.project_repository(&project_id).unwrap();

        let succeeded = scan_commit(registered_project.clone(), ScanStatus::Succeeded, "success");
        repository.commit_scan(&succeeded).unwrap();
        let current_run_id = succeeded.run.scan_run_id.clone();

        let cancelled = scan_commit(
            registered_project.clone(),
            ScanStatus::Cancelled,
            "cancelled",
        );
        repository.commit_scan(&cancelled).unwrap();
        assert_eq!(
            repository.latest_scan().unwrap().unwrap().scan_run_id,
            current_run_id
        );

        {
            let connection = repository.connection.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER simulate_scan_crash
                     BEFORE INSERT ON index_entities
                     BEGIN SELECT RAISE(ABORT, 'simulated scan crash'); END;",
                )
                .unwrap();
        }
        let mut crashed = scan_commit(registered_project, ScanStatus::Succeeded, "crashed");
        crashed.index_entities.push(IndexEntity {
            entity_key: "fixture:crash".to_owned(),
            kind: star_contracts::index::IndexEntityKind::TextToken,
            canonical_source_id: None,
            symbol_id: None,
            qualified_name: "crash".to_owned(),
            source_range: None,
            tier: star_contracts::index::IndexTier::Text,
            confidence: "fixture".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"crash-entity"),
        });
        assert!(repository.commit_scan(&crashed).is_err());
        assert!(
            repository
                .replay_scan(&crashed.idempotency_key, &crashed.payload_fingerprint)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            repository.latest_scan().unwrap().unwrap().scan_run_id,
            current_run_id
        );
        drop(repository);
        drop(repositories);

        let reopened = SqliteManagementRepositorySet::open(&management_root, "test").unwrap();
        assert_eq!(
            reopened
                .project_repository(&project_id)
                .unwrap()
                .latest_scan()
                .unwrap()
                .unwrap()
                .scan_run_id,
            current_run_id
        );
        reopened.verify_all().unwrap();
    }

    #[test]
    fn project_catalog_snapshot_replay_is_idempotent_and_identity_conflict_fails() {
        let root = std::env::temp_dir().join(format!(
            "star-catalog-replay-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let repositories = SqliteManagementRepositorySet::open(root, "test").unwrap();
        let snapshot: ProjectCatalogSnapshot = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/project-catalog-snapshot/minimal.json"
        ))
        .unwrap();
        repositories
            .global()
            .put_project_catalog_snapshot(&snapshot)
            .unwrap();
        let revision = repositories.global().status().unwrap().store_revision;
        let mut replay = snapshot.clone();
        replay.captured_at = Utc::now();
        repositories
            .global()
            .put_project_catalog_snapshot(&replay)
            .unwrap();
        assert_eq!(
            repositories.global().status().unwrap().store_revision,
            revision
        );
        assert_eq!(
            repositories
                .global()
                .latest_project_catalog_snapshot()
                .unwrap()
                .unwrap()
                .captured_at,
            snapshot.captured_at
        );

        let mut conflict = snapshot;
        conflict.content_fingerprint = Sha256Hash::digest(b"conflicting-catalog-content");
        assert_eq!(
            repositories
                .global()
                .put_project_catalog_snapshot(&conflict)
                .unwrap_err()
                .category,
            RepositoryErrorCategory::IntegrityFailed
        );
    }

    #[test]
    fn managed_registry_projection_is_immutable_idempotent_and_survives_restart() {
        let root = std::env::temp_dir().join(format!(
            "star-managed-registry-state-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let repositories = SqliteManagementRepositorySet::open(&root, "test").unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let registered_project = project(project_id.clone(), checkout_id.clone());
        let registered_checkout = checkout(
            project_id.clone(),
            checkout_id.clone(),
            RootBindingId::new(),
        );
        let registration_fingerprint =
            versioned_fingerprint("star.test.registry-registration", 1, &registered_project)
                .unwrap();
        repositories
            .global()
            .register_project(
                &registered_project,
                &registered_checkout,
                "register-managed-registry",
                &registration_fingerprint,
            )
            .unwrap();
        let repository = repositories.project_repository(&project_id).unwrap();

        let mut snapshot = ManagedRegistrySnapshot {
            schema_id: star_contracts::managed_registry::MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
                .to_owned(),
            schema_version: 2,
            managed_registry_snapshot_id: star_contracts::ManagedRegistrySnapshotId::new(),
            registry_id: "star-control.registry".to_owned(),
            registry_version: "1.0.0".to_owned(),
            owner_project_id: project_id.clone(),
            checkout_id,
            project_revision_id: ProjectRevisionId::new(),
            workspace_snapshot_id: WorkspaceSnapshotId::new(),
            git_revision: "a".repeat(40),
            manifest_sha256: Sha256Hash::digest(b"manifest"),
            manifest_source_refs: vec![star_contracts::managed_registry::RegistrySourceRef {
                path: ProjectPathRef::parse(".star-control/registry/manifest.toml".to_owned())
                    .unwrap(),
                source_sha256: Sha256Hash::digest(b"manifest-source"),
            }],
            namespace_claims: Vec::new(),
            declarations: Vec::new(),
            binding_observations: Vec::new(),
            consumers: Vec::new(),
            candidates: Vec::new(),
            local_constants: Vec::new(),
            code_index_snapshot_id: None,
            tombstones: Vec::new(),
            tombstone_set_fingerprint: Sha256Hash::digest(b"tombstones"),
            resolution_state: star_contracts::managed_registry::RegistryResolutionState::Valid,
            freshness: star_contracts::managed_registry::RegistryFreshness::Current,
            completeness: star_contracts::managed_registry::EvidenceCompleteness::Complete,
            limitations: Vec::new(),
            diagnostic_refs: Vec::new(),
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        };
        snapshot = snapshot.seal().unwrap();

        let mut record = RegistryConsistencyRecord {
            schema_id: star_contracts::managed_registry::REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID
                .to_owned(),
            schema_version: 1,
            registry_consistency_record_id: star_contracts::RegistryConsistencyRecordId::new(),
            registry_snapshot_ref: snapshot.reference().unwrap(),
            declaration_ref: None,
            status: star_contracts::managed_registry::RegistryConsistencyStatus::Current,
            subject: "registry:star-control.registry".to_owned(),
            expected_value: None,
            observed_value: None,
            completeness: star_contracts::managed_registry::EvidenceCompleteness::Complete,
            evidence_refs: Vec::new(),
            remediation: "no action required".to_owned(),
            record_fingerprint: Sha256Hash::digest(b"unsealed"),
        };
        record = record.seal().unwrap();
        let records = vec![record.clone()];

        repository
            .save_managed_registry_resolution(&snapshot, &records)
            .unwrap();
        let revision = repository.status().unwrap().store_revision;
        repository
            .save_managed_registry_resolution(&snapshot, &records)
            .unwrap();
        assert_eq!(repository.status().unwrap().store_revision, revision);
        assert_eq!(
            repository
                .latest_managed_registry_snapshot()
                .unwrap()
                .unwrap(),
            snapshot
        );
        assert_eq!(
            repository
                .list_registry_consistency_records(&snapshot.managed_registry_snapshot_id)
                .unwrap(),
            records
        );
        drop(repository);
        drop(repositories);

        let reopened = SqliteManagementRepositorySet::open(&root, "test").unwrap();
        let repository = reopened.project_repository(&project_id).unwrap();
        assert_eq!(
            repository
                .get_managed_registry_snapshot(&snapshot.managed_registry_snapshot_id)
                .unwrap()
                .unwrap(),
            snapshot
        );
        assert_eq!(
            repository
                .list_registry_consistency_records(&snapshot.managed_registry_snapshot_id)
                .unwrap(),
            records
        );
        reopened.verify_all().unwrap();
    }

    #[test]
    fn development_record_is_immutable_idempotent_and_survives_restart() {
        let root = std::env::temp_dir().join(format!(
            "star-development-record-state-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let repositories = SqliteManagementRepositorySet::open(&root, "test").unwrap();
        let project_id = ProjectId::new();
        let document = serde_json::json!({
            "schema_id": "star.test-development-document",
            "schema_version": 1,
            "status": "ready"
        });
        let record = DevelopmentRecord {
            schema_version: 1,
            record_kind: "compatibility_report".to_owned(),
            record_id: "compatibility:test".to_owned(),
            revision: 1,
            project_id: Some(project_id.clone()),
            state: "ready".to_owned(),
            document_schema_id: "star.test-development-document".to_owned(),
            document_schema_version: 1,
            document_fingerprint: canonical_sha256(&document).unwrap(),
            document,
            created_at: Utc::now().to_rfc3339(),
        };

        repositories
            .global()
            .put_development_record(&record)
            .unwrap();
        let revision = repositories.global().status().unwrap().store_revision;
        repositories
            .global()
            .put_development_record(&record)
            .unwrap();
        assert_eq!(
            repositories.global().status().unwrap().store_revision,
            revision
        );
        assert_eq!(
            repositories
                .global()
                .get_development_record("compatibility_report", "compatibility:test", None)
                .unwrap(),
            Some(record.clone())
        );
        drop(repositories);

        let reopened = SqliteManagementRepositorySet::open(&root, "test").unwrap();
        assert_eq!(
            reopened
                .global()
                .list_development_records("compatibility_report", Some(&project_id))
                .unwrap(),
            vec![record]
        );
        reopened.verify_all().unwrap();
    }
}
