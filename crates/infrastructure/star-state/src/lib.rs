//! Private embedded-relational repository and Windows root-binding adapters.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
    backup::Backup, limits::Limit, params,
};
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{ArtifactRef, GateDecision, GateScope},
    ids::{
        CoordinatedOperationId, EventId, FindingId, ManagementStoreId, PatchSetId, ProjectId,
        RootBindingId, ScanRunId, WorkspaceSnapshotId,
    },
    management::{
        Baseline, CanonicalSource, ChangePlan, CoordinatedOperation, Disposition, Finding,
        IntegrityState, MANAGEMENT_STORE_VERSION, ManagementStoreStatus, Occurrence,
        ParticipantReceipt, PatchSet, Project, ProjectRevision, REDACTION_CONTRACT_VERSION,
        ScanRun, StoreOpenMode, StoreScope, Suppression, Symbol, SymbolReference, ValidationResult,
        WorkspaceSnapshot,
    },
};
use star_domain::{
    PersistenceRedactor, validate_baseline, validate_coordination, validate_suppression,
    versioned_fingerprint,
};
use star_ports::{
    GlobalManagementRepository, ManagementRepositorySet, ProjectManagementRepository,
    ProjectRootAttachment, ProjectRootBindingStore, RepositoryError, RepositoryErrorCategory,
    RetentionApplyResult, RetentionCandidate, RetentionPlan, ScanCommit,
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
        Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    },
    core::{HSTRING, PCWSTR, w},
};

const STORE_FILENAME: &str = "management.v1.db";
const APPLICATION_ID: i32 = 0x5354_4152;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryInspection {
    Healthy,
    MigrationRequired,
    FutureVersion,
    Corrupt,
}

pub fn inspect_store_read_only(path: &Path) -> RecoveryInspection {
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
    match connection.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)) {
        Ok(result) if result == "ok" => RecoveryInspection::Healthy,
        _ => RecoveryInspection::Corrupt,
    }
}

pub fn inspect_management_root(root: &Path) -> Option<RecoveryInspection> {
    let global = root.join("global").join("active").join(STORE_FILENAME);
    global.exists().then(|| inspect_store_read_only(&global))
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

pub struct SqliteManagementRepositorySet {
    root: PathBuf,
    product_version: String,
    _lease: WriterLease,
    global: SqliteGlobalRepository,
    projects: Mutex<BTreeMap<ProjectId, Weak<SqliteProjectRepository>>>,
}

#[derive(Serialize)]
struct ActiveSetEntry {
    scope: StoreScope,
    store_id: ManagementStoreId,
    generation: u64,
    management_store_version: u32,
    relative_locator: String,
    header_fingerprint: Sha256Hash,
}

#[derive(Serialize)]
struct ActiveSetManifest {
    schema_version: u32,
    entries: Vec<ActiveSetEntry>,
    manifest_fingerprint: Sha256Hash,
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
        let global_path = root.join("global").join("active").join(STORE_FILENAME);
        let global = SqliteGlobalRepository::open(&global_path, &product_version)?;
        let repositories = Self {
            root,
            product_version,
            _lease: lease,
            global,
            projects: Mutex::new(BTreeMap::new()),
        };
        repositories.refresh_active_set()?;
        Ok(repositories)
    }

    fn project_path(&self, project_id: &ProjectId) -> PathBuf {
        self.root
            .join("projects")
            .join(project_id.as_str())
            .join("active")
            .join(STORE_FILENAME)
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
            &self.project_path(project_id),
            project_id,
            &self.product_version,
        )?);
        projects.insert(project_id.clone(), Arc::downgrade(&repository));
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
        for status in statuses {
            let relative_locator = match &status.store_scope {
                StoreScope::Global => "global/active".to_owned(),
                StoreScope::Project { project_id } => {
                    format!("projects/{}/active", project_id.as_str())
                }
            };
            let header_fingerprint = versioned_fingerprint(
                "star.management-store-generation-header",
                1,
                &serde_json::json!({
                    "scope":status.store_scope,
                    "store_id":status.store_id,
                    "generation":status.generation,
                    "management_store_version":status.management_store_version,
                    "relative_locator":relative_locator,
                }),
            )
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "active store header fingerprint failed",
                )
            })?;
            entries.push(ActiveSetEntry {
                scope: status.store_scope.clone(),
                store_id: status.store_id.clone(),
                generation: status.generation,
                management_store_version: status.management_store_version,
                relative_locator,
                header_fingerprint,
            });
        }
        entries.sort_by(|left, right| left.relative_locator.cmp(&right.relative_locator));
        let manifest_fingerprint = versioned_fingerprint("star.management-active-set", 1, &entries)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "active set fingerprint failed",
                )
            })?;
        let bytes = serde_json::to_vec_pretty(&ActiveSetManifest {
            schema_version: 1,
            entries,
            manifest_fingerprint,
        })
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Invalid,
                "active set serialization failed",
            )
        })?;
        write_private_atomic(&self.root.join("active-set.json"), &bytes)
    }
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

    fn backup_all(
        &self,
        destination: &Path,
    ) -> Result<Vec<ManagementStoreStatus>, RepositoryError> {
        create_private_dir(destination)?;
        let mut statuses = Vec::new();
        let mut entries = Vec::new();
        let global_destination = destination.join("global.db");
        self.global.backup(&global_destination)?;
        let global_status = self.global.status()?;
        let global_bytes = fs::metadata(&global_destination).map_err(map_io)?.len();
        let global_hash =
            Sha256Hash::digest_reader(fs::File::open(&global_destination).map_err(map_io)?)
                .map_err(map_io)?;
        entries.push(serde_json::json!({
            "relative_path":"global.db",
            "status":global_status,
            "size_bytes":global_bytes,
            "byte_sha256":global_hash,
        }));
        statuses.push(global_status);
        let project_destination = destination.join("projects");
        create_private_dir(&project_destination)?;
        for project in self.global.list_projects()? {
            let repository = self.project_repository(&project.project_id)?;
            let relative_path = format!("projects/{}.db", project.project_id.as_str());
            let project_backup = destination.join(&relative_path);
            repository.backup(&project_backup)?;
            let status = repository.status()?;
            let size_bytes = fs::metadata(&project_backup).map_err(map_io)?.len();
            let byte_sha256 =
                Sha256Hash::digest_reader(fs::File::open(&project_backup).map_err(map_io)?)
                    .map_err(map_io)?;
            entries.push(serde_json::json!({
                "relative_path":relative_path,
                "status":status,
                "size_bytes":size_bytes,
                "byte_sha256":byte_sha256,
            }));
            statuses.push(status);
        }
        let set_fingerprint = versioned_fingerprint("star.management-backup-set", 1, &entries)
            .map_err(|_| {
                repository_error(
                    RepositoryErrorCategory::Invalid,
                    "backup set fingerprint failed",
                )
            })?;
        let manifest = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "created_at": Utc::now(),
            "entries": entries,
            "set_fingerprint":set_fingerprint,
        }))
        .map_err(|_| {
            repository_error(
                RepositoryErrorCategory::Unavailable,
                "backup manifest serialization failed",
            )
        })?;
        write_private_atomic(&destination.join("backup-set.json"), &manifest)?;
        Ok(statuses)
    }

    fn plan_retention(&self) -> Result<RetentionPlan, RepositoryError> {
        let created_at = Utc::now();
        let cutoff = created_at - chrono::Duration::days(7);
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
            candidates.extend(
                repository.retention_candidates(cutoff, created_at - chrono::Duration::days(90))?,
            );
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
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Project, RepositoryError> {
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
        let identity_scope = serialized_enum_label(&project.identity_scope)?;
        transaction
            .execute(
                "INSERT INTO projects(project_id, identity_scope, root_binding_id, document_json, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(project_id) DO UPDATE SET
                    identity_scope=excluded.identity_scope,
                    root_binding_id=excluded.root_binding_id,
                    document_json=excluded.document_json,
                    updated_at=excluded.updated_at",
                params![
                    project.project_id.as_str(),
                    identity_scope,
                    project.root_binding_id.as_ref().map(|id| id.as_str()),
                    document,
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

    fn retention_candidates(
        &self,
        incomplete_cutoff: DateTime<Utc>,
        scan_detail_cutoff: DateTime<Utc>,
    ) -> Result<Vec<RetentionCandidate>, RepositoryError> {
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
            if index >= 2
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
}

impl Drop for SqliteProjectRepository {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.lock() {
            let _ = set_meta(&connection, "last_clean_shutdown", "true");
        }
    }
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
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, 16 * 1024 * 1024)
        .map_err(map_sql)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024)
        .map_err(map_sql)?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA trusted_schema=OFF;
             PRAGMA temp_store=MEMORY;",
        )
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

fn verify_connection(connection: &Connection) -> Result<(), RepositoryError> {
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

fn protected_snapshot_ids(
    connection: &Connection,
) -> Result<std::collections::BTreeSet<String>, RepositoryError> {
    let mut protected = std::collections::BTreeSet::new();
    let mut baselines: Vec<Baseline> = query_documents(
        connection,
        "SELECT document_json FROM baselines UNION ALL SELECT document_json FROM shared_baselines",
        [],
    )?;
    for baseline in baselines.drain(..) {
        protected.insert(baseline.workspace_snapshot_id.as_str().to_owned());
    }
    for plan in
        query_documents::<ChangePlan, _>(connection, "SELECT document_json FROM change_plans", [])?
    {
        protected.insert(plan.target_workspace_snapshot_id.as_str().to_owned());
    }
    for patch in
        query_documents::<PatchSet, _>(connection, "SELECT document_json FROM patch_sets", [])?
    {
        protected.insert(patch.base_workspace_snapshot_id.as_str().to_owned());
        if let Some(snapshot_id) = patch.applied_workspace_snapshot_id {
            protected.insert(snapshot_id.as_str().to_owned());
        }
    }
    for result in query_documents::<ValidationResult, _>(
        connection,
        "SELECT document_json FROM validation_results",
        [],
    )? {
        protected.insert(result.workspace_snapshot_id.as_str().to_owned());
    }
    for decision in query_documents::<GateDecision, _>(
        connection,
        "SELECT document_json FROM gate_decisions",
        [],
    )? {
        protected.insert(gate_workspace_snapshot_id(&decision)?.as_str().to_owned());
    }
    Ok(protected)
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
    }) {
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
        if envelope.schema_version != 1 || envelope.protection_kind != "windows_current_user" {
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
        attachments.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        if attachments
            .windows(2)
            .any(|pair| pair[0].project_id == pair[1].project_id)
        {
            return Err(repository_error(
                RepositoryErrorCategory::Corrupt,
                "a ProjectId has multiple active root bindings",
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
                        RepositoryErrorCategory::Corrupt,
                        "a ProjectId has multiple active root bindings",
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
        root: &Path,
    ) -> Result<RootBindingId, RepositoryError> {
        let canonical = canonical_project_root(root)?;
        if let Some(existing) = self.find_by_root(&canonical)? {
            if existing.project_id == *project_id {
                return Ok(existing.root_binding_id);
            }
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "project root is already attached to another ProjectId",
            ));
        }
        if self.find_by_project(project_id)?.is_some() {
            return Err(repository_error(
                RepositoryErrorCategory::Invalid,
                "ProjectId is already attached to another root",
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
            schema_version: 1,
            root_binding_id: binding_id.clone(),
            project_id: project_id.clone(),
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
            "current-user root protection failed",
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
            "current-user root unprotection failed",
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
        management::{IdentityScope, RegistrationState, RepositoryKind, ScanStatus},
    };

    fn project(project_id: ProjectId, binding_id: RootBindingId) -> Project {
        Project {
            schema_id: "star.project".to_owned(),
            schema_version: 1,
            project_id,
            identity_scope: IdentityScope::Local,
            display_name: "local-project".to_owned(),
            repository_kind: RepositoryKind::None,
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: Sha256Hash::digest(b"project"),
            registration_state: RegistrationState::Attached,
            root_binding_id: Some(binding_id),
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        }
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
        let binding_id = bindings.attach(&project_id, &source).unwrap();
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
        let project = project(project_id.clone(), binding_id);
        let fingerprint =
            star_domain::versioned_fingerprint("star.project-register", 1, &project).unwrap();
        repositories
            .global()
            .register_project(&project, "register-1", &fingerprint)
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
            repositories.backup_all(&root.join("backup")).unwrap().len(),
            2
        );
        let global_backup = root.join("backup/global.db");
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
            transaction.commit().unwrap();
        }
        let plan = repositories.plan_retention().unwrap();
        assert_eq!(plan.candidates.len(), 1);
        let applied = repositories
            .apply_retention(&plan, plan.plan_fingerprint.as_str())
            .unwrap();
        assert_eq!(applied.applied_count, 1);
        assert!(repositories.plan_retention().unwrap().candidates.is_empty());

        let database = fs::read(
            root.join("management")
                .join("global")
                .join("active")
                .join(STORE_FILENAME),
        )
        .unwrap();
        assert!(
            !String::from_utf8_lossy(&database).contains(&source.to_string_lossy().to_string())
        );
    }
}
