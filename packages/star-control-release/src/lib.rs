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
pub const RELEASE_REVIEW_PACK_MARKDOWN_FILE: &str = "release-review-pack.md";
pub const RELEASE_REVIEW_PACK_PATH: &str = "review-packs/release-review-pack.md";
pub const M9_REQUIRED_READINESS_CHECKS: &[&str] = &[
    "security-redaction",
    "audit-event-writer",
    "cost-budget-guard",
    "provider-conformance-hardening",
    "state-recovery-inspection",
    "release-readiness-writer",
    "release-readiness-api-read",
    "release-version-consistency",
    "release-evidence-file-checker",
    "release-profile-readiness",
    "release-readiness-ui-read",
    "release-readiness-cli-read",
    "release-review-pack",
    "recovery-command-surface",
    "destructive-actions-reserved",
    "release-automation-reserved",
];
pub const COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS: &[&str] = &[
    "m0-docs-decisions",
    "m1-runtime-foundation",
    "m2-provider-neutral-execution",
    "m3-validation-gate",
    "m4-v0-fake-e2e",
    "m5-local-provider",
    "m6-cloud-provider",
    "m7-daemon-api-control-plane",
    "m8-ui-shell",
    "m9-hardening-release-readiness",
    "full-local-validation",
    "remote-ci-evidence",
    "stacked-prs-clean",
    "reserved-actions-confirmed",
];

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
                "failed to write release artifact {}: {}",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M9ReadinessCheck {
    name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl M9ReadinessCheck {
    pub fn passed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            name: normalized_m9_readiness_check_name(name)?,
            passed: true,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers: Vec::new(),
        })
    }

    pub fn failed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
        blockers: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        let blockers = normalize_m9_readiness_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed M9 readiness check requires at least one blocker".to_string(),
            });
        }
        Ok(Self {
            name: normalized_m9_readiness_check_name(name)?,
            passed: false,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
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
            &self.name,
            check_status(self.passed),
            self.evidence_paths.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct M9ReadinessAuditBuilder;

impl M9ReadinessAuditBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        readiness_checks: Vec<M9ReadinessCheck>,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = Vec::with_capacity(readiness_checks.len());
        let mut blockers = Vec::new();
        let mut seen = Vec::with_capacity(readiness_checks.len());

        for readiness_check in &readiness_checks {
            if seen
                .iter()
                .any(|seen_name: &String| seen_name == readiness_check.name())
            {
                blockers.push(format!(
                    "duplicate M9 readiness check: {}",
                    readiness_check.name()
                ));
            } else {
                seen.push(readiness_check.name().to_string());
            }

            checks.push(readiness_check.to_check());

            if !readiness_check.is_passed() {
                for blocker in readiness_check.blockers() {
                    blockers.push(format!("{}: {}", readiness_check.name(), blocker));
                }
            }
        }

        for required_check in M9_REQUIRED_READINESS_CHECKS {
            if !seen.iter().any(|seen_name| seen_name == required_check) {
                blockers.push(format!("missing M9 readiness check: {}", required_check));
            }
        }

        if blockers.is_empty() {
            blockers.push(
                "final release/deploy/publish remains reserved until explicit approval".to_string(),
            );
            writer.readiness(release_id, target, version, "reserved", checks, blockers)
        } else {
            writer.not_ready(release_id, target, version, checks, blockers)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteImplementationAuditCheck {
    name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl CompleteImplementationAuditCheck {
    pub fn passed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            name: normalized_complete_implementation_check_name(name)?,
            passed: true,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers: Vec::new(),
        })
    }

    pub fn failed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
        blockers: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        let blockers = normalize_complete_implementation_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed complete implementation check requires at least one blocker"
                    .to_string(),
            });
        }
        Ok(Self {
            name: normalized_complete_implementation_check_name(name)?,
            passed: false,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
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
            &self.name,
            check_status(self.passed),
            self.evidence_paths.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CompleteImplementationAuditBuilder;

impl CompleteImplementationAuditBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        audit_checks: Vec<CompleteImplementationAuditCheck>,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = Vec::with_capacity(audit_checks.len());
        let mut blockers = Vec::new();
        let mut seen = Vec::with_capacity(audit_checks.len());

        for audit_check in &audit_checks {
            if seen
                .iter()
                .any(|seen_name: &String| seen_name == audit_check.name())
            {
                blockers.push(format!(
                    "duplicate complete implementation check: {}",
                    audit_check.name()
                ));
            } else {
                seen.push(audit_check.name().to_string());
            }

            checks.push(audit_check.to_check());

            if !audit_check.is_passed() {
                for blocker in audit_check.blockers() {
                    blockers.push(format!("{}: {}", audit_check.name(), blocker));
                }
            }
        }

        for required_check in COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS {
            if !seen.iter().any(|seen_name| seen_name == required_check) {
                blockers.push(format!(
                    "missing complete implementation check: {}",
                    required_check
                ));
            }
        }

        if blockers.is_empty() {
            blockers.push(
                "release/deploy/publish and external repository settings remain reserved until explicit approval"
                    .to_string(),
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

#[derive(Debug, Clone)]
pub struct ReleaseReviewPackWriter {
    readiness_writer: ReleaseReadinessWriter,
}

impl ReleaseReviewPackWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            readiness_writer: ReleaseReadinessWriter::new(schema_root),
        }
    }

    pub fn build_markdown(&self, readiness: &Value) -> Result<String, ReleaseReadinessError> {
        self.readiness_writer.validate_readiness(readiness)?;
        Ok(render_release_review_pack_markdown(readiness))
    }

    pub fn write(
        &self,
        store: &StateStore,
        job_id: &str,
        readiness: &Value,
    ) -> Result<Value, ReleaseReadinessError> {
        let markdown = self.build_markdown(readiness)?;
        let path = store.resolve_job_path(job_id, RELEASE_REVIEW_PACK_PATH)?;
        write_new_text(&path, &markdown)?;
        store
            .artifact_ref(
                job_id,
                RELEASE_REVIEW_PACK_PATH,
                ArtifactKind::ReviewPack,
                "star-control-release",
                None,
                Some("release review pack Markdown artifact"),
            )
            .map_err(ReleaseReadinessError::from)
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

fn write_new_text(path: &Path, content: &str) -> Result<(), ReleaseReadinessError> {
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
    file.write_all(content.as_bytes())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })
}

fn render_release_review_pack_markdown(readiness: &Value) -> String {
    let mut markdown = String::new();
    markdown.push_str("# Release Review Pack\n\n");
    markdown.push_str("## Summary\n\n");
    markdown.push_str(&format!(
        "- release_id: `{}`\n",
        markdown_inline(release_field(readiness, "release_id"))
    ));
    markdown.push_str(&format!(
        "- target: `{}`\n",
        markdown_inline(release_field(readiness, "target"))
    ));
    markdown.push_str(&format!(
        "- version: `{}`\n",
        markdown_inline(release_field(readiness, "version"))
    ));
    markdown.push_str(&format!(
        "- status: `{}`\n",
        markdown_inline(release_field(readiness, "status"))
    ));
    markdown.push_str(&format!(
        "- generated_at: `{}`\n\n",
        markdown_inline(release_field(readiness, "generated_at"))
    ));

    markdown.push_str("## Checks\n\n");
    let checks = readiness
        .get("checks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if checks.is_empty() {
        markdown.push_str("- none recorded\n\n");
    } else {
        for check in checks {
            let name = markdown_inline(release_field(check, "name"));
            let status = markdown_inline(release_field(check, "status"));
            let evidence_paths = release_string_array(check, "evidence_paths");
            markdown.push_str(&format!("- `{}`: `{}`", name, status));
            if !evidence_paths.is_empty() {
                markdown.push_str(&format!(
                    " (evidence: {})",
                    markdown_code_list(&evidence_paths)
                ));
            }
            markdown.push('\n');
        }
        markdown.push('\n');
    }

    markdown.push_str("## Blockers\n\n");
    push_markdown_bullets(
        &mut markdown,
        &release_string_array(readiness, "blockers"),
        "none recorded",
    );

    markdown.push_str("## Approvals\n\n");
    push_markdown_bullets(
        &mut markdown,
        &release_string_array(readiness, "approvals"),
        "none recorded",
    );

    markdown.push_str("## Guardrails\n\n");
    markdown.push_str("- This artifact is for human review only.\n");
    markdown.push_str(
        "- Release, deploy, publish, signing, repository settings, and external account actions remain reserved.\n",
    );
    markdown.push_str(
        "- A review pack is not an approval record and must not trigger release automation.\n",
    );
    markdown
}

fn release_field<'a>(value: &'a Value, field: &str) -> &'a str {
    value.get(field).and_then(Value::as_str).unwrap_or("")
}

fn release_string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(markdown_inline)
                .collect()
        })
        .unwrap_or_default()
}

fn push_markdown_bullets(markdown: &mut String, values: &[String], empty_label: &str) {
    if values.is_empty() {
        markdown.push_str(&format!("- {}\n\n", empty_label));
        return;
    }
    for value in values {
        markdown.push_str(&format!("- {}\n", value));
    }
    markdown.push('\n');
}

fn markdown_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{}`", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn markdown_inline(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "<empty>".to_string()
    } else {
        collapsed.replace('`', "'")
    }
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

fn normalized_m9_readiness_check_name(
    name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let name = name.into();
    let name = name.trim();
    if name.is_empty() {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "M9 readiness check name is required".to_string(),
        });
    }
    if !M9_REQUIRED_READINESS_CHECKS.contains(&name) {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: format!("unknown M9 readiness check: {}", name),
        });
    }
    Ok(name.to_string())
}

fn normalize_m9_readiness_blockers(
    blockers: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    let mut normalized = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        let blocker = blocker.trim();
        if blocker.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "M9 readiness blocker must not be empty".to_string(),
            });
        }
        normalized.push(blocker.to_string());
    }
    Ok(normalized)
}

fn normalized_complete_implementation_check_name(
    name: impl Into<String>,
) -> Result<String, ReleaseReadinessError> {
    let name = name.into();
    let name = name.trim();
    if name.is_empty() {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "complete implementation check name is required".to_string(),
        });
    }
    if !COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS.contains(&name) {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: format!("unknown complete implementation check: {}", name),
        });
    }
    Ok(name.to_string())
}

fn normalize_complete_implementation_blockers(
    blockers: Vec<String>,
) -> Result<Vec<String>, ReleaseReadinessError> {
    let mut normalized = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        let blocker = blocker.trim();
        if blocker.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "complete implementation blocker must not be empty".to_string(),
            });
        }
        normalized.push(blocker.to_string());
    }
    Ok(normalized)
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
    fn release_review_pack_writer_writes_markdown_without_release_action() {
        let project = temp_project("review-pack");
        let store = open_store(&project);
        create_job(&store);
        let readiness_writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = readiness_writer.not_ready(
            "release-0007",
            "star-control",
            "1.2.3",
            vec![
                readiness_writer.check(
                    "required-ci-passed",
                    "pass",
                    vec![".github/workflows/ci.yml".to_string()],
                ),
                readiness_writer.check(
                    "version-consistent",
                    "fail",
                    vec!["Cargo.toml".to_string()],
                ),
            ],
            vec!["version mismatch: expected 1.2.3, found 1.2.2".to_string()],
        );
        let review_pack_writer = ReleaseReviewPackWriter::new(schema_root());

        let artifact_ref = review_pack_writer
            .write(&store, "J-0001", &readiness)
            .expect("write release review pack");

        assert_eq!(artifact_ref["path"], RELEASE_REVIEW_PACK_PATH);
        assert_eq!(artifact_ref["kind"], "review_pack");
        assert_eq!(artifact_ref["producer"], "star-control-release");
        let path = project
            .join(".ai-runs")
            .join("J-0001")
            .join("review-packs")
            .join(RELEASE_REVIEW_PACK_MARKDOWN_FILE);
        let markdown = fs::read_to_string(&path).expect("read release review pack");
        assert!(markdown.contains("# Release Review Pack"));
        assert!(markdown.contains("release-0007"));
        assert!(markdown.contains("version-consistent"));
        assert!(markdown.contains("version mismatch: expected 1.2.3, found 1.2.2"));
        assert!(markdown.contains("release automation"));
        assert!(!project
            .join(".ai-runs")
            .join("J-0001")
            .join("release")
            .join("release-action.json")
            .exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_review_pack_rejects_ready_status_and_overwrite() {
        let project = temp_project("review-pack-overwrite");
        let store = open_store(&project);
        create_job(&store);
        let readiness_writer = ReleaseReadinessWriter::new(schema_root());
        let review_pack_writer = ReleaseReviewPackWriter::new(schema_root());
        let mut ready = readiness_writer.readiness(
            "release-0008",
            "star-control",
            "1.2.3",
            "ready",
            vec![readiness_writer.check("required-ci-passed", "pass", Vec::new())],
            Vec::new(),
        );
        ready["approvals"] = json!(["release approval recorded"]);

        let ready_error = review_pack_writer
            .build_markdown(&ready)
            .expect_err("ready status remains reserved");
        assert!(matches!(
            ready_error,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));

        let reserved = readiness_writer.reserved("release-0009", "star-control", "0.0.0-dev");
        review_pack_writer
            .write(&store, "J-0001", &reserved)
            .expect("first review pack write");
        let overwrite_error = review_pack_writer
            .write(&store, "J-0001", &reserved)
            .expect_err("second review pack write must not overwrite");
        assert!(matches!(
            overwrite_error,
            ReleaseReadinessError::WriteFailed { .. }
        ));

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

    #[test]
    fn m9_readiness_audit_builder_reserves_complete_audit() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = M9ReadinessAuditBuilder.build(
            &writer,
            "m9-audit-0001",
            "star-control",
            "m9",
            all_m9_readiness_checks_passed(),
        );

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid reserved M9 readiness");
        assert_eq!(readiness["status"], "reserved");
        assert_eq!(
            readiness["checks"].as_array().expect("checks").len(),
            M9_REQUIRED_READINESS_CHECKS.len()
        );
        assert_eq!(
            readiness["checks"][0]["name"],
            M9_REQUIRED_READINESS_CHECKS[0]
        );
        assert_eq!(readiness["checks"][0]["status"], "pass");
        assert!(readiness["blockers"]
            .as_array()
            .expect("blockers")
            .contains(&json!(
                "final release/deploy/publish remains reserved until explicit approval"
            )));
    }

    #[test]
    fn m9_readiness_audit_builder_blocks_missing_failed_and_duplicate_checks() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let mut checks = all_m9_readiness_checks_passed();
        checks.retain(|check| check.name() != "release-automation-reserved");
        checks.push(
            M9ReadinessCheck::failed(
                "cost-budget-guard",
                vec!["docs/implementation/briefs/E28-cost-metric-budget-guard.md".to_string()],
                vec!["cost budget acceptance evidence is missing".to_string()],
            )
            .expect("failed M9 check"),
        );

        let readiness =
            M9ReadinessAuditBuilder.build(&writer, "m9-audit-0002", "star-control", "m9", checks);

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid not_ready M9 readiness");
        assert_eq!(readiness["status"], "not_ready");
        let blockers = readiness["blockers"].as_array().expect("blockers");
        assert!(blockers.contains(&json!(
            "missing M9 readiness check: release-automation-reserved"
        )));
        assert!(blockers.contains(&json!("duplicate M9 readiness check: cost-budget-guard")));
        assert!(blockers.contains(&json!(
            "cost-budget-guard: cost budget acceptance evidence is missing"
        )));
    }

    #[test]
    fn m9_readiness_check_rejects_unknown_or_unsafe_inputs() {
        let unknown_check =
            M9ReadinessCheck::passed("unknown-check", Vec::new()).expect_err("unknown check");
        assert!(matches!(
            unknown_check,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));

        let unsafe_evidence = M9ReadinessCheck::passed(
            "security-redaction",
            vec!["../security-redaction.json".to_string()],
        )
        .expect_err("unsafe evidence");
        assert!(matches!(
            unsafe_evidence,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));

        let empty_blocker =
            M9ReadinessCheck::failed("cost-budget-guard", Vec::new(), vec![" ".to_string()])
                .expect_err("empty blocker");
        assert!(matches!(
            empty_blocker,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));
    }

    #[test]
    fn complete_implementation_audit_builder_reserves_complete_audit() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = CompleteImplementationAuditBuilder.build(
            &writer,
            "completion-audit-0001",
            "star-control",
            "m0-m9",
            all_complete_implementation_checks_passed(),
        );

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid reserved complete implementation readiness");
        assert_eq!(readiness["status"], "reserved");
        assert_eq!(
            readiness["checks"].as_array().expect("checks").len(),
            COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS.len()
        );
        assert_eq!(
            readiness["checks"][0]["name"],
            COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS[0]
        );
        assert_eq!(readiness["checks"][0]["status"], "pass");
        assert!(readiness["blockers"]
            .as_array()
            .expect("blockers")
            .contains(&json!(
                "release/deploy/publish and external repository settings remain reserved until explicit approval"
            )));
    }

    #[test]
    fn complete_implementation_audit_builder_blocks_missing_failed_and_duplicate_checks() {
        let writer = ReleaseReadinessWriter::new(schema_root());
        let mut checks = all_complete_implementation_checks_passed();
        checks.retain(|check| check.name() != "remote-ci-evidence");
        checks.push(
            CompleteImplementationAuditCheck::failed(
                "m6-cloud-provider",
                vec!["docs/implementation/cloud-provider-policy.md".to_string()],
                vec!["cloud API live transport remains approval-gated".to_string()],
            )
            .expect("failed complete implementation check"),
        );

        let readiness = CompleteImplementationAuditBuilder.build(
            &writer,
            "completion-audit-0002",
            "star-control",
            "m0-m9",
            checks,
        );

        writer
            .validate_readiness(&readiness)
            .expect("schema-valid not_ready complete implementation readiness");
        assert_eq!(readiness["status"], "not_ready");
        let blockers = readiness["blockers"].as_array().expect("blockers");
        assert!(blockers.contains(&json!(
            "missing complete implementation check: remote-ci-evidence"
        )));
        assert!(blockers.contains(&json!(
            "duplicate complete implementation check: m6-cloud-provider"
        )));
        assert!(blockers.contains(&json!(
            "m6-cloud-provider: cloud API live transport remains approval-gated"
        )));
    }

    #[test]
    fn complete_implementation_check_rejects_unknown_or_unsafe_inputs() {
        let unknown_check = CompleteImplementationAuditCheck::passed("m10-extra", Vec::new())
            .expect_err("unknown completion check");
        assert!(matches!(
            unknown_check,
            ReleaseReadinessError::InvalidReleaseReadiness { .. }
        ));

        let unsafe_evidence = CompleteImplementationAuditCheck::passed(
            "m0-docs-decisions",
            vec!["../complete-implementation-roadmap.md".to_string()],
        )
        .expect_err("unsafe evidence");
        assert!(matches!(
            unsafe_evidence,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));

        let empty_blocker = CompleteImplementationAuditCheck::failed(
            "stacked-prs-clean",
            Vec::new(),
            vec![" ".to_string()],
        )
        .expect_err("empty blocker");
        assert!(matches!(
            empty_blocker,
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

    fn all_m9_readiness_checks_passed() -> Vec<M9ReadinessCheck> {
        M9_REQUIRED_READINESS_CHECKS
            .iter()
            .map(|check_name| {
                M9ReadinessCheck::passed(
                    *check_name,
                    vec![format!("docs/implementation/briefs/{}.md", check_name)],
                )
                .expect("M9 readiness check")
            })
            .collect()
    }

    fn all_complete_implementation_checks_passed() -> Vec<CompleteImplementationAuditCheck> {
        COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS
            .iter()
            .map(|check_name| {
                CompleteImplementationAuditCheck::passed(
                    *check_name,
                    vec![format!("docs/implementation/audit/{}.md", check_name)],
                )
                .expect("complete implementation check")
            })
            .collect()
    }
}
