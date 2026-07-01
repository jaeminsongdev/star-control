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
    InvalidReleaseEvidence {
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
            Self::InvalidReleaseEvidence { message } => {
                write!(formatter, "invalid release evidence: {}", message)
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

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseConsistencyResult {
    checks: Vec<Value>,
    blockers: Vec<String>,
}

impl ReleaseConsistencyResult {
    pub fn checks(&self) -> &[Value] {
        &self.checks
    }

    pub fn blockers(&self) -> &[String] {
        &self.blockers
    }

    pub fn is_consistent(&self) -> bool {
        self.blockers.is_empty()
    }

    pub fn into_parts(self) -> (Vec<Value>, Vec<String>) {
        (self.checks, self.blockers)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseConsistencyChecker;

impl ReleaseConsistencyChecker {
    pub fn check(
        expected_version: impl Into<String>,
        declared_version: impl Into<String>,
        changelog_text: impl Into<String>,
        version_evidence_path: impl Into<String>,
        changelog_evidence_path: impl Into<String>,
    ) -> ReleaseConsistencyResult {
        let expected_version = expected_version.into();
        let declared_version = declared_version.into();
        let changelog_text = changelog_text.into();
        let version_evidence_path = version_evidence_path.into();
        let changelog_evidence_path = changelog_evidence_path.into();
        let expected = expected_version.trim();
        let declared = declared_version.trim();

        let mut blockers = Vec::new();
        let version_matches = !expected.is_empty() && declared == expected;
        let changelog_mentions_version =
            !expected.is_empty() && changelog_text.lines().any(|line| line.contains(expected));

        if expected.is_empty() {
            blockers.push("expected release version is empty".to_string());
        } else {
            if !version_matches {
                blockers.push(format!(
                    "version mismatch: expected {}, found {}",
                    expected,
                    display_or_empty(declared)
                ));
            }
            if !changelog_mentions_version {
                blockers.push(format!("changelog does not mention version {}", expected));
            }
        }

        ReleaseConsistencyResult {
            checks: vec![
                release_check(
                    "version-consistent",
                    check_status(version_matches),
                    evidence_paths(version_evidence_path),
                ),
                release_check(
                    "changelog-updated",
                    check_status(changelog_mentions_version),
                    evidence_paths(changelog_evidence_path),
                ),
            ],
            blockers,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseEvidenceFileChecker;

impl ReleaseEvidenceFileChecker {
    pub fn check(
        project_root: impl AsRef<Path>,
        expected_version: impl Into<String>,
        version_file: impl AsRef<str>,
        changelog_file: impl AsRef<str>,
    ) -> Result<ReleaseConsistencyResult, ReleaseReadinessError> {
        let project_root = project_root.as_ref();
        let version_file = version_file.as_ref();
        let changelog_file = changelog_file.as_ref();
        let version_path = resolve_project_file(project_root, version_file)?;
        let changelog_path = resolve_project_file(project_root, changelog_file)?;
        let version_text = read_release_text(&version_path)?;
        let changelog_text = read_release_text(&changelog_path)?;
        let declared_version = declared_version_from_text(&version_text).ok_or_else(|| {
            ReleaseReadinessError::InvalidReleaseEvidence {
                message: format!(
                    "declared version not found in release evidence {}",
                    version_file
                ),
            }
        })?;

        Ok(ReleaseConsistencyChecker::check(
            expected_version,
            declared_version,
            changelog_text,
            normalized_evidence_path(version_file)?,
            normalized_evidence_path(changelog_file)?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseProfileValidation {
    profile_name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl ReleaseProfileValidation {
    pub fn passed(
        profile_name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            profile_name: normalized_profile_name(profile_name)?,
            passed: true,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers: Vec::new(),
        })
    }

    pub fn failed(
        profile_name: impl Into<String>,
        evidence_paths: Vec<String>,
        blockers: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        let blockers = normalize_profile_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed release profile validation requires at least one blocker"
                    .to_string(),
            });
        }
        Ok(Self {
            profile_name: normalized_profile_name(profile_name)?,
            passed: false,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers,
        })
    }

    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    pub fn is_passed(&self) -> bool {
        self.passed
    }

    pub fn evidence_paths(&self) -> &[String] {
        &self.evidence_paths
    }

    pub fn blockers(&self) -> &[String] {
        &self.blockers
    }

    fn to_check(&self) -> Value {
        release_check(
            "release-profile-passed",
            check_status(self.passed),
            self.evidence_paths.clone(),
        )
    }

    fn into_blockers(self) -> Vec<String> {
        self.blockers
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseProfileReadinessBuilder;

impl ReleaseProfileReadinessBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        profile: ReleaseProfileValidation,
        consistency: ReleaseConsistencyResult,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = vec![profile.to_check()];
        let (mut consistency_checks, consistency_blockers) = consistency.into_parts();
        checks.append(&mut consistency_checks);
        let mut blockers = profile.into_blockers();
        blockers.extend(consistency_blockers);

        if blockers.is_empty() {
            blockers.push(
                "release approval/signing/publish/deploy automation remains reserved".to_string(),
            );
            writer.readiness(release_id, target, version, "reserved", checks, blockers)
        } else {
            writer.not_ready(release_id, target, version, checks, blockers)
        }
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

fn release_check(name: &str, status: &str, evidence_paths: Vec<String>) -> Value {
    json!({
        "name": name,
        "status": status,
        "evidence_paths": evidence_paths
    })
}

fn check_status(passed: bool) -> &'static str {
    if passed {
        "pass"
    } else {
        "fail"
    }
}

fn evidence_paths(path: String) -> Vec<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}

fn display_or_empty(value: &str) -> &str {
    if value.is_empty() {
        "<empty>"
    } else {
        value
    }
}

fn resolve_project_file(
    project_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ReleaseReadinessError> {
    let normalized = normalized_evidence_path(relative_path)?;
    let root =
        fs::canonicalize(project_root).map_err(|source| ReleaseReadinessError::ReadFailed {
            path: project_root.to_path_buf(),
            source,
        })?;
    let path = root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR));
    let canonical =
        fs::canonicalize(&path).map_err(|source| ReleaseReadinessError::ReadFailed {
            path: path.clone(),
            source,
        })?;
    if !canonical.starts_with(&root) {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!(
                "release evidence path escapes project root: {}",
                relative_path
            ),
        });
    }
    if !canonical.is_file() {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!("release evidence path is not a file: {}", relative_path),
        });
    }
    Ok(canonical)
}

fn normalized_evidence_path(path: &str) -> Result<String, ReleaseReadinessError> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(':')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ReleaseReadinessError::InvalidReleaseEvidence {
            message: format!("unsafe release evidence path: {}", display_or_empty(&path)),
        });
    }
    Ok(path)
}

fn normalize_evidence_paths(paths: Vec<String>) -> Result<Vec<String>, ReleaseReadinessError> {
    paths
        .into_iter()
        .map(|path| normalized_evidence_path(&path))
        .collect()
}

fn normalized_profile_name(
    profile_name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let profile_name = profile_name.into();
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "release profile name is required".to_string(),
        })
    } else {
        Ok(profile_name.to_string())
    }
}

fn normalize_profile_blockers(blockers: Vec<String>) -> Result<Vec<String>, ReleaseReadinessError> {
    let mut normalized = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        let blocker = blocker.trim();
        if blocker.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "release profile blocker must not be empty".to_string(),
            });
        }
        normalized.push(blocker.to_string());
    }
    Ok(normalized)
}

fn read_release_text(path: &Path) -> Result<String, ReleaseReadinessError> {
    fs::read_to_string(path).map_err(|source| ReleaseReadinessError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn declared_version_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.is_empty()
        && !trimmed.contains('\n')
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
    {
        return Some(trimmed.to_string());
    }

    text.lines().filter_map(version_assignment_value).next()
}

fn version_assignment_value(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with('#') || !line.starts_with("version") {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    if key.trim() != "version" {
        return None;
    }
    let value = value.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
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

    #[test]
    fn release_consistency_checker_passes_matching_version_and_changelog() {
        let result = ReleaseConsistencyChecker::check(
            "1.2.3",
            "1.2.3\n",
            "# Changelog\n\n## 1.2.3\n- release notes\n",
            "Cargo.toml",
            "CHANGELOG.md",
        );

        assert!(result.is_consistent());
        assert!(result.blockers().is_empty());
        assert_eq!(result.checks()[0]["name"], "version-consistent");
        assert_eq!(result.checks()[0]["status"], "pass");
        assert_eq!(result.checks()[0]["evidence_paths"][0], "Cargo.toml");
        assert_eq!(result.checks()[1]["name"], "changelog-updated");
        assert_eq!(result.checks()[1]["status"], "pass");
        assert_eq!(result.checks()[1]["evidence_paths"][0], "CHANGELOG.md");
    }

    #[test]
    fn release_consistency_checker_blocks_version_and_changelog_mismatch() {
        let result = ReleaseConsistencyChecker::check(
            "1.2.3",
            "1.2.2",
            "# Changelog\n\n## 1.2.2\n- previous release\n",
            "Cargo.toml",
            "CHANGELOG.md",
        );

        assert!(!result.is_consistent());
        assert_eq!(result.checks()[0]["status"], "fail");
        assert_eq!(result.checks()[1]["status"], "fail");
        assert!(result
            .blockers()
            .contains(&"version mismatch: expected 1.2.3, found 1.2.2".to_string()));
        assert!(result
            .blockers()
            .contains(&"changelog does not mention version 1.2.3".to_string()));
    }

    #[test]
    fn release_consistency_result_feeds_schema_valid_not_ready_readiness() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let result =
            ReleaseConsistencyChecker::check("1.2.3", "", "no version yet", "", "CHANGELOG.md");
        let (checks, blockers) = result.into_parts();
        let readiness = writer.not_ready("release-0004", "star-control", "1.2.3", checks, blockers);

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid not_ready release readiness");
        assert_eq!(readiness["status"], "not_ready");
        assert_eq!(readiness["checks"][0]["name"], "version-consistent");
        assert_eq!(readiness["checks"][1]["name"], "changelog-updated");
        assert!(!readiness["blockers"]
            .as_array()
            .expect("blockers")
            .is_empty());
    }

    #[test]
    fn release_evidence_file_checker_reads_version_and_changelog_inside_project() {
        let project = temp_project("evidence-pass");
        fs::write(
            project.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.2.3\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(
            project.join("CHANGELOG.md"),
            "# Changelog\n\n## 1.2.3\n- release notes\n",
        )
        .expect("write changelog");

        let result =
            ReleaseEvidenceFileChecker::check(&project, "1.2.3", "Cargo.toml", "CHANGELOG.md")
                .expect("file evidence result");

        assert!(result.is_consistent());
        assert_eq!(result.checks()[0]["status"], "pass");
        assert_eq!(result.checks()[0]["evidence_paths"][0], "Cargo.toml");
        assert_eq!(result.checks()[1]["status"], "pass");
        assert_eq!(result.checks()[1]["evidence_paths"][0], "CHANGELOG.md");
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_evidence_file_checker_blocks_mismatch_from_files() {
        let project = temp_project("evidence-mismatch");
        fs::write(project.join("VERSION"), "1.2.2\n").expect("write VERSION");
        fs::write(project.join("CHANGELOG.md"), "## 1.2.2\n").expect("write changelog");

        let result =
            ReleaseEvidenceFileChecker::check(&project, "1.2.3", "VERSION", "CHANGELOG.md")
                .expect("file evidence result");

        assert!(!result.is_consistent());
        assert_eq!(result.checks()[0]["status"], "fail");
        assert_eq!(result.checks()[1]["status"], "fail");
        assert!(result
            .blockers()
            .contains(&"version mismatch: expected 1.2.3, found 1.2.2".to_string()));
        assert!(result
            .blockers()
            .contains(&"changelog does not mention version 1.2.3".to_string()));
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_evidence_file_checker_rejects_unsafe_paths_and_missing_version() {
        let project = temp_project("evidence-invalid");
        fs::write(project.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
            .expect("write Cargo.toml");
        fs::write(project.join("VERSION"), "1.2.3\n").expect("write VERSION");
        fs::write(project.join("CHANGELOG.md"), "## 1.2.3\n").expect("write changelog");

        for unsafe_path in [
            "../Cargo.toml",
            "/Cargo.toml",
            "C:/Cargo.toml",
            "nested/../Cargo.toml",
        ] {
            let unsafe_error =
                ReleaseEvidenceFileChecker::check(&project, "1.2.3", unsafe_path, "CHANGELOG.md")
                    .expect_err("unsafe version evidence path");
            assert!(matches!(
                unsafe_error,
                ReleaseReadinessError::InvalidReleaseEvidence { .. }
            ));
        }

        let unsafe_changelog_error =
            ReleaseEvidenceFileChecker::check(&project, "1.2.3", "VERSION", "../CHANGELOG.md")
                .expect_err("unsafe changelog evidence path");
        assert!(matches!(
            unsafe_changelog_error,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));

        let missing_version_error =
            ReleaseEvidenceFileChecker::check(&project, "1.2.3", "Cargo.toml", "CHANGELOG.md")
                .expect_err("missing version declaration");
        assert!(matches!(
            missing_version_error,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_profile_readiness_builder_reserves_status_after_all_checks_pass() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let profile = ReleaseProfileValidation::passed(
            "star-sentinel-release",
            vec![".ai-runs/J-0001/review-packs/release-profile.json".to_string()],
        )
        .expect("profile validation");
        let consistency = ReleaseConsistencyChecker::check(
            "1.2.3",
            "1.2.3",
            "## 1.2.3\n- release notes\n",
            "VERSION",
            "CHANGELOG.md",
        );

        let readiness = ReleaseProfileReadinessBuilder.build(
            &writer,
            "release-0005",
            "star-control",
            "1.2.3",
            profile,
            consistency,
        );

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid reserved readiness");
        assert_eq!(readiness["status"], "reserved");
        assert_eq!(readiness["checks"][0]["name"], "release-profile-passed");
        assert_eq!(readiness["checks"][0]["status"], "pass");
        assert!(readiness["blockers"]
            .as_array()
            .expect("blockers")
            .contains(&json!(
                "release approval/signing/publish/deploy automation remains reserved"
            )));
    }

    #[test]
    fn release_profile_readiness_builder_blocks_profile_and_consistency_failures() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let profile = ReleaseProfileValidation::failed(
            "star-sentinel-release",
            vec![".ai-runs/J-0001/tool-output/star-sentinel/gate.json".to_string()],
            vec!["release profile blocked unresolved BLOCK diagnostic".to_string()],
        )
        .expect("profile validation");
        let consistency = ReleaseConsistencyChecker::check(
            "1.2.3",
            "1.2.2",
            "## 1.2.2\n- previous release\n",
            "VERSION",
            "CHANGELOG.md",
        );

        let readiness = ReleaseProfileReadinessBuilder.build(
            &writer,
            "release-0006",
            "star-control",
            "1.2.3",
            profile,
            consistency,
        );

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid not_ready readiness");
        assert_eq!(readiness["status"], "not_ready");
        assert_eq!(readiness["checks"][0]["status"], "fail");
        let blockers = readiness["blockers"].as_array().expect("blockers");
        assert!(blockers.contains(&json!(
            "release profile blocked unresolved BLOCK diagnostic"
        )));
        assert!(blockers.contains(&json!("version mismatch: expected 1.2.3, found 1.2.2")));
        assert!(blockers.contains(&json!("changelog does not mention version 1.2.3")));
    }

    #[test]
    fn release_profile_validation_rejects_unsafe_evidence_and_empty_failure() {
        let unsafe_error = ReleaseProfileValidation::passed(
            "star-sentinel-release",
            vec!["../release-profile.json".to_string()],
        )
        .expect_err("unsafe release profile evidence");
        assert!(matches!(
            unsafe_error,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));

        let empty_blocker_error = ReleaseProfileValidation::failed(
            "star-sentinel-release",
            Vec::new(),
            vec![" ".to_string()],
        )
        .expect_err("empty blocker");
        assert!(matches!(
            empty_blocker_error,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));

        let empty_profile_error =
            ReleaseProfileValidation::passed(" ", Vec::new()).expect_err("empty profile name");
        assert!(matches!(
            empty_profile_error,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));
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
