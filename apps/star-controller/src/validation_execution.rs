//! Bounded native validation execution and evidence retrieval precursor.
//!
//! This module intentionally delegates check selection and reporting to each
//! project's tracked `scripts/validate.ps1`. It does not claim to implement the
//! persisted M3 runner, authoritative GateDecision, or EvidenceBundle writer.
//! Complete stable pass evidence may be reused through the project-local
//! derived cache owned by `validation_cache`.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use serde_json::Value;
use star_contracts::{
    Sha256Hash,
    evidence::{PlannedCheckDisposition, ValidationPlan, ValidationProfile},
};
use star_controller::process_runtime::{
    DirectExeSpec, RuntimeCancellation, RuntimeError,
    execute_trusted_internal_powershell_cancellable,
};
use star_project::catalog::ProjectCatalogManifest;
use thiserror::Error;

#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, REG_VALUE_TYPE,
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
        },
    },
    core::PCWSTR,
};

use crate::validation_cache::{read_cached_validation_evidence, store_successful_validation_cache};
use crate::validation_planning::{
    ValidationPlanningObservationError, build_project_validation_plan,
    resolve_project_validation_target,
};

const DEFAULT_TIMEOUT_MS: u64 = 3_600_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 3_600_000;
const MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STREAM_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ValidationExecutionError {
    #[error(transparent)]
    Planning(#[from] ValidationPlanningObservationError),
    #[error("the requested validation timeout is outside the supported range")]
    TimeoutArgument,
    #[error("PowerShell 7 could not be resolved to an absolute executable")]
    PowerShellUnavailable,
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("the native validator did not return a recognized result code")]
    ExitCode,
    #[error("the native validator output is not a valid project validation report")]
    ReportInvalid,
    #[error("the native validator report and the sealed plan disagree")]
    PlanMismatch,
    #[error("the validation evidence reference is outside the project evidence root")]
    EvidenceBoundary,
    #[error("the validation evidence file is unavailable or exceeds its size limit")]
    EvidenceUnavailable,
}

pub async fn run_project_validation(
    catalog: &ProjectCatalogManifest,
    catalog_root: &Path,
    project_key: &str,
    requested_profile: Option<ValidationProfile>,
    requested_unit: Option<String>,
    timeout_ms: Option<u64>,
    cancellation: Option<RuntimeCancellation>,
) -> Result<Value, ValidationExecutionError> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(ValidationExecutionError::TimeoutArgument);
    }
    let mut plan = build_project_validation_plan(
        catalog,
        catalog_root,
        project_key,
        requested_profile,
        requested_unit.clone(),
    )?;
    let target = resolve_project_validation_target(catalog, catalog_root, project_key)?;
    let requested_profile = requested_profile.unwrap_or(ValidationProfile::Target);
    let mut refreshed_plan = None;
    if let Some(check) = plan.checks.first()
        && plan.checks.len() == 1
        && check.disposition == PlannedCheckDisposition::Reuse
        && let Some(source) = check.source_validation_run_ref.as_ref()
        && let Ok(cached) =
            read_cached_validation_evidence(&target.project_root, project_key, check, source)
    {
        let cached_report_is_bound = validate_report(&cached.report, project_key, Some(0))
            .is_ok_and(|evidence_ref| evidence_ref == cached.evidence_ref)
            && validate_plan_equivalence(
                &plan,
                requested_profile,
                requested_unit.as_deref(),
                &cached.report,
            )
            .is_ok();
        if cached_report_is_bound {
            let confirmed_plan = build_project_validation_plan(
                catalog,
                catalog_root,
                project_key,
                Some(requested_profile),
                requested_unit.clone(),
            )?;
            if same_cache_binding(&plan, &confirmed_plan) {
                return Ok(serde_json::json!({
                    "schema_id":"star.validation-run-view",
                    "schema_version":1,
                    "project_key":project_key,
                    "validation_plan_id":plan.validation_plan_id,
                    "plan_fingerprint":plan.plan_fingerprint,
                    "plan_readiness":plan.readiness,
                    "requested_profile":requested_profile,
                    "selected_profile":plan.profile.selected,
                    "process_exit_code":0,
                    "evidence_ref":cached.evidence_ref,
                    "evidence_sha256":Sha256Hash::digest(&cached.evidence_bytes),
                    "cache":{
                        "hit":true,
                        "stored":false,
                        "cache_key":check.cache_key,
                        "source_validation_run_ref":cached.source_validation_run_ref,
                    },
                    "report":cached.report,
                }));
            }
            refreshed_plan = Some(confirmed_plan);
        }
    }
    if let Some(confirmed_plan) = refreshed_plan {
        plan = confirmed_plan;
    }
    let pwsh = resolve_pwsh_executable().ok_or(ValidationExecutionError::PowerShellUnavailable)?;
    let mut argv = vec![
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-File"),
        powershell_script_argument(&target.validation_entrypoint),
        OsString::from("-Profile"),
        OsString::from(profile_name(requested_profile)),
        OsString::from("-OutputFormat"),
        OsString::from("json"),
    ];
    if let Some(unit) = requested_unit.as_ref() {
        argv.push(OsString::from("-Unit"));
        argv.push(OsString::from(unit));
    }
    let outcome = execute_trusted_internal_powershell_cancellable(
        &DirectExeSpec {
            executable: pwsh,
            argv,
            working_directory: target.project_root.clone(),
            environment: Vec::new(),
            stdin: None,
            timeout: Duration::from_millis(timeout_ms),
            max_stdout_bytes: MAX_STREAM_BYTES,
            max_stderr_bytes: MAX_STREAM_BYTES,
            max_memory_bytes: None,
            max_processes: 512,
            appcontainer_profile: None,
        },
        cancellation,
    )
    .await?;
    let exit_code = outcome
        .exit_code
        .ok_or(ValidationExecutionError::ExitCode)?;
    if !matches!(exit_code, 0 | 1 | 3) {
        return Err(ValidationExecutionError::ExitCode);
    }
    let report: Value = serde_json::from_slice(&outcome.stdout.captured)
        .map_err(|_| ValidationExecutionError::ReportInvalid)?;
    let evidence_ref = validate_report(&report, project_key, Some(exit_code))?;
    validate_plan_equivalence(&plan, requested_profile, requested_unit.as_deref(), &report)?;
    let (evidence_bytes, evidence_report) =
        read_evidence_report(&target.project_root, &evidence_ref)?;
    if evidence_report != report {
        return Err(ValidationExecutionError::ReportInvalid);
    }
    let check = plan
        .checks
        .first()
        .ok_or(ValidationExecutionError::PlanMismatch)?;
    let confirmed_plan = build_project_validation_plan(
        catalog,
        catalog_root,
        project_key,
        Some(requested_profile),
        requested_unit,
    )?;
    let stored = same_cache_binding(&plan, &confirmed_plan)
        && store_successful_validation_cache(
            &target.project_root,
            project_key,
            &plan,
            &evidence_ref,
            &report,
            timeout_ms,
        )
        .is_ok();

    Ok(serde_json::json!({
        "schema_id":"star.validation-run-view",
        "schema_version":1,
        "project_key":project_key,
        "validation_plan_id":plan.validation_plan_id,
        "plan_fingerprint":plan.plan_fingerprint,
        "plan_readiness":plan.readiness,
        "requested_profile":requested_profile,
        "selected_profile":plan.profile.selected,
        "process_exit_code":exit_code,
        "evidence_ref":evidence_ref,
        "evidence_sha256":Sha256Hash::digest(&evidence_bytes),
        "cache":{
            "hit":false,
            "stored":stored,
            "cache_key":check.cache_key,
            "source_validation_run_ref":Value::Null,
        },
        "report":report,
    }))
}

fn same_cache_binding(current: &ValidationPlan, confirmed: &ValidationPlan) -> bool {
    let Some(current_check) = current.checks.first() else {
        return false;
    };
    let Some(confirmed_check) = confirmed.checks.first() else {
        return false;
    };
    current.checks.len() == 1
        && confirmed.checks.len() == 1
        && current_check.check_id == confirmed_check.check_id
        && current_check.cache_key == confirmed_check.cache_key
}

pub fn read_project_validation_evidence(
    catalog: &ProjectCatalogManifest,
    catalog_root: &Path,
    project_key: &str,
    evidence_ref: &str,
) -> Result<Value, ValidationExecutionError> {
    let target = resolve_project_validation_target(catalog, catalog_root, project_key)?;
    if !safe_evidence_ref(evidence_ref) {
        return Err(ValidationExecutionError::EvidenceBoundary);
    }
    let (bytes, report) = read_evidence_report(&target.project_root, evidence_ref)?;
    let observed_ref = validate_report(&report, project_key, None)?;
    if observed_ref != evidence_ref {
        return Err(ValidationExecutionError::ReportInvalid);
    }
    Ok(serde_json::json!({
        "schema_id":"star.validation-evidence-view",
        "schema_version":1,
        "project_key":project_key,
        "evidence_ref":evidence_ref,
        "evidence_sha256":Sha256Hash::digest(&bytes),
        "report":report,
    }))
}

fn validate_plan_equivalence(
    plan: &ValidationPlan,
    requested_profile: ValidationProfile,
    requested_unit: Option<&str>,
    report: &Value,
) -> Result<(), ValidationExecutionError> {
    if report["requested_profile"].as_str() != Some(profile_name(requested_profile))
        || report["required_profile"].as_str() != Some(profile_name(plan.profile.required))
        || report["effective_profile"].as_str() != Some(profile_name(plan.profile.selected))
    {
        return Err(ValidationExecutionError::PlanMismatch);
    }
    match requested_unit {
        Some(unit) if report["unit"].as_str() != Some(unit) => {
            return Err(ValidationExecutionError::PlanMismatch);
        }
        None if !report["unit"].is_null() => {
            return Err(ValidationExecutionError::PlanMismatch);
        }
        _ => {}
    }
    let planned: BTreeSet<_> = plan
        .changed_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let observed: BTreeSet<_> = report["impact"]["changed_paths"]
        .as_array()
        .ok_or(ValidationExecutionError::ReportInvalid)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(ValidationExecutionError::ReportInvalid)
        })
        .collect::<Result<_, _>>()?;
    if planned != observed {
        return Err(ValidationExecutionError::PlanMismatch);
    }
    Ok(())
}

fn validate_report(
    report: &Value,
    project_key: &str,
    process_exit_code: Option<u32>,
) -> Result<String, ValidationExecutionError> {
    if report["schema_id"].as_str() != Some("star.project-validation-report")
        || report["schema_version"].as_u64() != Some(1)
        || report["project_id"].as_str() != Some(project_key)
    {
        return Err(ValidationExecutionError::ReportInvalid);
    }
    let status = one_of(
        &report["status"],
        &["pass", "fail", "not_run", "partial", "unverified", "flaky"],
    )?;
    let outcome = one_of(
        &report["outcome"],
        &["pass", "fail", "not_run", "error", "cancelled"],
    )?;
    let completeness = one_of(
        &report["completeness"],
        &["complete", "partial", "unverified"],
    )?;
    let stability = one_of(&report["stability"], &["stable", "flaky", "not_evaluated"])?;
    let fingerprint = report["input_fingerprint"]
        .as_str()
        .ok_or(ValidationExecutionError::ReportInvalid)?;
    if fingerprint.len() != 64
        || !fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ValidationExecutionError::ReportInvalid);
    }
    let summary = report["summary"]
        .as_object()
        .ok_or(ValidationExecutionError::ReportInvalid)?;
    let count = |name: &str| {
        summary
            .get(name)
            .and_then(Value::as_u64)
            .ok_or(ValidationExecutionError::ReportInvalid)
    };
    let failed = count("failed")?;
    let not_run = count("not_run")?;
    let partial = count("partial")?;
    let unverified = count("unverified")?;
    let flaky = count("flaky")?;
    if status == "pass"
        && (outcome != "pass"
            || completeness != "complete"
            || stability != "stable"
            || failed != 0
            || not_run != 0
            || partial != 0
            || unverified != 0
            || flaky != 0)
    {
        return Err(ValidationExecutionError::ReportInvalid);
    }
    if let Some(exit_code) = process_exit_code {
        let expected = match status {
            "pass" => 0,
            "fail" => 1,
            _ => 3,
        };
        if exit_code != expected {
            return Err(ValidationExecutionError::ReportInvalid);
        }
    }
    let refs = report["artifact_refs"]
        .as_array()
        .ok_or(ValidationExecutionError::ReportInvalid)?;
    let mut evidence_ref = None;
    for value in refs {
        let relative = value
            .as_str()
            .ok_or(ValidationExecutionError::ReportInvalid)?;
        if !safe_project_relative_path(relative) {
            return Err(ValidationExecutionError::ReportInvalid);
        }
        if safe_evidence_ref(relative) && evidence_ref.replace(relative.to_owned()).is_some() {
            return Err(ValidationExecutionError::ReportInvalid);
        }
    }
    evidence_ref.ok_or(ValidationExecutionError::ReportInvalid)
}

fn one_of<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a str, ValidationExecutionError> {
    let value = value
        .as_str()
        .ok_or(ValidationExecutionError::ReportInvalid)?;
    allowed
        .contains(&value)
        .then_some(value)
        .ok_or(ValidationExecutionError::ReportInvalid)
}

fn read_evidence_report(
    project_root: &Path,
    evidence_ref: &str,
) -> Result<(Vec<u8>, Value), ValidationExecutionError> {
    if !safe_evidence_ref(evidence_ref) {
        return Err(ValidationExecutionError::EvidenceBoundary);
    }
    let canonical_root = fs::canonicalize(project_root)
        .map_err(|_| ValidationExecutionError::EvidenceUnavailable)?;
    let evidence_root = fs::canonicalize(project_root.join("target/validation"))
        .map_err(|_| ValidationExecutionError::EvidenceUnavailable)?;
    if !evidence_root.starts_with(&canonical_root) {
        return Err(ValidationExecutionError::EvidenceBoundary);
    }
    let candidate = project_root.join(evidence_ref);
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|_| ValidationExecutionError::EvidenceUnavailable)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_REPORT_BYTES
    {
        return Err(ValidationExecutionError::EvidenceUnavailable);
    }
    let canonical =
        fs::canonicalize(candidate).map_err(|_| ValidationExecutionError::EvidenceUnavailable)?;
    if !canonical.starts_with(&evidence_root) {
        return Err(ValidationExecutionError::EvidenceBoundary);
    }
    let mut bytes = Vec::new();
    fs::File::open(canonical)
        .map_err(|_| ValidationExecutionError::EvidenceUnavailable)?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ValidationExecutionError::EvidenceUnavailable)?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(ValidationExecutionError::EvidenceUnavailable);
    }
    let report =
        serde_json::from_slice(&bytes).map_err(|_| ValidationExecutionError::ReportInvalid)?;
    Ok((bytes, report))
}

fn resolve_path_executable(name: &str) -> Option<PathBuf> {
    resolve_path_executable_from(std::env::var_os("PATH"), name)
}

fn resolve_path_executable_from(path: Option<OsString>, name: &str) -> Option<PathBuf> {
    path.into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
}

fn resolve_pwsh_executable() -> Option<PathBuf> {
    resolve_path_executable("pwsh.exe").or_else(resolve_registered_pwsh)
}

#[cfg(windows)]
fn resolve_registered_pwsh() -> Option<PathBuf> {
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(read_registered_pwsh)
        .filter(|path| path.is_absolute() && path.is_file())
        .and_then(|path| fs::canonicalize(path).ok())
}

#[cfg(not(windows))]
fn resolve_registered_pwsh() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn read_registered_pwsh(root: HKEY) -> Option<PathBuf> {
    let subkey =
        std::ffi::OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\App Paths\pwsh.exe")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
    let mut key = HKEY::default();
    if unsafe { RegOpenKeyExW(root, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut key) }
        != ERROR_SUCCESS
    {
        return None;
    }
    let result = read_registry_default_string(key);
    unsafe {
        let _ = RegCloseKey(key);
    }
    result.map(PathBuf::from)
}

#[cfg(windows)]
fn read_registry_default_string(key: HKEY) -> Option<String> {
    const MAX_REGISTRY_VALUE_BYTES: u32 = 32 * 1024;
    let mut kind = REG_VALUE_TYPE(0);
    let mut bytes = 0_u32;
    if unsafe {
        RegQueryValueExW(
            key,
            PCWSTR::null(),
            None,
            Some(&mut kind),
            None,
            Some(&mut bytes),
        )
    } != ERROR_SUCCESS
        || kind != REG_SZ
        || bytes == 0
        || bytes > MAX_REGISTRY_VALUE_BYTES
        || !bytes.is_multiple_of(2)
    {
        return None;
    }
    let mut raw = vec![0_u8; bytes as usize];
    if unsafe {
        RegQueryValueExW(
            key,
            PCWSTR::null(),
            None,
            Some(&mut kind),
            Some(raw.as_mut_ptr()),
            Some(&mut bytes),
        )
    } != ERROR_SUCCESS
        || kind != REG_SZ
    {
        return None;
    }
    let words = raw
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&words)
        .ok()
        .filter(|value| !value.is_empty())
}

fn safe_evidence_ref(value: &str) -> bool {
    let parts: Vec<_> = value.split('/').collect();
    parts.len() == 4
        && parts[0] == "target"
        && parts[1] == "validation"
        && !parts[2].is_empty()
        && parts[3] == "report.json"
        && safe_project_relative_path(value)
}

fn safe_project_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.contains(':')
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn profile_name(profile: ValidationProfile) -> &'static str {
    match profile {
        ValidationProfile::Quick => "quick",
        ValidationProfile::Target => "target",
        ValidationProfile::Full => "full",
        ValidationProfile::Release => "release",
    }
}

#[cfg(windows)]
fn powershell_script_argument(path: &Path) -> OsString {
    const VERBATIM_PREFIX: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const VERBATIM_UNC_PREFIX: &[u16] = &[
        b'\\' as u16,
        b'\\' as u16,
        b'?' as u16,
        b'\\' as u16,
        b'U' as u16,
        b'N' as u16,
        b'C' as u16,
        b'\\' as u16,
    ];
    let words = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if let Some(rest) = words.strip_prefix(VERBATIM_UNC_PREFIX) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(rest);
        return OsString::from_wide(&normalized);
    }
    if let Some(rest) = words.strip_prefix(VERBATIM_PREFIX)
        && rest.len() >= 3
        && matches!(rest[0], 0x41..=0x5a | 0x61..=0x7a)
        && rest[1] == b':' as u16
        && rest[2] == b'\\' as u16
    {
        return OsString::from_wide(rest);
    }
    path.as_os_str().to_owned()
}

#[cfg(not(windows))]
fn powershell_script_argument(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_application::{
        ValidationCheckDefinition, ValidationPlanningInput, build_validation_plan,
    };
    use star_contracts::evidence::{
        EVIDENCE_CONTRACT_SCHEMA_VERSION, VALIDATION_POLICY_SCHEMA_VERSION, ValidationCommand,
        ValidationInputFingerprintComponents,
    };

    fn cache_binding_plan(seed: &str) -> ValidationPlan {
        let hash = |value: &str| Sha256Hash::digest(value.as_bytes());
        build_validation_plan(ValidationPlanningInput {
            project_key: "star-control".to_owned(),
            revision: "a".repeat(40),
            requested_profile: Some(ValidationProfile::Target),
            requested_unit: None,
            requested_unit_required_profile: None,
            workspace_unit_id: "workspace".to_owned(),
            changed_files: Vec::new(),
            dependencies: Vec::new(),
            checks: vec![ValidationCheckDefinition {
                profile: ValidationProfile::Target,
                check_id: "native-validation".to_owned(),
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
                staged_diff: hash(seed),
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

    fn report(status: &str, partial: u64, unverified: u64, flaky: u64) -> Value {
        serde_json::json!({
            "schema_id":"star.project-validation-report",
            "schema_version":1,
            "project_id":"star-control",
            "requested_profile":"target",
            "required_profile":"target",
            "effective_profile":"target",
            "status":status,
            "outcome":if status == "pass" { "pass" } else { "fail" },
            "completeness":if status == "pass" { "complete" } else { "partial" },
            "stability":if status == "pass" { "stable" } else { "not_evaluated" },
            "input_fingerprint":"0".repeat(64),
            "summary":{
                "failed":if status == "fail" { 1 } else { 0 },
                "not_run":0,
                "partial":partial,
                "unverified":unverified,
                "flaky":flaky
            },
            "artifact_refs":["target/validation/run-1/report.json"]
        })
    }

    #[test]
    fn complete_pass_and_evidence_path_are_accepted() {
        assert_eq!(
            validate_report(&report("pass", 0, 0, 0), "star-control", Some(0)).unwrap(),
            "target/validation/run-1/report.json"
        );
        assert!(safe_evidence_ref("target/validation/run-1/report.json"));
        assert!(!safe_evidence_ref("target/validation/../report.json"));
    }

    #[test]
    fn partial_unverified_or_flaky_counts_cannot_be_reported_as_pass() {
        for report in [
            report("pass", 1, 0, 0),
            report("pass", 0, 1, 0),
            report("pass", 0, 0, 1),
        ] {
            assert!(matches!(
                validate_report(&report, "star-control", Some(0)),
                Err(ValidationExecutionError::ReportInvalid)
            ));
        }
    }

    #[test]
    fn cache_binding_requires_one_identical_post_observation_key() {
        let current = cache_binding_plan("staged");
        let identical = cache_binding_plan("staged");
        let changed = cache_binding_plan("changed-staged");
        assert!(same_cache_binding(&current, &identical));
        assert!(!same_cache_binding(&current, &changed));

        let mut multiple = identical;
        multiple.checks.push(multiple.checks[0].clone());
        assert!(!same_cache_binding(&current, &multiple));
    }

    #[test]
    fn native_report_must_match_required_profile_and_explicit_unit() {
        let plan = cache_binding_plan("staged");
        let mut observed = serde_json::json!({
            "requested_profile":"target",
            "required_profile":"target",
            "effective_profile":"target",
            "unit":null,
            "impact":{"changed_paths":[]}
        });
        assert!(
            validate_plan_equivalence(&plan, ValidationProfile::Target, None, &observed).is_ok()
        );

        observed["required_profile"] = Value::String("quick".to_owned());
        assert!(matches!(
            validate_plan_equivalence(&plan, ValidationProfile::Target, None, &observed),
            Err(ValidationExecutionError::PlanMismatch)
        ));

        observed["required_profile"] = Value::String("target".to_owned());
        observed["unit"] = Value::String("docs".to_owned());
        assert!(
            validate_plan_equivalence(&plan, ValidationProfile::Target, Some("docs"), &observed)
                .is_ok()
        );
        assert!(matches!(
            validate_plan_equivalence(&plan, ValidationProfile::Target, None, &observed),
            Err(ValidationExecutionError::PlanMismatch)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn powershell_script_argument_removes_only_supported_verbatim_prefixes() {
        assert_eq!(
            powershell_script_argument(Path::new(
                r"\\?\D:\개발\관제\Star-Control\scripts\validate.ps1"
            )),
            OsString::from(r"D:\개발\관제\Star-Control\scripts\validate.ps1")
        );
        assert_eq!(
            powershell_script_argument(Path::new(r"\\?\UNC\server\share\scripts\validate.ps1")),
            OsString::from(r"\\server\share\scripts\validate.ps1")
        );
        assert_eq!(
            powershell_script_argument(Path::new(r"D:\scripts\validate.ps1")),
            OsString::from(r"D:\scripts\validate.ps1")
        );
        assert_eq!(
            powershell_script_argument(Path::new(r"\\?\Volume{test}\validate.ps1")),
            OsString::from(r"\\?\Volume{test}\validate.ps1")
        );
        assert_eq!(
            powershell_script_argument(Path::new(r"\\?\ń:\validate.ps1")),
            OsString::from(r"\\?\ń:\validate.ps1")
        );
    }

    #[cfg(windows)]
    #[test]
    fn registered_pwsh_is_resolved_when_path_is_unavailable() {
        let registered =
            resolve_registered_pwsh().expect("PowerShell App Paths entry is available");
        assert!(registered.is_absolute());
        assert!(registered.is_file());
        assert!(resolve_path_executable_from(None, "pwsh.exe").is_none());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn resolved_pwsh_runs_through_trusted_internal_runtime() {
        let pwsh = resolve_pwsh_executable().expect("PowerShell 7 executable resolves");
        let outcome = execute_trusted_internal_powershell_cancellable(
            &DirectExeSpec {
                executable: pwsh,
                argv: vec![
                    OsString::from("-NoLogo"),
                    OsString::from("-NoProfile"),
                    OsString::from("-NonInteractive"),
                    OsString::from("-Command"),
                    OsString::from("[Console]::Out.Write('pwsh-runtime-ok')"),
                ],
                working_directory: std::env::current_dir().expect("current directory resolves"),
                environment: Vec::new(),
                stdin: None,
                timeout: Duration::from_secs(30),
                max_stdout_bytes: 4 * 1024,
                max_stderr_bytes: 4 * 1024,
                max_memory_bytes: None,
                max_processes: 8,
                appcontainer_profile: None,
            },
            None,
        )
        .await
        .expect("PowerShell 7 runs through the trusted internal launcher");

        assert_eq!(outcome.exit_code, Some(0));
        assert_eq!(outcome.stdout.captured, b"pwsh-runtime-ok");
        assert!(outcome.stderr.captured.is_empty());
    }
}
