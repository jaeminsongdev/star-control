//! Derived validation cache backed by immutable native validator evidence.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use star_application::{CacheValidationStability, ValidationCacheCandidate};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        CatalogRef, Completeness, DocumentRef, EVIDENCE_CONTRACT_SCHEMA_VERSION, OutputLimits,
        PlannedCheck, ProducerRef, ProjectPathKind, ProjectPathRef, TaskInvocation,
        TerminationReason, VALIDATION_PLAN_SCHEMA_ID, VALIDATION_POLICY_SCHEMA_VERSION,
        ValidationCache, ValidationOutcome, ValidationPlan, ValidationRun, ValidationRunRef,
        ValidationRunSchemaId,
    },
    ids::{ProjectId, TaskInvocationId, ValidationRunId},
};
use thiserror::Error;

const CACHE_SCHEMA_ID: &str = "star.validation-cache-entry";
const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_DIRECTORY: &str = "target/validation/star-control-cache";
const MAX_CACHE_ENTRY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedValidationCacheEntry {
    schema_id: String,
    schema_version: u32,
    project_key: String,
    check_id: String,
    cache_key: Sha256Hash,
    validation_run: ValidationRun,
    validation_run_ref: ValidationRunRef,
    native_evidence_ref: String,
    artifact_hashes: BTreeMap<String, Sha256Hash>,
    suppression_applied: bool,
    policy_schema_version: u32,
    evidence_schema_version: u32,
}

#[derive(Clone, Debug)]
pub struct CachedValidationEvidence {
    pub evidence_ref: String,
    pub evidence_bytes: Vec<u8>,
    pub report: Value,
    pub source_validation_run_ref: ValidationRunRef,
}

#[derive(Debug, Error)]
pub enum ValidationCacheError {
    #[error("validation cache entry is invalid")]
    Invalid,
    #[error("validation cache evidence is unavailable")]
    Unavailable,
    #[error("validation cache write failed")]
    Write,
}

pub fn load_validation_cache_candidates(
    project_root: &Path,
    project_key: &str,
    checks: &[PlannedCheck],
) -> Vec<ValidationCacheCandidate> {
    checks
        .iter()
        .filter_map(|check| {
            let entry = load_entry(project_root, &check.cache_key).ok()?;
            candidate_from_entry(project_root, project_key, check, &entry).ok()
        })
        .collect()
}

pub fn read_cached_validation_evidence(
    project_root: &Path,
    project_key: &str,
    check: &PlannedCheck,
    expected_source: &ValidationRunRef,
) -> Result<CachedValidationEvidence, ValidationCacheError> {
    let entry = load_entry(project_root, &check.cache_key)?;
    let candidate = candidate_from_entry(project_root, project_key, check, &entry)?;
    if candidate.validation_run_ref != *expected_source {
        return Err(ValidationCacheError::Invalid);
    }
    let evidence_bytes = read_artifact(project_root, &entry.native_evidence_ref)?;
    let report =
        serde_json::from_slice(&evidence_bytes).map_err(|_| ValidationCacheError::Invalid)?;
    Ok(CachedValidationEvidence {
        evidence_ref: entry.native_evidence_ref,
        evidence_bytes,
        report,
        source_validation_run_ref: candidate.validation_run_ref,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn store_successful_validation_cache(
    project_root: &Path,
    project_key: &str,
    plan: &ValidationPlan,
    evidence_ref: &str,
    report: &Value,
    timeout_ms: u64,
) -> Result<ValidationRunRef, ValidationCacheError> {
    let check = plan.checks.first().ok_or(ValidationCacheError::Invalid)?;
    if plan.checks.len() != 1 || !cached_report_reusable(report) {
        return Err(ValidationCacheError::Invalid);
    }
    let artifact_hashes = hash_report_artifacts(project_root, report)?;
    if !artifact_hashes.contains_key(evidence_ref) {
        return Err(ValidationCacheError::Invalid);
    }
    let started_at = report_time(report, "started_at")?;
    let finished_at = report_time(report, "finished_at")?;
    let tool_ref = CatalogRef {
        catalog_id: "scripts/validate.ps1".to_owned(),
        format_version: 1,
        item_version: "1".to_owned(),
        sha256: check.cache_key_inputs.inputs.validation_scripts.clone(),
    };
    let validation_run = ValidationRun {
        schema_id: ValidationRunSchemaId::ValidationRun,
        schema_version: 1,
        validation_run_id: ValidationRunId::new(),
        revision: 1,
        created_at: started_at,
        updated_at: finished_at,
        producer: ProducerRef {
            component: "star-controller".to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: plan.plan_fingerprint.to_string(),
            platform: report["environment"]["platform"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned(),
        },
        extensions: BTreeMap::new(),
        validation_plan_ref: DocumentRef {
            schema_id: VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: plan.validation_plan_id.to_string(),
            revision: 1,
            sha256: plan.plan_fingerprint.clone(),
        },
        check_ref: CatalogRef {
            catalog_id: check.check_id.clone(),
            format_version: 1,
            item_version: "1".to_owned(),
            sha256: check.cache_key_inputs.command.clone(),
        },
        tool_ref: tool_ref.clone(),
        attempt: 1,
        invocation: TaskInvocation {
            invocation_id: TaskInvocationId::new(),
            tool_ref,
            executable: check.command.executable.clone(),
            args: check.command.args.clone(),
            cwd: ProjectPathRef {
                project_id: ProjectId::from_stable_bytes(project_key.as_bytes()),
                path: ".".to_owned(),
                path_kind: ProjectPathKind::Directory,
            },
            env_refs: BTreeMap::new(),
            stdin_ref: None,
            timeout_ms,
            permission_action: "local_validation".to_owned(),
            idempotency_key: check.cache_key.to_string(),
            expected_exit_codes: BTreeSet::from([0]),
            output_limits: OutputLimits {
                stdout_bytes: MAX_ARTIFACT_BYTES,
                stderr_bytes: MAX_ARTIFACT_BYTES,
                artifact_bytes: MAX_ARTIFACT_BYTES,
            },
        },
        started_at: Some(started_at),
        finished_at: Some(finished_at),
        outcome: ValidationOutcome::Pass,
        completeness: Completeness::Complete,
        exit_code: Some(0),
        termination_reason: Some(TerminationReason::Exited),
        diagnostic_refs: Vec::new(),
        stdout_ref: None,
        stderr_ref: None,
        result_artifact_refs: Vec::new(),
        observed_tool: None,
        cache: Some(ValidationCache {
            hit: false,
            cache_key: check.cache_key.to_string(),
            source_validation_run_ref: None,
        }),
    };
    validation_run
        .validate()
        .map_err(|_| ValidationCacheError::Invalid)?;
    let validation_run_ref = ValidationRunRef {
        validation_run_id: validation_run.validation_run_id.clone(),
        revision: validation_run.revision,
        sha256: canonical_sha256(
            &serde_json::to_value(&validation_run).map_err(|_| ValidationCacheError::Invalid)?,
        )
        .map_err(|_| ValidationCacheError::Invalid)?,
    };
    let entry = PersistedValidationCacheEntry {
        schema_id: CACHE_SCHEMA_ID.to_owned(),
        schema_version: CACHE_SCHEMA_VERSION,
        project_key: project_key.to_owned(),
        check_id: check.check_id.clone(),
        cache_key: check.cache_key.clone(),
        validation_run,
        validation_run_ref: validation_run_ref.clone(),
        native_evidence_ref: evidence_ref.to_owned(),
        artifact_hashes,
        suppression_applied: false,
        policy_schema_version: VALIDATION_POLICY_SCHEMA_VERSION,
        evidence_schema_version: EVIDENCE_CONTRACT_SCHEMA_VERSION,
    };
    write_entry(project_root, &entry)?;
    Ok(validation_run_ref)
}

fn candidate_from_entry(
    project_root: &Path,
    project_key: &str,
    check: &PlannedCheck,
    entry: &PersistedValidationCacheEntry,
) -> Result<ValidationCacheCandidate, ValidationCacheError> {
    if entry.schema_id != CACHE_SCHEMA_ID
        || entry.schema_version != CACHE_SCHEMA_VERSION
        || entry.project_key != project_key
        || entry.check_id != check.check_id
        || entry.cache_key != check.cache_key
        || entry.policy_schema_version != VALIDATION_POLICY_SCHEMA_VERSION
        || entry.evidence_schema_version != EVIDENCE_CONTRACT_SCHEMA_VERSION
        || entry.suppression_applied
    {
        return Err(ValidationCacheError::Invalid);
    }
    entry
        .validation_run
        .validate()
        .map_err(|_| ValidationCacheError::Invalid)?;
    let observed_ref = ValidationRunRef {
        validation_run_id: entry.validation_run.validation_run_id.clone(),
        revision: entry.validation_run.revision,
        sha256: canonical_sha256(
            &serde_json::to_value(&entry.validation_run)
                .map_err(|_| ValidationCacheError::Invalid)?,
        )
        .map_err(|_| ValidationCacheError::Invalid)?,
    };
    if observed_ref != entry.validation_run_ref {
        return Err(ValidationCacheError::Invalid);
    }
    for (artifact_ref, expected_hash) in &entry.artifact_hashes {
        let bytes = read_artifact(project_root, artifact_ref)?;
        if Sha256Hash::digest(&bytes) != *expected_hash {
            return Err(ValidationCacheError::Unavailable);
        }
    }
    let report_bytes = read_artifact(project_root, &entry.native_evidence_ref)?;
    let report: Value =
        serde_json::from_slice(&report_bytes).map_err(|_| ValidationCacheError::Invalid)?;
    if !cached_report_reusable(&report) {
        return Err(ValidationCacheError::Invalid);
    }
    Ok(ValidationCacheCandidate {
        check_id: entry.check_id.clone(),
        cache_key: entry.cache_key.clone(),
        validation_run: entry.validation_run.clone(),
        validation_run_ref: entry.validation_run_ref.clone(),
        stability: CacheValidationStability::Stable,
        suppression_applied: entry.suppression_applied,
        artifacts_available: true,
        policy_schema_version: entry.policy_schema_version,
        evidence_schema_version: entry.evidence_schema_version,
    })
}

fn cached_report_reusable(report: &Value) -> bool {
    report["schema_id"] == "star.project-validation-report"
        && report["schema_version"] == 1
        && report["status"] == "pass"
        && report["outcome"] == "pass"
        && report["completeness"] == "complete"
        && report["stability"] == "stable"
        && ["failed", "not_run", "partial", "unverified", "flaky"]
            .iter()
            .all(|key| report["summary"][key].as_u64() == Some(0))
}

fn report_time(report: &Value, field: &str) -> Result<DateTime<Utc>, ValidationCacheError> {
    DateTime::parse_from_rfc3339(
        report[field]
            .as_str()
            .ok_or(ValidationCacheError::Invalid)?,
    )
    .map(|value| value.with_timezone(&Utc))
    .map_err(|_| ValidationCacheError::Invalid)
}

fn hash_report_artifacts(
    project_root: &Path,
    report: &Value,
) -> Result<BTreeMap<String, Sha256Hash>, ValidationCacheError> {
    let refs = report["artifact_refs"]
        .as_array()
        .ok_or(ValidationCacheError::Invalid)?;
    let mut hashes = BTreeMap::new();
    for value in refs {
        let artifact_ref = value.as_str().ok_or(ValidationCacheError::Invalid)?;
        let bytes = read_artifact(project_root, artifact_ref)?;
        if hashes
            .insert(artifact_ref.to_owned(), Sha256Hash::digest(&bytes))
            .is_some()
        {
            return Err(ValidationCacheError::Invalid);
        }
    }
    if hashes.is_empty() {
        return Err(ValidationCacheError::Invalid);
    }
    Ok(hashes)
}

fn load_entry(
    project_root: &Path,
    cache_key: &Sha256Hash,
) -> Result<PersistedValidationCacheEntry, ValidationCacheError> {
    let path = cache_entry_path(project_root, cache_key)?;
    let bytes = read_confined_file(project_root, &path, MAX_CACHE_ENTRY_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|_| ValidationCacheError::Invalid)
}

fn write_entry(
    project_root: &Path,
    entry: &PersistedValidationCacheEntry,
) -> Result<(), ValidationCacheError> {
    let path = cache_entry_path(project_root, &entry.cache_key)?;
    let parent = path.parent().ok_or(ValidationCacheError::Write)?;
    fs::create_dir_all(parent).map_err(|_| ValidationCacheError::Write)?;
    let canonical_root = fs::canonicalize(project_root).map_err(|_| ValidationCacheError::Write)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ValidationCacheError::Write)?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(ValidationCacheError::Write);
    }
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        cache_hex(&entry.cache_key)?,
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(entry).map_err(|_| ValidationCacheError::Write)?;
    if bytes.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return Err(ValidationCacheError::Write);
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|_| ValidationCacheError::Write)?;
    let write_result = file.write_all(&bytes).and_then(|_| file.sync_all());
    drop(file);
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
        return Err(ValidationCacheError::Write);
    }
    if path.exists() {
        fs::remove_file(&path).map_err(|_| ValidationCacheError::Write)?;
    }
    fs::rename(&temp, &path).map_err(|_| {
        let _ = fs::remove_file(&temp);
        ValidationCacheError::Write
    })
}

fn cache_entry_path(
    project_root: &Path,
    cache_key: &Sha256Hash,
) -> Result<PathBuf, ValidationCacheError> {
    Ok(project_root
        .join(CACHE_DIRECTORY)
        .join(format!("{}.json", cache_hex(cache_key)?)))
}

fn cache_hex(cache_key: &Sha256Hash) -> Result<&str, ValidationCacheError> {
    cache_key
        .as_str()
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(ValidationCacheError::Invalid)
}

fn read_artifact(project_root: &Path, value: &str) -> Result<Vec<u8>, ValidationCacheError> {
    if !safe_artifact_ref(value) {
        return Err(ValidationCacheError::Invalid);
    }
    let path = project_root.join(value);
    read_confined_file(project_root, &path, MAX_ARTIFACT_BYTES)
}

fn read_confined_file(
    project_root: &Path,
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, ValidationCacheError> {
    let link_metadata =
        fs::symlink_metadata(path).map_err(|_| ValidationCacheError::Unavailable)?;
    if !link_metadata.is_file() || link_metadata.file_type().is_symlink() {
        return Err(ValidationCacheError::Unavailable);
    }
    let canonical_root =
        fs::canonicalize(project_root).map_err(|_| ValidationCacheError::Unavailable)?;
    let canonical_path = fs::canonicalize(path).map_err(|_| ValidationCacheError::Unavailable)?;
    if !canonical_path.starts_with(&canonical_root) || link_metadata.len() > max_bytes {
        return Err(ValidationCacheError::Unavailable);
    }
    fs::read(canonical_path).map_err(|_| ValidationCacheError::Unavailable)
}

fn safe_artifact_ref(value: &str) -> bool {
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() >= 4
        && parts[0] == "target"
        && parts[1] == "validation"
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.contains(':')
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_application::{
        ValidationCheckDefinition, ValidationPlanningInput, build_validation_plan,
    };
    use star_contracts::evidence::{
        ValidationCommand, ValidationInputFingerprintComponents, ValidationProfile,
    };

    fn hash(value: &str) -> Sha256Hash {
        Sha256Hash::digest(value.as_bytes())
    }

    fn test_plan() -> ValidationPlan {
        build_validation_plan(ValidationPlanningInput {
            project_key: "demo".to_owned(),
            revision: "a".repeat(40),
            requested_profile: Some(ValidationProfile::Target),
            requested_unit: None,
            requested_unit_required_profile: None,
            workspace_unit_id: "workspace".to_owned(),
            changed_files: Vec::new(),
            dependencies: Vec::new(),
            checks: vec![ValidationCheckDefinition {
                profile: ValidationProfile::Target,
                check_id: "native-target".to_owned(),
                unit_id: "workspace".to_owned(),
                command: ValidationCommand {
                    executable: "pwsh".to_owned(),
                    args: vec!["scripts/validate.ps1".to_owned()],
                    working_directory: ".".to_owned(),
                    expected_exit_codes: BTreeSet::from([0, 1, 3]),
                },
                selection_reason: "test".to_owned(),
            }],
            cache_candidates: Vec::new(),
            fingerprints: ValidationInputFingerprintComponents {
                revision: "a".repeat(40),
                staged_diff: hash("staged"),
                unstaged_diff: hash("unstaged"),
                untracked_content: hash("untracked"),
                toolchain: hash("toolchain"),
                lockfile: hash("lockfile"),
                project_manifest: hash("manifest"),
                validation_scripts: hash("scripts"),
                config: hash("config"),
                policy_schema_version: VALIDATION_POLICY_SCHEMA_VERSION,
                evidence_schema_version: EVIDENCE_CONTRACT_SCHEMA_VERSION,
            },
            fingerprints_complete: true,
            impact_complete: true,
            repeated_failures: false,
        })
        .unwrap()
    }

    #[test]
    fn cache_paths_are_confined_to_the_validation_artifact_root() {
        assert!(safe_artifact_ref("target/validation/run-1/logs/check.log"));
        assert!(!safe_artifact_ref("target/validation/../report.json"));
        assert!(!safe_artifact_ref("C:/target/validation/run/report.json"));
        assert!(!safe_artifact_ref("target\\validation\\run\\report.json"));
    }

    #[test]
    fn only_complete_stable_pass_reports_are_reusable() {
        let mut report = serde_json::json!({
            "schema_id":"star.project-validation-report",
            "schema_version":1,
            "status":"pass",
            "outcome":"pass",
            "completeness":"complete",
            "stability":"stable",
            "summary":{"failed":0,"not_run":0,"partial":0,"unverified":0,"flaky":0}
        });
        assert!(cached_report_reusable(&report));
        for status in ["partial", "unverified", "flaky", "fail", "not_run"] {
            report["status"] = Value::String(status.to_owned());
            assert!(!cached_report_reusable(&report));
        }
    }

    #[test]
    fn persisted_success_is_reused_only_while_every_artifact_exists() {
        let root = std::env::temp_dir().join(format!(
            "star-validation-cache-test-{}-{}",
            std::process::id(),
            TaskInvocationId::new()
        ));
        let artifact_root = root.join("target/validation/run-1");
        fs::create_dir_all(artifact_root.join("logs")).unwrap();
        fs::write(artifact_root.join("paths.json"), b"[]").unwrap();
        fs::write(artifact_root.join("logs/native.log"), b"ok").unwrap();
        let evidence_ref = "target/validation/run-1/report.json";
        let report = serde_json::json!({
            "schema_id":"star.project-validation-report",
            "schema_version":1,
            "project_id":"demo",
            "requested_profile":"target",
            "required_profile":"target",
            "effective_profile":"target",
            "status":"pass",
            "outcome":"pass",
            "completeness":"complete",
            "stability":"stable",
            "started_at":"2026-07-17T00:00:00Z",
            "finished_at":"2026-07-17T00:00:01Z",
            "input_fingerprint":"0".repeat(64),
            "environment":{"platform":"windows-x64"},
            "impact":{"changed_paths":[]},
            "summary":{"failed":0,"not_run":0,"partial":0,"unverified":0,"flaky":0},
            "artifact_refs":[
                evidence_ref,
                "target/validation/run-1/paths.json",
                "target/validation/run-1/logs/native.log"
            ]
        });
        fs::write(
            artifact_root.join("report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        let plan = test_plan();
        store_successful_validation_cache(&root, "demo", &plan, evidence_ref, &report, 1_000)
            .unwrap();
        assert_eq!(
            load_validation_cache_candidates(&root, "demo", &plan.checks).len(),
            1
        );
        fs::write(artifact_root.join("logs/native.log"), b"tampered").unwrap();
        assert!(load_validation_cache_candidates(&root, "demo", &plan.checks).is_empty());
        fs::write(artifact_root.join("logs/native.log"), b"ok").unwrap();
        assert_eq!(
            load_validation_cache_candidates(&root, "demo", &plan.checks).len(),
            1
        );
        fs::remove_file(artifact_root.join("logs/native.log")).unwrap();
        assert!(load_validation_cache_candidates(&root, "demo", &plan.checks).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
