//! Shared, one-shot update operations owned by `star-updater.exe`.
//!
//! The stable CLI may invoke this engine, but it does not own activation
//! mutation.  This preserves the Bootstrap v2 Runtime Generation invariants
//! while allowing the dedicated Updater to become the sole update process.

pub mod integration_restart;
pub mod process_census;
pub mod restart;

use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Serialize;
use star_adapter_windows::{
    InstallationManager, WindowsAdapterError, load_runtime_generation_manifest,
};
use star_contracts::{
    Sha256Hash,
    installation::{RuntimeActivationRecord, RuntimeCandidateReview, RuntimeGenerationRef},
};
use star_ipc::controller_start::{ControllerStartError, VerifiedControllerImage};
use thiserror::Error;

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE,
        },
        System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE},
    },
    core::HSTRING,
};

/// Starts a one-shot updater outside the Codex process tree.  A direct child
/// can be killed when the Desktop Job is torn down, even if a normal process
/// breakaway flag was requested.
#[cfg(windows)]
pub fn spawn_background_updater(
    updater: &std::path::Path,
    arguments: &[String],
) -> Result<u32, ControllerStartError> {
    // An updater under the program root would lock its own image during an
    // offline installer/self-update.  Always execute a uniquely staged copy
    // outside that root so the installer can atomically replace all four
    // runtime EXEs while this one-shot process remains alive to relaunch
    // Codex.
    let staged_updater = stage_updater_for_detached_launch(updater)?;
    spawn_updater_via_local_wmi(&staged_updater, arguments)
}

/// Uses the local WMI provider as a short-lived Windows process broker.  The
/// provider is outside the Desktop Job tree, so closing Codex cannot also
/// close the updater before it applies and relaunches the Desktop.  The
/// command line is passed only through the broker child's environment and the
/// returned PID is re-verified against the staged image before arming a
/// restart.
#[cfg(windows)]
fn spawn_updater_via_local_wmi(
    staged_updater: &Path,
    arguments: &[String],
) -> Result<u32, ControllerStartError> {
    let staged_updater = staged_updater
        .canonicalize()
        .map_err(|_| ControllerStartError::Start)?;
    let command_line = format!(
        "{} {}",
        quote_windows_argument(staged_updater.as_os_str().to_string_lossy().as_ref()),
        windows_command_line(arguments)
    );
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or(ControllerStartError::Start)?;
    let powershell = system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if !powershell.is_file() {
        return Err(ControllerStartError::Start);
    }
    let broker_script = concat!(
        "$commandLine=[Environment]::GetEnvironmentVariable('STAR_CONTROL_UPDATER_COMMAND','Process');",
        "$result=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine=$commandLine};",
        "if($result.ReturnValue -ne 0){exit [int]$result.ReturnValue};",
        "[Console]::Write($result.ProcessId)"
    );
    let output = std::process::Command::new(&powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", broker_script])
        .env("STAR_CONTROL_UPDATER_COMMAND", command_line)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| ControllerStartError::Start)?;
    if !output.status.success() {
        return Err(ControllerStartError::Start);
    }
    let pid = std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(|stdout| stdout.trim().parse::<u32>().ok())
        .filter(|pid| *pid != 0)
        .ok_or(ControllerStartError::Start)?;
    for _ in 0..20 {
        let staged_matches = process_census::snapshot()
            .map_err(|_| ControllerStartError::Start)?
            .iter()
            .any(|process| {
                process.pid == pid
                    && process
                        .image
                        .as_ref()
                        .and_then(|path| path.canonicalize().ok())
                        .is_some_and(|path| path == staged_updater)
            });
        if staged_matches {
            return Ok(pid);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(ControllerStartError::Start)
}

#[cfg(windows)]
fn windows_command_line(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn quote_windows_argument(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '\n', '\r', '"']) {
        return argument.to_owned();
    }
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(windows)]
fn stage_updater_for_detached_launch(updater: &Path) -> Result<PathBuf, ControllerStartError> {
    if !updater.is_absolute() || !updater.is_file() {
        return Err(ControllerStartError::Start);
    }
    let local_appdata = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or(ControllerStartError::Start)?;
    let staging_root = star_adapter_windows::ensure_fixed_directory(
        &local_appdata.join("Star-Control").join("updater-staging"),
    )
    .map_err(|_| ControllerStartError::Start)?;
    let source_hash =
        Sha256Hash::digest_reader(File::open(updater).map_err(|_| ControllerStartError::Start)?)
            .map_err(|_| ControllerStartError::Start)?;
    let staged = staging_root.join(format!("star-updater-{}.exe", star_ipc::nonce()));
    std::fs::copy(updater, &staged).map_err(|_| ControllerStartError::Start)?;
    let staged_hash =
        Sha256Hash::digest_reader(File::open(&staged).map_err(|_| ControllerStartError::Start)?)
            .map_err(|_| ControllerStartError::Start)?;
    if staged_hash != source_hash {
        return Err(ControllerStartError::Start);
    }
    Ok(staged)
}

/// Process-scoped lock for the whole update transaction.  It is intentionally
/// held through the countdown, Codex drain, activation and relaunch so a
/// duplicate request cannot cause another Desktop shutdown midway through an
/// already armed update.
#[cfg(windows)]
pub struct UpdateLease(HANDLE);

#[cfg(windows)]
impl Drop for UpdateLease {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Debug, Error)]
pub enum UpdateLeaseError {
    #[error("the current-user updater mutex could not be created")]
    Unavailable,
    #[error("another Star-Control update is already active")]
    Busy,
}

#[cfg(windows)]
pub fn acquire_update_lease() -> Result<UpdateLease, UpdateLeaseError> {
    let sid_hash =
        star_ipc::client::current_user_sid_hash().map_err(|_| UpdateLeaseError::Unavailable)?;
    let name = HSTRING::from(format!("Local\\Star-Control.Updater.{sid_hash}.v1"));
    let mutex =
        unsafe { CreateMutexW(None, false, &name) }.map_err(|_| UpdateLeaseError::Unavailable)?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(mutex);
        }
        return Err(UpdateLeaseError::Busy);
    }
    Ok(UpdateLease(mutex))
}

/// Observes whether an updater currently owns the per-user transaction
/// namespace without creating or acquiring that mutex. Controllers use this
/// to reject new mutation admission during countdown, drain, and apply.
#[cfg(windows)]
pub fn update_lease_active() -> Result<bool, UpdateLeaseError> {
    let sid_hash =
        star_ipc::client::current_user_sid_hash().map_err(|_| UpdateLeaseError::Unavailable)?;
    let name = HSTRING::from(format!("Local\\Star-Control.Updater.{sid_hash}.v1"));
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) } {
        Ok(handle) => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            Ok(true)
        }
        Err(_) if unsafe { GetLastError() } == ERROR_FILE_NOT_FOUND => Ok(false),
        Err(_) => Err(UpdateLeaseError::Unavailable),
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeApplyRequest {
    pub install_root: PathBuf,
    pub generation_id: String,
    pub state_generation_id: String,
    pub approval_scope_sha256: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RuntimeApplyOutcome {
    Committed {
        activation_revision: u64,
        active: RuntimeGenerationRef,
        candidate_review: Box<RuntimeCandidateReview>,
        requires_codex_restart: bool,
    },
    RolledBack {
        failure: String,
    },
}

#[derive(Debug, Error)]
pub enum RuntimeApplyError {
    #[error("runtime candidate does not satisfy the approved apply gate")]
    CandidateRejected,
    #[error("update lease could not be acquired: {0}")]
    UpdateLease(#[from] UpdateLeaseError),
    #[error("Windows update operation failed: {0}")]
    Windows(#[from] WindowsAdapterError),
    #[error("Controller bootstrap failed: {0}")]
    ControllerStart(#[from] ControllerStartError),
    #[error("runtime candidate manifest is invalid: {0}")]
    CandidateManifest(String),
    #[error("controller did not quiesce through the verified updater path: {0}")]
    ControllerQuiesce(String),
    #[error("active Runtime Controller did not pass the installed CLI postcheck")]
    ControllerPostcheck,
    #[error("runtime update failed ({apply_failure}); rollback also failed: {rollback_failure}")]
    RollbackFailed {
        apply_failure: String,
        rollback_failure: String,
    },
}

pub async fn apply_runtime_generation(
    request: RuntimeApplyRequest,
) -> Result<RuntimeApplyOutcome, RuntimeApplyError> {
    let _update_lease = acquire_update_lease()?;
    let manager = InstallationManager::for_current_user()?;
    manager.status(&request.install_root)?;
    let review =
        manager.inspect_runtime_candidate(&request.install_root, &request.generation_id)?;
    if review.approval_scope_sha256 != request.approval_scope_sha256
        || !review.handler_ready
        || !review.bridge_compatible
        || !review.rollback_available
        || review.breaking_schema
        || review.risk_lane_widened
        || review.permission_widened
        || review.requires_codex_restart
        || review.requires_new_task
        || review.hook_review_required
    {
        return Err(RuntimeApplyError::CandidateRejected);
    }
    let prior = manager.load_runtime_activation_record(&request.install_root)?;
    let candidate_root = request
        .install_root
        .join("runtime")
        .join("generations")
        .join(&request.generation_id);
    let candidate_manifest = load_runtime_generation_manifest(&candidate_root)?;
    let candidate = RuntimeGenerationRef {
        generation_id: candidate_manifest.generation.generation_id,
        runtime_root: candidate_root
            .canonicalize()
            .unwrap_or(candidate_root)
            .display()
            .to_string(),
        release_manifest_sha256: candidate_manifest.generation.release_manifest_sha256,
    };
    let old_bootstrap = VerifiedControllerImage::from_install_directory(&request.install_root)?;
    integration_restart::shutdown_installed_runtime_controllers(&request.install_root)
        .await
        .map_err(|error| RuntimeApplyError::ControllerQuiesce(error.to_string()))?;
    let next = RuntimeActivationRecord {
        schema_id: "star.runtime-activation-record".to_owned(),
        schema_version: 1,
        activation_revision: prior.activation_revision.saturating_add(1),
        active: candidate.clone(),
        previous: Some(prior.active.clone()),
        state_generation_id: request.state_generation_id,
        bridge_contract_version: prior.bridge_contract_version,
        activated_at: chrono::Utc::now(),
    };
    if let Err(error) =
        manager.activate_runtime_bridge(&request.install_root, &next, prior.bridge_contract_version)
    {
        let _ = old_bootstrap.start_background();
        return Err(RuntimeApplyError::Windows(error));
    }
    let new_bootstrap = match VerifiedControllerImage::from_install_directory(&request.install_root)
    {
        Ok(image) => image,
        Err(error) => {
            return rollback_runtime_generation(
                &manager,
                &request.install_root,
                &prior,
                candidate,
                None,
                error.to_string(),
            )
            .await;
        }
    };
    let candidate_controller = new_bootstrap.path().to_path_buf();
    if let Err(error) = new_bootstrap.start_background() {
        return rollback_runtime_generation(
            &manager,
            &request.install_root,
            &prior,
            candidate,
            Some(&candidate_controller),
            error.to_string(),
        )
        .await;
    }
    if !installed_cli_controller_postcheck(&request.install_root).await {
        return rollback_runtime_generation(
            &manager,
            &request.install_root,
            &prior,
            candidate,
            Some(&candidate_controller),
            "new controller postcheck failed".to_owned(),
        )
        .await;
    }
    Ok(RuntimeApplyOutcome::Committed {
        activation_revision: next.activation_revision,
        active: next.active,
        candidate_review: Box::new(review),
        requires_codex_restart: false,
    })
}

async fn rollback_runtime_generation(
    manager: &InstallationManager,
    install_root: &std::path::Path,
    prior: &RuntimeActivationRecord,
    candidate: RuntimeGenerationRef,
    candidate_controller: Option<&Path>,
    failure: String,
) -> Result<RuntimeApplyOutcome, RuntimeApplyError> {
    if candidate_controller.is_some()
        && let Err(error) =
            integration_restart::shutdown_installed_runtime_controllers(install_root).await
    {
        return Err(RuntimeApplyError::RollbackFailed {
            apply_failure: failure,
            rollback_failure: error.to_string(),
        });
    }
    let rollback = RuntimeActivationRecord {
        schema_id: "star.runtime-activation-record".to_owned(),
        schema_version: 1,
        activation_revision: prior.activation_revision.saturating_add(2),
        active: prior.active.clone(),
        previous: Some(candidate),
        state_generation_id: prior.state_generation_id.clone(),
        bridge_contract_version: prior.bridge_contract_version,
        activated_at: chrono::Utc::now(),
    };
    if let Err(error) =
        manager.activate_runtime_bridge(install_root, &rollback, prior.bridge_contract_version)
    {
        return Err(RuntimeApplyError::RollbackFailed {
            apply_failure: failure,
            rollback_failure: error.to_string(),
        });
    }
    let rollback_start = VerifiedControllerImage::from_install_directory(install_root)
        .and_then(|image| image.start_background());
    if let Err(error) = rollback_start {
        return Err(RuntimeApplyError::RollbackFailed {
            apply_failure: failure,
            rollback_failure: error.to_string(),
        });
    }
    if !installed_cli_controller_postcheck(install_root).await {
        return Err(RuntimeApplyError::RollbackFailed {
            apply_failure: failure,
            rollback_failure: RuntimeApplyError::ControllerPostcheck.to_string(),
        });
    }
    Ok(RuntimeApplyOutcome::RolledBack { failure })
}

async fn installed_cli_controller_postcheck(install_root: &Path) -> bool {
    const POSTCHECK_WINDOW: Duration = Duration::from_secs(15);
    const SINGLE_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);
    const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

    let cli = install_root.join("star.exe");
    let deadline = tokio::time::Instant::now() + POSTCHECK_WINDOW;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let output = tokio::time::timeout(
            remaining.min(SINGLE_ATTEMPT_TIMEOUT),
            tokio::process::Command::new(&cli)
                .args(["doctor", "--json"])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output(),
        )
        .await;
        if let Ok(Ok(output)) = output
            && output.status.success()
            && output.stdout.len() <= MAX_OUTPUT_BYTES
            && installed_cli_postcheck_json(&output.stdout)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn installed_cli_postcheck_json(stdout: &[u8]) -> bool {
    std::str::from_utf8(stdout)
        .ok()
        .and_then(|text| star_contracts::parse_no_duplicate_keys(text.trim()).ok())
        .is_some_and(|value| {
            value.get("schema_id").and_then(serde_json::Value::as_str) == Some("star.ipc.response")
                && value.get("status").and_then(serde_json::Value::as_str) == Some("ok")
                && value
                    .pointer("/data/result/status")
                    .and_then(serde_json::Value::as_str)
                    == Some("pass")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_lease_excludes_a_second_transaction_until_released() {
        assert!(!update_lease_active().unwrap());
        let first = acquire_update_lease().expect("first updater owns the lease");
        assert!(update_lease_active().unwrap());
        assert!(matches!(
            acquire_update_lease(),
            Err(UpdateLeaseError::Busy)
        ));
        drop(first);
        assert!(!update_lease_active().unwrap());
        let second = acquire_update_lease().expect("released updater lease is reusable");
        drop(second);
    }

    #[test]
    fn installed_cli_postcheck_requires_one_successful_ipc_response() {
        assert!(installed_cli_postcheck_json(
            br#"{"schema_id":"star.ipc.response","schema_version":1,"status":"ok","data":{"result":{"status":"pass"}}}"#
        ));
        assert!(!installed_cli_postcheck_json(
            br#"{"schema_id":"star.ipc.response","schema_version":1,"status":"error","data":{"result":{"status":"pass"}}}"#
        ));
        assert!(!installed_cli_postcheck_json(
            br#"{"schema_id":"star.ipc.response","status":"ok","status":"error","data":{"result":{"status":"pass"}}}"#
        ));
        assert!(!installed_cli_postcheck_json(
            br#"{"schema_id":"star.ipc.response","schema_version":1,"status":"ok","data":{"result":{"status":"fail"}}}"#
        ));
        assert!(!installed_cli_postcheck_json(b"not-json"));
    }

    #[cfg(windows)]
    #[test]
    fn detached_launch_command_line_quotes_space_and_embedded_quote_arguments() {
        assert_eq!(
            windows_command_line(&[
                "offline-installer-restart".to_owned(),
                r"C:\\Program Files\\Codex\\ChatGPT.exe".to_owned(),
                "quote\"value".to_owned(),
            ]),
            "offline-installer-restart \"C:\\\\Program Files\\\\Codex\\\\ChatGPT.exe\" \"quote\\\"value\""
        );
    }
}
