//! Controller-only management CLI.
//!
//! Parsing and rendering live here; package state, trust mutation and process
//! lifecycle are always requested through authenticated Controller IPC.

use std::path::PathBuf;

use star_contracts::{ids::RequestId, ipc::IpcStatus};
use star_ipc::client::{ControllerClient, cli_client_config};

const HELP: &str = "star tools list [--source release|user|project] [--readiness <value>] [--json]\n\
star tools describe <tool-id> [--json]\n\
star tools status [<package-id>] [--json]\n\
star tools validate <manifest-path> --source user|project [--json]\n\
star tools probe <package-id> [--executable <id>] [--json]\n\
star tools trust <package-id> --manifest-hash <sha256> [--expires <rfc3339>] [--json]\n\
star tools revoke <package-id> [--cancel-running] --reason <text> [--json]\n\
star tools scaffold <exe-path> --output <toml-path>\n\
star controller start [--background]\n\
star controller autostart enable|disable|status";

#[derive(Debug)]
struct Parsed {
    command: String,
    payload: serde_json::Value,
    json: bool,
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let json = args.iter().any(|arg| arg == "--json");
    let args: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--json")
        .cloned()
        .collect();
    let field = |name: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg.as_str() == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    match args.as_slice() {
        [first, second, ..] if first == "tools" && second == "list" => Ok(Parsed {
            command: "tool.search".to_owned(),
            payload: serde_json::json!({"query":"","sources":field("--source").map(|source| vec![source]),"readiness":field("--readiness").map(|readiness| vec![readiness])}),
            json,
        }),
        [first, second, tool_id, ..] if first == "tools" && second == "describe" => Ok(Parsed {
            command: "tool.describe".to_owned(),
            payload: serde_json::json!({"tool_id":tool_id}),
            json,
        }),
        [first, second, rest @ ..] if first == "tools" && second == "status" => Ok(Parsed {
            command: "tool.registry.status".to_owned(),
            payload: serde_json::json!({"package_id":rest.first(),"sources":field("--source").map(|source| vec![source])}),
            json,
        }),
        [first, second, manifest, ..] if first == "tools" && second == "validate" => Ok(Parsed {
            command: "tool.validate".to_owned(),
            payload: serde_json::json!({"manifest_path":manifest,"source":field("--source").ok_or("validate requires --source")?}),
            json,
        }),
        [first, second, package_id, ..] if first == "tools" && second == "probe" => Ok(Parsed {
            command: "tool.probe".to_owned(),
            payload: serde_json::json!({"package_id":package_id,"executable_id":field("--executable")}),
            json,
        }),
        [first, second, package_id, ..] if first == "tools" && second == "trust" => Ok(Parsed {
            command: "tool.trust".to_owned(),
            payload: serde_json::json!({"package_id":package_id,"manifest_hash":field("--manifest-hash").ok_or("trust requires --manifest-hash")?,"expires":field("--expires")}),
            json,
        }),
        [first, second, package_id, ..] if first == "tools" && second == "revoke" => Ok(Parsed {
            command: "tool.revoke".to_owned(),
            payload: serde_json::json!({"package_id":package_id,"cancel_running":args.iter().any(|arg| arg == "--cancel-running"),"reason":field("--reason").ok_or("revoke requires --reason")?}),
            json,
        }),
        [first, second, executable, ..] if first == "tools" && second == "scaffold" => Ok(Parsed {
            command: "tool.scaffold".to_owned(),
            payload: serde_json::json!({"executable_path":executable,"output_path":field("--output").ok_or("scaffold requires --output")?}),
            json,
        }),
        [first, second, rest @ ..] if first == "controller" && second == "start" => Ok(Parsed {
            command: "controller.start".to_owned(),
            payload: serde_json::json!({"background":rest.iter().any(|arg| arg == "--background")}),
            json,
        }),
        [first, second, action]
            if first == "controller"
                && second == "autostart"
                && ["enable", "disable", "status"].contains(&action.as_str()) =>
        {
            Ok(Parsed {
                command: format!("controller.autostart.{action}"),
                payload: serde_json::json!({}),
                json,
            })
        }
        _ => Err(HELP.to_owned()),
    }
}

fn controller_image() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|_| "cannot locate star.exe".to_owned())?;
    Ok(executable
        .parent()
        .ok_or("star.exe has no installation directory")?
        .join("star-controller.exe"))
}

fn exit_code(status: &IpcStatus) -> i32 {
    match status {
        IpcStatus::Ok | IpcStatus::Accepted => 0,
        IpcStatus::ApprovalRequired | IpcStatus::QuestionRequired | IpcStatus::Blocked => 3,
        IpcStatus::Error => 4,
    }
}

#[tokio::main]
async fn main() {
    let raw: Vec<_> = std::env::args().skip(1).collect();
    if raw
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "help")
        || raw.is_empty()
    {
        println!("{HELP}");
        return;
    }
    let parsed = match parse(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let config = match controller_image()
        .and_then(|image| cli_client_config(image).map_err(|error| error.to_string()))
    {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(4);
        }
    };
    let response = match ControllerClient::new(config)
        .call(&parsed.command, parsed.payload, RequestId::new())
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(4);
        }
    };
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&response).expect("response serializes")
        );
    } else if let Some(error) = &response.error {
        eprintln!("{}: {}", error.code, error.message);
    } else if let Some(data) = &response.data {
        println!(
            "{}",
            serde_json::to_string_pretty(data).expect("response data serializes")
        );
    } else {
        println!("{:?}", response.status);
    }
    std::process::exit(exit_code(&response.status));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn help_and_frozen_commands_parse_without_local_state() {
        let parsed = parse(&["tools".to_owned(), "list".to_owned(), "--json".to_owned()]).unwrap();
        assert_eq!(parsed.command, "tool.search");
        assert!(parsed.json);
        assert_eq!(
            parse(&["tools".to_owned(), "trust".to_owned(), "x".to_owned()]).unwrap_err(),
            "trust requires --manifest-hash"
        );
    }
}
