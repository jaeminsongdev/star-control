//! One-shot Star-Control update process.
//!
//! Integration restart orchestration is added after Controller lifecycle
//! admission is wired.  The Runtime Generation path is already delegated here
//! so `star.exe` no longer owns activation mutation.

#![windows_subsystem = "windows"]

use std::{path::PathBuf, str::FromStr};

use star_contracts::Sha256Hash;
use star_updater_core::{
    RuntimeApplyRequest, apply_runtime_generation,
    integration_restart::{
        IntegrationCandidateRestartRequest, IntegrationRepairRestartRequest,
        OfflineInstallerRestartRequest, apply_codex_integration_candidate_and_restart,
        repair_codex_integration_and_restart, run_offline_installer_and_restart,
    },
    process_census::{exact_image_instances, owned_process_tree, snapshot},
};

const HELP: &str = "star-updater runtime-apply <generation-id> --install-root <path> --state-generation <id> --approve <sha256> [--json]\nstar-updater offline-installer-restart --installer <absolute-path> --install-root <path> --codex-desktop <absolute-path>\nstar-updater integration-apply-restart <candidate-release-root> --install-root <path> --codex-desktop <absolute-path> --approve <sha256>\nstar-updater integration-repair-restart --install-root <path> --codex-desktop <absolute-path>\nstar-updater census --codex-exe <absolute-path>";

enum Command {
    RuntimeApply(RuntimeApplyRequest),
    IntegrationCandidateRestart(IntegrationCandidateRestartRequest),
    OfflineInstallerRestart(OfflineInstallerRestartRequest),
    IntegrationRepairRestart(IntegrationRepairRestartRequest),
    Census { codex_executable: PathBuf },
}

#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let exit = match parse(&args) {
        Ok(Some(Command::RuntimeApply(request))) => match apply_runtime_generation(request).await {
            Ok(outcome) => {
                println!(
                    "{}",
                    serde_json::to_string(&outcome).expect("runtime outcome serializes")
                );
                if matches!(
                    outcome,
                    star_updater_core::RuntimeApplyOutcome::Committed { .. }
                ) {
                    0
                } else {
                    4
                }
            }
            Err(error) => {
                eprintln!("{error}");
                4
            }
        },
        Ok(Some(Command::Census { codex_executable })) => match snapshot() {
            Ok(processes) => {
                let owned = owned_process_tree(&processes, &codex_executable);
                let instances = exact_image_instances(&processes, &codex_executable);
                println!(
                    "{}",
                    serde_json::json!({
                        "codex_executable": codex_executable,
                        "instance_count": instances.len(),
                        "instance_pids": instances.into_iter().map(|process| process.pid).collect::<Vec<_>>(),
                        "owned_processes": owned.into_iter().map(|process| serde_json::json!({
                            "pid": process.pid,
                            "parent_pid": process.parent_pid,
                            "image": process.image,
                        })).collect::<Vec<_>>(),
                    })
                );
                0
            }
            Err(error) => {
                eprintln!("{error}");
                4
            }
        },
        Ok(Some(Command::IntegrationRepairRestart(request))) => {
            match repair_codex_integration_and_restart(request).await {
                Ok(outcome) => {
                    println!(
                        "{}",
                        serde_json::to_string(&outcome).expect("outcome serializes")
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    4
                }
            }
        }
        Ok(Some(Command::IntegrationCandidateRestart(request))) => {
            match apply_codex_integration_candidate_and_restart(request).await {
                Ok(outcome) => {
                    println!(
                        "{}",
                        serde_json::to_string(&outcome).expect("outcome serializes")
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    4
                }
            }
        }
        Ok(Some(Command::OfflineInstallerRestart(request))) => {
            match run_offline_installer_and_restart(request).await {
                Ok(outcome) => {
                    println!(
                        "{}",
                        serde_json::to_string(&outcome).expect("outcome serializes")
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    4
                }
            }
        }
        Ok(None) => {
            eprintln!("{HELP}");
            2
        }
        Err(error) => {
            eprintln!("{error}\n{HELP}");
            2
        }
    };
    std::process::exit(exit);
}

fn parse(args: &[String]) -> Result<Option<Command>, String> {
    if let [command, tail @ ..] = args
        && command == "offline-installer-restart"
    {
        let mut install_root = None;
        let mut codex_desktop = None;
        let mut installer = None;
        let mut index = 0;
        while index < tail.len() {
            if index + 1 >= tail.len() {
                return Err(format!("{} requires one value", tail[index]));
            }
            match tail[index].as_str() {
                "--install-root" if install_root.is_none() => {
                    install_root = Some(PathBuf::from(&tail[index + 1]))
                }
                "--codex-desktop" if codex_desktop.is_none() => {
                    codex_desktop = Some(PathBuf::from(&tail[index + 1]))
                }
                "--installer" if installer.is_none() => {
                    installer = Some(PathBuf::from(&tail[index + 1]))
                }
                value => return Err(format!("unknown or duplicate option: {value}")),
            }
            index += 2;
        }
        let installer_executable = installer.ok_or("--installer is required".to_owned())?;
        let codex_desktop_executable =
            codex_desktop.ok_or("--codex-desktop is required".to_owned())?;
        if !installer_executable.is_absolute() || !codex_desktop_executable.is_absolute() {
            return Err("--installer and --codex-desktop must be absolute paths".to_owned());
        }
        return Ok(Some(Command::OfflineInstallerRestart(
            OfflineInstallerRestartRequest {
                install_root: install_root.ok_or("--install-root is required".to_owned())?,
                installer_executable,
                codex_desktop_executable,
            },
        )));
    }
    if let [command, candidate_root, tail @ ..] = args
        && command == "integration-apply-restart"
    {
        let mut install_root = None;
        let mut codex_desktop = None;
        let mut approval_scope_sha256 = None;
        let mut index = 0;
        while index < tail.len() {
            if index + 1 >= tail.len() {
                return Err(format!("{} requires one value", tail[index]));
            }
            match tail[index].as_str() {
                "--install-root" if install_root.is_none() => {
                    install_root = Some(PathBuf::from(&tail[index + 1]))
                }
                "--codex-desktop" if codex_desktop.is_none() => {
                    codex_desktop = Some(PathBuf::from(&tail[index + 1]))
                }
                "--approve" if approval_scope_sha256.is_none() => {
                    approval_scope_sha256 = Some(
                        Sha256Hash::from_str(&tail[index + 1])
                            .map_err(|_| "--approve must be a sha256 digest".to_owned())?,
                    )
                }
                value => return Err(format!("unknown or duplicate option: {value}")),
            }
            index += 2;
        }
        let candidate_root = PathBuf::from(candidate_root);
        let codex_desktop_executable =
            codex_desktop.ok_or("--codex-desktop is required".to_owned())?;
        if !candidate_root.is_absolute() || !codex_desktop_executable.is_absolute() {
            return Err("candidate root and --codex-desktop must be absolute paths".to_owned());
        }
        return Ok(Some(Command::IntegrationCandidateRestart(
            IntegrationCandidateRestartRequest {
                install_root: install_root.ok_or("--install-root is required".to_owned())?,
                candidate_root,
                approval_scope_sha256: approval_scope_sha256
                    .ok_or("--approve is required".to_owned())?,
                codex_desktop_executable,
            },
        )));
    }
    if let [command, tail @ ..] = args
        && command == "integration-repair-restart"
    {
        let mut install_root = None;
        let mut codex_desktop = None;
        let mut index = 0;
        while index < tail.len() {
            if index + 1 >= tail.len() {
                return Err(format!("{} requires one value", tail[index]));
            }
            match tail[index].as_str() {
                "--install-root" if install_root.is_none() => {
                    install_root = Some(PathBuf::from(&tail[index + 1]))
                }
                "--codex-desktop" if codex_desktop.is_none() => {
                    codex_desktop = Some(PathBuf::from(&tail[index + 1]))
                }
                value => return Err(format!("unknown or duplicate option: {value}")),
            }
            index += 2;
        }
        let codex_desktop_executable =
            codex_desktop.ok_or("--codex-desktop is required".to_owned())?;
        if !codex_desktop_executable.is_absolute() {
            return Err("--codex-desktop must be an absolute path".to_owned());
        }
        return Ok(Some(Command::IntegrationRepairRestart(
            IntegrationRepairRestartRequest {
                install_root: install_root.ok_or("--install-root is required".to_owned())?,
                codex_desktop_executable,
            },
        )));
    }
    if let [command, option, executable] = args
        && command == "census"
        && option == "--codex-exe"
    {
        let executable = PathBuf::from(executable);
        if !executable.is_absolute() {
            return Err("--codex-exe must be an absolute executable path".to_owned());
        }
        return Ok(Some(Command::Census {
            codex_executable: executable,
        }));
    }
    let [command, generation_id, tail @ ..] = args else {
        return Ok(None);
    };
    if command != "runtime-apply" || generation_id.trim().is_empty() {
        return Ok(None);
    }
    let mut install_root = None;
    let mut state_generation_id = None;
    let mut approval_scope_sha256 = None;
    let mut index = 0;
    while index < tail.len() {
        if tail[index] == "--json" {
            index += 1;
            continue;
        }
        if index + 1 >= tail.len() {
            return Err(format!("{} requires one value", tail[index]));
        }
        match tail[index].as_str() {
            "--install-root" if install_root.is_none() => {
                install_root = Some(PathBuf::from(&tail[index + 1]))
            }
            "--state-generation" if state_generation_id.is_none() => {
                let value = tail[index + 1].trim();
                if value.is_empty() || value.chars().count() > 128 {
                    return Err("--state-generation must be a bounded non-empty id".to_owned());
                }
                state_generation_id = Some(value.to_owned());
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
    Ok(Some(Command::RuntimeApply(RuntimeApplyRequest {
        install_root: install_root.ok_or("--install-root is required".to_owned())?,
        generation_id: generation_id.to_owned(),
        state_generation_id: state_generation_id
            .ok_or("--state-generation is required".to_owned())?,
        approval_scope_sha256: approval_scope_sha256.ok_or("--approve is required".to_owned())?,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_apply_requires_exact_mutation_binding() {
        let args = vec!["runtime-apply".to_owned(), "rt_example".to_owned()];
        assert!(parse(&args).is_err());
    }

    #[test]
    fn census_requires_an_exact_absolute_codex_image() {
        assert!(
            parse(&[
                "census".to_owned(),
                "--codex-exe".to_owned(),
                "Codex.exe".to_owned()
            ])
            .is_err()
        );
        assert!(matches!(
            parse(&[
                "census".to_owned(),
                "--codex-exe".to_owned(),
                r"C:\\Codex.exe".to_owned()
            ])
            .unwrap(),
            Some(Command::Census { .. })
        ));
    }

    #[test]
    fn integration_restart_requires_both_bound_paths() {
        assert!(parse(&["integration-repair-restart".to_owned()]).is_err());
        assert!(matches!(
            parse(&[
                "integration-repair-restart".to_owned(),
                "--install-root".to_owned(),
                r"D:\\Star-Control".to_owned(),
                "--codex-desktop".to_owned(),
                r"C:\\Codex\\ChatGPT.exe".to_owned(),
            ])
            .unwrap(),
            Some(Command::IntegrationRepairRestart(_))
        ));
    }

    #[test]
    fn integration_candidate_restart_requires_candidate_desktop_and_approval() {
        assert!(
            parse(&[
                "integration-apply-restart".to_owned(),
                r"D:\\stage".to_owned(),
            ])
            .is_err()
        );
        assert!(matches!(
            parse(&[
                "integration-apply-restart".to_owned(),
                r"D:\\stage".to_owned(),
                "--install-root".to_owned(),
                r"D:\\Star-Control".to_owned(),
                "--codex-desktop".to_owned(),
                r"C:\\Codex\\ChatGPT.exe".to_owned(),
                "--approve".to_owned(),
                format!("sha256:{}", "0".repeat(64)),
            ])
            .unwrap(),
            Some(Command::IntegrationCandidateRestart(_))
        ));
    }

    #[test]
    fn offline_installer_restart_requires_every_bound_path() {
        assert!(parse(&["offline-installer-restart".to_owned()]).is_err());
        assert!(matches!(
            parse(&[
                "offline-installer-restart".to_owned(),
                "--installer".to_owned(),
                r"D:\\dist\\setup.exe".to_owned(),
                "--install-root".to_owned(),
                r"D:\\Star-Control".to_owned(),
                "--codex-desktop".to_owned(),
                r"C:\\Codex\\ChatGPT.exe".to_owned(),
            ])
            .unwrap(),
            Some(Command::OfflineInstallerRestart(_))
        ));
    }
}
