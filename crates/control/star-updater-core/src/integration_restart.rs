//! Restart transaction for replacing the rendered Codex integration bundle.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_adapter_codex::{CodexIntegrationManager, IntegrationOptions};
use star_adapter_windows::{
    InstallationManager, WindowsAdapterError, atomic_write_json, canonical_fixed_directory,
    ensure_fixed_directory,
};
use star_contracts::{Sha256Hash, ids::RequestId, ipc::IpcStatus};
use star_ipc::{
    client::{ControllerClient, ControllerClientError, cli_client_config},
    controller_start::{ControllerStartError, VerifiedControllerImage},
};
use thiserror::Error;

use crate::{
    UpdateLeaseError, acquire_update_lease,
    process_census::{
        ProcessIdentity, exact_image_instances, request_graceful_close, snapshot,
        terminate_verified_tree, terminate_verified_tree_excluding,
    },
    restart::{RestartState, RestartTransaction},
};

const GRACEFUL_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROLLER_DRAIN_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone, Debug)]
pub struct IntegrationRepairRestartRequest {
    pub install_root: PathBuf,
    pub codex_desktop_executable: PathBuf,
}

#[derive(Clone, Debug)]
pub struct IntegrationCandidateRestartRequest {
    pub install_root: PathBuf,
    pub candidate_root: PathBuf,
    pub approval_scope_sha256: Sha256Hash,
    pub codex_desktop_executable: PathBuf,
}

/// Offline installer handoff used for the first transition from a legacy
/// three-EXE installation (and later for updater self replacement).  The
/// staged updater owns Codex shutdown/relaunch; the installer remains the
/// only writer of the program root.
#[derive(Clone, Debug)]
pub struct OfflineInstallerRestartRequest {
    pub install_root: PathBuf,
    pub installer_executable: PathBuf,
    pub codex_desktop_executable: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationRepairRestartOutcome {
    pub operation_id: String,
    pub final_state: RestartState,
    pub affected_instance_count: u32,
    pub graceful_close_pids: Vec<u32>,
    pub fallback_terminated_pids: Vec<u32>,
    pub relaunched_pid: u32,
    pub integration_state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationRestartReceipt {
    pub schema_id: String,
    schema_version: u32,
    pub operation_id: String,
    pub state: RestartState,
    pub install_root: PathBuf,
    pub codex_desktop_executable: PathBuf,
    pub affected_instance_count: u32,
    pub affected_task_count: Option<u32>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum IntegrationReceiptError {
    #[error("LOCALAPPDATA is unavailable")]
    LocalAppData,
    #[error("update receipt directory is unsafe")]
    UnsafeDirectory,
    #[error("update receipt could not be read")]
    Read,
    #[error("update receipt is malformed")]
    Malformed,
}

#[derive(Debug, Error)]
pub enum IntegrationRestartError {
    #[error("installed Star-Control state is not valid: {0}")]
    Installation(#[from] star_adapter_windows::WindowsAdapterError),
    #[error("update lease could not be acquired: {0}")]
    UpdateLease(#[from] UpdateLeaseError),
    #[error("Codex Desktop executable must be an existing absolute file")]
    DesktopExecutable,
    #[error("official Codex CLI beside the Desktop executable could not be verified")]
    CodexCli,
    #[error("no exact running Codex Desktop instance was found")]
    NoInstance,
    #[error("Codex did not close during the bounded restart window")]
    CloseTimeout,
    #[error("exact Codex process census failed: {0}")]
    Census(#[from] crate::process_census::CensusError),
    #[error("Codex integration repair failed: {0}")]
    Integration(#[from] star_adapter_codex::CodexAdapterError),
    #[error("the integration candidate no longer satisfies its approved restart-only apply gate")]
    CandidateRejected,
    #[error("integration repair failed and the prior verified release was restored")]
    RolledBack,
    #[error("integration update failed and rollback also failed: {0}")]
    Rollback(WindowsAdapterError),
    #[error(
        "integration update rollback restored files but could not repair the prior Codex integration: {0}"
    )]
    RollbackIntegration(star_adapter_codex::CodexAdapterError),
    #[error("Codex Desktop could not be relaunched")]
    Relaunch,
    #[error("offline installer must be an existing absolute executable")]
    InstallerExecutable,
    #[error("offline installer failed with exit code {0}")]
    Installer(i32),
    #[error("Controller update handoff failed: {0}")]
    Controller(#[from] ControllerClientError),
    #[error("Controller image could not be verified for update handoff: {0}")]
    ControllerImage(#[from] ControllerStartError),
    #[error("restart receipt could not be persisted: {0}")]
    Receipt(WindowsAdapterError),
    #[error("restart transaction reached an invalid internal state")]
    StateTransition,
}

pub async fn repair_codex_integration_and_restart(
    request: IntegrationRepairRestartRequest,
) -> Result<IntegrationRepairRestartOutcome, IntegrationRestartError> {
    restart_codex_integration(request, None).await
}

pub async fn apply_codex_integration_candidate_and_restart(
    request: IntegrationCandidateRestartRequest,
) -> Result<IntegrationRepairRestartOutcome, IntegrationRestartError> {
    let repair = IntegrationRepairRestartRequest {
        install_root: request.install_root,
        codex_desktop_executable: request.codex_desktop_executable,
    };
    restart_codex_integration(
        repair,
        Some((request.candidate_root, request.approval_scope_sha256)),
    )
    .await
}

pub async fn run_offline_installer_and_restart(
    request: OfflineInstallerRestartRequest,
) -> Result<IntegrationRepairRestartOutcome, IntegrationRestartError> {
    let _update_lease = acquire_update_lease()?;
    let installer = verified_installer(&request.installer_executable)?;
    let desktop = verified_desktop(&request.codex_desktop_executable)?;
    let observed = snapshot()?;
    let instances = exact_image_instances(&observed, &desktop);
    if instances.is_empty() {
        return Err(IntegrationRestartError::NoInstance);
    }
    let receipt_request = IntegrationRepairRestartRequest {
        install_root: request.install_root.clone(),
        codex_desktop_executable: desktop.clone(),
    };
    let mut transaction = RestartTransaction::new(format!("upd_{}", star_ipc::nonce()));
    transition_or_error(transaction.stage())?;
    transition_or_error(transaction.verify_candidate(instances.len() as u32, None))?;
    persist_receipt(&transaction, &receipt_request)?;
    let deadline = transaction
        .arm(Utc::now())
        .expect("verified candidate arms");
    persist_receipt(&transaction, &receipt_request)?;
    tokio::time::sleep((deadline - Utc::now()).to_std().unwrap_or_default()).await;
    transition_or_error(transaction.begin_draining(Utc::now()))?;
    persist_receipt(&transaction, &receipt_request)?;
    let graceful_close_pids = request_graceful_close(&desktop)?;
    let grace_deadline = tokio::time::Instant::now() + GRACEFUL_CLOSE_TIMEOUT;
    while tokio::time::Instant::now() < grace_deadline {
        if exact_image_instances(&snapshot()?, &desktop).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let fallback_terminated_pids = if exact_image_instances(&snapshot()?, &desktop).is_empty() {
        Vec::new()
    } else {
        terminate_verified_tree_excluding(&desktop, Some(std::process::id()))?
    };
    if !exact_image_instances(&snapshot()?, &desktop).is_empty() {
        return Err(IntegrationRestartError::CloseTimeout);
    }
    transition_or_error(
        transaction.transition(RestartState::Draining, RestartState::CodexStopped),
    )?;
    persist_receipt(&transaction, &receipt_request)?;
    // An offline installer is specifically the recovery boundary for a
    // legacy/partially-installed tree.  A malformed old bridge must not turn
    // into "Codex was closed but the repair never began".  Prefer the normal
    // authenticated handoff, then drain only exact Star images under the
    // explicitly selected installation root before setup touches files.
    let _controller_handoff = shutdown_controller_for_update(&request.install_root).await;
    let _ = wait_for_installed_star_processes_to_exit(&request.install_root).await?;
    let installed_star_fallback_terminated_pids =
        terminate_installed_star_processes(&request.install_root)?;
    transition_or_error(
        transaction.transition(RestartState::CodexStopped, RestartState::Applying),
    )?;
    let installer_temp = safe_installer_temp_directory()?;
    let status = std::process::Command::new(&installer)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            &format!("/DIR={}", request.install_root.display()),
        ])
        // Inno Setup creates its extraction directory from GetTempPath.  The
        // user's TEMP may legitimately be a reparse-point based redirector
        // (such as TempLink), which Inno correctly rejects.  Use a fixed,
        // Star-owned directory only for this child process; no system
        // environment setting is changed.
        .env("TEMP", &installer_temp)
        .env("TMP", &installer_temp)
        .status()
        .map_err(|_| IntegrationRestartError::InstallerExecutable)?;
    if !status.success() {
        persist_rollback_required(&mut transaction, &receipt_request);
        relaunch_after_failure(&desktop);
        return Err(IntegrationRestartError::Installer(
            status.code().unwrap_or(-1),
        ));
    }
    let installation = InstallationManager::for_current_user()?;
    let installation_status = installation.status(&request.install_root);
    let integration_status = CodexIntegrationManager::for_current_user()
        .and_then(|manager| manager.status(&request.install_root));
    let integration = match (installation_status, integration_status) {
        (Ok(_), Ok(integration)) => integration,
        (Err(error), _) => {
            persist_rollback_required(&mut transaction, &receipt_request);
            relaunch_after_failure(&desktop);
            return Err(IntegrationRestartError::Installation(error));
        }
        (_, Err(error)) => {
            persist_rollback_required(&mut transaction, &receipt_request);
            relaunch_after_failure(&desktop);
            return Err(IntegrationRestartError::Integration(error));
        }
    };
    transition_or_error(
        transaction.transition(RestartState::Applying, RestartState::OfflineVerified),
    )?;
    persist_receipt(&transaction, &receipt_request)?;
    transition_or_error(
        transaction.transition(RestartState::OfflineVerified, RestartState::Relaunching),
    )?;
    let relaunched_pid = std::process::Command::new(&desktop)
        .spawn()
        .map_err(|_| IntegrationRestartError::Relaunch)?
        .id();
    transition_or_error(
        transaction.transition(RestartState::Relaunching, RestartState::OnlinePostcheck),
    )?;
    transition_or_error(transaction.transition(
        RestartState::OnlinePostcheck,
        RestartState::AppliedValidationPending,
    ))?;
    transition_or_error(
        transaction.transition(RestartState::AppliedValidationPending, RestartState::Exited),
    )?;
    persist_receipt(&transaction, &receipt_request)?;
    Ok(IntegrationRepairRestartOutcome {
        operation_id: transaction.operation_id,
        final_state: transaction.state,
        affected_instance_count: instances.len() as u32,
        graceful_close_pids,
        fallback_terminated_pids: fallback_terminated_pids
            .into_iter()
            .chain(installed_star_fallback_terminated_pids)
            .collect(),
        relaunched_pid,
        integration_state: integration.local_state,
    })
}

async fn restart_codex_integration(
    request: IntegrationRepairRestartRequest,
    candidate: Option<(PathBuf, Sha256Hash)>,
) -> Result<IntegrationRepairRestartOutcome, IntegrationRestartError> {
    let _update_lease = acquire_update_lease()?;
    let installation = InstallationManager::for_current_user()?;
    installation.recover_interrupted_codex_integration_candidates(&request.install_root)?;
    installation.status(&request.install_root)?;
    if let Some((candidate_root, approval_scope_sha256)) = &candidate {
        let review =
            installation.inspect_integration_candidate(&request.install_root, candidate_root)?;
        if review.candidate_class
            != star_contracts::installation::IntegrationCandidateClass::CodexIntegrationUpdate
            || review.approval_scope_sha256 != *approval_scope_sha256
            || !review.requires_codex_restart
            || !review.rollback_available
        {
            return Err(IntegrationRestartError::CandidateRejected);
        }
    }
    let desktop = verified_desktop(&request.codex_desktop_executable)?;
    let codex_cli = verified_codex_cli(&desktop)?;
    let observed = snapshot()?;
    let instances = exact_image_instances(&observed, &desktop);
    if instances.is_empty() {
        return Err(IntegrationRestartError::NoInstance);
    }

    let mut transaction = RestartTransaction::new(format!("upd_{}", star_ipc::nonce()));
    transition_or_error(transaction.stage())?;
    transition_or_error(transaction.verify_candidate(instances.len() as u32, None))?;
    persist_receipt(&transaction, &request)?;
    let deadline = transaction
        .arm(Utc::now())
        .expect("verified candidate arms");
    persist_receipt(&transaction, &request)?;
    let delay = (deadline - Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(delay).await;
    transition_or_error(transaction.begin_draining(Utc::now()))?;
    persist_receipt(&transaction, &request)?;

    let graceful_close_pids = request_graceful_close(&desktop)?;
    let grace_deadline = tokio::time::Instant::now() + GRACEFUL_CLOSE_TIMEOUT;
    while tokio::time::Instant::now() < grace_deadline {
        if exact_image_instances(&snapshot()?, &desktop).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let fallback_terminated_pids = if exact_image_instances(&snapshot()?, &desktop).is_empty() {
        Vec::new()
    } else {
        terminate_verified_tree_excluding(&desktop, Some(std::process::id()))?
    };
    if !exact_image_instances(&snapshot()?, &desktop).is_empty() {
        return Err(IntegrationRestartError::CloseTimeout);
    }
    transition_or_error(
        transaction.transition(RestartState::Draining, RestartState::CodexStopped),
    )?;
    persist_receipt(&transaction, &request)?;
    // The Desktop is already closed.  Its MCP-owned Controller commonly
    // exits on EOF before an authenticated shutdown response can be read, so
    // an unavailable handoff is not permission to strand the user with a
    // closed Codex.  Use the same exact-root bounded drain as offline repair;
    // any drain failure remains an aborted transaction and immediately
    // relaunches the original Desktop.
    let _controller_handoff = shutdown_controller_for_update(&request.install_root).await;
    if let Err(error) = wait_for_installed_star_processes_to_exit(&request.install_root).await {
        abort_and_relaunch(&mut transaction, &request, &desktop);
        return Err(error);
    }
    let installed_star_fallback_terminated_pids =
        match terminate_installed_star_processes(&request.install_root) {
            Ok(processes) => processes,
            Err(error) => {
                abort_and_relaunch(&mut transaction, &request, &desktop);
                return Err(error);
            }
        };
    transition_or_error(
        transaction.transition(RestartState::CodexStopped, RestartState::Applying),
    )?;

    let backup = match candidate {
        Some((candidate_root, approval_scope_sha256)) => match installation
            .apply_codex_integration_candidate(
                &request.install_root,
                &candidate_root,
                &approval_scope_sha256,
                &transaction.operation_id,
            ) {
            Ok(backup) => Some(backup),
            Err(error) => {
                abort_and_relaunch(&mut transaction, &request, &desktop);
                return Err(error.into());
            }
        },
        None => None,
    };
    let manager = match CodexIntegrationManager::for_current_user() {
        Ok(manager) => manager,
        Err(error) => {
            abort_and_relaunch(&mut transaction, &request, &desktop);
            return Err(error.into());
        }
    };
    let integration = match manager.repair(
        &request.install_root,
        &IntegrationOptions {
            codex_executable: Some(codex_cli.clone()),
            skip_register: false,
        },
    ) {
        Ok(integration) => integration,
        Err(error) => {
            if let Some(backup) = backup {
                rollback_and_relaunch(
                    &installation,
                    &request,
                    &mut transaction,
                    &backup,
                    &desktop,
                    &codex_cli,
                )?;
                return Err(IntegrationRestartError::RolledBack);
            }
            persist_rollback_required(&mut transaction, &request);
            relaunch_after_failure(&desktop);
            return Err(IntegrationRestartError::Integration(error));
        }
    };
    if let Some(backup) = &backup
        && let Err(error) = installation.commit_codex_integration_candidate(backup)
    {
        abort_and_relaunch(&mut transaction, &request, &desktop);
        return Err(error.into());
    }
    transition_or_error(
        transaction.transition(RestartState::Applying, RestartState::OfflineVerified),
    )?;
    persist_receipt(&transaction, &request)?;
    transition_or_error(
        transaction.transition(RestartState::OfflineVerified, RestartState::Relaunching),
    )?;
    let relaunched_pid = match std::process::Command::new(&desktop).spawn() {
        Ok(child) => child.id(),
        Err(_) => {
            persist_relaunch_failed(&mut transaction, &request);
            return Err(IntegrationRestartError::Relaunch);
        }
    };
    transition_or_error(
        transaction.transition(RestartState::Relaunching, RestartState::OnlinePostcheck),
    )?;
    // A new SessionStart is an online signal owned by Codex.  The updater must
    // not wait indefinitely or synthesize a task/turn, so retain pending
    // validation and exit after launching the same Desktop executable.
    transition_or_error(transaction.transition(
        RestartState::OnlinePostcheck,
        RestartState::AppliedValidationPending,
    ))?;
    transition_or_error(
        transaction.transition(RestartState::AppliedValidationPending, RestartState::Exited),
    )?;
    persist_receipt(&transaction, &request)?;
    Ok(IntegrationRepairRestartOutcome {
        operation_id: transaction.operation_id,
        final_state: transaction.state,
        affected_instance_count: instances.len() as u32,
        graceful_close_pids,
        fallback_terminated_pids: fallback_terminated_pids
            .into_iter()
            .chain(installed_star_fallback_terminated_pids)
            .collect(),
        relaunched_pid,
        integration_state: integration.local_state,
    })
}

fn transition_or_error(transitioned: bool) -> Result<(), IntegrationRestartError> {
    transitioned
        .then_some(())
        .ok_or(IntegrationRestartError::StateTransition)
}

/// A candidate payload was written but Codex-side repair rejected it. Restore
/// the exact manifest-bound files first, repair the prior rendered plugin
/// while every Codex Desktop instance remains closed, and then bring the same
/// Desktop executable back.  The caller receives `RolledBack`, never success.
fn rollback_and_relaunch(
    installation: &InstallationManager,
    request: &IntegrationRepairRestartRequest,
    transaction: &mut RestartTransaction,
    backup: &star_adapter_windows::IntegrationCandidateBackup,
    desktop: &Path,
    codex_cli: &Path,
) -> Result<(), IntegrationRestartError> {
    transition_or_error(
        transaction.transition(RestartState::Applying, RestartState::RollbackRequired),
    )?;
    persist_receipt(transaction, request)?;
    transition_or_error(
        transaction.transition(RestartState::RollbackRequired, RestartState::RollingBack),
    )?;
    if let Err(error) =
        installation.rollback_codex_integration_candidate(&request.install_root, backup)
    {
        transition_or_error(
            transaction.transition(RestartState::RollingBack, RestartState::RollbackFailed),
        )?;
        persist_receipt(transaction, request)?;
        relaunch_after_failure(desktop);
        return Err(IntegrationRestartError::Rollback(error));
    }
    let codex = match CodexIntegrationManager::for_current_user() {
        Ok(codex) => codex,
        Err(error) => {
            transition_or_error(
                transaction.transition(RestartState::RollingBack, RestartState::RollbackFailed),
            )?;
            persist_receipt(transaction, request)?;
            relaunch_after_failure(desktop);
            return Err(IntegrationRestartError::RollbackIntegration(error));
        }
    };
    if let Err(error) = codex.repair(
        &request.install_root,
        &IntegrationOptions {
            codex_executable: Some(codex_cli.to_path_buf()),
            skip_register: false,
        },
    ) {
        transition_or_error(
            transaction.transition(RestartState::RollingBack, RestartState::RollbackFailed),
        )?;
        persist_receipt(transaction, request)?;
        relaunch_after_failure(desktop);
        return Err(IntegrationRestartError::RollbackIntegration(error));
    }
    transition_or_error(
        transaction.transition(RestartState::RollingBack, RestartState::RolledBack),
    )?;
    persist_receipt(transaction, request)?;
    std::process::Command::new(desktop)
        .spawn()
        .map_err(|_| IntegrationRestartError::Relaunch)?;
    Ok(())
}

fn persist_aborted(
    transaction: &mut RestartTransaction,
    request: &IntegrationRepairRestartRequest,
) {
    let _ = transaction.transition(transaction.state, RestartState::Aborted);
    let _ = persist_receipt(transaction, request);
}

/// Once Codex has been closed for an approved restart transaction, every
/// abort path restores the same Desktop executable best-effort.  The durable
/// receipt remains `aborted`; relaunch is interaction recovery, not success.
fn abort_and_relaunch(
    transaction: &mut RestartTransaction,
    request: &IntegrationRepairRestartRequest,
    desktop: &Path,
) {
    persist_aborted(transaction, request);
    relaunch_after_failure(desktop);
}

fn persist_rollback_required(
    transaction: &mut RestartTransaction,
    request: &IntegrationRepairRestartRequest,
) {
    let _ = transaction.transition(RestartState::Applying, RestartState::RollbackRequired);
    let _ = persist_receipt(transaction, request);
}

fn persist_relaunch_failed(
    transaction: &mut RestartTransaction,
    request: &IntegrationRepairRestartRequest,
) {
    let _ = transaction.transition(RestartState::Relaunching, RestartState::RelaunchFailed);
    let _ = persist_receipt(transaction, request);
}

fn persist_receipt(
    transaction: &RestartTransaction,
    request: &IntegrationRepairRestartRequest,
) -> Result<(), IntegrationRestartError> {
    let local_appdata = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or(WindowsAdapterError::UnsafePath)?;
    let root = ensure_fixed_directory(&local_appdata.join("Star-Control/updates"))
        .map_err(IntegrationRestartError::Receipt)?;
    let receipt = IntegrationRestartReceipt {
        schema_id: "star.integration-restart-receipt".to_owned(),
        schema_version: 1,
        operation_id: transaction.operation_id.clone(),
        state: transaction.state,
        install_root: request.install_root.clone(),
        codex_desktop_executable: request.codex_desktop_executable.clone(),
        affected_instance_count: transaction.affected_instance_count,
        affected_task_count: transaction.affected_task_count,
        updated_at: Utc::now(),
    };
    atomic_write_json(
        &root.join(format!("{}.json", transaction.operation_id)),
        &receipt,
    )
    .map_err(IntegrationRestartError::Receipt)?;
    Ok(())
}

/// Read-only latest receipt lookup used by `star update status`. A malformed
/// Star-owned receipt is surfaced instead of being ignored, so interrupted
/// restart work cannot look like a clean installation.
pub fn latest_integration_restart_receipt()
-> Result<Option<IntegrationRestartReceipt>, IntegrationReceiptError> {
    let local_appdata = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or(IntegrationReceiptError::LocalAppData)?;
    let root = local_appdata.join("Star-Control/updates");
    if !root.exists() {
        return Ok(None);
    }
    let root =
        canonical_fixed_directory(&root).map_err(|_| IntegrationReceiptError::UnsafeDirectory)?;
    let mut newest = None;
    for entry in std::fs::read_dir(root).map_err(|_| IntegrationReceiptError::Read)? {
        let entry = entry.map_err(|_| IntegrationReceiptError::Read)?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .map_err(|_| IntegrationReceiptError::Read)?
            .modified()
            .map_err(|_| IntegrationReceiptError::Read)?;
        if newest
            .as_ref()
            .is_none_or(|(current, _): &(std::time::SystemTime, PathBuf)| modified > *current)
        {
            newest = Some((modified, path));
        }
    }
    let Some((_, path)) = newest else {
        return Ok(None);
    };
    let bytes = std::fs::read(path).map_err(|_| IntegrationReceiptError::Read)?;
    if bytes.len() > 128 * 1024 {
        return Err(IntegrationReceiptError::Malformed);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| IntegrationReceiptError::Malformed)?;
    let value = star_contracts::parse_no_duplicate_keys(text)
        .map_err(|_| IntegrationReceiptError::Malformed)?;
    let receipt = serde_json::from_value::<IntegrationRestartReceipt>(value)
        .map_err(|_| IntegrationReceiptError::Malformed)?;
    if receipt.schema_id != "star.integration-restart-receipt" || receipt.schema_version != 1 {
        return Err(IntegrationReceiptError::Malformed);
    }
    Ok(Some(receipt))
}

async fn shutdown_controller_for_update(
    install_root: &Path,
) -> Result<(), IntegrationRestartError> {
    let image = VerifiedControllerImage::from_install_directory(install_root)?;
    let client = ControllerClient::new(cli_client_config(image.path().to_path_buf())?);
    match client
        .call(
            "controller.shutdown",
            serde_json::json!({}),
            RequestId::new(),
        )
        .await
    {
        Ok(response) if response.status == IpcStatus::Ok => Ok(()),
        Ok(_) => Err(IntegrationRestartError::Controller(
            ControllerClientError::Unavailable,
        )),
        Err(ControllerClientError::Unavailable) => Ok(()),
        Err(error) => Err(IntegrationRestartError::Controller(error)),
    }
}

/// Wait for a normal `controller.shutdown` handoff without making an older
/// install manifest a prerequisite for the offline installer.  This only
/// observes exact executable paths that are inside the caller-selected root.
async fn wait_for_installed_star_processes_to_exit(
    install_root: &Path,
) -> Result<bool, IntegrationRestartError> {
    let deadline = tokio::time::Instant::now() + CONTROLLER_DRAIN_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if installed_star_images(&snapshot()?, install_root).is_empty() {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(installed_star_images(&snapshot()?, install_root).is_empty())
}

/// Terminates only proved Star runtime roots under `install_root` after
/// Codex has stopped and the bounded graceful controller drain has elapsed.
/// A same-named executable from another installation, staging folder, or
/// unrelated process is not eligible.
fn terminate_installed_star_processes(
    install_root: &Path,
) -> Result<Vec<u32>, IntegrationRestartError> {
    let targets = installed_star_images(&snapshot()?, install_root);
    let mut terminated = Vec::new();
    for target in targets {
        terminated.extend(terminate_verified_tree(&target)?);
    }
    Ok(terminated)
}

fn installed_star_images(snapshot: &[ProcessIdentity], install_root: &Path) -> Vec<PathBuf> {
    let Ok(install_root) = install_root.canonicalize() else {
        return Vec::new();
    };
    let mut images = BTreeSet::new();
    for process in snapshot {
        let Some(image) = &process.image else {
            continue;
        };
        let Some(name) = image.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "star.exe" | "star-controller.exe" | "star-mcp.exe" | "star-updater.exe"
        ) {
            continue;
        }
        let Ok(image) = image.canonicalize() else {
            continue;
        };
        if image.ancestors().any(|ancestor| {
            ancestor
                .as_os_str()
                .eq_ignore_ascii_case(install_root.as_os_str())
        }) {
            images.insert(image);
        }
    }
    images.into_iter().collect()
}

fn verified_desktop(path: &Path) -> Result<PathBuf, IntegrationRestartError> {
    if !path.is_absolute() || !path.is_file() {
        return Err(IntegrationRestartError::DesktopExecutable);
    }
    Ok(path.to_path_buf())
}

fn verified_installer(path: &Path) -> Result<PathBuf, IntegrationRestartError> {
    if !path.is_absolute()
        || !path.is_file()
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("exe"))
    {
        return Err(IntegrationRestartError::InstallerExecutable);
    }
    Ok(path.to_path_buf())
}

fn safe_installer_temp_directory() -> Result<PathBuf, IntegrationRestartError> {
    let local_appdata = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or(WindowsAdapterError::UnsafePath)?;
    ensure_fixed_directory(&local_appdata.join("Star-Control").join("installer-temp"))
        .map_err(IntegrationRestartError::Installation)
}

/// A failed offline installer must not leave the user with a manually closed
/// Codex application.  `rollback_required` remains the durable outcome; this
/// best-effort relaunch only restores the pre-update interaction surface.
fn relaunch_after_failure(desktop: &Path) {
    let _ = std::process::Command::new(desktop).spawn();
}

fn verified_codex_cli(desktop: &Path) -> Result<PathBuf, IntegrationRestartError> {
    let app = desktop.parent().ok_or(IntegrationRestartError::CodexCli)?;
    let cli = app.join("resources").join("codex.exe");
    if !cli.is_file() {
        return Err(IntegrationRestartError::CodexCli);
    }
    Ok(cli)
}
