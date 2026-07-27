//! Registered, typed process execution for M3 validation checks.
//!
//! The executor never accepts a shell command string. A caller must resolve an
//! absolute executable and project root up front; the persisted invocation only
//! carries the logical executable and typed argv.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ArtifactRef, Completeness, DiagnosticConfidence, DiagnosticSeverity, DiagnosticStatus,
        ObservedTool, TerminationReason,
    },
    evidence_v2::{InvocationWorkingDirectoryV2, TaskInvocationV2, ValidationStabilityV2},
    planning::CheckOutputNormalizer,
};
use thiserror::Error;

use crate::runner::{CheckExecutionObservation, CheckExecutor, CheckExecutorError, RawDiagnostic};

const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_INVOCATION_TIMEOUT_MS: u64 = 86_400_000;
const MAX_INVOCATION_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXECUTION_ENVIRONMENT_BYTES: usize = 256 * 1024;
const EXECUTION_ENVIRONMENT_ALLOWLIST: &[&str] = &[
    "APPDATA",
    "CARGO_HOME",
    "CARGO_INCREMENTAL",
    "CARGO_TARGET_DIR",
    "CC",
    "CXX",
    "HOME",
    "INCLUDE",
    "JAVA_HOME",
    "LIB",
    "LIBPATH",
    "LOCALAPPDATA",
    "NODE_PATH",
    "NPM_CONFIG_CACHE",
    "PATH",
    "PATHEXT",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTDOC",
    "RUSTFLAGS",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SystemDrive",
    "SystemRoot",
    "TEMP",
    "TMP",
    "TMPDIR",
    "USERPROFILE",
    "WINDIR",
];
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const OUTPUT_DRAIN_GRACE: Duration = Duration::from_secs(5);
const TERMINATED_OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(250);
type ExecutionEnvironment = BTreeMap<String, OsString>;
type BoundedExecutionEnvironment = (ExecutionEnvironment, Sha256Hash);

#[derive(Clone, Debug)]
pub struct ResolvedExecutableV2 {
    pub logical_executable: String,
    pub absolute_path: PathBuf,
    pub project_root: PathBuf,
    pub executable_binding_fingerprint: Sha256Hash,
    pub execution_environment_fingerprint: Sha256Hash,
    pub observed_tool: ObservedTool,
    execution_environment: ExecutionEnvironment,
}

#[derive(Debug, Error)]
pub enum ProcessExecutorError {
    #[error("resolved executable identity is invalid")]
    Executable,
    #[error("project execution root is invalid")]
    ProjectRoot,
    #[error("executable bytes exceed the bounded identity limit")]
    ExecutableTooLarge,
    #[error("executable identity could not be calculated")]
    Fingerprint,
    #[error("bounded execution environment is invalid")]
    Environment,
}

impl ResolvedExecutableV2 {
    pub fn resolve(
        logical_executable: &str,
        absolute_path: &Path,
        project_root: &Path,
        version: &str,
    ) -> Result<Self, ProcessExecutorError> {
        if logical_executable.trim().is_empty()
            || logical_executable.contains(['/', '\\', ':', '\0'])
            || version.trim().is_empty()
            || !absolute_path.is_absolute()
            || !project_root.is_absolute()
        {
            return Err(ProcessExecutorError::Executable);
        }
        let executable_metadata =
            fs::symlink_metadata(absolute_path).map_err(|_| ProcessExecutorError::Executable)?;
        if !executable_metadata.is_file() || executable_metadata.file_type().is_symlink() {
            return Err(ProcessExecutorError::Executable);
        }
        if executable_metadata.len() > MAX_EXECUTABLE_BYTES {
            return Err(ProcessExecutorError::ExecutableTooLarge);
        }
        let root_metadata =
            fs::symlink_metadata(project_root).map_err(|_| ProcessExecutorError::ProjectRoot)?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err(ProcessExecutorError::ProjectRoot);
        }
        let executable =
            fs::canonicalize(absolute_path).map_err(|_| ProcessExecutorError::Executable)?;
        let root = fs::canonicalize(project_root).map_err(|_| ProcessExecutorError::ProjectRoot)?;
        let bytes = fs::read(&executable).map_err(|_| ProcessExecutorError::Executable)?;
        let executable_sha256 = Sha256Hash::digest(&bytes);
        let opaque_locator = Sha256Hash::digest(
            executable
                .as_os_str()
                .to_string_lossy()
                .replace('/', "\\")
                .to_ascii_lowercase()
                .as_bytes(),
        );
        let binding = canonical_sha256(&serde_json::json!({
            "domain":"star.executable-binding",
            "version":2,
            "logical_executable":logical_executable,
            "executable_sha256":executable_sha256,
            "opaque_locator":opaque_locator,
        }))
        .map_err(|_| ProcessExecutorError::Fingerprint)?;
        let (execution_environment, execution_environment_fingerprint) =
            bounded_execution_environment()?;
        Ok(Self {
            logical_executable: logical_executable.to_owned(),
            absolute_path: executable,
            project_root: root,
            executable_binding_fingerprint: binding,
            execution_environment_fingerprint,
            observed_tool: ObservedTool {
                executable_path: format!("registered://{}", opaque_locator.as_str()),
                version: version.to_owned(),
                sha256: executable_sha256,
            },
            execution_environment,
        })
    }
}

fn bounded_execution_environment() -> Result<BoundedExecutionEnvironment, ProcessExecutorError> {
    let mut environment = BTreeMap::new();
    let mut fingerprint_entries = BTreeMap::new();
    let mut total_bytes = 0_usize;
    for name in EXECUTION_ENVIRONMENT_ALLOWLIST {
        let Some(value) = std::env::var_os(name) else {
            continue;
        };
        let exact_bytes = environment_value_bytes(&value);
        total_bytes = total_bytes
            .checked_add(name.len())
            .and_then(|total| total.checked_add(exact_bytes.len()))
            .ok_or(ProcessExecutorError::Environment)?;
        if total_bytes > MAX_EXECUTION_ENVIRONMENT_BYTES
            || exact_bytes
                .chunks_exact(environment_code_unit_bytes())
                .any(|unit| unit.iter().all(|byte| *byte == 0))
        {
            return Err(ProcessExecutorError::Environment);
        }
        environment.insert((*name).to_owned(), value);
        fingerprint_entries.insert((*name).to_owned(), Sha256Hash::digest(&exact_bytes));
    }
    let fingerprint = canonical_sha256(&serde_json::json!({
        "domain":"star.registered-process-environment",
        "version":1,
        "variables":fingerprint_entries,
    }))
    .map_err(|_| ProcessExecutorError::Fingerprint)?;
    Ok((environment, fingerprint))
}

#[cfg(windows)]
fn environment_value_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    value
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
}

#[cfg(not(windows))]
fn environment_value_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    value.as_bytes().to_vec()
}

const fn environment_code_unit_bytes() -> usize {
    if cfg!(windows) { 2 } else { 1 }
}

#[derive(Clone, Debug)]
pub struct NormalizerInput<'a> {
    pub executable_binding_fingerprint: &'a Sha256Hash,
    pub exit_code: Option<i32>,
    pub expected_exit: bool,
    pub termination_reason: TerminationReason,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub output_read_failed: bool,
}

#[derive(Clone, Debug)]
pub struct NormalizerOutput {
    pub diagnostics: Vec<RawDiagnostic>,
    pub completeness: Option<Completeness>,
    pub sarif: Option<crate::sarif::SarifNormalization>,
}

pub trait ExternalDiagnosticNormalizer: Send {
    fn normalize(&mut self, input: NormalizerInput<'_>) -> NormalizerOutput;
}

pub struct CheckOutputArtifactInput<'a> {
    pub invocation: &'a TaskInvocationV2,
    pub exit_code: Option<i32>,
    pub termination_reason: TerminationReason,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub output_read_failed: bool,
    pub static_analysis: Option<StaticAnalysisArtifactInput<'a>>,
}

pub struct StaticAnalysisArtifactInput<'a> {
    pub candidates: &'a [crate::sarif::SarifFindingCandidate],
    pub imported_count: usize,
    pub rejected_count: usize,
    pub completeness: crate::sarif::SarifCompleteness,
}

#[derive(Clone, Debug, Error)]
#[error("check output artifact persistence failed: {code}")]
pub struct CheckOutputArtifactError {
    pub code: String,
}

pub trait CheckOutputArtifactSink: Send {
    fn persist(
        &mut self,
        input: CheckOutputArtifactInput<'_>,
    ) -> Result<Vec<ArtifactRef>, CheckOutputArtifactError>;
}

#[derive(Default)]
pub struct SafeExitDiagnosticNormalizer;

impl ExternalDiagnosticNormalizer for SafeExitDiagnosticNormalizer {
    fn normalize(&mut self, input: NormalizerInput<'_>) -> NormalizerOutput {
        let mut diagnostics = Vec::new();
        if input.stdout_truncated || input.stderr_truncated {
            diagnostics.push(RawDiagnostic {
                code: "CHECK_OUTPUT_LIMIT_EXCEEDED".to_owned(),
                title: "Check output exceeded its declared limit".to_owned(),
                message: "Output was drained but only bounded bytes were retained; the result is partial."
                    .to_owned(),
                severity: DiagnosticSeverity::Error,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            });
        }
        if input.output_read_failed {
            diagnostics.push(RawDiagnostic {
                code: "CHECK_OUTPUT_READ_FAILED".to_owned(),
                title: "Check output could not be read completely".to_owned(),
                message:
                    "The process pipe could not be drained completely; the result is unverified."
                        .to_owned(),
                severity: DiagnosticSeverity::Error,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            });
        }
        match input.termination_reason {
            TerminationReason::Timeout => diagnostics.push(RawDiagnostic {
                code: "CHECK_TIMEOUT".to_owned(),
                title: "Check timed out".to_owned(),
                message: "The registered process exceeded the typed invocation timeout.".to_owned(),
                severity: DiagnosticSeverity::Error,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            }),
            TerminationReason::Cancelled => diagnostics.push(RawDiagnostic {
                code: "CHECK_CANCELLED".to_owned(),
                title: "Check was cancelled".to_owned(),
                message: "The registered process ended before complete evidence was produced."
                    .to_owned(),
                severity: DiagnosticSeverity::Error,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            }),
            TerminationReason::OutcomeUnknown => diagnostics.push(RawDiagnostic {
                code: "CHECK_OUTCOME_UNKNOWN".to_owned(),
                title: "Check outcome is unknown".to_owned(),
                message: "Process completion could not be verified and is not treated as pass."
                    .to_owned(),
                severity: DiagnosticSeverity::Critical,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            }),
            TerminationReason::Exited if !input.expected_exit => {
                diagnostics.push(RawDiagnostic {
                    code: "EXTERNAL_CHECK_FAILED".to_owned(),
                    title: "Registered external check failed".to_owned(),
                    message: "The process returned a non-success exit code; raw output remains outside the Diagnostic contract."
                        .to_owned(),
                    severity: DiagnosticSeverity::Error,
                    confidence: DiagnosticConfidence::High,
                    status: DiagnosticStatus::Confirmed,
                    blocking: true,
                    package_id: None,
                    workspace_id: None,
                    locations: vec![],
                });
            }
            TerminationReason::Exited | TerminationReason::LaunchError => {}
        }
        let _ = (input.stdout, input.stderr);
        NormalizerOutput {
            diagnostics,
            completeness: None,
            sarif: None,
        }
    }
}

/// Selects a parser only from the bound CheckDescriptor output contract. The
/// executable bytes or content cannot select a parser by themselves.
pub struct RegisteredOutputNormalizer {
    project_id: star_contracts::ids::ProjectId,
    project_root: PathBuf,
    kinds: BTreeMap<Sha256Hash, CheckOutputNormalizer>,
    safe_exit: SafeExitDiagnosticNormalizer,
}

impl RegisteredOutputNormalizer {
    pub fn new(
        project_id: star_contracts::ids::ProjectId,
        project_root: PathBuf,
        kinds: BTreeMap<Sha256Hash, CheckOutputNormalizer>,
    ) -> Self {
        Self {
            project_id,
            project_root,
            kinds,
            safe_exit: SafeExitDiagnosticNormalizer,
        }
    }
}

impl ExternalDiagnosticNormalizer for RegisteredOutputNormalizer {
    fn normalize(&mut self, input: NormalizerInput<'_>) -> NormalizerOutput {
        let mut output = self.safe_exit.normalize(NormalizerInput {
            executable_binding_fingerprint: input.executable_binding_fingerprint,
            exit_code: input.exit_code,
            expected_exit: input.expected_exit,
            termination_reason: input.termination_reason,
            stdout: input.stdout,
            stderr: input.stderr,
            stdout_truncated: input.stdout_truncated,
            stderr_truncated: input.stderr_truncated,
            output_read_failed: input.output_read_failed,
        });
        if self
            .kinds
            .get(input.executable_binding_fingerprint)
            .copied()
            == Some(CheckOutputNormalizer::SarifV210)
            && input.termination_reason == TerminationReason::Exited
            && input.expected_exit
            && !input.stdout_truncated
            && !input.stderr_truncated
            && !input.output_read_failed
        {
            let sarif = crate::sarif::normalize_sarif_2_1(
                input.stdout,
                &self.project_id,
                &self.project_root,
            );
            output.diagnostics.extend(sarif.diagnostics.clone());
            output.completeness = Some(match sarif.completeness {
                crate::sarif::SarifCompleteness::Complete => Completeness::Complete,
                crate::sarif::SarifCompleteness::Partial => Completeness::Partial,
                crate::sarif::SarifCompleteness::Unverified => Completeness::Unverified,
            });
            output.sarif = Some(sarif);
        }
        output
    }
}

pub struct RegisteredProcessCheckExecutor<N = SafeExitDiagnosticNormalizer> {
    bindings: BTreeMap<Sha256Hash, ResolvedExecutableV2>,
    normalizer: N,
    output_sink: Option<Box<dyn CheckOutputArtifactSink>>,
}

impl RegisteredProcessCheckExecutor<SafeExitDiagnosticNormalizer> {
    pub fn new(bindings: Vec<ResolvedExecutableV2>) -> Result<Self, ProcessExecutorError> {
        Self::with_normalizer(bindings, SafeExitDiagnosticNormalizer)
    }
}

impl<N> RegisteredProcessCheckExecutor<N> {
    pub fn with_normalizer(
        bindings: Vec<ResolvedExecutableV2>,
        normalizer: N,
    ) -> Result<Self, ProcessExecutorError> {
        let map = bindings
            .into_iter()
            .map(|binding| (binding.executable_binding_fingerprint.clone(), binding))
            .collect::<BTreeMap<_, _>>();
        if map.is_empty() {
            return Err(ProcessExecutorError::Executable);
        }
        Ok(Self {
            bindings: map,
            normalizer,
            output_sink: None,
        })
    }

    pub fn with_output_sink(mut self, output_sink: Box<dyn CheckOutputArtifactSink>) -> Self {
        self.output_sink = Some(output_sink);
        self
    }
}

impl<N: ExternalDiagnosticNormalizer> CheckExecutor for RegisteredProcessCheckExecutor<N> {
    fn execute(
        &mut self,
        invocation: &TaskInvocationV2,
    ) -> Result<CheckExecutionObservation, CheckExecutorError> {
        let resealed = invocation.clone().seal().map_err(|_| {
            executor_error(
                "CHECK_INVOCATION_BINDING_MISMATCH",
                TerminationReason::LaunchError,
            )
        })?;
        if &resealed != invocation {
            return Err(executor_error(
                "CHECK_INVOCATION_BINDING_MISMATCH",
                TerminationReason::LaunchError,
            ));
        }
        if invocation.timeout_ms > MAX_INVOCATION_TIMEOUT_MS
            || invocation.output_limits.stdout_bytes > MAX_INVOCATION_OUTPUT_BYTES
            || invocation.output_limits.stderr_bytes > MAX_INVOCATION_OUTPUT_BYTES
            || invocation.output_limits.artifact_bytes > MAX_INVOCATION_OUTPUT_BYTES
        {
            return Err(executor_error(
                "CHECK_INVOCATION_RESOURCE_LIMIT_INVALID",
                TerminationReason::LaunchError,
            ));
        }
        let binding = self
            .bindings
            .get(&invocation.executable_binding_fingerprint)
            .ok_or_else(|| {
                executor_error(
                    "CHECK_EXECUTABLE_NOT_REGISTERED",
                    TerminationReason::LaunchError,
                )
            })?;
        if binding.logical_executable != invocation.executable
            || !invocation.env_refs.is_empty()
            || invocation.stdin_ref.is_some()
        {
            return Err(executor_error(
                "CHECK_INVOCATION_BINDING_MISMATCH",
                TerminationReason::LaunchError,
            ));
        }
        let executable_metadata = fs::symlink_metadata(&binding.absolute_path).map_err(|_| {
            executor_error(
                "CHECK_EXECUTABLE_REVALIDATION_FAILED",
                TerminationReason::LaunchError,
            )
        })?;
        if !executable_metadata.is_file()
            || executable_metadata.file_type().is_symlink()
            || executable_metadata.len() > MAX_EXECUTABLE_BYTES
            || fs::read(&binding.absolute_path)
                .map(|bytes| Sha256Hash::digest(&bytes) != binding.observed_tool.sha256)
                .unwrap_or(true)
        {
            return Err(executor_error(
                "CHECK_EXECUTABLE_DRIFTED",
                TerminationReason::LaunchError,
            ));
        }
        let cwd = resolve_cwd(&binding.project_root, &invocation.cwd).ok_or_else(|| {
            executor_error(
                "CHECK_WORKING_DIRECTORY_INVALID",
                TerminationReason::LaunchError,
            )
        })?;
        let mut command = Command::new(&binding.absolute_path);
        command
            .env_clear()
            .envs(&binding.execution_environment)
            .args(&invocation.args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started_at = Utc::now();
        let mut child = command.spawn().map_err(|_| {
            executor_error(
                "CHECK_PROCESS_LAUNCH_FAILED",
                TerminationReason::LaunchError,
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            executor_error(
                "CHECK_STDOUT_PIPE_FAILED",
                TerminationReason::OutcomeUnknown,
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            executor_error(
                "CHECK_STDERR_PIPE_FAILED",
                TerminationReason::OutcomeUnknown,
            )
        })?;
        let stdout_limit = invocation.output_limits.stdout_bytes as usize;
        let stderr_limit = invocation.output_limits.stderr_bytes as usize;
        let stdout_reader = drain_bounded_async(stdout, stdout_limit);
        let stderr_reader = drain_bounded_async(stderr, stderr_limit);
        let deadline = Instant::now() + Duration::from_millis(invocation.timeout_ms);
        let (exit_code, termination_reason) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.code(), TerminationReason::Exited),
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, TerminationReason::Timeout);
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, TerminationReason::OutcomeUnknown);
                }
            }
        };
        let drain_grace = if termination_reason == TerminationReason::Exited {
            OUTPUT_DRAIN_GRACE
        } else {
            TERMINATED_OUTPUT_DRAIN_GRACE
        };
        let (stdout, stdout_truncated, stdout_read_failed) =
            receive_bounded_drain(&stdout_reader, drain_grace);
        let (stderr, stderr_truncated, stderr_read_failed) =
            receive_bounded_drain(&stderr_reader, drain_grace);
        let expected_exit =
            exit_code.is_some_and(|code| invocation.expected_exit_codes.contains(&code));
        let output_read_failed = stdout_read_failed || stderr_read_failed;
        let normalized = self.normalizer.normalize(NormalizerInput {
            executable_binding_fingerprint: &invocation.executable_binding_fingerprint,
            exit_code,
            expected_exit,
            termination_reason,
            stdout: &stdout,
            stderr: &stderr,
            stdout_truncated,
            stderr_truncated,
            output_read_failed,
        });
        let mut diagnostics = normalized.diagnostics;
        let truncated = stdout_truncated || stderr_truncated;
        let mut artifact_write_failed = false;
        let artifact_refs = if let Some(output_sink) = self.output_sink.as_mut() {
            match output_sink.persist(CheckOutputArtifactInput {
                invocation,
                exit_code,
                termination_reason,
                stdout: &stdout,
                stderr: &stderr,
                stdout_truncated,
                stderr_truncated,
                output_read_failed,
                static_analysis: normalized.sarif.as_ref().map(|sarif| {
                    StaticAnalysisArtifactInput {
                        candidates: &sarif.candidates,
                        imported_count: sarif.imported_count,
                        rejected_count: sarif.rejected_count,
                        completeness: sarif.completeness,
                    }
                }),
            }) {
                Ok(artifact_refs) if artifact_refs.len() >= 2 => artifact_refs,
                Ok(_) => {
                    artifact_write_failed = true;
                    Vec::new()
                }
                Err(_) => {
                    artifact_write_failed = true;
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        if artifact_write_failed {
            diagnostics.push(RawDiagnostic {
                code: "CHECK_OUTPUT_ARTIFACT_WRITE_FAILED".to_owned(),
                title: "Check output evidence could not be persisted".to_owned(),
                message: "The process result is unverified because its bounded output artifacts were not durably recorded."
                    .to_owned(),
                severity: DiagnosticSeverity::Error,
                confidence: DiagnosticConfidence::High,
                status: DiagnosticStatus::Confirmed,
                blocking: true,
                package_id: None,
                workspace_id: None,
                locations: vec![],
            });
        }
        Ok(CheckExecutionObservation {
            started_at,
            finished_at: Utc::now(),
            exit_code,
            termination_reason,
            completeness: if output_read_failed || artifact_write_failed {
                Completeness::Unverified
            } else if truncated {
                Completeness::Partial
            } else if let Some(completeness) = normalized.completeness {
                completeness
            } else if termination_reason == TerminationReason::Exited {
                Completeness::Complete
            } else {
                Completeness::Unverified
            },
            stability: if termination_reason == TerminationReason::Exited && !output_read_failed {
                ValidationStabilityV2::Stable
            } else {
                ValidationStabilityV2::NotEvaluated
            },
            artifact_refs: artifact_refs.clone(),
            observed_tool: Some(binding.observed_tool.clone()),
            diagnostics,
            static_analysis_import: normalized.sarif.map(|sarif| {
                crate::runner::StaticAnalysisImportObservation {
                    executable_binding_fingerprint: invocation
                        .executable_binding_fingerprint
                        .clone(),
                    candidates: sarif.candidates,
                    imported_count: sarif.imported_count,
                    rejected_count: sarif.rejected_count,
                    completeness: sarif.completeness,
                    artifact_refs: artifact_refs.clone(),
                }
            }),
        })
    }
}

fn resolve_cwd(root: &Path, working_directory: &InvocationWorkingDirectoryV2) -> Option<PathBuf> {
    let candidate = match working_directory {
        InvocationWorkingDirectoryV2::ProjectRoot => root.to_path_buf(),
        InvocationWorkingDirectoryV2::ProjectPath { path } => root.join(path.as_str()),
    };
    let metadata = fs::symlink_metadata(&candidate).ok()?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return None;
    }
    let final_path = fs::canonicalize(candidate).ok()?;
    final_path.starts_with(root).then_some(final_path)
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> (Vec<u8>, bool, bool) {
    let mut retained = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 8192];
    let mut total = 0_usize;
    let mut read_failed = false;
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Err(_) => {
                read_failed = true;
                break;
            }
            Ok(read) => read,
        };
        total = total.saturating_add(read);
        if retained.len() < limit {
            let remaining = limit - retained.len();
            retained.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }
    (retained, total > limit, read_failed)
}

fn drain_bounded_async(
    reader: impl Read + Send + 'static,
    limit: usize,
) -> mpsc::Receiver<(Vec<u8>, bool, bool)> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(drain_bounded(reader, limit));
    });
    receiver
}

fn receive_bounded_drain(
    receiver: &mpsc::Receiver<(Vec<u8>, bool, bool)>,
    grace: Duration,
) -> (Vec<u8>, bool, bool) {
    receiver
        .recv_timeout(grace)
        .unwrap_or_else(|_| (Vec::new(), false, true))
}

fn executor_error(code: &str, termination_reason: TerminationReason) -> CheckExecutorError {
    CheckExecutorError {
        code: code.to_owned(),
        message: "The typed process executor rejected or could not verify the invocation."
            .to_owned(),
        termination_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use star_contracts::{
        evidence::{CatalogRef, OutputLimits},
        evidence_v2::{TASK_INVOCATION_V2_SCHEMA_ID, empty_fingerprint},
        ids::TaskInvocationId,
    };

    fn current_executable() -> PathBuf {
        std::env::current_exe().unwrap()
    }

    fn invocation(binding: &ResolvedExecutableV2) -> TaskInvocationV2 {
        TaskInvocationV2 {
            schema_id: TASK_INVOCATION_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            invocation_id: TaskInvocationId::new(),
            tool_ref: CatalogRef {
                catalog_id: "fixture".to_owned(),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: Sha256Hash::digest(b"fixture"),
            },
            executable: binding.logical_executable.clone(),
            executable_binding_fingerprint: binding.executable_binding_fingerprint.clone(),
            args: vec!["--list".to_owned()],
            cwd: InvocationWorkingDirectoryV2::ProjectRoot,
            env_refs: BTreeMap::new(),
            stdin_ref: None,
            timeout_ms: 30_000,
            permission_action: "local_validation".to_owned(),
            idempotency_key: "fixture-execution".to_owned(),
            expected_exit_codes: BTreeSet::from([0]),
            output_limits: OutputLimits {
                stdout_bytes: 128 * 1024,
                stderr_bytes: 128 * 1024,
                artifact_bytes: 1024,
            },
            input_fingerprint: empty_fingerprint(),
        }
        .seal()
        .unwrap()
    }

    #[test]
    fn registered_executor_binds_absolute_image_and_enforces_typed_invocation() {
        let root = std::env::current_dir().unwrap();
        let binding = ResolvedExecutableV2::resolve(
            "star-validation-test",
            &current_executable(),
            &root,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
        let call = invocation(&binding);
        let mut executor = RegisteredProcessCheckExecutor::new(vec![binding]).unwrap();
        let observation = executor.execute(&call).unwrap();
        assert_eq!(observation.termination_reason, TerminationReason::Exited);
        assert_eq!(observation.completeness, Completeness::Complete);
        assert!(observation.observed_tool.is_some());
    }

    #[test]
    fn unregistered_binding_is_rejected_before_process_start() {
        let root = std::env::current_dir().unwrap();
        let binding = ResolvedExecutableV2::resolve(
            "star-validation-test",
            &current_executable(),
            &root,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
        let mut call = invocation(&binding);
        call.executable_binding_fingerprint = Sha256Hash::digest(b"unregistered");
        let mut executor = RegisteredProcessCheckExecutor::new(vec![binding]).unwrap();
        let error = executor.execute(&call).unwrap_err();
        assert_eq!(error.termination_reason, TerminationReason::LaunchError);
    }

    struct IncompleteOutputSink;

    impl CheckOutputArtifactSink for IncompleteOutputSink {
        fn persist(
            &mut self,
            _input: CheckOutputArtifactInput<'_>,
        ) -> Result<Vec<ArtifactRef>, CheckOutputArtifactError> {
            Ok(vec![])
        }
    }

    #[test]
    fn missing_output_artifacts_make_an_executed_check_unverified() {
        let root = std::env::current_dir().unwrap();
        let binding = ResolvedExecutableV2::resolve(
            "star-validation-test",
            &current_executable(),
            &root,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
        let call = invocation(&binding);
        let mut executor = RegisteredProcessCheckExecutor::new(vec![binding])
            .unwrap()
            .with_output_sink(Box::new(IncompleteOutputSink));
        let observation = executor.execute(&call).unwrap();
        assert_eq!(observation.completeness, Completeness::Unverified);
        assert!(observation.artifact_refs.is_empty());
        assert!(
            observation
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "CHECK_OUTPUT_ARTIFACT_WRITE_FAILED")
        );
    }

    #[test]
    fn normalizer_uses_the_typed_expected_exit_set_and_fails_closed_on_read_error() {
        let mut normalizer = SafeExitDiagnosticNormalizer;
        let binding_fingerprint = Sha256Hash::digest(b"fixture-normalizer");
        let expected_nonzero = normalizer.normalize(NormalizerInput {
            executable_binding_fingerprint: &binding_fingerprint,
            exit_code: Some(7),
            expected_exit: true,
            termination_reason: TerminationReason::Exited,
            stdout: &[],
            stderr: &[],
            stdout_truncated: false,
            stderr_truncated: false,
            output_read_failed: false,
        });
        assert!(expected_nonzero.diagnostics.is_empty());

        let read_failure = normalizer.normalize(NormalizerInput {
            executable_binding_fingerprint: &binding_fingerprint,
            exit_code: Some(0),
            expected_exit: true,
            termination_reason: TerminationReason::Exited,
            stdout: &[],
            stderr: &[],
            stdout_truncated: false,
            stderr_truncated: false,
            output_read_failed: true,
        });
        assert_eq!(read_failure.diagnostics.len(), 1);
        assert_eq!(read_failure.diagnostics[0].code, "CHECK_OUTPUT_READ_FAILED");
        assert!(read_failure.diagnostics[0].blocking);
    }

    #[test]
    fn output_drain_timeout_is_unverified_instead_of_blocking_forever() {
        let (_sender, receiver) = mpsc::sync_channel(1);
        let started = Instant::now();
        let (output, truncated, read_failed) =
            receive_bounded_drain(&receiver, Duration::from_millis(1));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(output.is_empty());
        assert!(!truncated);
        assert!(read_failed);
    }

    #[test]
    fn registered_sarif_normalizer_is_selected_by_bound_output_contract() {
        let fingerprint = Sha256Hash::digest(b"fixture-sarif-binding");
        let mut kinds = BTreeMap::new();
        kinds.insert(fingerprint.clone(), CheckOutputNormalizer::SarifV210);
        let project_id = star_contracts::ids::ProjectId::new();
        let mut normalizer = RegisteredOutputNormalizer::new(
            project_id.clone(),
            PathBuf::from("C:/workspace/project"),
            kinds,
        );
        let output = normalizer.normalize(NormalizerInput {
            executable_binding_fingerprint: &fingerprint,
            exit_code: Some(0),
            expected_exit: true,
            termination_reason: TerminationReason::Exited,
            stdout: br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.rule","message":{"text":"secret=never-persist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"}}}]}]}]}"#,
            stderr: &[],
            stdout_truncated: false,
            stderr_truncated: false,
            output_read_failed: false,
        });
        assert_eq!(output.completeness, Some(Completeness::Complete));
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "SARIF:fixture.rule");
        assert_eq!(
            output.diagnostics[0].locations[0].path.project_id,
            project_id
        );
        assert!(!output.diagnostics[0].message.contains("never-persist"));
    }

    #[test]
    fn sarif_is_never_imported_from_truncated_or_unknown_process_output() {
        let fingerprint = Sha256Hash::digest(b"fixture-sarif-fail-closed");
        let mut kinds = BTreeMap::new();
        kinds.insert(fingerprint.clone(), CheckOutputNormalizer::SarifV210);
        let mut normalizer = RegisteredOutputNormalizer::new(
            star_contracts::ids::ProjectId::new(),
            PathBuf::from("C:/workspace/project"),
            kinds,
        );
        for (termination_reason, stdout_truncated, stderr_truncated, expected_code) in [
            (
                TerminationReason::Exited,
                true,
                false,
                "CHECK_OUTPUT_LIMIT_EXCEEDED",
            ),
            (
                TerminationReason::Exited,
                false,
                true,
                "CHECK_OUTPUT_LIMIT_EXCEEDED",
            ),
            (TerminationReason::Timeout, false, false, "CHECK_TIMEOUT"),
            (
                TerminationReason::OutcomeUnknown,
                false,
                false,
                "CHECK_OUTCOME_UNKNOWN",
            ),
        ] {
            let output = normalizer.normalize(NormalizerInput {
                executable_binding_fingerprint: &fingerprint,
                exit_code: Some(0),
                expected_exit: true,
                termination_reason,
                stdout: br#"{"version":"2.1.0","runs":[]}"#,
                stderr: &[],
                stdout_truncated,
                stderr_truncated,
                output_read_failed: false,
            });
            assert!(output.sarif.is_none());
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|item| item.code == expected_code)
            );
        }
    }

    #[test]
    fn executor_rejects_post_seal_mutation_and_excessive_resource_limits() {
        let root = std::env::current_dir().unwrap();
        let binding = ResolvedExecutableV2::resolve(
            "star-validation-test",
            &current_executable(),
            &root,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
        let mut tampered = invocation(&binding);
        tampered.args.push("--tampered-after-seal".to_owned());
        let mut executor = RegisteredProcessCheckExecutor::new(vec![binding.clone()]).unwrap();
        let error = executor.execute(&tampered).unwrap_err();
        assert_eq!(error.code, "CHECK_INVOCATION_BINDING_MISMATCH");

        let mut oversized = invocation(&binding);
        oversized.output_limits.stdout_bytes = MAX_INVOCATION_OUTPUT_BYTES + 1;
        oversized = oversized.seal().unwrap();
        let mut executor = RegisteredProcessCheckExecutor::new(vec![binding]).unwrap();
        let error = executor.execute(&oversized).unwrap_err();
        assert_eq!(error.code, "CHECK_INVOCATION_RESOURCE_LIMIT_INVALID");
    }
}
