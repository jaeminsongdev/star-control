use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{ArtifactKind, StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "1.0.0";
const RELEASE_READINESS_SCHEMA: &str = "release-readiness.schema.json";
pub const RELEASE_READINESS_PATH: &str = "release/release-readiness.json";

#[derive(Debug)]
pub enum ReleaseReadinessError {
    State {
        source: StateStoreError,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
    InvalidReleaseReadiness {
        message: String,
    },
    WriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for ReleaseReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State { source } => write!(formatter, "state store error: {}", source),
            Self::SchemaLoadFailed { path, message } => write!(
                formatter,
                "release readiness schema load failed at {}: {}",
                path.display(),
                message
            ),
            Self::SchemaValidationFailed { path, errors } => write!(
                formatter,
                "release readiness schema validation failed for {} with {} error(s)",
                path.display(),
                errors.len()
            ),
            Self::InvalidReleaseReadiness { message } => {
                write!(formatter, "invalid release readiness: {}", message)
            }
            Self::WriteFailed { path, source } => write!(
                formatter,
                "failed to write release readiness artifact {}: {}",
                path.display(),
                source
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read release readiness artifact {}: {}",
                path.display(),
                source
            ),
            Self::InvalidJson { path, source } => write!(
                formatter,
                "invalid release readiness JSON at {}: {}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for ReleaseReadinessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State { source } => Some(source),
            Self::WriteFailed { source, .. } => Some(source),
            Self::ReadFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ReleaseReadinessError {
    fn from(source: StateStoreError) -> Self {
        Self::State { source }
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseReadinessWriter {
    schema_root: PathBuf,
}

impl ReleaseReadinessWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn check(
        &self,
        name: impl Into<String>,
        status: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Value {
        json!({
            "name": name.into(),
            "status": status.into(),
            "evidence_paths": evidence_paths
        })
    }

    pub fn reserved(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
    ) -> Value {
        self.readiness(
            release_id,
            target,
            version,
            "reserved",
            vec![
                self.check("required-ci-passed", "reserved", Vec::new()),
                self.check("release-profile-passed", "reserved", Vec::new()),
                self.check("changelog-updated", "reserved", Vec::new()),
                self.check("version-consistent", "reserved", Vec::new()),
                self.check("artifact-signing-ready", "reserved", Vec::new()),
                self.check("rollback-plan-ready", "reserved", Vec::new()),
                self.check("package-publishing-approved", "reserved", Vec::new()),
            ],
            vec!["release automation is not implemented yet".to_string()],
        )
    }

    pub fn not_ready(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        checks: Vec<Value>,
        blockers: Vec<String>,
    ) -> Value {
        self.readiness(release_id, target, version, "not_ready", checks, blockers)
    }

    pub fn readiness(
        &self,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        status: impl Into<String>,
        checks: Vec<Value>,
        blockers: Vec<String>,
    ) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "release_id": release_id.into(),
            "target": target.into(),
            "version": version.into(),
            "status": status.into(),
            "checks": checks,
            "blockers": blockers,
            "approvals": [],
            "generated_at": timestamp_string()
        })
    }

    pub fn write(
        &self,
        store: &StateStore,
        job_id: &str,
        readiness: &Value,
    ) -> Result<Value, ReleaseReadinessError> {
        self.validate_readiness(readiness)?;
        let path = store.resolve_job_path(job_id, RELEASE_READINESS_PATH)?;
        write_new_json(&path, readiness)?;
        store
            .artifact_ref(
                job_id,
                RELEASE_READINESS_PATH,
                ArtifactKind::Other,
                "star-control-release",
                Some("specs/schemas/release-readiness.schema.json"),
                Some("release readiness artifact"),
            )
            .map_err(ReleaseReadinessError::from)
    }

    pub fn read(
        &self,
        store: &StateStore,
        job_id: &str,
    ) -> Result<Option<Value>, ReleaseReadinessError> {
        let path = store.resolve_job_path(job_id, RELEASE_READINESS_PATH)?;
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).map_err(|source| ReleaseReadinessError::ReadFailed {
                path: path.clone(),
                source,
            })?;
        let value: Value = serde_json::from_str(&content).map_err(|source| {
            ReleaseReadinessError::InvalidJson {
                path: path.clone(),
                source,
            }
        })?;
        self.validate_readiness(&value)?;
        Ok(Some(value))
    }

    pub fn validate_readiness(&self, readiness: &Value) -> Result<(), ReleaseReadinessError> {
        self.validate_schema(readiness)?;
        let status = readiness
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| ReleaseReadinessError::InvalidReleaseReadiness {
                message: "status is required".to_string(),
            })?;
        if status == "ready" {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "ready status is reserved until release process approval is implemented"
                    .to_string(),
            });
        }
        if status == "reserved" {
            let blockers = readiness
                .get("blockers")
                .and_then(Value::as_array)
                .ok_or_else(|| ReleaseReadinessError::InvalidReleaseReadiness {
                    message: "blockers array is required".to_string(),
                })?;
            if blockers.is_empty() {
                return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                    message: "reserved readiness must explain why release automation is reserved"
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_schema(&self, readiness: &Value) -> Result<(), ReleaseReadinessError> {
        let schema_path = self.schema_root.join(RELEASE_READINESS_SCHEMA);
        let schema = load_schema(&schema_path).map_err(|source| {
            ReleaseReadinessError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            }
        })?;
        let result = validate_json(readiness, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ReleaseReadinessError::SchemaValidationFailed {
                path: PathBuf::from(RELEASE_READINESS_PATH),
                errors: result.errors,
            })
        }
    }
}

fn write_new_json(path: &Path, value: &Value) -> Result<(), ReleaseReadinessError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ReleaseReadinessError::WriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| ReleaseReadinessError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })
}

fn timestamp_string() -> String {
    format!("unix:{}", timestamp_nanos())
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use star_control_state::StateStore;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn writes_reserved_release_readiness_inside_job_dir() {
        let project = temp_project("reserved");
        let store = open_store(&project);
        create_job(&store);
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

        let artifact_ref = writer
            .write(&store, "J-0001", &readiness)
            .expect("write release readiness");

        assert_eq!(artifact_ref["path"], RELEASE_READINESS_PATH);
        assert_eq!(artifact_ref["kind"], "other");
        assert_eq!(artifact_ref["producer"], "star-control-release");
        assert_eq!(
            artifact_ref["schema_path"],
            "specs/schemas/release-readiness.schema.json"
        );
        let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
        assert!(path.is_file());
        let read = writer
            .read(&store, "J-0001")
            .expect("read release readiness")
            .expect("release readiness exists");
        assert_eq!(read["status"], "reserved");
        assert!(read["blockers"]
            .as_array()
            .expect("blockers")
            .contains(&json!("release automation is not implemented yet")));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn rejects_ready_status_until_release_approval_flow_exists() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let mut readiness = writer.readiness(
            "release-0002",
            "star-control",
            "0.1.0",
            "ready",
            vec![writer.check("required-ci-passed", "pass", Vec::new())],
            Vec::new(),
        );
        readiness["approvals"] = json!(["release approval recorded"]);

        let error = writer
            .validate_readiness(&readiness)
            .expect_err("ready status is reserved");
        assert!(matches!(
            error,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));
    }

    #[test]
    fn rejects_reserved_status_without_blocker_explanation() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = writer.readiness(
            "release-0003",
            "star-control",
            "0.0.0-dev",
            "reserved",
            vec![writer.check("required-ci-passed", "reserved", Vec::new())],
            Vec::new(),
        );

        let error = writer
            .validate_readiness(&readiness)
            .expect_err("reserved status needs blocker explanation");
        assert!(matches!(
            error,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));
    }

    #[test]
    fn refuses_to_overwrite_existing_release_readiness() {
        let project = temp_project("overwrite");
        let store = open_store(&project);
        create_job(&store);
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

        writer
            .write(&store, "J-0001", &readiness)
            .expect("first write");
        let error = writer
            .write(&store, "J-0001", &readiness)
            .expect_err("second write must not overwrite");

        assert!(matches!(error, ReleaseReadinessError::WriteFailed { .. }));
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn rejects_path_traversal_job_id_without_writing() {
        let project = temp_project("traversal");
        let store = open_store(&project);
        create_job(&store);
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

        let error = writer
            .write(&store, "../J-0001", &readiness)
            .expect_err("unsafe job id");
        assert!(matches!(error, ReleaseReadinessError::State { .. }));
        assert!(!project
            .join(".ai-runs/release/release-readiness.json")
            .exists());
        fs::remove_dir_all(project).ok();
    }

    fn create_job(store: &StateStore) {
        store
            .create_job("request", "cli", Vec::new())
            .expect("create job");
    }

    fn open_store(project_root: &Path) -> StateStore {
        StateStore::open(project_root, schema_root()).expect("open state store")
    }

    fn schema_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .join("specs")
            .join("schemas")
    }

    fn temp_project(label: &str) -> PathBuf {
        let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "star-control-release-{}-{}-{}",
            std::process::id(),
            counter,
            label
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }
}
