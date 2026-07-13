//! Redacted ArtifactRef-backed `.ai-runs` evidence storage.

use std::{fs, path::Path};

use chrono::Utc;
use star_contracts::{
    Sha256Hash,
    evidence::{ArtifactKind, ArtifactRef, ProducerRef, RedactionStatus, RetentionClass},
    ids::{ArtifactId, ProjectId},
    management::ProjectPathRef,
};
use star_domain::PersistenceRedactor;
use star_ports::{ArtifactStore, RepositoryError, RepositoryErrorCategory};

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

impl ArtifactStore for LocalArtifactStore {
    fn put_json(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        relative_path: &str,
        subject_kind: &str,
        subject_id: &str,
        value: &serde_json::Value,
    ) -> Result<ArtifactRef, RepositoryError> {
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
        let destination = relative
            .as_str()
            .split('/')
            .fold(project_root.to_path_buf(), |path, segment| {
                path.join(segment)
            });
        let bytes = serde_json::to_vec_pretty(value).map_err(|_| {
            error(
                RepositoryErrorCategory::Invalid,
                "artifact serialization failed",
            )
        })?;
        let sha256 = Sha256Hash::digest(&bytes);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                error(
                    RepositoryErrorCategory::Unavailable,
                    "artifact directory creation failed",
                )
            })?;
        }
        if destination.exists() {
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
            let temporary = destination.with_extension(format!(
                "tmp-{}",
                ArtifactId::new().as_str().trim_start_matches("art_")
            ));
            fs::write(&temporary, &bytes).map_err(|_| {
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
            fs::rename(&temporary, &destination).map_err(|_| {
                error(
                    RepositoryErrorCategory::Unavailable,
                    "artifact finalize failed",
                )
            })?;
        }
        let identity = format!(
            "{}\n{}\n{}",
            project_id.as_str(),
            relative.as_str(),
            sha256.as_str()
        );
        Ok(ArtifactRef {
            artifact_id: ArtifactId::from_stable_bytes(identity.as_bytes()),
            kind: ArtifactKind::Report,
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
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        })
    }

    fn verify(&self, project_root: &Path, artifact: &ArtifactRef) -> Result<(), RepositoryError> {
        let path = artifact
            .relative_path
            .split('/')
            .fold(project_root.to_path_buf(), |path, segment| {
                path.join(segment)
            });
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
        let path = artifact
            .relative_path
            .split('/')
            .fold(project_root.to_path_buf(), |path, segment| {
                path.join(segment)
            });
        let bytes = fs::read(path).map_err(|_| {
            error(
                RepositoryErrorCategory::Unavailable,
                "artifact byte is unavailable",
            )
        })?;
        serde_json::from_slice(&bytes).map_err(|_| {
            error(
                RepositoryErrorCategory::IntegrityFailed,
                "artifact JSON is invalid",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
