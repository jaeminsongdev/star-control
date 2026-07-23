//! Redacted ArtifactRef-backed `.ai-runs` evidence storage.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use star_contracts::{
    Sha256Hash,
    evidence::{ArtifactRef, ProducerRef, RedactionStatus},
    ids::{ArtifactId, ProjectId},
    management::ProjectPathRef,
    parse_no_duplicate_keys,
};
use star_domain::PersistenceRedactor;
use star_ports::{
    ArtifactDiscovery, ArtifactStore, ArtifactWriteRequest, RepositoryError,
    RepositoryErrorCategory,
};

const ARTIFACT_PREFIX: &str = ".ai-runs/star-control/";
const ARTIFACT_REF_SUFFIX: &str = ".artifact-ref.json";
const MAX_DISCOVERED_ARTIFACT_REFS: usize = 4_096;
const MAX_ARTIFACT_REF_BYTES: u64 = 256 * 1024;
const MAX_ARTIFACT_TREE_DEPTH: usize = 32;

pub struct LocalArtifactStore {
    redactor: PersistenceRedactor,
}

impl Default for LocalArtifactStore {
    fn default() -> Self {
        Self {
            redactor: PersistenceRedactor::for_current_user(),
        }
    }
}

fn error(category: RepositoryErrorCategory, message: &'static str) -> RepositoryError {
    RepositoryError::new(category, message)
}

fn validate_json_strings(
    redactor: &PersistenceRedactor,
    value: &serde_json::Value,
) -> Result<(), RepositoryError> {
    match value {
        serde_json::Value::String(value) => redactor.validate(value).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact contains a prohibited raw value",
            )
        }),
        serde_json::Value::Array(values) => {
            for value in values {
                validate_json_strings(redactor, value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                redactor.validate(key).map_err(|_| {
                    error(
                        RepositoryErrorCategory::Invalid,
                        "artifact key contains a prohibited raw value",
                    )
                })?;
                validate_json_strings(redactor, value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn artifact_identity(
    project_id: &ProjectId,
    relative_path: &str,
    sha256: &Sha256Hash,
) -> ArtifactId {
    ArtifactId::from_stable_bytes(
        format!(
            "{}\n{}\n{}",
            project_id.as_str(),
            relative_path,
            sha256.as_str()
        )
        .as_bytes(),
    )
}

fn artifact_path(project_root: &Path, relative_path: &str) -> Result<PathBuf, RepositoryError> {
    let relative = ProjectPathRef::parse(relative_path).map_err(|_| {
        error(
            RepositoryErrorCategory::Invalid,
            "artifact path is not project-relative",
        )
    })?;
    if !relative.as_str().starts_with(ARTIFACT_PREFIX)
        || relative.as_str().ends_with(ARTIFACT_REF_SUFFIX)
    {
        return Err(error(
            RepositoryErrorCategory::Invalid,
            "artifact path is outside the Star-Control artifact namespace",
        ));
    }
    Ok(relative
        .as_str()
        .split('/')
        .fold(project_root.to_path_buf(), |path, segment| {
            path.join(segment)
        }))
}

fn artifact_ref_path(artifact: &Path) -> Result<PathBuf, RepositoryError> {
    let name = artifact
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact filename is invalid",
            )
        })?;
    Ok(artifact.with_file_name(format!("{name}{ARTIFACT_REF_SUFFIX}")))
}

fn validate_existing_file(project_root: &Path, path: &Path) -> Result<(), RepositoryError> {
    let canonical_root = project_root.canonicalize().map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "project root is unavailable",
        )
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact byte is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(error(
            RepositoryErrorCategory::IntegrityFailed,
            "artifact path is not a regular file",
        ));
    }
    let canonical = path.canonicalize().map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact byte is unavailable",
        )
    })?;
    if !canonical.starts_with(canonical_root) {
        return Err(error(
            RepositoryErrorCategory::IntegrityFailed,
            "artifact path escapes the project root",
        ));
    }
    Ok(())
}

fn ensure_artifact_parent(project_root: &Path, relative_path: &str) -> Result<(), RepositoryError> {
    let canonical_root = project_root.canonicalize().map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "project root is unavailable",
        )
    })?;
    let mut current = canonical_root.clone();
    let mut segments = relative_path.split('/').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            break;
        }
        current.push(segment);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(io_error) if io_error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => {
                        return Err(error(
                            RepositoryErrorCategory::Unavailable,
                            "artifact parent directory creation failed",
                        ));
                    }
                }
                fs::symlink_metadata(&current).map_err(|_| {
                    error(
                        RepositoryErrorCategory::Unavailable,
                        "artifact parent directory is unavailable",
                    )
                })?
            }
            Err(_) => {
                return Err(error(
                    RepositoryErrorCategory::Unavailable,
                    "artifact parent directory is unavailable",
                ));
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact parent is not a regular directory",
            ));
        }
    }
    let canonical_parent = current.canonicalize().map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact parent directory is unavailable",
        )
    })?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err(error(
            RepositoryErrorCategory::IntegrityFailed,
            "artifact parent escapes the project root",
        ));
    }
    Ok(())
}

fn write_immutable(path: &Path, bytes: &[u8]) -> Result<(), RepositoryError> {
    if path.exists() {
        let existing = fs::read(path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "existing immutable artifact metadata cannot be read",
            )
        })?;
        if existing != bytes {
            return Err(error(
                RepositoryErrorCategory::RevisionConflict,
                "immutable artifact metadata already contains different bytes",
            ));
        }
        return Ok(());
    }
    let temporary = path.with_extension(format!(
        "tmp-{}",
        ArtifactId::new().as_str().trim_start_matches("art_")
    ));
    fs::write(&temporary, bytes).map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact temporary write failed",
        )
    })?;
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact temporary file cannot be opened",
            )
        })?;
    file.sync_all().map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact flush failed",
        )
    })?;
    fs::rename(&temporary, path).map_err(|_| {
        error(
            RepositoryErrorCategory::Unavailable,
            "artifact finalize failed",
        )
    })
}

fn collect_artifact_ref_paths(
    directory: &Path,
    depth: usize,
    paths: &mut Vec<PathBuf>,
) -> Result<(), RepositoryError> {
    if depth > MAX_ARTIFACT_TREE_DEPTH {
        return Err(error(
            RepositoryErrorCategory::QuotaExceeded,
            "artifact tree depth exceeds the recovery limit",
        ));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact directory cannot be enumerated",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact directory entry cannot be read",
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type().map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact directory entry type cannot be read",
            )
        })?;
        if file_type.is_symlink() {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact tree contains a symbolic link",
            ));
        }
        if file_type.is_dir() {
            collect_artifact_ref_paths(&entry.path(), depth + 1, paths)?;
        } else if file_type.is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(ARTIFACT_REF_SUFFIX))
        {
            paths.push(entry.path());
            if paths.len() > MAX_DISCOVERED_ARTIFACT_REFS {
                return Err(error(
                    RepositoryErrorCategory::QuotaExceeded,
                    "artifact reference count exceeds the recovery limit",
                ));
            }
        }
    }
    Ok(())
}

impl LocalArtifactStore {
    fn read_verified_artifact_ref(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        sidecar_path: &Path,
    ) -> Result<ArtifactRef, RepositoryError> {
        validate_existing_file(project_root, sidecar_path)?;
        let metadata = fs::metadata(sidecar_path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact reference metadata is unavailable",
            )
        })?;
        if metadata.len() > MAX_ARTIFACT_REF_BYTES {
            return Err(error(
                RepositoryErrorCategory::QuotaExceeded,
                "artifact reference exceeds its read limit",
            ));
        }
        let bytes = fs::read(sidecar_path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact reference cannot be read",
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference is not UTF-8",
            )
        })?;
        let value = parse_no_duplicate_keys(text).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference JSON is invalid",
            )
        })?;
        validate_json_strings(&self.redactor, &value)?;
        let artifact: ArtifactRef = serde_json::from_value(value).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference shape is invalid",
            )
        })?;
        artifact.validate().map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference invariant is invalid",
            )
        })?;
        if artifact.project_id.as_ref() != Some(project_id)
            || artifact.media_type != "application/json"
            || artifact.redaction_status == RedactionStatus::Unknown
            || artifact.source_artifact_ref.is_some()
            || artifact.artifact_id
                != artifact_identity(project_id, &artifact.relative_path, &artifact.sha256)
        {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference identity is invalid",
            ));
        }
        let artifact_path = artifact_path(project_root, &artifact.relative_path)?;
        if artifact_ref_path(&artifact_path)? != sidecar_path {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact reference is not bound to its sidecar path",
            ));
        }
        self.verify(project_root, &artifact)?;
        let artifact_bytes = fs::read(&artifact_path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact byte is unavailable",
            )
        })?;
        let artifact_text = std::str::from_utf8(&artifact_bytes).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact JSON is not UTF-8",
            )
        })?;
        let artifact_value = parse_no_duplicate_keys(artifact_text).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact JSON is invalid",
            )
        })?;
        validate_json_strings(&self.redactor, &artifact_value)?;
        Ok(artifact)
    }
}

impl ArtifactStore for LocalArtifactStore {
    fn put_json_with_policy(
        &self,
        request: ArtifactWriteRequest<'_>,
    ) -> Result<ArtifactRef, RepositoryError> {
        let ArtifactWriteRequest {
            project_id,
            project_root,
            relative_path,
            subject_kind,
            subject_id,
            policy,
            value,
        } = request;
        if matches!(policy.redaction_status, RedactionStatus::Unknown) {
            return Err(error(
                RepositoryErrorCategory::Invalid,
                "artifact redaction status must be resolved before persistence",
            ));
        }
        validate_json_strings(&self.redactor, value)?;
        self.redactor.validate(subject_kind).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact subject kind is prohibited",
            )
        })?;
        self.redactor.validate(subject_id).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact subject ID is prohibited",
            )
        })?;
        let relative = ProjectPathRef::parse(format!(
            ".ai-runs/star-control/{}",
            relative_path.trim_start_matches('/')
        ))
        .map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact path is not project-relative",
            )
        })?;
        let destination = artifact_path(project_root, relative.as_str())?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact serialization failed",
            )
        })?;
        let sha256 = Sha256Hash::digest(&bytes);
        ensure_artifact_parent(project_root, relative.as_str())?;
        if destination.exists() {
            validate_existing_file(project_root, &destination)?;
            let existing = fs::read(&destination).map_err(|_| {
                error(
                    RepositoryErrorCategory::Unavailable,
                    "existing artifact cannot be read",
                )
            })?;
            if Sha256Hash::digest(&existing) != sha256 {
                return Err(error(
                    RepositoryErrorCategory::RevisionConflict,
                    "immutable artifact path already contains different bytes",
                ));
            }
        } else {
            write_immutable(&destination, &bytes)?;
        }
        let sidecar = artifact_ref_path(&destination)?;
        if sidecar.exists() {
            return self.read_verified_artifact_ref(project_id, project_root, &sidecar);
        }
        let artifact = ArtifactRef {
            artifact_id: artifact_identity(project_id, relative.as_str(), &sha256),
            kind: policy.kind,
            project_id: Some(project_id.clone()),
            relative_path: relative.as_str().to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: bytes.len() as u64,
            sha256,
            created_at: Utc::now(),
            producer: ProducerRef {
                component: "star-evidence".to_owned(),
                product_version: env!("CARGO_PKG_VERSION").to_owned(),
                build_id: option_env!("STAR_CONTROL_BUILD_ID")
                    .unwrap_or(env!("CARGO_PKG_VERSION"))
                    .to_owned(),
                platform: std::env::consts::OS.to_owned(),
            },
            redaction_status: policy.redaction_status,
            retention_class: policy.retention_class,
            source_artifact_ref: None,
        };
        let sidecar_bytes = serde_json::to_vec_pretty(&artifact).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact reference serialization failed",
            )
        })?;
        write_immutable(&sidecar, &sidecar_bytes)?;
        self.read_verified_artifact_ref(project_id, project_root, &sidecar)
    }

    fn verify(&self, project_root: &Path, artifact: &ArtifactRef) -> Result<(), RepositoryError> {
        artifact.validate().map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact reference invariant is invalid",
            )
        })?;
        let path = artifact_path(project_root, &artifact.relative_path)?;
        validate_existing_file(project_root, &path)?;
        let bytes = fs::read(path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact byte is unavailable",
            )
        })?;
        if bytes.len() as u64 != artifact.size_bytes
            || Sha256Hash::digest(&bytes) != artifact.sha256
        {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact hash or size does not match ArtifactRef",
            ));
        }
        Ok(())
    }

    fn read_json(
        &self,
        project_root: &Path,
        artifact: &ArtifactRef,
    ) -> Result<serde_json::Value, RepositoryError> {
        self.verify(project_root, artifact)?;
        let path = artifact_path(project_root, &artifact.relative_path)?;
        let bytes = fs::read(path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact byte is unavailable",
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact JSON is invalid",
            )
        })?;
        parse_no_duplicate_keys(text).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact JSON is invalid",
            )
        })
    }

    fn discover_verified(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
    ) -> Result<ArtifactDiscovery, RepositoryError> {
        let artifact_root = project_root.join(".ai-runs").join("star-control");
        if !artifact_root.exists() {
            return Ok(ArtifactDiscovery {
                verified: Vec::new(),
                rejected_count: 0,
            });
        }
        let metadata = fs::symlink_metadata(&artifact_root).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact root metadata is unavailable",
            )
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact root is not a regular directory",
            ));
        }
        let canonical_project = project_root.canonicalize().map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "project root is unavailable",
            )
        })?;
        let canonical_artifact_root = artifact_root.canonicalize().map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact root is unavailable",
            )
        })?;
        if !canonical_artifact_root.starts_with(canonical_project) {
            return Err(error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact root escapes the project root",
            ));
        }

        let mut sidecars = Vec::new();
        collect_artifact_ref_paths(&artifact_root, 0, &mut sidecars)?;
        sidecars.sort();
        let mut verified = BTreeMap::new();
        let mut rejected_count = 0_u64;
        for sidecar in sidecars {
            match self.read_verified_artifact_ref(project_id, project_root, &sidecar) {
                Ok(artifact) => {
                    let key = artifact.artifact_id.as_str().to_owned();
                    if let Some(existing) = verified.get(&key)
                        && existing != &artifact
                    {
                        rejected_count = rejected_count.saturating_add(1);
                        continue;
                    }
                    verified.insert(key, artifact);
                }
                Err(_) => rejected_count = rejected_count.saturating_add(1),
            }
        }
        Ok(ArtifactDiscovery {
            verified: verified.into_values().collect(),
            rejected_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::evidence::{ArtifactKind, RedactionStatus, RetentionClass};
    use star_ports::{ArtifactWritePolicy, ArtifactWriteRequest};

    #[test]
    fn explicit_artifact_policy_preserves_kind_redaction_and_retention() {
        let root = std::env::temp_dir().join(format!(
            "star-evidence-policy-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(&root).unwrap();
        let store = LocalArtifactStore::default();
        let project_id = ProjectId::new();
        let artifact = store
            .put_json_with_policy(ArtifactWriteRequest {
                project_id: &project_id,
                project_root: &root,
                relative_path: "validation/output.json",
                subject_kind: "validation_process_output",
                subject_id: "safe-output",
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Log,
                    redaction_status: RedactionStatus::Redacted,
                    retention_class: RetentionClass::Run,
                },
                value: &serde_json::json!({"content_status":"redacted"}),
            })
            .unwrap();
        assert_eq!(artifact.kind, ArtifactKind::Log);
        assert_eq!(artifact.redaction_status, RedactionStatus::Redacted);
        assert_eq!(artifact.retention_class, RetentionClass::Run);
        assert!(
            store
                .put_json_with_policy(ArtifactWriteRequest {
                    project_id: &project_id,
                    project_root: &root,
                    relative_path: "validation/unknown.json",
                    subject_kind: "validation_process_output",
                    subject_id: "unknown-output",
                    policy: ArtifactWritePolicy {
                        kind: ArtifactKind::Log,
                        redaction_status: RedactionStatus::Unknown,
                        retention_class: RetentionClass::Run,
                    },
                    value: &serde_json::json!({"content_status":"unknown"}),
                })
                .is_err()
        );
    }

    #[test]
    fn artifact_is_relative_immutable_hashed_and_redacted_before_write() {
        let root = std::env::temp_dir().join(format!(
            "star-evidence-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(&root).unwrap();
        let store = LocalArtifactStore::default();
        let project_id = ProjectId::new();
        let artifact = store
            .put_json(
                &project_id,
                &root,
                "management/scans/scn/report.json",
                "scan",
                "scn_safe",
                &serde_json::json!({"count":1,"message_code":"SCAN_OK"}),
            )
            .unwrap();
        store.verify(&root, &artifact).unwrap();
        assert!(artifact.relative_path.starts_with(".ai-runs/"));
        let replayed = store
            .put_json(
                &project_id,
                &root,
                "management/scans/scn/report.json",
                "scan",
                "scn_safe",
                &serde_json::json!({"count":1,"message_code":"SCAN_OK"}),
            )
            .unwrap();
        assert_eq!(replayed, artifact);
        let discovered = store.discover_verified(&project_id, &root).unwrap();
        assert_eq!(discovered.verified, vec![artifact.clone()]);
        assert_eq!(discovered.rejected_count, 0);
        let artifact_path = artifact_path(&root, &artifact.relative_path).unwrap();
        assert!(artifact_ref_path(&artifact_path).unwrap().is_file());
        assert!(
            store
                .put_json(
                    &project_id,
                    &root,
                    "management/scans/scn/secret.json",
                    "scan",
                    "scn_safe",
                    &serde_json::json!({"value":"token=do-not-store"}),
                )
                .is_err()
        );
        assert!(
            !root
                .join(".ai-runs/star-control/management/scans/scn/secret.json")
                .exists()
        );

        fs::write(&artifact_path, b"tampered").unwrap();
        let discovered = store.discover_verified(&project_id, &root).unwrap();
        assert!(discovered.verified.is_empty());
        assert_eq!(discovered.rejected_count, 1);
    }
}
