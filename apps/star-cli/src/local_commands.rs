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
    ControllerAutostart {
        action: String,
    },
    HookSessionStart,
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
            let (state_generation_id, approval_scope_sha256) = parse_update_apply_options(tail)?;
            LocalCommand::UpdateApply {
                generation_id: generation_id.clone(),
                state_generation_id,
                approval_scope_sha256,
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
        [first, second] if first == "hook" && second == "session-start" && !json => {
            LocalCommand::HookSessionStart
        }
        [first, second] if first == "hook" && second == "session-start" => {
            return Err("hook session-start does not accept --json".to_owned());
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
    if matches!(parsed.command, LocalCommand::HookSessionStart) {
        return run_session_start_hook();
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
            print_value(
                &RuntimeUpdateStatus {
                    activation_record_path: path.display().to_string(),
                    active_runtime_generation,
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
            match manager.inspect_runtime_candidate(&install_root, &generation_id) {
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
        LocalCommand::HookSessionStart => unreachable!(),
    }
}

async fn apply_runtime_generation(
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

fn run_session_start_hook() -> i32 {
    let mut input = Vec::new();
    if std::io::stdin()
        .take(HOOK_INPUT_MAX_BYTES + 1)
        .read_to_end(&mut input)
        .is_err()
        || input.is_empty()
        || input.len() as u64 > HOOK_INPUT_MAX_BYTES
    {
        eprintln!("invalid SessionStart hook input");
        return 2;
    }
    let Ok(text) = std::str::from_utf8(&input) else {
        eprintln!("invalid SessionStart hook input");
        return 2;
    };
    let Ok(value) = parse_no_duplicate_keys(text) else {
        eprintln!("invalid SessionStart hook input");
        return 2;
    };
    if value
        .get("hook_event_name")
        .and_then(|value| value.as_str())
        != Some("SessionStart")
    {
        eprintln!("hook_event_name must be SessionStart");
        return 2;
    }
    let output = session_start_hook_output();
    println!(
        "{}",
        serde_json::to_string(&output).expect("hook output serializes")
    );
    0
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
            LocalCommand::HookSessionStart
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
