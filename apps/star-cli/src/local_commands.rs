use std::{
    collections::BTreeSet,
    future::Future,
    io::Read,
    path::{Component, Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use star_adapter_codex::{CodexAdapterError, CodexIntegrationManager, IntegrationOptions};
use star_adapter_windows::autostart::{self, AutostartError, AutostartState};
#[cfg(test)]
use star_adapter_windows::compiled_architecture;
use star_adapter_windows::{InstallationManager, WindowsAdapterError};
use star_contracts::{
    Sha256Hash,
    fixed_mcp::SERVER_INSTRUCTIONS,
    ids::RequestId,
    installation::{RuntimeActivationRecord, TargetArchitecture},
    parse_no_duplicate_keys,
};
use star_ipc::{
    client::{ControllerClient, cli_client_config},
    controller_start::VerifiedControllerImage,
};
use star_updater_core::{
    RuntimeApplyError, RuntimeApplyOutcome, RuntimeApplyRequest,
    integration_restart::latest_integration_restart_receipt, spawn_background_updater,
};

const HOOK_INPUT_MAX_BYTES: u64 = 1024 * 1024;
const SESSION_START_SKILL_NAME: &str = "star-control-operations";
const PARALLEL_IMPLEMENTATION_SKILL_NAME: &str = "orchestrate-parallel-implementation";
const SESSION_END_CODEX_HOST_TIMEOUT_SECONDS: u64 = 3;
const SESSION_END_LIFECYCLE_REPORT_TIMEOUT: Duration =
    Duration::from_secs(SESSION_END_CODEX_HOST_TIMEOUT_SECONDS - 1);

#[derive(Clone, Debug, PartialEq, Eq)]
enum LocalCommand {
    InstallationFinalize {
        architecture: TargetArchitecture,
        replace_existing: bool,
    },
    InstallationBridgeInitialize {
        state_generation_id: String,
    },
    InstallationStatus,
    IntegrationInstall {
        repair: bool,
        codex: Option<PathBuf>,
        skip_register: bool,
    },
    IntegrationStatus,
    IntegrationUninstall {
        codex: Option<PathBuf>,
    },
    IntegrationRepairRestart {
        codex_desktop: PathBuf,
    },
    UpdateStatus,
    UpdateVerify,
    UpdateStage {
        source_generation_root: PathBuf,
    },
    UpdateInspect {
        generation_id: String,
    },
    UpdateApply {
        generation_id: String,
        state_generation_id: String,
        approval_scope_sha256: Sha256Hash,
    },
    UpdateIntegrationApply {
        candidate_root: PathBuf,
        codex_desktop: PathBuf,
        approval_scope_sha256: Sha256Hash,
    },
    UpdateOfflineInstallerRestart {
        target_install_root: PathBuf,
        installer: PathBuf,
        codex_desktop: PathBuf,
    },
    UpdateReconcileInstalledRuntime {
        target_install_root: PathBuf,
    },
    ControllerAutostart {
        action: String,
    },
    Hook {
        event: HookEvent,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HookEvent {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    Stop,
    PreToolUse,
    PostToolUse,
    SubagentStart,
    SubagentStop,
}

impl HookEvent {
    fn hook_event_name(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Stop => "Stop",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
        }
    }

    fn lifecycle_event(self) -> &'static str {
        match self {
            Self::SessionStart => "session_start",
            Self::SessionEnd => "root_stop",
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::Stop => "root_stop",
            Self::PreToolUse => "tool_started",
            Self::PostToolUse => "tool_finished",
            Self::SubagentStart => "subagent_started",
            Self::SubagentStop => "subagent_finished",
        }
    }

    fn lifecycle_report_timeout(self) -> Option<Duration> {
        match self {
            Self::SessionEnd => Some(SESSION_END_LIFECYCLE_REPORT_TIMEOUT),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedLocal {
    command: LocalCommand,
    json: bool,
}

#[derive(serde::Serialize)]
struct RuntimeUpdateStatus {
    activation_record_path: String,
    active_runtime_generation: Option<RuntimeActivationRecord>,
    latest_integration_restart:
        Option<star_updater_core::integration_restart::IntegrationRestartReceipt>,
}

pub async fn dispatch(args: &[String]) -> Option<i32> {
    let parsed = match parse(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return None,
        Err(error) => {
            eprintln!("{error}");
            return Some(2);
        }
    };
    Some(run(parsed).await)
}

fn parse(args: &[String]) -> Result<Option<ParsedLocal>, String> {
    let is_local = args.first().is_some_and(|value| {
        matches!(value.as_str(), "installation" | "integration" | "hook")
            || value == "update"
            || (value == "controller" && args.get(1).is_some_and(|value| value == "autostart"))
    });
    if !is_local {
        return Ok(None);
    }
    let json_count = args
        .iter()
        .filter(|value| value.as_str() == "--json")
        .count();
    if json_count > 1 {
        return Err("--json may be supplied only once".to_owned());
    }
    let json = json_count == 1;
    let filtered = args
        .iter()
        .filter(|value| value.as_str() != "--json")
        .cloned()
        .collect::<Vec<_>>();
    let command = match filtered.as_slice() {
        [first, second, tail @ ..] if first == "installation" && second == "finalize" => {
            let mut architecture = None;
            let mut replace_existing = false;
            let mut index = 0;
            while index < tail.len() {
                match tail[index].as_str() {
                    "--architecture" => {
                        if architecture.is_some() || index + 1 >= tail.len() {
                            return Err("installation finalize requires one --architecture value"
                                .to_owned());
                        }
                        architecture = Some(
                            tail[index + 1]
                                .parse::<TargetArchitecture>()
                                .map_err(str::to_owned)?,
                        );
                        index += 2;
                    }
                    "--replace-existing" if !replace_existing => {
                        replace_existing = true;
                        index += 1;
                    }
                    value => return Err(format!("unknown or duplicate option: {value}")),
                }
            }
            LocalCommand::InstallationFinalize {
                architecture: architecture
                    .ok_or("installation finalize requires --architecture x64|arm64".to_owned())?,
                replace_existing,
            }
        }
        [first, second] if first == "installation" && second == "status" => {
            LocalCommand::InstallationStatus
        }
        [first, second, third, tail @ ..]
            if first == "installation" && second == "bridge" && third == "initialize" =>
        {
            LocalCommand::InstallationBridgeInitialize {
                state_generation_id: parse_bootstrap_state_generation(tail)?,
            }
        }
        [first, second, third, tail @ ..]
            if first == "integration" && second == "repair" && third == "restart" =>
        {
            let desktop = match tail {
                [flag, path] if flag == "--codex-desktop" => PathBuf::from(path),
                _ => {
                    return Err(
                        "integration repair restart requires --codex-desktop <absolute-path>"
                            .to_owned(),
                    );
                }
            };
            if !desktop.is_absolute() {
                return Err("--codex-desktop must be an absolute path".to_owned());
            }
            LocalCommand::IntegrationRepairRestart {
                codex_desktop: desktop,
            }
        }
        [first, second, tail @ ..]
            if first == "integration" && matches!(second.as_str(), "install" | "repair") =>
        {
            let (codex, skip_register) = parse_integration_options(tail, true)?;
            LocalCommand::IntegrationInstall {
                repair: second == "repair",
                codex,
                skip_register,
            }
        }
        [first, second] if first == "integration" && second == "status" => {
            LocalCommand::IntegrationStatus
        }
        [first, second] if first == "update" && second == "status" => LocalCommand::UpdateStatus,
        [first, second] if first == "update" && second == "verify" => LocalCommand::UpdateVerify,
        [first, second, source] if first == "update" && second == "stage" => {
            LocalCommand::UpdateStage {
                source_generation_root: PathBuf::from(source),
            }
        }
        [first, second, generation_id] if first == "update" && second == "inspect" => {
            LocalCommand::UpdateInspect {
                generation_id: generation_id.clone(),
            }
        }
        [first, second, generation_id, tail @ ..] if first == "update" && second == "apply" => {
            let candidate_root = PathBuf::from(generation_id);
            if candidate_root.is_absolute() {
                let (codex_desktop, approval_scope_sha256) =
                    parse_integration_update_apply_options(tail)?;
                LocalCommand::UpdateIntegrationApply {
                    candidate_root,
                    codex_desktop,
                    approval_scope_sha256,
                }
            } else {
                let (state_generation_id, approval_scope_sha256) =
                    parse_update_apply_options(tail)?;
                LocalCommand::UpdateApply {
                    generation_id: generation_id.clone(),
                    state_generation_id,
                    approval_scope_sha256,
                }
            }
        }
        [first, second, tail @ ..]
            if first == "update" && second == "offline-installer-restart" =>
        {
            let (target_install_root, installer, codex_desktop) =
                parse_offline_installer_restart_options(tail)?;
            LocalCommand::UpdateOfflineInstallerRestart {
                target_install_root,
                installer,
                codex_desktop,
            }
        }
        [first, second, tail @ ..]
            if first == "update" && second == "reconcile-installed-runtime" =>
        {
            let [flag, value] = tail else {
                return Err(
                    "update reconcile-installed-runtime requires one --install-root value"
                        .to_owned(),
                );
            };
            if flag != "--install-root" {
                return Err("update reconcile-installed-runtime requires --install-root".to_owned());
            }
            let target_install_root = PathBuf::from(value);
            if !target_install_root.is_absolute() {
                return Err("--install-root must be an absolute path".to_owned());
            }
            LocalCommand::UpdateReconcileInstalledRuntime {
                target_install_root,
            }
        }
        [first, second, tail @ ..] if first == "integration" && second == "uninstall" => {
            let (codex, skip_register) = parse_integration_options(tail, false)?;
            if skip_register {
                return Err("integration uninstall does not accept --skip-register".to_owned());
            }
            LocalCommand::IntegrationUninstall { codex }
        }
        [first, second, action]
            if first == "controller"
                && second == "autostart"
                && matches!(action.as_str(), "enable" | "disable" | "status") =>
        {
            LocalCommand::ControllerAutostart {
                action: action.clone(),
            }
        }
        [first, second] if first == "hook" && !json => {
            let event = match second.as_str() {
                "session-start" => HookEvent::SessionStart,
                "session-end" => HookEvent::SessionEnd,
                "user-prompt-submit" => HookEvent::UserPromptSubmit,
                "stop" => HookEvent::Stop,
                "pre-tool-use" => HookEvent::PreToolUse,
                "post-tool-use" => HookEvent::PostToolUse,
                "subagent-start" => HookEvent::SubagentStart,
                "subagent-stop" => HookEvent::SubagentStop,
                _ => return Err(format!("unsupported hook event: {second}")),
            };
            LocalCommand::Hook { event }
        }
        [first, _] if first == "hook" => {
            return Err("hook commands do not accept --json".to_owned());
        }
        _ => {
            return Err(
                "unsupported local command; use star --help for installation, integration and hook syntax"
                    .to_owned(),
            );
        }
    };
    Ok(Some(ParsedLocal { command, json }))
}

fn parse_update_apply_options(tail: &[String]) -> Result<(String, Sha256Hash), String> {
    let mut state_generation_id = None;
    let mut approval_scope_sha256 = None;
    let mut index = 0;
    while index < tail.len() {
        let value = &tail[index];
        if index + 1 >= tail.len() {
            return Err(format!("{value} requires one value"));
        }
        match value.as_str() {
            "--state-generation" if state_generation_id.is_none() => {
                let state = tail[index + 1].trim();
                if state.is_empty() || state.chars().count() > 128 {
                    return Err("--state-generation must be a bounded non-empty id".to_owned());
                }
                state_generation_id = Some(state.to_owned());
            }
            "--approve" if approval_scope_sha256.is_none() => {
                approval_scope_sha256 = Some(
                    Sha256Hash::from_str(&tail[index + 1])
                        .map_err(|_| "--approve must be a sha256 digest".to_owned())?,
                );
            }
            _ => return Err(format!("unknown or duplicate option: {value}")),
        }
        index += 2;
    }
    Ok((
        state_generation_id.ok_or("update apply requires --state-generation <id>".to_owned())?,
        approval_scope_sha256.ok_or("update apply requires --approve <sha256>".to_owned())?,
    ))
}

fn parse_integration_update_apply_options(
    tail: &[String],
) -> Result<(PathBuf, Sha256Hash), String> {
    let mut codex_desktop = None;
    let mut approval_scope_sha256 = None;
    let mut index = 0;
    while index < tail.len() {
        if index + 1 >= tail.len() {
            return Err(format!("{} requires one value", tail[index]));
        }
        match tail[index].as_str() {
            "--codex-desktop" if codex_desktop.is_none() => {
                let path = PathBuf::from(&tail[index + 1]);
                if !path.is_absolute() {
                    return Err("--codex-desktop must be an absolute path".to_owned());
                }
                codex_desktop = Some(path);
            }
            "--approve" if approval_scope_sha256.is_none() => {
                approval_scope_sha256 = Some(
                    Sha256Hash::from_str(&tail[index + 1])
                        .map_err(|_| "--approve must be a sha256 digest".to_owned())?,
                );
            }
            value => return Err(format!("unknown or duplicate option: {value}")),
        }
        index += 2;
    }
    Ok((
        codex_desktop.ok_or(
            "integration update apply requires --codex-desktop <absolute-path>".to_owned(),
        )?,
        approval_scope_sha256
            .ok_or("integration update apply requires --approve <sha256>".to_owned())?,
    ))
}

fn parse_offline_installer_restart_options(
    tail: &[String],
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let mut target_install_root = None;
    let mut installer = None;
    let mut codex_desktop = None;
    let mut index = 0;
    while index < tail.len() {
        if index + 1 >= tail.len() {
            return Err(format!("{} requires one value", tail[index]));
        }
        match tail[index].as_str() {
            "--install-root" if target_install_root.is_none() => {
                let path = PathBuf::from(&tail[index + 1]);
                if !path.is_absolute() {
                    return Err("--install-root must be an absolute path".to_owned());
                }
                target_install_root = Some(path);
            }
            "--installer" if installer.is_none() => {
                let path = PathBuf::from(&tail[index + 1]);
                if !path.is_absolute() {
                    return Err("--installer must be an absolute path".to_owned());
                }
                installer = Some(path);
            }
            "--codex-desktop" if codex_desktop.is_none() => {
                let path = PathBuf::from(&tail[index + 1]);
                if !path.is_absolute() {
                    return Err("--codex-desktop must be an absolute path".to_owned());
                }
                codex_desktop = Some(path);
            }
            value => return Err(format!("unknown or duplicate option: {value}")),
        }
        index += 2;
    }
    Ok((
        target_install_root.ok_or(
            "offline installer restart requires --install-root <absolute-path>".to_owned(),
        )?,
        installer
            .ok_or("offline installer restart requires --installer <absolute-path>".to_owned())?,
        codex_desktop.ok_or(
            "offline installer restart requires --codex-desktop <absolute-path>".to_owned(),
        )?,
    ))
}

fn parse_bootstrap_state_generation(tail: &[String]) -> Result<String, String> {
    match tail {
        [flag, value] if flag == "--state-generation" => {
            let value = value.trim();
            if value.is_empty() || value.chars().count() > 128 {
                Err("--state-generation must be a bounded non-empty id".to_owned())
            } else {
                Ok(value.to_owned())
            }
        }
        _ => Err("installation bridge initialize requires --state-generation <id>".to_owned()),
    }
}

fn parse_integration_options(
    tail: &[String],
    allow_skip: bool,
) -> Result<(Option<PathBuf>, bool), String> {
    let mut codex = None;
    let mut skip_register = false;
    let mut index = 0;
    while index < tail.len() {
        match tail[index].as_str() {
            "--codex" => {
                if codex.is_some() || index + 1 >= tail.len() {
                    return Err("--codex requires one executable path".to_owned());
                }
                codex = Some(PathBuf::from(&tail[index + 1]));
                index += 2;
            }
            "--skip-register" if allow_skip && !skip_register => {
                skip_register = true;
                index += 1;
            }
            value => return Err(format!("unknown or duplicate option: {value}")),
        }
    }
    Ok((codex, skip_register))
}

async fn run(parsed: ParsedLocal) -> i32 {
    if let LocalCommand::Hook { event } = &parsed.command {
        return run_hook(*event).await;
    }
    let install_root = match current_install_root() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 4;
        }
    };
    match parsed.command {
        LocalCommand::InstallationFinalize {
            architecture,
            replace_existing,
        } => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            match manager.finalize(&install_root, architecture, replace_existing) {
                Ok(record) => print_value(&record, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::InstallationBridgeInitialize {
            state_generation_id,
        } => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            match manager.initialize_runtime_bridge(&install_root, &state_generation_id) {
                Ok(record) => print_value(&record, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::InstallationStatus => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            match manager.status(&install_root) {
                Ok(status) => print_value(&status, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::IntegrationInstall {
            repair,
            codex,
            skip_register,
        } => {
            let manager = match CodexIntegrationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_codex_error(error),
            };
            let options = IntegrationOptions {
                codex_executable: codex,
                skip_register,
            };
            let result = if repair {
                manager.repair(&install_root, &options)
            } else {
                manager.install(&install_root, &options)
            };
            match result {
                Ok(result) => print_value(&result, parsed.json),
                Err(error) => print_codex_error(error),
            }
        }
        LocalCommand::IntegrationStatus => {
            let manager = match CodexIntegrationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_codex_error(error),
            };
            match manager.status(&install_root) {
                Ok(result) => print_value(&result, parsed.json),
                Err(error) => print_codex_error(error),
            }
        }
        LocalCommand::IntegrationUninstall { codex } => {
            let manager = match CodexIntegrationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_codex_error(error),
            };
            match manager.uninstall(&install_root, codex.as_deref()) {
                Ok(result) => {
                    let needs_action = result.registration_state
                        == star_contracts::installation::CodexRegistrationState::ManualActionRequired;
                    let exit = print_value(&result, parsed.json);
                    if needs_action { 3 } else { exit }
                }
                Err(error) => print_codex_error(error),
            }
        }
        LocalCommand::IntegrationRepairRestart { codex_desktop } => {
            let updater = install_root.join("star-updater.exe");
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            if let Err(error) = manager.status(&install_root) {
                return print_windows_error(error);
            }
            let arguments = vec![
                "integration-repair-restart".to_owned(),
                "--install-root".to_owned(),
                install_root.display().to_string(),
                "--codex-desktop".to_owned(),
                codex_desktop.display().to_string(),
            ];
            match spawn_background_updater(&updater, &arguments) {
                Ok(pid) => print_value(
                    &serde_json::json!({"state":"restart_armed","delay_seconds":10,"updater_pid":pid}),
                    parsed.json,
                ),
                Err(error) => {
                    eprintln!("updater background breakaway failed: {error}");
                    4
                }
            }
        }
        LocalCommand::UpdateStatus => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            let path = manager.runtime_activation_record_path();
            let active_runtime_generation = if path.exists() {
                match manager.load_runtime_activation_record(&install_root) {
                    Ok(record) => Some(record),
                    Err(error) => return print_windows_error(error),
                }
            } else {
                None
            };
            let latest_integration_restart = match latest_integration_restart_receipt() {
                Ok(receipt) => receipt,
                Err(error) => {
                    eprintln!("restart receipt status is unavailable: {error}");
                    return 4;
                }
            };
            print_value(
                &RuntimeUpdateStatus {
                    activation_record_path: path.display().to_string(),
                    active_runtime_generation,
                    latest_integration_restart,
                },
                parsed.json,
            )
        }
        LocalCommand::UpdateVerify => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            match manager.load_runtime_activation_record(&install_root) {
                Ok(record) => print_value(&record, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::UpdateStage {
            source_generation_root,
        } => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            match manager.stage_runtime_generation(&install_root, &source_generation_root) {
                Ok(staged) => print_value(&staged, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::UpdateInspect { generation_id } => {
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            let stage = PathBuf::from(&generation_id);
            let inspected = if stage.is_absolute() {
                manager
                    .inspect_integration_candidate(&install_root, &stage)
                    .and_then(|review| {
                        serde_json::to_value(review).map_err(WindowsAdapterError::from)
                    })
            } else {
                manager
                    .inspect_runtime_candidate(&install_root, &generation_id)
                    .and_then(|review| {
                        serde_json::to_value(review).map_err(WindowsAdapterError::from)
                    })
            };
            match inspected {
                Ok(review) => print_value(&review, parsed.json),
                Err(error) => print_windows_error(error),
            }
        }
        LocalCommand::UpdateApply {
            generation_id,
            state_generation_id,
            approval_scope_sha256,
        } => {
            apply_runtime_generation(
                &install_root,
                generation_id,
                state_generation_id,
                approval_scope_sha256,
                parsed.json,
            )
            .await
        }
        LocalCommand::UpdateIntegrationApply {
            candidate_root,
            codex_desktop,
            approval_scope_sha256,
        } => {
            let updater = install_root.join("star-updater.exe");
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            let review = match manager.inspect_integration_candidate(&install_root, &candidate_root)
            {
                Ok(review) => review,
                Err(error) => return print_windows_error(error),
            };
            if review.candidate_class
                != star_contracts::installation::IntegrationCandidateClass::CodexIntegrationUpdate
                || review.approval_scope_sha256 != approval_scope_sha256
                || !review.requires_codex_restart
            {
                eprintln!(
                    "candidate is not the approved restart-required Codex integration update"
                );
                return 4;
            }
            if !updater.is_file() {
                eprintln!("installed star-updater.exe is unavailable");
                return 4;
            }
            let arguments = vec![
                "integration-apply-restart".to_owned(),
                candidate_root.display().to_string(),
                "--install-root".to_owned(),
                install_root.display().to_string(),
                "--codex-desktop".to_owned(),
                codex_desktop.display().to_string(),
                "--approve".to_owned(),
                approval_scope_sha256.to_string(),
            ];
            match spawn_background_updater(&updater, &arguments) {
                Ok(pid) => print_value(
                    &serde_json::json!({
                        "state":"restart_armed",
                        "delay_seconds":10,
                        "updater_pid":pid,
                        "candidate_release_manifest_sha256":review.candidate_release_manifest_sha256,
                    }),
                    parsed.json,
                ),
                Err(error) => {
                    eprintln!("updater background breakaway failed: {error}");
                    4
                }
            }
        }
        LocalCommand::UpdateOfflineInstallerRestart {
            target_install_root,
            installer,
            codex_desktop,
        } => {
            let updater = install_root.join("star-updater.exe");
            let manager = match InstallationManager::for_current_user() {
                Ok(manager) => manager,
                Err(error) => return print_windows_error(error),
            };
            if let Err(error) = manager.status(&install_root) {
                return print_windows_error(error);
            }
            let arguments = vec![
                "offline-installer-restart".to_owned(),
                "--installer".to_owned(),
                installer.display().to_string(),
                "--install-root".to_owned(),
                target_install_root.display().to_string(),
                "--codex-desktop".to_owned(),
                codex_desktop.display().to_string(),
            ];
            match spawn_background_updater(&updater, &arguments) {
                Ok(pid) => print_value(
                    &serde_json::json!({
                        "state":"restart_armed",
                        "delay_seconds":10,
                        "updater_pid":pid,
                        "mode":"offline_installer",
                    }),
                    parsed.json,
                ),
                Err(error) => {
                    eprintln!("updater background breakaway failed: {error}");
                    4
                }
            }
        }
        LocalCommand::UpdateReconcileInstalledRuntime {
            target_install_root,
        } => reconcile_installed_runtime(&install_root, &target_install_root, parsed.json).await,
        LocalCommand::ControllerAutostart { action } => {
            let expected =
                match autostart::expected_command(&install_root.join("star-controller.exe")) {
                    Ok(expected) => expected,
                    Err(error) => return print_autostart_error(error),
                };
            let result = match action.as_str() {
                "enable" => autostart::enable(&expected).map(|_| "enabled"),
                "disable" => autostart::disable(&expected).map(|_| "disabled"),
                "status" => autostart::status(&expected).map(|state| match state {
                    AutostartState::Owned => "enabled",
                    AutostartState::Missing => "disabled",
                    AutostartState::Conflict => "conflict",
                }),
                _ => unreachable!(),
            };
            match result {
                Ok(state) => print_value(&serde_json::json!({"state": state}), parsed.json),
                Err(error) => print_autostart_error(error),
            }
        }
        LocalCommand::Hook { .. } => unreachable!(),
    }
}

async fn apply_runtime_generation(
    install_root: &std::path::Path,
    generation_id: String,
    state_generation_id: String,
    approval_scope_sha256: Sha256Hash,
    json: bool,
) -> i32 {
    // P-0039 packages the dedicated updater beside the stable CLI.  Keep the
    // in-process P-0038 path only for an already-installed pre-updater release
    // so repair/rollback of that release remains possible.
    let updater = install_root.join("star-updater.exe");
    if updater.is_file() {
        let manager = match InstallationManager::for_current_user() {
            Ok(manager) => manager,
            Err(error) => return print_windows_error(error),
        };
        // Do not execute a same-directory binary merely because its filename
        // matches.  A P-0039 package must pass the release-manifest file-set
        // verification before the stable CLI delegates any mutation to it.
        if let Err(error) = manager.status(install_root) {
            return print_windows_error(error);
        }
        let output = match tokio::process::Command::new(&updater)
            .arg("runtime-apply")
            .arg(&generation_id)
            .arg("--install-root")
            .arg(install_root)
            .arg("--state-generation")
            .arg(&state_generation_id)
            .arg("--approve")
            .arg(approval_scope_sha256.to_string())
            .arg("--json")
            .output()
            .await
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("star-updater could not start: {error}");
                return 4;
            }
        };
        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        return output.status.code().unwrap_or(4);
    }
    apply_runtime_generation_legacy(
        install_root,
        generation_id,
        state_generation_id,
        approval_scope_sha256,
        json,
    )
    .await
}

async fn reconcile_installed_runtime(
    command_root: &std::path::Path,
    target_install_root: &std::path::Path,
    json: bool,
) -> i32 {
    let updater = command_root.join("star-updater.exe");
    if !updater.is_file() {
        eprintln!("star-updater.exe is unavailable beside the invoking CLI");
        return 4;
    }
    let manager = match InstallationManager::for_current_user() {
        Ok(manager) => manager,
        Err(error) => return print_windows_error(error),
    };
    let same_root = match (
        command_root.canonicalize(),
        target_install_root.canonicalize(),
    ) {
        (Ok(command_root), Ok(target_install_root)) => command_root
            .as_os_str()
            .eq_ignore_ascii_case(target_install_root.as_os_str()),
        _ => command_root
            .as_os_str()
            .eq_ignore_ascii_case(target_install_root.as_os_str()),
    };
    let verified = if same_root {
        manager.status(target_install_root).map(|_| ())
    } else {
        manager
            .inspect_integration_candidate(target_install_root, command_root)
            .map(|_| ())
    };
    if let Err(error) = verified {
        return print_windows_error(error);
    }
    let output = match tokio::process::Command::new(&updater)
        .arg("reconcile-installed-runtime")
        .arg("--install-root")
        .arg(target_install_root)
        .output()
        .await
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("star-updater could not start: {error}");
            return 4;
        }
    };
    if !output.status.success() {
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        return output.status.code().unwrap_or(4);
    }
    let value = match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("star-updater returned malformed reconcile output: {error}");
            return 4;
        }
    };
    print_value(&value, json)
}

async fn apply_runtime_generation_legacy(
    install_root: &std::path::Path,
    generation_id: String,
    state_generation_id: String,
    approval_scope_sha256: Sha256Hash,
    json: bool,
) -> i32 {
    let request = RuntimeApplyRequest {
        install_root: install_root.to_path_buf(),
        generation_id,
        state_generation_id,
        approval_scope_sha256,
    };
    match star_updater_core::apply_runtime_generation(request).await {
        Ok(outcome @ RuntimeApplyOutcome::Committed { .. }) => print_value(
            &serde_json::to_value(outcome).expect("serializable outcome"),
            json,
        ),
        Ok(outcome @ RuntimeApplyOutcome::RolledBack { .. }) => {
            print_value(
                &serde_json::to_value(outcome).expect("serializable outcome"),
                json,
            );
            4
        }
        Err(RuntimeApplyError::CandidateRejected) => {
            eprintln!("runtime candidate does not satisfy the approved apply gate");
            3
        }
        Err(error @ RuntimeApplyError::RollbackFailed { .. }) => {
            eprintln!("{error}");
            5
        }
        Err(error) => {
            eprintln!("{error}");
            4
        }
    }
}

fn current_install_root() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|_| "cannot locate star.exe".to_owned())?;
    executable
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "star.exe has no installation directory".to_owned())
}

fn session_start_hook_output() -> serde_json::Value {
    serde_json::json!({
        "continue": true,
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!(
                "`{SESSION_START_SKILL_NAME}` 지침을 따른다. `{PARALLEL_IMPLEMENTATION_SKILL_NAME}`의 implicit invocation은 새 Codex task/thread 승인이나 병렬 위임이 아니다. 일반 구현은 현재 task single-agent로 수행한다. 명시 승인 Bundle은 `list_projects({{}})` 뒤 unique bundle_id/BOOTSTRAP_ONLY만 든 prompt와 `target:{{type:project, projectId, environment:{{type:worktree}}}}`로 bootstrap하고, threadId/hostId/cwd identity 확인 뒤 같은 threadId에 complete Context Pack + ACTIVATE_BUNDLE을 보낸다. activation 전 Goal/commentary/mutation/test/commit은 금지하며 clientThreadId만 있으면 bounded list_threads unique match가 resolve될 때까지 fail-closed한다. Terra는 WORKER_COMPLETE 한 번 뒤 멈추며 controller만 Sol review를 관찰하고, same threadId/Goal correction은 exact diff 승인 뒤에만 complete할 수 있다. {SERVER_INSTRUCTIONS}"
            )
        }
    })
}

#[derive(Debug)]
struct ValidatedHookInput<'a> {
    session_id: &'a str,
    cwd: &'a str,
    tool_name: Option<&'a str>,
    tool_input: Option<&'a serde_json::Value>,
    tool_response: Option<&'a serde_json::Value>,
}

fn bounded_hook_string<'a>(
    value: &'a serde_json::Value,
    field: &str,
    max_len: usize,
) -> Result<&'a str, String> {
    let text = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("hook input has no string {field}"))?;
    if text.is_empty() || text.len() > max_len || text.contains('\0') {
        return Err(format!("hook input has invalid {field}"));
    }
    Ok(text)
}

fn validate_optional_string(value: &serde_json::Value, field: &str) -> Result<(), String> {
    match value.get(field) {
        None | Some(serde_json::Value::Null) => Ok(()),
        Some(serde_json::Value::String(text)) if text.len() <= HOOK_INPUT_MAX_BYTES as usize => {
            Ok(())
        }
        Some(_) => Err(format!("hook input has invalid {field}")),
    }
}

fn validate_permission_mode(value: &serde_json::Value) -> Result<(), String> {
    match bounded_hook_string(value, "permission_mode", 64)? {
        "default" | "acceptEdits" | "plan" | "dontAsk" | "bypassPermissions" => Ok(()),
        _ => Err("hook input has unsupported permission_mode".to_owned()),
    }
}

fn validate_typed_hook_input<'a>(
    event: HookEvent,
    value: &'a serde_json::Value,
) -> Result<ValidatedHookInput<'a>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "hook input must be one JSON object".to_owned())?;
    let event_name = bounded_hook_string(value, "hook_event_name", 64)?;
    if event_name != event.hook_event_name() {
        return Err(format!(
            "hook_event_name must be {}",
            event.hook_event_name()
        ));
    }
    let session_id = bounded_hook_string(value, "session_id", 256)?;
    if !lifecycle_identifier_valid(session_id) {
        return Err(format!(
            "{} hook input has an invalid session_id",
            event.hook_event_name()
        ));
    }
    let cwd = bounded_hook_string(value, "cwd", 32_768)?;
    let _model = bounded_hook_string(value, "model", 256)?;
    validate_optional_string(value, "transcript_path")?;
    if event != HookEvent::SessionEnd {
        validate_permission_mode(value)?;
    }

    let turn_scoped = matches!(
        event,
        HookEvent::UserPromptSubmit
            | HookEvent::Stop
            | HookEvent::PreToolUse
            | HookEvent::PostToolUse
            | HookEvent::SubagentStart
            | HookEvent::SubagentStop
    );
    if turn_scoped {
        bounded_hook_string(value, "turn_id", 256)?;
    }

    let mut tool_name = None;
    let mut tool_input = None;
    let mut tool_response = None;
    match event {
        HookEvent::SessionStart => match bounded_hook_string(value, "source", 32)? {
            "startup" | "resume" | "clear" | "compact" => {}
            _ => return Err("SessionStart hook input has invalid source".to_owned()),
        },
        HookEvent::SessionEnd => {
            if bounded_hook_string(value, "reason", 32)? != "other" {
                return Err("SessionEnd hook input has invalid reason".to_owned());
            }
        }
        HookEvent::UserPromptSubmit => {
            bounded_hook_string(value, "prompt", HOOK_INPUT_MAX_BYTES as usize)?;
        }
        HookEvent::Stop => {
            if !object
                .get("stop_hook_active")
                .is_some_and(serde_json::Value::is_boolean)
            {
                return Err("Stop hook input has invalid stop_hook_active".to_owned());
            }
            validate_optional_string(value, "last_assistant_message")?;
        }
        HookEvent::PreToolUse | HookEvent::PostToolUse => {
            tool_name = Some(bounded_hook_string(value, "tool_name", 512)?);
            bounded_hook_string(value, "tool_use_id", 256)?;
            tool_input = Some(
                object
                    .get("tool_input")
                    .ok_or_else(|| "tool hook input has no tool_input".to_owned())?,
            );
            if event == HookEvent::PostToolUse {
                tool_response = Some(
                    object
                        .get("tool_response")
                        .ok_or_else(|| "PostToolUse input has no tool_response".to_owned())?,
                );
            }
        }
        HookEvent::SubagentStart => {
            bounded_hook_string(value, "agent_id", 256)?;
            bounded_hook_string(value, "agent_type", 256)?;
        }
        HookEvent::SubagentStop => {
            bounded_hook_string(value, "agent_id", 256)?;
            bounded_hook_string(value, "agent_type", 256)?;
            validate_optional_string(value, "agent_transcript_path")?;
            validate_optional_string(value, "last_assistant_message")?;
            if !object
                .get("stop_hook_active")
                .is_some_and(serde_json::Value::is_boolean)
            {
                return Err("SubagentStop hook input has invalid stop_hook_active".to_owned());
            }
        }
    }
    Ok(ValidatedHookInput {
        session_id,
        cwd,
        tool_name,
        tool_input,
        tool_response,
    })
}

fn pre_tool_use_deny_output(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
}

fn print_hook_json(value: &serde_json::Value) {
    let output = serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"decision":"deny","reason":"HOOK_OUTPUT_SERIALIZATION_FAILED"}"#.to_owned()
    });
    println!("{output}");
}

fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = command.chars().peekable();
    while let Some(character) = chars.next() {
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else if character == '\\' && active_quote == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else {
                current.push(character);
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            ';' | '|' | '&' | '\n' | '\r' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(";".to_owned());
                while chars
                    .peek()
                    .is_some_and(|next| matches!(next, ';' | '|' | '&' | '\n' | '\r'))
                {
                    chars.next();
                }
            }
            character if character.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn command_segments(command: &str) -> Vec<Vec<String>> {
    let mut segments = Vec::new();
    let mut segment = Vec::new();
    for token in shell_tokens(command) {
        if token == ";" {
            if !segment.is_empty() {
                segments.push(std::mem::take(&mut segment));
            }
        } else {
            segment.push(token);
        }
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

fn executable_name(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn git_command_tokens(segment: &[String]) -> Option<&[String]> {
    let position = segment
        .iter()
        .position(|token| executable_name(token) == "git")?;
    let wrappers_are_safe = segment[..position].iter().all(|token| {
        let token = executable_name(token);
        matches!(
            token.as_str(),
            "call" | "command" | "sudo" | "cmd" | "pwsh" | "powershell"
        ) || token.eq_ignore_ascii_case("/c")
            || token.eq_ignore_ascii_case("-command")
    });
    wrappers_are_safe.then_some(&segment[position + 1..])
}

fn git_subcommand(tokens: &[String]) -> Option<(&str, &[String])> {
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if matches!(
            token,
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace"
        ) {
            index = index.saturating_add(2);
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        return Some((token, &tokens[index + 1..]));
    }
    None
}

fn force_push_or_destructive_git_reason(command: &str) -> Option<&'static str> {
    for segment in command_segments(command) {
        let Some(tokens) = git_command_tokens(&segment) else {
            continue;
        };
        let Some((subcommand, arguments)) = git_subcommand(tokens) else {
            continue;
        };
        if subcommand.eq_ignore_ascii_case("push")
            && arguments.iter().any(|argument| {
                matches!(
                    argument.as_str(),
                    "-f" | "--force" | "--force-with-lease" | "--force-if-includes"
                ) || argument.starts_with("--force=")
                    || argument.starts_with("--force-with-lease=")
                    || (argument.starts_with('+') && argument.len() > 1)
                    || (argument.starts_with('-')
                        && !argument.starts_with("--")
                        && argument[1..].contains('f'))
            })
        {
            return Some("force push is forbidden regardless of flag position");
        }
        if subcommand.eq_ignore_ascii_case("reset")
            && arguments
                .iter()
                .any(|argument| argument == "--hard" || argument.starts_with("--hard="))
        {
            return Some("git reset --hard is forbidden");
        }
        if subcommand.eq_ignore_ascii_case("clean") {
            return Some("git clean is forbidden; preserve dirty and untracked worktrees");
        }
    }
    None
}

fn path_contains_dynamic_syntax(value: &str) -> bool {
    value.is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '*' | '?' | '[' | ']' | '$' | '%' | '`'))
        || value.starts_with('~')
        || value.contains("$(")
        || value.contains("..")
}

fn lexically_normalize(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
        }
    }
    Some(normalized)
}

fn explicit_bounded_path(value: &str, cwd: &str) -> bool {
    if path_contains_dynamic_syntax(value) {
        return false;
    }
    let cwd = PathBuf::from(cwd);
    if !cwd.is_absolute() {
        return false;
    }
    let raw = PathBuf::from(value);
    let joined = if raw.is_absolute() {
        raw
    } else {
        cwd.join(raw)
    };
    let Some(cwd) = lexically_normalize(&cwd) else {
        return false;
    };
    let Some(target) = lexically_normalize(&joined) else {
        return false;
    };
    target.starts_with(&cwd) && target != cwd && target.parent().is_some()
}

fn recursive_delete_or_move_is_unverified(command: &str, cwd: &str) -> bool {
    for segment in command_segments(command) {
        let Some(first) = segment.first() else {
            continue;
        };
        let executable = executable_name(first);
        if executable == "remove-item" {
            let recursive = segment
                .iter()
                .any(|token| token.eq_ignore_ascii_case("-recurse"));
            if !recursive {
                continue;
            }
            let literal = segment
                .iter()
                .position(|token| token.eq_ignore_ascii_case("-literalpath"))
                .and_then(|index| segment.get(index + 1));
            if !literal.is_some_and(|path| explicit_bounded_path(path, cwd)) {
                return true;
            }
        } else if matches!(executable.as_str(), "rm" | "rmdir") {
            let recursive = segment.iter().any(|token| {
                token == "--recursive"
                    || (token.starts_with('-')
                        && token[1..].chars().any(|flag| flag == 'r' || flag == 'R'))
            });
            if recursive {
                let targets: Vec<_> = segment
                    .iter()
                    .skip(1)
                    .filter(|token| !token.starts_with('-'))
                    .collect();
                if targets.is_empty()
                    || targets
                        .iter()
                        .any(|target| !explicit_bounded_path(target, cwd))
                {
                    return true;
                }
            }
        } else if executable == "move-item" {
            let source = segment
                .iter()
                .position(|token| token.eq_ignore_ascii_case("-literalpath"))
                .and_then(|index| segment.get(index + 1));
            let destination = segment
                .iter()
                .position(|token| token.eq_ignore_ascii_case("-destination"))
                .and_then(|index| segment.get(index + 1));
            if !source.is_some_and(|path| explicit_bounded_path(path, cwd))
                || !destination.is_some_and(|path| explicit_bounded_path(path, cwd))
            {
                return true;
            }
        } else if matches!(executable.as_str(), "mv" | "move") {
            let targets: Vec<_> = segment
                .iter()
                .skip(1)
                .filter(|token| !token.starts_with('-'))
                .collect();
            if targets.len() < 2
                || targets
                    .iter()
                    .any(|target| !explicit_bounded_path(target, cwd))
            {
                return true;
            }
        }
    }
    false
}

fn protected_generated_state_reference(value: &str) -> bool {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("/.codex/plugins/cache/")
        || normalized.contains("/.codex/state")
        || normalized.contains("/.codex/trust")
        || normalized.contains("/appdata/roaming/star-control/")
        || normalized.contains("/appdata/local/star-control/")
        || (normalized.contains("/.codex/")
            && (normalized.contains(".db") || normalized.contains(".sqlite")))
}

fn mutation_intent(command: &str) -> bool {
    command_segments(command).iter().any(|segment| {
        segment.first().is_some_and(|first| {
            matches!(
                executable_name(first).as_str(),
                "remove-item"
                    | "move-item"
                    | "copy-item"
                    | "rename-item"
                    | "set-content"
                    | "add-content"
                    | "out-file"
                    | "new-item"
                    | "rm"
                    | "rmdir"
                    | "mv"
                    | "move"
                    | "cp"
                    | "copy"
                    | "del"
                    | "erase"
                    | "sqlite3"
            )
        }) || segment
            .iter()
            .any(|token| matches!(token.as_str(), ">" | ">>"))
    })
}

fn tool_input_command<'a>(tool_name: &str, input: &'a serde_json::Value) -> Option<&'a str> {
    if matches!(tool_name, "Bash" | "apply_patch") {
        return input.get("command").and_then(serde_json::Value::as_str);
    }
    input.get("command").and_then(serde_json::Value::as_str)
}

fn pre_tool_use_denial_reason(input: &ValidatedHookInput<'_>) -> Option<String> {
    let tool_name = input.tool_name?;
    let tool_input = input.tool_input?;
    if matches!(tool_name, "Bash" | "apply_patch")
        && tool_input_command(tool_name, tool_input).is_none()
    {
        return Some(format!("malformed {tool_name} input has no string command"));
    }
    if let Some(command) = tool_input_command(tool_name, tool_input) {
        if let Some(reason) = force_push_or_destructive_git_reason(command) {
            return Some(reason.to_owned());
        }
        if recursive_delete_or_move_is_unverified(command, input.cwd) {
            return Some(
                "recursive delete or move target is not an explicit bounded path below cwd"
                    .to_owned(),
            );
        }
        if protected_generated_state_reference(command)
            && (tool_name == "apply_patch" || mutation_intent(command))
        {
            return Some(
                "direct mutation of Codex trust/runtime DB, plugin cache, or Star-Control generated runtime state is forbidden"
                    .to_owned(),
            );
        }
    }
    let serialized = serde_json::to_string(tool_input).ok()?;
    let mutating_tool = tool_name == "apply_patch"
        || [
            "write", "edit", "delete", "remove", "move", "rename", "patch",
        ]
        .iter()
        .any(|marker| tool_name.to_ascii_lowercase().contains(marker));
    if mutating_tool && protected_generated_state_reference(&serialized) {
        return Some(
            "direct mutation of Codex trust/runtime DB, plugin cache, or Star-Control generated runtime state is forbidden"
                .to_owned(),
        );
    }
    None
}

fn collect_post_tool_markers(
    value: &serde_json::Value,
    parent_key: Option<&str>,
    markers: &mut BTreeSet<String>,
    operation_ids: &mut BTreeSet<String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let normalized_key = key.to_ascii_lowercase();
                if normalized_key == "operation_id"
                    && let Some(id) = value
                        .as_str()
                        .filter(|id| !id.is_empty() && id.len() <= 256)
                {
                    operation_ids.insert(id.to_owned());
                }
                if normalized_key == "approval_required" && value == &serde_json::Value::Bool(true)
                {
                    markers.insert("approval_required".to_owned());
                }
                if normalized_key == "terminal" && value == &serde_json::Value::Bool(false) {
                    markers.insert("terminal=false".to_owned());
                }
                collect_post_tool_markers(value, Some(&normalized_key), markers, operation_ids);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_post_tool_markers(value, parent_key, markers, operation_ids);
            }
        }
        serde_json::Value::String(value) => {
            let normalized = value.to_ascii_lowercase();
            let status_key = parent_key.is_some_and(|key| {
                matches!(
                    key,
                    "status"
                        | "state"
                        | "outcome"
                        | "completeness"
                        | "decision"
                        | "result"
                        | "verification"
                )
            });
            if status_key
                && matches!(
                    normalized.as_str(),
                    "accepted"
                        | "approval_required"
                        | "partial"
                        | "stale"
                        | "unverified"
                        | "flaky"
                        | "outcome_unknown"
                        | "not_run"
                )
            {
                markers.insert(normalized);
            }
        }
        _ => {}
    }
}

fn post_tool_use_context(input: &ValidatedHookInput<'_>) -> Option<serde_json::Value> {
    let response = input.tool_response?;
    let mut markers = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    collect_post_tool_markers(response, None, &mut markers, &mut operation_ids);
    if markers.is_empty() && operation_ids.is_empty() {
        return None;
    }
    let markers = markers.into_iter().collect::<Vec<_>>().join(",");
    let operations = operation_ids.into_iter().collect::<Vec<_>>().join(",");
    Some(serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": format!(
                "Star-Control terminal-state guard: markers=[{markers}] operation_ids=[{operations}]. accepted/approval_required/partial/stale/unverified/flaky/outcome_unknown/not_run 또는 Operation ID만으로 완료를 선언하지 말고 terminal receipt와 exact readback을 확인한다. PostToolUse는 이미 발생한 side effect를 되돌리지 않는다."
            )
        }
    }))
}

async fn run_hook(event: HookEvent) -> i32 {
    let mut input = Vec::new();
    if std::io::stdin()
        .take(HOOK_INPUT_MAX_BYTES + 1)
        .read_to_end(&mut input)
        .is_err()
        || input.is_empty()
        || input.len() as u64 > HOOK_INPUT_MAX_BYTES
    {
        if event == HookEvent::PreToolUse {
            print_hook_json(&pre_tool_use_deny_output(
                "malformed or oversized PreToolUse input",
            ));
            return 0;
        }
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    }
    let Ok(text) = std::str::from_utf8(&input) else {
        if event == HookEvent::PreToolUse {
            print_hook_json(&pre_tool_use_deny_output(
                "PreToolUse input is not valid UTF-8",
            ));
            return 0;
        }
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    };
    let Ok(value) = parse_no_duplicate_keys(text) else {
        if event == HookEvent::PreToolUse {
            print_hook_json(&pre_tool_use_deny_output(
                "PreToolUse input is malformed JSON or contains duplicate keys",
            ));
            return 0;
        }
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    };
    let validated = match validate_typed_hook_input(event, &value) {
        Ok(input) => input,
        Err(error) if event == HookEvent::PreToolUse => {
            print_hook_json(&pre_tool_use_deny_output(&format!(
                "malformed PreToolUse input: {error}"
            )));
            return 0;
        }
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    if event == HookEvent::PreToolUse
        && let Some(reason) = pre_tool_use_denial_reason(&validated)
    {
        print_hook_json(&pre_tool_use_deny_output(&reason));
        return 0;
    }
    if let Err(error) = report_hook_lifecycle_with_host_budget(event, validated.session_id).await {
        // A Hook must not turn a healthy Codex task into a failure merely
        // because the optional Controller is currently unavailable.  The
        // updater treats missing census evidence as a block, never as proof
        // that a task is absent.
        eprintln!("Star-Control lifecycle observation was not recorded: {error}");
    }
    if event == HookEvent::SessionStart {
        print_hook_json(&session_start_hook_output());
    } else if event == HookEvent::PostToolUse {
        if let Some(output) = post_tool_use_context(&validated) {
            print_hook_json(&output);
        }
    } else if matches!(event, HookEvent::Stop | HookEvent::SubagentStop) {
        print_hook_json(&serde_json::json!({"continue":true}));
    }
    0
}

async fn report_hook_lifecycle_with_host_budget(
    event: HookEvent,
    session_id: &str,
) -> Result<(), String> {
    enforce_hook_lifecycle_report_timeout(event, report_hook_lifecycle(event, session_id)).await
}

async fn enforce_hook_lifecycle_report_timeout<F>(event: HookEvent, report: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    let Some(timeout) = event.lifecycle_report_timeout() else {
        return report.await;
    };
    tokio::time::timeout(timeout, report).await.map_err(|_| {
        format!(
            "{} lifecycle observation exceeded the {} ms internal Hook budget",
            event.hook_event_name(),
            timeout.as_millis()
        )
    })?
}

fn lifecycle_identifier_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.contains('\0')
}

async fn report_hook_lifecycle(event: HookEvent, session_id: &str) -> Result<(), String> {
    let install_root = current_install_root()?;
    let controller = VerifiedControllerImage::from_install_directory(&install_root)
        .map_err(|_| "installed Controller identity could not be verified".to_owned())?;
    let client = ControllerClient::new(
        cli_client_config(controller.path().to_path_buf())
            .map_err(|_| "Controller IPC configuration is unavailable".to_owned())?,
    );
    // Hook input intentionally exposes a stable session ID but not a desktop
    // PID.  Attribute a parent only when the local process snapshot proves a
    // `ChatGPT.exe` ancestor; update shutdown continues to require the
    // updater's stricter exact-image census.
    let owner_pid = star_updater_core::process_census::current_codex_desktop_owner_pid();
    let instance_id = owner_pid.map_or_else(
        || format!("codex-session:{session_id}"),
        |pid| format!("codex-desktop:{pid}"),
    );
    let response = client
        .call_with_verified_start(
            &controller,
            "lifecycle.observe",
            serde_json::json!({
                "event": event.lifecycle_event(),
                "instance_id": instance_id,
                "task_id": session_id,
                "owner_pid": owner_pid,
            }),
            RequestId::new(),
        )
        .await
        .map_err(|error| error.to_string())?;
    if response.status != star_contracts::ipc::IpcStatus::Ok {
        return Err("Controller rejected lifecycle observation".to_owned());
    }
    Ok(())
}

fn print_value(value: &impl serde::Serialize, json: bool) -> i32 {
    let rendered = if json {
        serde_json::to_string(value)
    } else {
        serde_json::to_string_pretty(value)
    };
    match rendered {
        Ok(rendered) => {
            println!("{rendered}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            4
        }
    }
}

fn print_windows_error(error: WindowsAdapterError) -> i32 {
    let exit = match error {
        WindowsAdapterError::ArchitectureMismatch => 6,
        WindowsAdapterError::InstallationConflict => 3,
        WindowsAdapterError::InvalidReleaseManifest
        | WindowsAdapterError::InvalidInstallationRecord
        | WindowsAdapterError::InvalidIntegrationRecord
        | WindowsAdapterError::InvalidRuntimeActivation => 2,
        _ => 4,
    };
    eprintln!("{error}");
    exit
}

fn print_codex_error(error: CodexAdapterError) -> i32 {
    let exit = match &error {
        CodexAdapterError::ActiveCodexDesktop => 7,
        CodexAdapterError::Installation(WindowsAdapterError::ArchitectureMismatch) => 6,
        CodexAdapterError::Installation(WindowsAdapterError::InstallationConflict) => 3,
        CodexAdapterError::InvalidTemplate | CodexAdapterError::InvalidRenderedPlugin => 2,
        _ => 4,
    };
    eprintln!("{error}");
    exit
}

fn print_autostart_error(error: AutostartError) -> i32 {
    let exit = if matches!(error, AutostartError::Conflict) {
        3
    } else {
        4
    };
    eprintln!("{error}");
    exit
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::fixed_mcp::FIXED_TOOLS;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn unknown_fixed_tool_references(value: &str) -> Vec<&str> {
        value
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .filter(|token| {
                token.starts_with("star_") && !FIXED_TOOLS.iter().any(|tool| tool.name == *token)
            })
            .collect()
    }

    #[test]
    fn session_start_hook_output_matches_operations_snapshot() {
        let output = session_start_hook_output();
        let serialized = serde_json::to_string(&output).unwrap();
        assert_eq!(
            serialized,
            r#"{"continue":true,"hookSpecificOutput":{"additionalContext":"`star-control-operations` 지침을 따른다. `orchestrate-parallel-implementation`의 implicit invocation은 새 Codex task/thread 승인이나 병렬 위임이 아니다. 일반 구현은 현재 task single-agent로 수행한다. 명시 승인 Bundle은 `list_projects({})` 뒤 unique bundle_id/BOOTSTRAP_ONLY만 든 prompt와 `target:{type:project, projectId, environment:{type:worktree}}`로 bootstrap하고, threadId/hostId/cwd identity 확인 뒤 같은 threadId에 complete Context Pack + ACTIVATE_BUNDLE을 보낸다. activation 전 Goal/commentary/mutation/test/commit은 금지하며 clientThreadId만 있으면 bounded list_threads unique match가 resolve될 때까지 fail-closed한다. Terra는 WORKER_COMPLETE 한 번 뒤 멈추며 controller만 Sol review를 관찰하고, same threadId/Goal correction은 exact diff 승인 뒤에만 complete할 수 있다. Star-Control 작업은 fixed MCP Gateway와 Catalog-declared CLI-only 경계를 구분한다. MCP action을 사용할 때는 `star_tool_search`로 현재 registry를 검색하고 action readiness가 `ready`인 결과만 `star_tool_describe`로 다시 확인한다. describe에서 현재 Schema, 위험 lane, `descriptor_hash`, `required_call_tool`을 받은 뒤 그 tool에 `tool_id`, `descriptor_hash`, `arguments`를 전달한다. package나 manifest의 ready 상태는 action readiness가 아니다. Catalog에 MCP action이 없는 기능과 C01 Profile만 설치된 `star` CLI의 선언된 command로 실행하고 live `profile show|resolve` 결과를 고정한다. MCP action이 non-ready인 기능을 CLI로 우회하지 않는다. ready MCP action도 CLI-only command도 사용할 수 없으면 일반 Codex 개발 작업을 막지 말고 프로젝트 native 도구를 사용하며 fallback 사실과 이유와 Star-Control evidence 부재를 결과에 기록한다. `star_tool_registry_status`는 진단용이며 필수 선행 Gate가 아니다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다. `approval_required`, `question_required`와 Operation ID 반환은 완료가 아니다.","hookEventName":"SessionStart"}}"#
        );
        let context = output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(unknown_fixed_tool_references(context).is_empty());
        assert_eq!(
            unknown_fixed_tool_references(concat!("call `star_", "goal_start`")),
            [concat!("star_", "goal_start")]
        );
        assert!(context.contains("프로젝트 native 도구"));
        assert!(context.contains("fallback 사실과 이유"));
        assert!(
            context
                .contains("implicit invocation은 새 Codex task/thread 승인이나 병렬 위임이 아니다")
        );
        assert!(context.contains("list_projects({})"));
        assert!(context.contains("BOOTSTRAP_ONLY만 든 prompt"));
    }

    #[test]
    fn parses_local_lifecycle_without_controller_state() {
        assert!(
            parse(&args(&[
                "installation",
                "finalize",
                "--architecture",
                compiled_architecture().unwrap().as_str(),
                "--replace-existing",
                "--json",
            ]))
            .unwrap()
            .unwrap()
            .json
        );
        assert!(matches!(
            parse(&args(&["integration", "repair", "--skip-register"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::IntegrationInstall {
                repair: true,
                skip_register: true,
                ..
            }
        ));
        assert!(matches!(
            parse(&args(&["hook", "session-start"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::Hook {
                event: HookEvent::SessionStart
            }
        ));
        assert!(matches!(
            parse(&args(&["hook", "session-end"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::Hook {
                event: HookEvent::SessionEnd
            }
        ));
        assert!(matches!(
            parse(&args(&[
                "integration",
                "repair",
                "restart",
                "--codex-desktop",
                r"C:\\Codex\\ChatGPT.exe",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::IntegrationRepairRestart { .. }
        ));
        assert!(matches!(
            parse(&args(&["controller", "autostart", "enable"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::ControllerAutostart { .. }
        ));
        assert!(matches!(
            parse(&args(&["update", "verify", "--json"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::UpdateVerify
        ));
        assert!(matches!(
            parse(&args(&[
                "installation",
                "bridge",
                "initialize",
                "--state-generation",
                "bootstrap_v2",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::InstallationBridgeInitialize { .. }
        ));
        assert!(matches!(
            parse(&args(&["update", "stage", "D:\\stage\\rt_candidate"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::UpdateStage { .. }
        ));
        assert!(matches!(
            parse(&args(&["update", "inspect", "rt_candidate"]))
                .unwrap()
                .unwrap()
                .command,
            LocalCommand::UpdateInspect { .. }
        ));
        assert!(matches!(
            parse(&args(&[
                "update",
                "inspect",
                r"D:\\stage\\star-control-x64",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::UpdateInspect { .. }
        ));
        assert!(matches!(
            parse(&args(&[
                "update",
                "apply",
                "rt_candidate",
                "--state-generation",
                "state_2",
                "--approve",
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::UpdateApply { .. }
        ));
        assert!(matches!(
            parse(&args(&[
                "update",
                "apply",
                r"D:\\stage\\star-control-x64",
                "--codex-desktop",
                r"C:\\Codex\\ChatGPT.exe",
                "--approve",
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::UpdateIntegrationApply { .. }
        ));
        assert!(matches!(
            parse(&args(&[
                "update",
                "offline-installer-restart",
                "--install-root",
                r"D:\\Star-Control",
                "--installer",
                r"D:\\dist\\setup.exe",
                "--codex-desktop",
                r"C:\\Codex\\ChatGPT.exe",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::UpdateOfflineInstallerRestart { .. }
        ));
        assert!(matches!(
            parse(&args(&[
                "update",
                "reconcile-installed-runtime",
                "--install-root",
                r"D:\Star-Control",
            ]))
            .unwrap()
            .unwrap()
            .command,
            LocalCommand::UpdateReconcileInstalledRuntime { .. }
        ));
    }

    #[test]
    fn session_end_uses_the_existing_bounded_root_stop_lifecycle() {
        assert_eq!(HookEvent::SessionEnd.hook_event_name(), "SessionEnd");
        assert_eq!(HookEvent::SessionEnd.lifecycle_event(), "root_stop");
    }

    #[test]
    fn session_end_lifecycle_report_budget_stays_inside_codex_host_timeout() {
        assert_eq!(
            HookEvent::SessionEnd.lifecycle_report_timeout(),
            Some(SESSION_END_LIFECYCLE_REPORT_TIMEOUT)
        );
        assert!(
            SESSION_END_LIFECYCLE_REPORT_TIMEOUT
                < Duration::from_secs(SESSION_END_CODEX_HOST_TIMEOUT_SECONDS)
        );
        assert_eq!(HookEvent::Stop.lifecycle_report_timeout(), None);
    }

    fn pre_tool_hook_input(command: &str) -> serde_json::Value {
        serde_json::json!({
            "session_id":"thr_fixture",
            "transcript_path":null,
            "cwd":r"D:\work\repo",
            "hook_event_name":"PreToolUse",
            "model":"gpt-5.6-sol",
            "permission_mode":"default",
            "turn_id":"turn_fixture",
            "tool_name":"Bash",
            "tool_use_id":"tool_fixture",
            "tool_input":{"command":command},
        })
    }

    #[test]
    fn typed_pre_tool_hook_denies_malformed_and_every_force_flag_position() {
        let malformed = serde_json::json!({
            "hook_event_name":"PreToolUse",
            "session_id":"thr_fixture",
        });
        assert!(validate_typed_hook_input(HookEvent::PreToolUse, &malformed).is_err());

        for command in [
            "git push --force origin main",
            "git push origin main --force",
            "git -C D:\\work\\repo push origin main -f",
            "pwsh -Command git push upstream HEAD --force-with-lease=abc",
            "git push origin +main:main",
            "git push -vf origin main",
        ] {
            let value = pre_tool_hook_input(command);
            let input = validate_typed_hook_input(HookEvent::PreToolUse, &value).unwrap();
            assert_eq!(
                pre_tool_use_denial_reason(&input).as_deref(),
                Some("force push is forbidden regardless of flag position"),
                "{command}"
            );
        }
        let safe = pre_tool_hook_input("git push origin main");
        let safe = validate_typed_hook_input(HookEvent::PreToolUse, &safe).unwrap();
        assert!(pre_tool_use_denial_reason(&safe).is_none());
    }

    #[test]
    fn typed_pre_tool_hook_denies_destructive_git_generated_state_and_unbounded_paths() {
        for (command, reason_fragment) in [
            ("git reset HEAD --hard", "git reset --hard"),
            ("git clean -fd", "git clean"),
            (
                r"Set-Content C:\Users\u\.codex\plugins\cache\x\SKILL.md changed",
                "generated runtime state",
            ),
            ("Remove-Item -Recurse $target", "recursive delete or move"),
            ("Move-Item source destination", "recursive delete or move"),
        ] {
            let value = pre_tool_hook_input(command);
            let input = validate_typed_hook_input(HookEvent::PreToolUse, &value).unwrap();
            let reason = pre_tool_use_denial_reason(&input).unwrap();
            assert!(reason.contains(reason_fragment), "{command}: {reason}");
        }

        for command in [
            r"Remove-Item -LiteralPath D:\work\repo\tmp -Recurse",
            r"Move-Item -LiteralPath D:\work\repo\from -Destination D:\work\repo\to",
        ] {
            let value = pre_tool_hook_input(command);
            let input = validate_typed_hook_input(HookEvent::PreToolUse, &value).unwrap();
            assert!(pre_tool_use_denial_reason(&input).is_none(), "{command}");
        }
    }

    #[test]
    fn post_tool_hook_marks_non_terminal_states_without_claiming_rollback() {
        let value = serde_json::json!({
            "session_id":"thr_fixture",
            "transcript_path":null,
            "cwd":r"D:\work\repo",
            "hook_event_name":"PostToolUse",
            "model":"gpt-5.6-sol",
            "permission_mode":"default",
            "turn_id":"turn_fixture",
            "tool_name":"mcp__star__invoke",
            "tool_use_id":"tool_fixture",
            "tool_input":{"tool_id":"star.example"},
            "tool_response":{
                "status":"accepted",
                "operation_id":"op_fixture",
                "terminal":false,
                "evidence":{"completeness":"partial"}
            },
        });
        let input = validate_typed_hook_input(HookEvent::PostToolUse, &value).unwrap();
        let output = post_tool_use_context(&input).unwrap();
        let context = output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(context.contains("accepted"));
        assert!(context.contains("partial"));
        assert!(context.contains("op_fixture"));
        assert!(context.contains("side effect를 되돌리지 않는다"));
    }

    #[tokio::test]
    async fn session_end_cancels_a_stalled_lifecycle_report() {
        let result = enforce_hook_lifecycle_report_timeout(
            HookEvent::SessionEnd,
            std::future::pending::<Result<(), String>>(),
        )
        .await;
        assert_eq!(
            result,
            Err(
                "SessionEnd lifecycle observation exceeded the 2000 ms internal Hook budget"
                    .to_owned()
            )
        );
    }

    #[test]
    fn rejects_ambiguous_local_options() {
        assert!(parse(&args(&["installation", "finalize"])).is_err());
        assert!(
            parse(&args(&[
                "integration",
                "install",
                "--codex",
                "a.exe",
                "--codex",
                "b.exe",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["hook", "session-start", "--json"])).is_err());
        assert!(parse(&args(&["update", "apply", "rt_candidate"])).is_err());
        assert!(parse(&args(&["installation", "bridge", "initialize"])).is_err());
    }
}
