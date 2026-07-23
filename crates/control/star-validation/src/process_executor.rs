//! Registered, typed process execution for M3 validation checks.
//!
//! The executor never accepts a shell command string. A caller must resolve an
//! absolute executable and project root up front; the persisted invocation only
//! carries the logical executable and typed argv.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
};
use thiserror::Error;

use crate::runner::{CheckExecutionObservation, CheckExecutor, CheckExecutorError, RawDiagnostic};

const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug)]
pub struct ResolvedExecutableV2 {
    pub logical_executable: String,
    pub absolute_path: PathBuf,
    pub project_root: PathBuf,
    pub executable_binding_fingerprint: Sha256Hash,
    pub observed_tool: ObservedTool,
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
        Ok(Self {
            logical_executable: logical_executable.to_owned(),
            absolute_path: executable,
            project_root: root,
            executable_binding_fingerprint: binding,
            observed_tool: ObservedTool {
                executable_path: format!("registered://{}", opaque_locator.as_str()),
                version: version.to_owned(),
                sha256: executable_sha256,
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct NormalizerInput<'a> {
    pub exit_code: Option<i32>,
    pub expected_exit: bool,
    pub termination_reason: TerminationReason,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub output_read_failed: bool,
}

pub trait ExternalDiagnosticNormalizer: Send {
    fn normalize(&mut self, input: NormalizerInput<'_>) -> Vec<RawDiagnostic>;
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
    fn normalize(&mut self, input: NormalizerInput<'_>) -> Vec<RawDiagnostic> {
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
                });
            }
            TerminationReason::Exited | TerminationReason::LaunchError => {}
        }
        let _ = (input.stdout, input.stderr);
        diagnostics
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
        let stdout_reader = thread::spawn(move || drain_bounded(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || drain_bounded(stderr, stderr_limit));
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
        let (stdout, stdout_truncated, stdout_read_failed) =
            stdout_reader.join().map_err(|_| {
                executor_error(
                    "CHECK_STDOUT_DRAIN_FAILED",
                    TerminationReason::OutcomeUnknown,
                )
            })?;
        let (stderr, stderr_truncated, stderr_read_failed) =
            stderr_reader.join().map_err(|_| {
                executor_error(
                    "CHECK_STDERR_DRAIN_FAILED",
                    TerminationReason::OutcomeUnknown,
                )
            })?;
        let expected_exit =
            exit_code.is_some_and(|code| invocation.expected_exit_codes.contains(&code));
        let output_read_failed = stdout_read_failed || stderr_read_failed;
        let mut diagnostics = self.normalizer.normalize(NormalizerInput {
            exit_code,
            expected_exit,
            termination_reason,
            stdout: &stdout,
            stderr: &stderr,
            stdout_truncated,
            stderr_truncated,
            output_read_failed,
        });
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
            artifact_refs,
            observed_tool: Some(binding.observed_tool.clone()),
            diagnostics,
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
        let expected_nonzero = normalizer.normalize(NormalizerInput {
            exit_code: Some(7),
            expected_exit: true,
            termination_reason: TerminationReason::Exited,
            stdout: &[],
            stderr: &[],
            stdout_truncated: false,
            stderr_truncated: false,
            output_read_failed: false,
        });
        assert!(expected_nonzero.is_empty());

        let read_failure = normalizer.normalize(NormalizerInput {
            exit_code: Some(0),
            expected_exit: true,
            termination_reason: TerminationReason::Exited,
            stdout: &[],
            stderr: &[],
            stdout_truncated: false,
            stderr_truncated: false,
            output_read_failed: true,
        });
        assert_eq!(read_failure.len(), 1);
        assert_eq!(read_failure[0].code, "CHECK_OUTPUT_READ_FAILED");
        assert!(read_failure[0].blocking);
    }
}
