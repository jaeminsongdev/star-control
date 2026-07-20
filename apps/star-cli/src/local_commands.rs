use std::{io::Read, path::PathBuf, str::FromStr, time::Duration};

use star_adapter_codex::{CodexAdapterError, CodexIntegrationManager, IntegrationOptions};
use star_adapter_windows::autostart::{self, AutostartError, AutostartState};
#[cfg(test)]
use star_adapter_windows::compiled_architecture;
use star_adapter_windows::{
    InstallationManager, WindowsAdapterError, load_runtime_generation_manifest,
};
use star_contracts::{
    Sha256Hash,
    fixed_mcp::SERVER_INSTRUCTIONS,
    ids::RequestId,
    installation::{RuntimeActivationRecord, RuntimeGenerationRef, TargetArchitecture},
    parse_no_duplicate_keys,
};
use star_ipc::{
    client::{ControllerClient, ControllerClientError, cli_client_config},
    controller_start::VerifiedControllerImage,
};
use star_updater_core::{
    integration_restart::latest_integration_restart_receipt, spawn_background_updater,
};

const HOOK_INPUT_MAX_BYTES: u64 = 1024 * 1024;
const SESSION_START_SKILL_NAME: &str = "star-control-operations";

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
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::Stop => "root_stop",
            Self::PreToolUse => "tool_started",
            Self::PostToolUse => "tool_finished",
            Self::SubagentStart => "subagent_started",
            Self::SubagentStop => "subagent_finished",
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

async fn apply_runtime_generation_legacy(
    install_root: &std::path::Path,
    generation_id: String,
    state_generation_id: String,
    approval_scope_sha256: Sha256Hash,
    json: bool,
) -> i32 {
    let manager = match InstallationManager::for_current_user() {
        Ok(manager) => manager,
        Err(error) => return print_windows_error(error),
    };
    let review = match manager.inspect_runtime_candidate(install_root, &generation_id) {
        Ok(review) => review,
        Err(error) => return print_windows_error(error),
    };
    if review.approval_scope_sha256 != approval_scope_sha256
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
        eprintln!("runtime candidate does not satisfy the approved apply gate");
        return 3;
    }
    let prior = match manager.load_runtime_activation_record(install_root) {
        Ok(record) => record,
        Err(error) => return print_windows_error(error),
    };
    let candidate_root = install_root
        .join("runtime")
        .join("generations")
        .join(&generation_id);
    let candidate_manifest = match load_runtime_generation_manifest(&candidate_root) {
        Ok(manifest) => manifest,
        Err(error) => return print_windows_error(error),
    };
    let candidate = RuntimeGenerationRef {
        generation_id: candidate_manifest.generation.generation_id,
        runtime_root: candidate_root
            .canonicalize()
            .unwrap_or(candidate_root)
            .display()
            .to_string(),
        release_manifest_sha256: candidate_manifest.generation.release_manifest_sha256,
    };
    let old_bootstrap = match VerifiedControllerImage::from_install_directory(install_root) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("{error}");
            return 4;
        }
    };
    let old_client = match cli_client_config(old_bootstrap.path().to_path_buf()) {
        Ok(config) => ControllerClient::new(config),
        Err(error) => {
            eprintln!("{error}");
            return 4;
        }
    };
    match old_client
        .call(
            "controller.shutdown",
            serde_json::json!({}),
            RequestId::new(),
        )
        .await
    {
        Ok(response) if response.status == star_contracts::ipc::IpcStatus::Ok => {}
        Ok(_) => {
            eprintln!("controller refused the supervised shutdown request");
            return 4;
        }
        Err(ControllerClientError::Unavailable) => {}
        Err(error) => {
            eprintln!("{error}");
            return 4;
        }
    }
    let mut stopped = false;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if matches!(
            old_client
                .call("controller.start", serde_json::json!({}), RequestId::new())
                .await,
            Err(ControllerClientError::Unavailable)
        ) {
            stopped = true;
            break;
        }
    }
    if !stopped {
        eprintln!("controller did not quiesce within the bounded update window");
        return 4;
    }
    let next = RuntimeActivationRecord {
        schema_id: "star.runtime-activation-record".to_owned(),
        schema_version: 1,
        activation_revision: prior.activation_revision.saturating_add(1),
        active: candidate.clone(),
        previous: Some(prior.active.clone()),
        state_generation_id,
        bridge_contract_version: prior.bridge_contract_version,
        activated_at: chrono::Utc::now(),
    };
    if let Err(error) =
        manager.activate_runtime_bridge(install_root, &next, prior.bridge_contract_version)
    {
        let _ = old_bootstrap.start_background();
        return print_windows_error(error);
    }
    let new_bootstrap = match VerifiedControllerImage::from_install_directory(install_root) {
        Ok(image) => image,
        Err(error) => {
            return rollback_runtime_generation(
                &manager,
                install_root,
                &prior,
                candidate,
                error.to_string(),
                json,
            );
        }
    };
    if let Err(error) = new_bootstrap.start_background() {
        return rollback_runtime_generation(
            &manager,
            install_root,
            &prior,
            candidate,
            error.to_string(),
            json,
        );
    }
    let new_client = match cli_client_config(new_bootstrap.path().to_path_buf()) {
        Ok(config) => ControllerClient::new(config),
        Err(error) => {
            return rollback_runtime_generation(
                &manager,
                install_root,
                &prior,
                candidate,
                error.to_string(),
                json,
            );
        }
    };
    let mut postcheck_ok = false;
    for _ in 0..40 {
        match new_client
            .call("controller.start", serde_json::json!({}), RequestId::new())
            .await
        {
            Ok(response) if response.status == star_contracts::ipc::IpcStatus::Ok => {
                postcheck_ok = true;
                break;
            }
            Ok(_) => break,
            Err(ControllerClientError::Unavailable) => {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(_) => break,
        }
    }
    if !postcheck_ok {
        return rollback_runtime_generation(
            &manager,
            install_root,
            &prior,
            candidate,
            "new controller postcheck failed".to_owned(),
            json,
        );
    }
    print_value(
        &serde_json::json!({
            "state":"committed",
            "activation_revision":next.activation_revision,
            "active":next.active,
            "candidate_review":review,
            "requires_codex_restart":false,
        }),
        json,
    )
}

fn rollback_runtime_generation(
    manager: &InstallationManager,
    install_root: &std::path::Path,
    prior: &RuntimeActivationRecord,
    candidate: RuntimeGenerationRef,
    failure: String,
    json: bool,
) -> i32 {
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
    match manager.activate_runtime_bridge(install_root, &rollback, prior.bridge_contract_version) {
        Ok(()) => {
            let _ = VerifiedControllerImage::from_install_directory(install_root)
                .and_then(|image| image.start_background());
            print_value(
                &serde_json::json!({"state":"rolled_back","failure":failure}),
                json,
            );
            4
        }
        Err(error) => {
            eprintln!("runtime update failed ({failure}); rollback also failed: {error}");
            5
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
                "`{SESSION_START_SKILL_NAME}` 지침을 따른다. {SERVER_INSTRUCTIONS}"
            )
        }
    })
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
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    }
    let Ok(text) = std::str::from_utf8(&input) else {
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    };
    let Ok(value) = parse_no_duplicate_keys(text) else {
        eprintln!("invalid {} hook input", event.hook_event_name());
        return 2;
    };
    if value
        .get("hook_event_name")
        .and_then(|value| value.as_str())
        != Some(event.hook_event_name())
    {
        eprintln!("hook_event_name must be {}", event.hook_event_name());
        return 2;
    }
    let Some(session_id) = value.get("session_id").and_then(serde_json::Value::as_str) else {
        eprintln!("{} hook input has no session_id", event.hook_event_name());
        return 2;
    };
    if !lifecycle_identifier_valid(session_id) {
        eprintln!(
            "{} hook input has an invalid session_id",
            event.hook_event_name()
        );
        return 2;
    }
    if let Err(error) = report_hook_lifecycle(event, session_id).await {
        // A Hook must not turn a healthy Codex task into a failure merely
        // because the optional Controller is currently unavailable.  The
        // updater treats missing census evidence as a block, never as proof
        // that a task is absent.
        eprintln!("Star-Control lifecycle observation was not recorded: {error}");
    }
    if event == HookEvent::SessionStart {
        let output = session_start_hook_output();
        println!(
            "{}",
            serde_json::to_string(&output).expect("hook output serializes")
        );
    }
    0
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
            r#"{"continue":true,"hookSpecificOutput":{"additionalContext":"`star-control-operations` 지침을 따른다. Star-Control action을 사용할 때는 `star_tool_search`로 현재 registry를 검색하고 action readiness가 `ready`인 결과만 `star_tool_describe`로 다시 확인한다. describe에서 현재 Schema, 위험 lane, `descriptor_hash`, `required_call_tool`을 받은 뒤 그 tool에 `tool_id`, `descriptor_hash`, `arguments`를 전달한다. package나 manifest의 ready 상태는 action readiness가 아니다. 검색 결과가 없거나 action이 non-ready이거나 MCP 연결이 실패하면 일반 Codex 개발 작업을 막지 말고 프로젝트 native 도구를 사용하며 fallback 사실과 이유를 결과에 기록한다. `star_tool_registry_status`는 진단용이며 필수 선행 Gate가 아니다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다. `approval_required`, `question_required`와 Operation ID 반환은 완료가 아니다.","hookEventName":"SessionStart"}}"#
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
