//! Controller-only management CLI.
//!
//! Parsing and rendering live here; package state, trust mutation and process
//! lifecycle are always requested through authenticated Controller IPC.

mod local_commands;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use star_contracts::{
    ids::RequestId,
    ipc::{IpcResponse, IpcStatus},
};
use star_ipc::client::{ControllerClient, ControllerClientError, cli_client_config};
use star_ipc::controller_start::VerifiedControllerImage;

const VERSION: &str = concat!("Star-Control ", env!("CARGO_PKG_VERSION"));
const HELP: &str = "star tools list [--source release|user|project] [--readiness <value>] [--json]\n\
star tools describe <tool-id> [--json]\n\
star tools status [<package-id>] [--json]\n\
star tools validate <manifest-path> --source user|project [--json]\n\
star tools probe <package-id> [--executable <id>] [--json]\n\
star tools trust <package-id> --manifest-hash <sha256> [--expires <rfc3339>] [--json]\n\
star tools revoke <package-id> [--cancel-running] --reason <text> [--json]\n\
star tools scaffold <exe-path> --output <toml-path>\n\
star doctor [--json]\n\
star project register [--json]\n\
star project list [--json]\n\
star project status <project-key> [--json]\n\
star validation plan <project-key> [--profile quick|target|full|release] [--unit <unit>] [--json]\n\
star validation run <project-key> [--profile quick|target|full|release] [--unit <unit>] [--timeout-ms <milliseconds>] [--json]\n\
star evidence get <project-key> <evidence-ref> [--json]\n\
star scan run <project-id> [--idempotency <key>] [--json]\n\
star finding list <project-id> [--json]\n\
star patch prepare <project-id> <finding-id> [--json]\n\
star patch apply <project-id> <patch-set-id> --approve <sha256> [--json]\n\
star management status [--json]\n\
star management retention plan [--json]\n\
star management retention apply --approve <sha256> [--json]\n\
star management rebuild plan [--json]\n\
star management rebuild apply --approve <sha256> [--json]\n\
star installation finalize --architecture x64|arm64 [--replace-existing] [--json]\n\
star installation bridge initialize --state-generation <id> [--json]\n\
star installation status [--json]\n\
star integration install|repair [--codex <exe>] [--skip-register] [--json]\n\
star integration repair restart --codex-desktop <absolute-exe> [--json]\n\
star integration status [--json]\n\
star integration uninstall [--codex <exe>] [--json]\n\
star update status [--json]\n\
star update verify [--json]\n\
star update stage <runtime-generation-dir> [--json]\n\
star update inspect <generation-id|absolute-release-stage> [--json]\n\
star update apply <generation-id> --state-generation <id> --approve <sha256> [--json]\n\
star hook session-start\n\
star controller start [--background]\n\
star controller autostart enable|disable|status";

#[derive(Debug)]
struct Parsed {
    command: String,
    payload: serde_json::Value,
    json: bool,
}

type ParsedTail = (Vec<String>, BTreeMap<String, Option<String>>);

fn parse(args: &[String]) -> Result<Parsed, String> {
    let json_count = args.iter().filter(|arg| arg.as_str() == "--json").count();
    if json_count > 1 {
        return Err("--json may be supplied only once".to_owned());
    }
    let json = json_count == 1;
    let args: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--json")
        .cloned()
        .collect();
    match args.as_slice() {
        [first, second, tail @ ..] if first == "tools" && second == "list" => {
            let (positionals, options) = parse_tail(tail, &["--source", "--readiness"], &[])?;
            require_positionals(&positionals, 0, "tools list")?;
            let source = options.get("--source").and_then(Clone::clone);
            if source
                .as_ref()
                .is_some_and(|value| !matches!(value.as_str(), "release" | "user" | "project"))
            {
                return Err("--source must be release, user, or project".to_owned());
            }
            let readiness = options.get("--readiness").and_then(Clone::clone);
            if readiness.as_ref().is_some_and(|value| {
                !matches!(
                    value.as_str(),
                    "ready" | "unavailable" | "untrusted" | "incompatible" | "degraded"
                )
            }) {
                return Err("--readiness has an unsupported value".to_owned());
            }
            Ok(Parsed {
                command: "tool.search".to_owned(),
                payload: serde_json::json!({
                    "query":"",
                    "sources":source.map(|value| vec![value]),
                    "readiness":readiness.map_or_else(
                        || ["ready","unavailable","untrusted","incompatible","degraded"]
                            .into_iter()
                            .map(str::to_owned)
                            .collect::<Vec<_>>(),
                        |value| vec![value]
                    ),
                    "limit":50
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "describe" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "tools describe")?;
            Ok(Parsed {
                command: "tool.describe".to_owned(),
                payload: serde_json::json!({"tool_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            if positionals.len() > 1 {
                return Err("tools status accepts at most one package ID".to_owned());
            }
            Ok(Parsed {
                command: "tool.registry.status".to_owned(),
                payload: serde_json::json!({"package_id":positionals.first(),"limit":200}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "validate" => {
            let (positionals, options) = parse_tail(tail, &["--source"], &[])?;
            require_positionals(&positionals, 1, "tools validate")?;
            let source = required_option(&options, "--source")?;
            if !matches!(source.as_str(), "user" | "project") {
                return Err("validate --source must be user or project".to_owned());
            }
            Ok(Parsed {
                command: "tool.validate".to_owned(),
                payload: serde_json::json!({"manifest_path":absolute_path(&positionals[0])?,"source":source}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "probe" => {
            let (positionals, options) = parse_tail(tail, &["--executable"], &[])?;
            require_positionals(&positionals, 1, "tools probe")?;
            Ok(Parsed {
                command: "tool.probe".to_owned(),
                payload: serde_json::json!({"package_id":positionals[0],"executable_id":options.get("--executable").and_then(Clone::clone)}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "trust" => {
            let (positionals, options) = parse_tail(tail, &["--manifest-hash", "--expires"], &[])?;
            require_positionals(&positionals, 1, "tools trust")?;
            let manifest_hash = required_option(&options, "--manifest-hash")?;
            manifest_hash
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--manifest-hash must be a lowercase sha256 value".to_owned())?;
            if let Some(expires) = options.get("--expires").and_then(Clone::clone) {
                chrono::DateTime::parse_from_rfc3339(&expires)
                    .map_err(|_| "--expires must be an RFC 3339 timestamp".to_owned())?;
            }
            Ok(Parsed {
                command: "tool.trust".to_owned(),
                payload: serde_json::json!({"package_id":positionals[0],"manifest_hash":manifest_hash,"expires":options.get("--expires").and_then(Clone::clone)}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "revoke" => {
            let (positionals, options) = parse_tail(tail, &["--reason"], &["--cancel-running"])?;
            require_positionals(&positionals, 1, "tools revoke")?;
            let reason = required_option(&options, "--reason")?;
            if reason.contains('\0') || reason.is_empty() || reason.chars().count() > 1_000 {
                return Err("--reason must contain 1 through 1000 characters".to_owned());
            }
            Ok(Parsed {
                command: "tool.revoke".to_owned(),
                payload: serde_json::json!({"package_id":positionals[0],"cancel_running":options.contains_key("--cancel-running"),"reason":reason}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "tools" && second == "scaffold" => {
            if json {
                return Err("tools scaffold does not support --json".to_owned());
            }
            let (positionals, options) = parse_tail(tail, &["--output"], &[])?;
            require_positionals(&positionals, 1, "tools scaffold")?;
            Ok(Parsed {
                command: "tool.scaffold".to_owned(),
                payload: serde_json::json!({"executable_path":absolute_path(&positionals[0])?,"output_path":absolute_path(&required_option(&options,"--output")?)?}),
                json,
            })
        }
        [first, tail @ ..] if first == "doctor" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "doctor")?;
            Ok(Parsed {
                command: "doctor.run".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "project" && second == "register" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "project register")?;
            Ok(Parsed {
                command: "project.register".to_owned(),
                payload: serde_json::json!({
                    "idempotency_key":RequestId::new().as_str(),
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "project" && second == "list" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "project list")?;
            Ok(Parsed {
                command: "project.list".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "project" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "project status")?;
            Ok(Parsed {
                command: "project.status".to_owned(),
                payload: serde_json::json!({"project_key":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "validation" && second == "plan" => {
            let (positionals, options) = parse_tail(tail, &["--profile", "--unit"], &[])?;
            require_positionals(&positionals, 1, "validation plan")?;
            let mut payload = serde_json::Map::from_iter([(
                "project_key".to_owned(),
                serde_json::Value::String(positionals[0].clone()),
            )]);
            if let Some(profile) = options.get("--profile").and_then(Clone::clone) {
                if !matches!(profile.as_str(), "quick" | "target" | "full" | "release") {
                    return Err("--profile must be quick, target, full, or release".to_owned());
                }
                payload.insert(
                    "requested_profile".to_owned(),
                    serde_json::Value::String(profile),
                );
            }
            if let Some(unit) = options.get("--unit").and_then(Clone::clone) {
                if unit.is_empty()
                    || unit.len() > 128
                    || !unit
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                {
                    return Err("--unit contains an invalid unit identifier".to_owned());
                }
                payload.insert("unit".to_owned(), serde_json::Value::String(unit));
            }
            Ok(Parsed {
                command: "validation.plan".to_owned(),
                payload: serde_json::Value::Object(payload),
                json,
            })
        }
        [first, second, tail @ ..] if first == "validation" && second == "run" => {
            let (positionals, options) =
                parse_tail(tail, &["--profile", "--unit", "--timeout-ms"], &[])?;
            require_positionals(&positionals, 1, "validation run")?;
            let mut payload = serde_json::Map::from_iter([(
                "project_key".to_owned(),
                serde_json::Value::String(positionals[0].clone()),
            )]);
            if let Some(profile) = options.get("--profile").and_then(Clone::clone) {
                if !matches!(profile.as_str(), "quick" | "target" | "full" | "release") {
                    return Err("--profile must be quick, target, full, or release".to_owned());
                }
                payload.insert(
                    "requested_profile".to_owned(),
                    serde_json::Value::String(profile),
                );
            }
            if let Some(unit) = options.get("--unit").and_then(Clone::clone) {
                if unit.is_empty()
                    || unit.len() > 128
                    || !unit
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                {
                    return Err("--unit contains an invalid unit identifier".to_owned());
                }
                payload.insert("unit".to_owned(), serde_json::Value::String(unit));
            }
            if let Some(timeout) = options.get("--timeout-ms").and_then(Clone::clone) {
                let timeout = timeout
                    .parse::<u64>()
                    .map_err(|_| "--timeout-ms must be an integer".to_owned())?;
                if !(1_000..=3_600_000).contains(&timeout) {
                    return Err("--timeout-ms must be between 1000 and 3600000".to_owned());
                }
                payload.insert("timeout_ms".to_owned(), timeout.into());
            }
            Ok(Parsed {
                command: "validation.run".to_owned(),
                payload: serde_json::Value::Object(payload),
                json,
            })
        }
        [first, second, tail @ ..] if first == "evidence" && second == "get" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "evidence get")?;
            let evidence_ref = &positionals[1];
            let parts: Vec<_> = evidence_ref.split('/').collect();
            if parts.len() != 4
                || parts[0] != "target"
                || parts[1] != "validation"
                || parts[2].is_empty()
                || !parts[2]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                || parts[3] != "report.json"
            {
                return Err("evidence-ref must be target/validation/<run>/report.json".to_owned());
            }
            Ok(Parsed {
                command: "evidence.get".to_owned(),
                payload: serde_json::json!({
                    "project_key":positionals[0],
                    "evidence_ref":evidence_ref,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "scan" && second == "run" => {
            let (positionals, options) = parse_tail(tail, &["--idempotency"], &[])?;
            require_positionals(&positionals, 1, "scan run")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "scan run requires a valid ProjectId".to_owned())?;
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            if idempotency_key.trim().is_empty()
                || idempotency_key.chars().count() > 128
                || idempotency_key.contains('\0')
            {
                return Err(
                    "--idempotency must contain 1 through 128 non-NUL characters".to_owned(),
                );
            }
            Ok(Parsed {
                command: "scan.run".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "finding" && second == "list" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "finding list")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "finding list requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "finding.list".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "patch" && second == "prepare" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "patch prepare")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "patch prepare requires a valid ProjectId".to_owned())?;
            star_contracts::ids::FindingId::parse(positionals[1].clone())
                .map_err(|_| "patch prepare requires a valid FindingId".to_owned())?;
            Ok(Parsed {
                command: "patch.prepare".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "finding_id":positionals[1],
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "patch" && second == "apply" => {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(&positionals, 2, "patch apply")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "patch apply requires a valid ProjectId".to_owned())?;
            star_contracts::ids::PatchSetId::parse(positionals[1].clone())
                .map_err(|_| "patch apply requires a valid PatchSetId".to_owned())?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact lowercase patch sha256".to_owned())?;
            Ok(Parsed {
                command: "patch.apply".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "patch_set_id":positionals[1],
                    "approved_patch_fingerprint":approval,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "management" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "management status")?;
            Ok(Parsed {
                command: "management.status".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "management" && second == "retention" && third == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "management retention plan")?;
            Ok(Parsed {
                command: "management.retention.plan".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "management" && second == "retention" && third == "apply" =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(&positionals, 0, "management retention apply")?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact retention plan sha256".to_owned())?;
            Ok(Parsed {
                command: "management.retention.apply".to_owned(),
                payload: serde_json::json!({"approved_plan_fingerprint":approval}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "management" && second == "rebuild" && third == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "management rebuild plan")?;
            Ok(Parsed {
                command: "management.rebuild.plan".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "management" && second == "rebuild" && third == "apply" =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(&positionals, 0, "management rebuild apply")?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact source rebuild plan sha256".to_owned())?;
            Ok(Parsed {
                command: "management.rebuild.apply".to_owned(),
                payload: serde_json::json!({"approved_plan_fingerprint":approval}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "controller" && second == "start" => {
            if json {
                return Err("controller start does not support --json".to_owned());
            }
            let (positionals, options) = parse_tail(tail, &[], &["--background"])?;
            require_positionals(&positionals, 0, "controller start")?;
            Ok(Parsed {
                command: "controller.start".to_owned(),
                payload: serde_json::json!({"background":options.contains_key("--background")}),
                json,
            })
        }
        [first, second, action]
            if first == "controller"
                && second == "autostart"
                && ["enable", "disable", "status"].contains(&action.as_str()) =>
        {
            if json {
                return Err("controller autostart does not support --json".to_owned());
            }
            Ok(Parsed {
                command: format!("controller.autostart.{action}"),
                payload: serde_json::json!({}),
                json,
            })
        }
        _ => Err(HELP.to_owned()),
    }
}

fn parse_tail(
    tail: &[String],
    value_options: &[&str],
    flag_options: &[&str],
) -> Result<ParsedTail, String> {
    let mut positionals = Vec::new();
    let mut options = BTreeMap::new();
    let mut index = 0;
    while index < tail.len() {
        let token = &tail[index];
        if !token.starts_with("--") {
            positionals.push(token.clone());
            index += 1;
            continue;
        }
        if options.contains_key(token) {
            return Err(format!("{token} may be supplied only once"));
        }
        if value_options.contains(&token.as_str()) {
            let value = tail
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("{token} requires a value"))?
                .clone();
            options.insert(token.clone(), Some(value));
            index += 2;
        } else if flag_options.contains(&token.as_str()) {
            options.insert(token.clone(), None);
            index += 1;
        } else {
            return Err(format!("unknown option: {token}"));
        }
    }
    Ok((positionals, options))
}

fn require_positionals(values: &[String], expected: usize, command: &str) -> Result<(), String> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{command} requires exactly {expected} positional argument(s)"
        ))
    }
}

fn required_option(
    options: &BTreeMap<String, Option<String>>,
    name: &str,
) -> Result<String, String> {
    options
        .get(name)
        .and_then(Clone::clone)
        .ok_or_else(|| format!("command requires {name}"))
}

fn absolute_path(value: &str) -> Result<String, String> {
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|_| "cannot resolve the current directory".to_owned())?
            .join(path)
    };
    Ok(path.to_string_lossy().into_owned())
}

fn install_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|_| "cannot locate star.exe".to_owned())?;
    Ok(executable
        .parent()
        .ok_or("star.exe has no installation directory")?
        .to_path_buf())
}

fn exit_code(response: &IpcResponse) -> i32 {
    match &response.status {
        IpcStatus::Ok | IpcStatus::Accepted => 0,
        IpcStatus::ApprovalRequired | IpcStatus::QuestionRequired | IpcStatus::Blocked => 3,
        IpcStatus::Error => {
            let code = response
                .error
                .as_ref()
                .map(|error| error.code.as_str())
                .unwrap_or("INTERNAL_UNKNOWN");
            if code.contains("PROTOCOL")
                || code.contains("VERSION")
                || code.contains("INCOMPATIBLE")
            {
                6
            } else if code.contains("UNTRUSTED")
                || code.starts_with("POLICY_")
                || code.contains("APPROVAL")
            {
                3
            } else if code.contains("MANIFEST")
                || code.contains("ARGUMENT")
                || code.contains("SCAFFOLD")
                || code.contains("VALIDATE")
            {
                2
            } else {
                4
            }
        }
    }
}

async fn call_controller_command(
    client: &ControllerClient,
    bootstrap: &VerifiedControllerImage,
    command: &str,
    mut payload: serde_json::Value,
) -> Result<IpcResponse, ControllerClientError> {
    let pagination = match command {
        "tool.search" if payload.get("query").and_then(serde_json::Value::as_str) == Some("") => {
            Some((50_u64, 512_usize, 11_usize, "tool_id", false))
        }
        "tool.registry.status"
            if payload
                .get("package_id")
                .is_none_or(serde_json::Value::is_null) =>
        {
            Some((200_u64, 512_usize, 3_usize, "package_id", true))
        }
        _ => None,
    };
    let Some((page_size, max_items, max_pages, item_id_key, require_diagnostic_revision)) =
        pagination
    else {
        return client
            .call_with_verified_start(bootstrap, command, payload, RequestId::new())
            .await;
    };

    let object = payload
        .as_object_mut()
        .ok_or(ControllerClientError::MalformedResponse)?;
    object.insert("limit".to_owned(), page_size.into());
    object.remove("cursor");
    let mut combined = None;
    let mut items = Vec::new();
    let mut item_ids = BTreeSet::new();
    let mut cursors = BTreeSet::new();
    let mut snapshot_hash = None;
    let mut registry_revision = None;
    let mut diagnostic_revision = None;

    for _ in 0..max_pages {
        let response = client
            .call_with_verified_start(bootstrap, command, payload.clone(), RequestId::new())
            .await?;
        if response.status != IpcStatus::Ok {
            return Ok(response);
        }
        let data = response
            .data
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .ok_or(ControllerClientError::MalformedResponse)?;
        let page_snapshot = data
            .get("snapshot_hash")
            .and_then(serde_json::Value::as_str)
            .ok_or(ControllerClientError::MalformedResponse)?;
        let page_revision = data
            .get("registry_revision")
            .and_then(serde_json::Value::as_u64)
            .ok_or(ControllerClientError::MalformedResponse)?;
        let page_diagnostic_revision = require_diagnostic_revision
            .then(|| {
                data.get("diagnostic_revision")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ControllerClientError::MalformedResponse)
            })
            .transpose()?;
        if snapshot_hash
            .as_deref()
            .is_some_and(|value| value != page_snapshot)
            || registry_revision.is_some_and(|value| value != page_revision)
            || diagnostic_revision
                .zip(page_diagnostic_revision)
                .is_some_and(|(value, page_value)| value != page_value)
        {
            return Err(ControllerClientError::MalformedResponse);
        }
        snapshot_hash.get_or_insert_with(|| page_snapshot.to_owned());
        registry_revision.get_or_insert(page_revision);
        if let Some(page_diagnostic_revision) = page_diagnostic_revision {
            diagnostic_revision.get_or_insert(page_diagnostic_revision);
        }
        for item in data
            .get("items")
            .and_then(serde_json::Value::as_array)
            .ok_or(ControllerClientError::MalformedResponse)?
        {
            let item_id = item
                .get(item_id_key)
                .and_then(serde_json::Value::as_str)
                .ok_or(ControllerClientError::MalformedResponse)?;
            if !item_ids.insert(item_id.to_owned()) || items.len() >= max_items {
                return Err(ControllerClientError::MalformedResponse);
            }
            items.push(item.clone());
        }
        if combined.is_none() {
            combined = Some(response.clone());
        }
        let next_cursor = match data.get("next_cursor") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or(ControllerClientError::MalformedResponse)?
                    .to_owned(),
            ),
        };
        let Some(next_cursor) = next_cursor else {
            let mut response = combined.ok_or(ControllerClientError::MalformedResponse)?;
            let data = response
                .data
                .as_mut()
                .and_then(serde_json::Value::as_object_mut)
                .ok_or(ControllerClientError::MalformedResponse)?;
            data.insert("items".to_owned(), serde_json::Value::Array(items));
            data.insert("next_cursor".to_owned(), serde_json::Value::Null);
            return Ok(response);
        };
        if !cursors.insert(next_cursor.clone()) {
            return Err(ControllerClientError::MalformedResponse);
        }
        payload
            .as_object_mut()
            .ok_or(ControllerClientError::MalformedResponse)?
            .insert("cursor".to_owned(), next_cursor.into());
    }
    Err(ControllerClientError::MalformedResponse)
}

#[tokio::main]
async fn main() {
    let raw: Vec<_> = std::env::args().skip(1).collect();
    if raw.len() == 1 && matches!(raw[0].as_str(), "--version" | "-V") {
        println!("{VERSION}");
        return;
    }
    if raw
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "help")
        || raw.is_empty()
    {
        println!("{HELP}");
        return;
    }
    if let Some(exit) = local_commands::dispatch(&raw).await {
        std::process::exit(exit);
    }
    let parsed = match parse(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let (bootstrap, config) = match install_directory().and_then(|directory| {
        let bootstrap = VerifiedControllerImage::from_install_directory(&directory)
            .map_err(|error| error.to_string())?;
        let config =
            cli_client_config(bootstrap.path().to_path_buf()).map_err(|error| error.to_string())?;
        Ok((bootstrap, config))
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(4);
        }
    };
    let Parsed {
        command,
        payload,
        json,
    } = parsed;
    let response = match call_controller_command(
        &ControllerClient::new(config),
        &bootstrap,
        &command,
        payload,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            let exit = if matches!(
                error,
                star_ipc::client::ControllerClientError::ProtocolMismatch
            ) {
                6
            } else {
                4
            };
            eprintln!("{error}");
            std::process::exit(exit);
        }
    };
    if json {
        if let Some(error) = &response.error {
            eprintln!("{}", error.code);
        }
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
    std::process::exit(exit_code(&response));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn help_and_frozen_commands_parse_without_local_state() {
        let parsed = parse(&args(&["tools", "list", "--json"])).unwrap();
        assert_eq!(parsed.command, "tool.search");
        assert!(parsed.json);
        assert_eq!(parsed.payload["limit"], 50);
        assert_eq!(parsed.payload["readiness"].as_array().unwrap().len(), 5);
        let status = parse(&args(&["tools", "status"])).unwrap();
        assert_eq!(status.payload["limit"], 200);
        assert_eq!(
            parse(&args(&["tools", "trust", "x"])).unwrap_err(),
            "command requires --manifest-hash"
        );
        assert!(
            parse(&args(&[
                "tools", "list", "--source", "user", "--source", "project",
            ]))
            .unwrap_err()
            .contains("only once")
        );
        assert!(
            parse(&args(
                &["tools", "describe", "core.test.echo", "--unknown",]
            ))
            .unwrap_err()
            .contains("unknown option")
        );
    }

    #[test]
    fn every_frozen_cli_syntax_maps_to_the_exact_controller_command() {
        let hash = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cases = [
            (args(&["tools", "list"]), "tool.search"),
            (
                args(&["tools", "describe", "user.fake.echo.run"]),
                "tool.describe",
            ),
            (args(&["tools", "status"]), "tool.registry.status"),
            (
                args(&["tools", "validate", "package.toml", "--source", "user"]),
                "tool.validate",
            ),
            (args(&["tools", "probe", "user.fake.echo"]), "tool.probe"),
            (
                args(&[
                    "tools",
                    "trust",
                    "user.fake.echo",
                    "--manifest-hash",
                    hash,
                    "--expires",
                    "2026-07-12T00:00:00Z",
                ]),
                "tool.trust",
            ),
            (
                args(&[
                    "tools",
                    "revoke",
                    "user.fake.echo",
                    "--cancel-running",
                    "--reason",
                    "test",
                ]),
                "tool.revoke",
            ),
            (
                args(&["tools", "scaffold", "tool.exe", "--output", "tool.toml"]),
                "tool.scaffold",
            ),
            (
                args(&["controller", "start", "--background"]),
                "controller.start",
            ),
            (
                args(&["controller", "autostart", "enable"]),
                "controller.autostart.enable",
            ),
            (
                args(&["controller", "autostart", "disable"]),
                "controller.autostart.disable",
            ),
            (
                args(&["controller", "autostart", "status"]),
                "controller.autostart.status",
            ),
        ];
        for (arguments, expected) in cases {
            assert_eq!(parse(&arguments).unwrap().command, expected);
        }
    }

    #[test]
    fn help_tools_lines_match_the_authoritative_manifest_reference_table() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let reference =
            std::fs::read_to_string(root.join("docs/contracts/tool-package-manifest-reference.md"))
                .unwrap();
        let expected: Vec<_> = reference
            .lines()
            .filter_map(|line| {
                let command = [
                    "list", "describe", "status", "validate", "probe", "trust", "revoke",
                    "scaffold",
                ]
                .into_iter()
                .find(|command| line.starts_with(&format!("| {command} |")))?;
                let _ = command;
                let start = line.find('\x60')? + 1;
                let end = start + line[start..].find('\x60')?;
                Some(line[start..end].replace("\\|", "|"))
            })
            .collect();
        assert_eq!(
            HELP.lines().take(8).collect::<Vec<_>>(),
            expected.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }

    #[test]
    fn unsupported_json_and_invalid_timestamp_forms_fail_before_ipc() {
        for command in [
            args(&[
                "tools",
                "scaffold",
                "tool.exe",
                "--output",
                "tool.toml",
                "--json",
            ]),
            args(&["controller", "start", "--json"]),
            args(&["controller", "autostart", "status", "--json"]),
        ] {
            assert!(
                parse(&command)
                    .unwrap_err()
                    .contains("does not support --json")
            );
        }
        let hash = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(
            parse(&args(&[
                "tools",
                "trust",
                "user.fake.echo",
                "--manifest-hash",
                hash,
                "--expires",
                "tomorrow",
            ]))
            .unwrap_err()
            .contains("RFC 3339")
        );
    }

    #[test]
    fn p0_management_cli_maps_to_controller_commands_without_db_access() {
        let project = star_contracts::ids::ProjectId::new();
        let finding = star_contracts::ids::FindingId::new();
        let patch = star_contracts::ids::PatchSetId::new();
        let fingerprint = star_contracts::Sha256Hash::digest(b"patch").to_string();
        let cases = [
            (args(&["doctor"]), "doctor.run"),
            (args(&["project", "register"]), "project.register"),
            (args(&["project", "list"]), "project.list"),
            (
                args(&["project", "status", "star-control"]),
                "project.status",
            ),
            (
                args(&[
                    "validation",
                    "plan",
                    "star-control",
                    "--profile",
                    "full",
                    "--unit",
                    "star-contracts",
                ]),
                "validation.plan",
            ),
            (
                args(&[
                    "validation",
                    "run",
                    "star-control",
                    "--profile",
                    "target",
                    "--timeout-ms",
                    "60000",
                ]),
                "validation.run",
            ),
            (
                args(&[
                    "evidence",
                    "get",
                    "star-control",
                    "target/validation/run-1/report.json",
                ]),
                "evidence.get",
            ),
            (
                vec!["scan".into(), "run".into(), project.to_string()],
                "scan.run",
            ),
            (
                vec!["finding".into(), "list".into(), project.to_string()],
                "finding.list",
            ),
            (
                vec![
                    "patch".into(),
                    "prepare".into(),
                    project.to_string(),
                    finding.to_string(),
                ],
                "patch.prepare",
            ),
            (
                vec![
                    "patch".into(),
                    "apply".into(),
                    project.to_string(),
                    patch.to_string(),
                    "--approve".into(),
                    fingerprint.clone(),
                ],
                "patch.apply",
            ),
            (args(&["management", "status"]), "management.status"),
            (
                args(&["management", "retention", "plan"]),
                "management.retention.plan",
            ),
            (
                vec![
                    "management".into(),
                    "retention".into(),
                    "apply".into(),
                    "--approve".into(),
                    fingerprint,
                ],
                "management.retention.apply",
            ),
            (
                args(&["management", "rebuild", "plan"]),
                "management.rebuild.plan",
            ),
            (
                vec![
                    "management".into(),
                    "rebuild".into(),
                    "apply".into(),
                    "--approve".into(),
                    star_contracts::Sha256Hash::digest(b"rebuild").to_string(),
                ],
                "management.rebuild.apply",
            ),
        ];
        for (arguments, command) in cases {
            assert_eq!(parse(&arguments).unwrap().command, command);
        }
        assert_eq!(VERSION, concat!("Star-Control ", env!("CARGO_PKG_VERSION")));

        let oversized_key = "x".repeat(129);
        assert!(
            parse(&[
                "scan".into(),
                "run".into(),
                project.to_string(),
                "--idempotency".into(),
                oversized_key,
            ])
            .unwrap_err()
            .contains("1 through 128")
        );
    }

    fn response(status: IpcStatus, code: Option<&str>) -> IpcResponse {
        IpcResponse {
            schema_id: "star.ipc.response".to_owned(),
            schema_version: 1,
            request_id: RequestId::new(),
            status,
            data: None,
            operation_id: None,
            diagnostics: vec![],
            error: code.map(|code| {
                star_contracts::ipc::ErrorEnvelope::new(
                    code,
                    "test",
                    false,
                    "test",
                    "star-cli-test",
                )
            }),
            registry_revision: None,
            correlation_id: "test".to_owned(),
        }
    }

    #[test]
    fn cli_exit_codes_follow_the_frozen_management_contract() {
        assert_eq!(exit_code(&response(IpcStatus::Ok, None)), 0);
        assert_eq!(exit_code(&response(IpcStatus::Accepted, None)), 0);
        assert_eq!(exit_code(&response(IpcStatus::ApprovalRequired, None)), 3);
        assert_eq!(exit_code(&response(IpcStatus::QuestionRequired, None)), 3);
        assert_eq!(exit_code(&response(IpcStatus::Blocked, None)), 3);
        assert_eq!(
            exit_code(&response(IpcStatus::Error, Some("IPC_PROTOCOL_MISMATCH"))),
            6
        );
        assert_eq!(
            exit_code(&response(
                IpcStatus::Error,
                Some("TOOL_EXECUTABLE_UNTRUSTED")
            )),
            3
        );
        assert_eq!(
            exit_code(&response(IpcStatus::Error, Some("TOOL_MANIFEST_INVALID"))),
            2
        );
        assert_eq!(
            exit_code(&response(
                IpcStatus::Error,
                Some("TOOL_PROCESS_START_FAILED")
            )),
            4
        );
    }
}
