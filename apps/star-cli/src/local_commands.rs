use std::{io::Read, path::PathBuf};

use star_adapter_codex::{CodexAdapterError, CodexIntegrationManager, IntegrationOptions};
use star_adapter_windows::autostart::{self, AutostartError, AutostartState};
#[cfg(test)]
use star_adapter_windows::compiled_architecture;
use star_adapter_windows::{InstallationManager, WindowsAdapterError};
use star_contracts::{installation::TargetArchitecture, parse_no_duplicate_keys};

const HOOK_INPUT_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
enum LocalCommand {
    InstallationFinalize {
        architecture: TargetArchitecture,
        replace_existing: bool,
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

pub fn dispatch(args: &[String]) -> Option<i32> {
    let parsed = match parse(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return None,
        Err(error) => {
            eprintln!("{error}");
            return Some(2);
        }
    };
    Some(run(parsed))
}

fn parse(args: &[String]) -> Result<Option<ParsedLocal>, String> {
    let is_local = args.first().is_some_and(|value| {
        matches!(value.as_str(), "installation" | "integration" | "hook")
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

fn run(parsed: ParsedLocal) -> i32 {
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

fn current_install_root() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|_| "cannot locate star.exe".to_owned())?;
    executable
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "star.exe has no installation directory".to_owned())
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
    let output = serde_json::json!({
        "continue": true,
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": "Star-Control 연동이 활성화되어 있다. 개발 변경 목표는 Star-Control MCP의 star_goal_start로 시작하고 반환된 단계·승인·검증 상태를 통해 계속 관리한다. MCP를 사용할 수 없으면 우회하지 말고 연결 실패를 사용자에게 알린다."
        }
    });
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
        | WindowsAdapterError::InvalidIntegrationRecord => 2,
        _ => 4,
    };
    eprintln!("{error}");
    exit
}

fn print_codex_error(error: CodexAdapterError) -> i32 {
    let exit = match &error {
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

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
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
    }
}
