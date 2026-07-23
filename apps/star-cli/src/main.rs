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
star tools call <tool-id> <descriptor-sha256> <arguments-json> --lane read_closed|read_open|write_closed|write_open|destructive_closed|destructive_open [--wait auto|accepted|completed] [--timeout-ms <milliseconds>] [--idempotency <key>] [--json]\n\
star approvals resolve <approval-id> <scope-sha256> <approve|deny> [--reason <text>] [--json]\n\
star operations get <operation-id> [--after-sequence <n>] [--wait-ms <milliseconds>] [--json]\n\
star operations cancel <operation-id> [--reason <text>] [--json]\n\
star doctor [--json]\n\
star project register <project-key> [--idempotency <key>] [--json]\n\
star project discover [<explicit-root> ...] [--idempotency <key>] [--json]\n\
star project checkout attach <explicit-root> [--idempotency <key>] [--json]\n\
star project checkout list <project-id> [--json]\n\
star project checkout show <checkout-id> [--json]\n\
star project list [--json]\n\
star project status <project-key> [--json]\n\
star planning create <task-json> [--idempotency <key>] [--json]\n\
star planning get|show <task-spec-id> [--json]\n\
star planning status <task-spec-id> [--json]\n\
star planning history <task-spec-id> [--json]\n\
star planning scope revise <task-spec-id> <task-json> --reason <text> [--idempotency <key>] [--json]\n\
star planning impact inspect <task-spec-id> [--json]\n\
star planning affected-checks show <task-spec-id> [--json]\n\
star planning override <task-spec-id> <family> --kind add|promote|omit --reason <text> [--idempotency <key>] [--json]\n\
star planning waiver <task-spec-id> <family> --reason <text> [--idempotency <key>] [--json]\n\
star planning invalidate <task-spec-id> --reason <text> [--idempotency <key>] [--json]\n\
star planning replan <task-spec-id> --reason <text> [--idempotency <key>] [--json]\n\
star validation preflight <task-spec-id> [--claims <json-file>] [--guard-evidence <json-file>] [--json]\n\
star validation run-plan <task-spec-id> [--claims <json-file>] [--guard-evidence <json-file>] [--json]\n\
star validation status <project-id> [--json]\n\
star diagnostic list <project-id> [--json]\n\
star diagnostic show <project-id> <diagnostic-id> [--json]\n\
star baseline inspect <project-id> [--json]\n\
star suppression inspect <project-id> [--json]\n\
star gate show <project-id> <gate-id> [--json]\n\
star evidence bundle export <project-id> <evidence-bundle-id> [--json]\n\
star review-pack export <project-id> <review-pack-id> [--json]\n\
star validation plan <project-key> [--profile quick|target|full|release] [--unit <unit>] [--json]\n\
star validation run <project-key> [--profile quick|target|full|release] [--unit <unit>] [--timeout-ms <milliseconds>] [--json]\n\
star evidence get <project-key> <evidence-ref> [--json]\n\
star scan run <project-id> [--mode full|incremental] [--idempotency <key>] [--json]\n\
star index status <project-id> [--json]\n\
star index files <project-id> [query] [--allow-stale] [--json]\n\
star index search <project-id> <query> [--tier text|syntax|semantic] [--allow-stale] [--json]\n\
star index definitions <project-id> <query> [--allow-stale] [--json]\n\
star index references <project-id> <symbol-id> [--allow-stale] [--json]\n\
star index hardcoding <project-id> [--allow-stale] [--json]\n\
star registry list <project-id> [--manifest <project-relative-path>] [--json]\n\
star registry show <project-id> <managed-declaration-id> [--manifest <project-relative-path>] [--json]\n\
star registry candidate inspect <project-id> [--manifest <project-relative-path>] [--json]\n\
star registry candidate classify <project-id> <candidate-id> <managed_declaration|candidate|local_implementation_constant> --reason <text> [--manifest <project-relative-path>] [--json]\n\
star registry declaration plan <project-id> <change-kind> --desired <json-file> --reason <text> [--declaration <managed-declaration-id>] [--consumers <json-file>] [--manifest <project-relative-path>] [--json]\n\
star registry status <project-id> [--manifest <project-relative-path>] [--json]\n\
star contract snapshot <project-id> <snapshot-id> --role baseline|current [--source-revision <git-revision>] [--manifest <project-relative-path>] [--registry-snapshot <ref>] [--revision <n>] [--json]\n\
star contract compare <project-id> <report-id> <baseline-snapshot-id> <current-snapshot-id> [--manifest <project-relative-path>] [--revision <n>] [--json]\n\
star docs check <project-id> <snapshot-id> --registrations <json-file> [--manifest <project-relative-path>] [--revision <n>] [--json]\n\
star config trace <project-id> <trace-id> --input <json-file> [--revision <n>] [--json]\n\
star environment fingerprint <project-id> <snapshot-id> [--revision <n>] [--json]\n\
star project doctor <project-id> <report-id> <environment-snapshot-id> --input <json-file> [--manifest <project-relative-path>] [--revision <n>] [--json]\n\
star clean-room specification publish <project-id> <json-file> [--revision <n>] [--json]\n\
star clean-room readiness <project-id> <report-id> <environment-snapshot-id> --input <json-file> [--manifest <project-relative-path>] [--revision <n>] [--json]\n\
star dependency-security input <project-id> <manifest-id> <environment-snapshot-id> [--revision <n>] [--json]\n\
star failures inspect <project-id> <failure-record-id> --input <json-file> [--revision <n>] [--json]\n\
star failures reproduce <project-id> <reproduction-pack-id> <failure-record-id> --input <json-file> [--revision <n>] [--json]\n\
star failures compare <project-id> <regression-record-json> [--revision <n>] [--json]\n\
star failures recovery-plan <project-id> <recovery-plan-json> [--revision <n>] [--json]\n\
star security inspect <project-id> <snapshot-id> <effect-receipt-id> --input <json-file> [--revision <n>] [--json]\n\
star security release-manifest <project-id> <snapshot-id> <dependency-snapshot-id> --input <json-file> [--revision <n>] [--json]\n\
star deps scan <project-id> <snapshot-id> [--revision <n>] [--json]\n\
star deps candidates|prepare <project-id> <plan-id> <dependency-snapshot-id> --input <json-file> [--revision <n>] [--json]\n\
star deps status <project-id> <plan-id> [--json]\n\
star deps rollback-plan <project-id> <recovery-plan-json> [--revision <n>] [--json]\n\
star maintenance radar <snapshot-id> --input <json-file> [--revision <n>] [--json]\n\
star migration inspect <project-id> <target-id> [--manifest <project-relative-path>] [--json]\n\
star migration plan <project-id> <input-json> [--manifest <project-relative-path>] [--revision <n>] [--json]\n\
star migration checkpoint <project-id> <plan-id> <checkpoint-json> [--revision <n>] [--json]\n\
star migration dry-run|backup|rehearse|execute|resume|validate|rollback <project-id> <plan-id> <attempt-json> --fingerprint <sha256> [--revision <n>] [--json]\n\
star migration validation-report <project-id> <report-json> [--revision <n>] [--json]\n\
star migration restore-verify <project-id> <record-json> [--revision <n>] [--json]\n\
star migration status <project-id> <plan-id> [--json]\n\
star migration handoff <handoff-json> [--revision <n>] [--json]\n\
star performance plan <project-id> <workload-json> [--revision <n>] [--json]\n\
star performance run <project-id> <workload-id> <run-json> [--revision <n>] [--json]\n\
star performance compare <project-id> <workload-id> <comparison-id> --baseline-runs <json-file> --candidate-runs <json-file> [--revision <n>] [--json]\n\
star language-migration plan <project-id> <plan-json> [--revision <n>] [--json]\n\
star language-migration equivalence <project-id> <plan-id> <report-json> [--revision <n>] [--json]\n\
star language-migration cutover <project-id> <plan-id> <equivalence-report-id> <effect-receipt-id> --fingerprint <sha256> [--json]\n\
star language-migration status <project-id> <plan-id> [--json]\n\
star change-bundle goal publish <goal-json> [--revision <n>] [--json]\n\
star change-bundle participant publish <participant-json> [--revision <n>] [--json]\n\
star change-bundle plan|hold|resume <goal-id> <bundle-json> --participants <json-file> [--revision <n>] [--json]\n\
star change-bundle show|status|conflicts <bundle-id> [--json]\n\
star change-bundle preflight <bundle-id> <analysis-id> --subjects <json-file> --ordered-pairs <json-file> [--revision <n>] [--json]\n\
star change-bundle apply <project-id> <participant-json> --patch-applications <json-file> --migration-attempts <json-file> [--revision <n>] [--json]\n\
star change-bundle validate <project-id> <participant-json> [--revision <n>] [--json]\n\
star change-bundle worktree plan <project-id> <record-json> [--revision <n>] [--json]\n\
star change-bundle worktree create <project-id> <worktree-id> --branch <star-branch> --permission <decision-ref> --gate <decision-ref> --approve <sha256> [--json]\n\
star change-bundle merge plan <project-id> <plan-json> <overlap-analysis-id> [--revision <n>] [--json]\n\
star change-bundle merge enqueue <project-id> <queue-json> [--revision <n>] [--json]\n\
star change-bundle merge run <project-id> <merge-plan-id> <input-commit-oid> <result-json> --permission <decision-ref> --gate <decision-ref> --approve <sha256> [--revision <n>] [--json]\n\
star change-bundle merge result <project-id> <result-json> [--revision <n>] [--json]\n\
star change-bundle conflict publish <project-id> <conflict-json> [--revision <n>] [--json]\n\
star change-bundle remote refresh <project-id> <remote-name> <snapshot-id> --captured-at <rfc3339> --valid-until <rfc3339> [--revision <n>] [--json]\n\
star change-bundle remote operation prepare|observe <operation-json> [--revision <n>] [--json]\n\
star change-bundle remote operation apply <operation-id> --request-fingerprint <sha256> [--json]\n\
star change-bundle release-handoff plan <handoff-json> [--revision <n>] [--json]\n\
star change-bundle recovery plan <project-id> <plan-json> [--revision <n>] [--json]\n\
star change-bundle recovery apply <project-id> <plan-id> --approve <sha256> --permission <decision-ref> --gate <decision-ref> --receipt <effect-receipt-id> [--json]\n\
star release candidate create <project-id> <candidate-json> [--json]\n\
star release artifacts verify <project-id> <release-manifest-id> <artifacts-json> [--json]\n\
star release verification record <project-id> <release-manifest-id> <verification-json> [--json]\n\
star release promote|show|status <release-manifest-id> [--json]\n\
star release lifecycle publish <project-id> <lifecycle-id> <evidence-json> [--revision <n>] [--json]\n\
star release publish prepare <release-manifest-id> <before-snapshot-id> [--json]\n\
star release publish authorize <release-manifest-id> <approval-id> [--json]\n\
star release publish apply <release-manifest-id> [--json]\n\
star evaluation run <project-id> <input-json> [--json]\n\
star evaluation show <evaluation-run-id> [--json]\n\
star evaluation catalog publish <item-json> [--revision <n>] [--json]\n\
star evaluation catalog transition <record-id> <deprecated|retired|rejected> [--trial-candidate] [--revision <n>] [--json]\n\
star profile list [--json]\n\
star profile show <profile-id> [--json]\n\
star profile resolve <profile-id> [<profile-id> ...] [--json]\n\
star development effect record <project-id> <receipt-id> <effect-kind> <subject-ref> <subject-sha256> <operation-id> --arguments <json-file> [--approval <ref>] [--permission <ref>] [--gate <ref>] [--revision <n>] [--json]\n\
star development record show <record-kind> <record-id> [--revision <n>] [--json]\n\
star development record list <record-kind> [--project <project-id>] [--json]\n\
star graph neighbors <project-id> <entity-key> [--allow-stale] [--json]\n\
star style rust inspect <project-id> [--json]\n\
star style rust check <project-id> [--scope workspace|package] [--package <package>] [--json]\n\
star style rust prepare <project-id> --scope workspace|package [--package <package>] [--json]\n\
star style rust auto-apply <project-id> --scope workspace|package [--package <package>] [--json]\n\
star finding list <project-id> [--json]\n\
star recipe list [--language <language>] [--rewrite-kind text_exact|syntax_aware|symbol_aware|codegen_assured] [--json]\n\
star recipe describe <recipe-id@semver> [--json]\n\
star recipe validate <recipe-json> [--json]\n\
star change prepare <project-id> <checkout-id> <recipe-id@semver> --selector <json-file> --parameters <json-file> [--workspace current|isolated] [--json]\n\
star patch show <patch-set-id> [--json]\n\
star patch apply <patch-set-id> --fingerprint <sha256> [--manual-approval <approval-id>] [--guard-evidence <json-file>] [--json]\n\
star patch status <patch-application-id> [--json]\n\
star patch recover <patch-application-id> --strategy reverse-patch|discard-isolated [--json]\n\
star patch prepare <project-id> <finding-id> [--json]\n\
star patch apply <project-id> <patch-set-id> --approve <sha256> [--json]\n\
star management status [--json]\n\
star management backup plan <backup-root> [--json]\n\
star management backup apply <backup-root> <plan-json> --approve <sha256> [--json]\n\
star management restore plan <backup-root> [--json]\n\
star management restore apply <backup-root> <plan-json> --approve <sha256> [--json]\n\
star management local-state export plan <project-id> <destination-json> [--json]\n\
star management local-state export apply <destination-json> <plan-json> --approve <sha256> [--json]\n\
star management local-state import plan <source-json> [--json]\n\
star management local-state import apply <source-json> <plan-json> --approve <sha256> [--json]\n\
star management retention plan [--json]\n\
star management retention apply --approve <sha256> [--json]\n\
star management rebuild plan [--json]\n\
star management rebuild apply <plan-json> --approve <sha256> [--json]\n\
star management migrate project-v1-v2 plan [--json]\n\
star management migrate project-v1-v2 apply <plan-json> --approve <sha256> [--json]\n\
star management migrate project-v1-v2 rollback <plan-json> --approve <sha256> [--json]\n\
star management migrate patch-v1-v2 plan <project-id> [--json]\n\
star management migrate patch-v1-v2 apply <plan-json> --approve <sha256> [--json]\n\
star management migrate patch-v1-v2 rollback <plan-json> --approve <sha256> [--json]\n\
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
        [first, second, tail @ ..] if first == "tools" && second == "call" => {
            let (positionals, options) = parse_tail(
                tail,
                &["--lane", "--wait", "--timeout-ms", "--idempotency"],
                &[],
            )?;
            require_positionals(&positionals, 3, "tools call")?;
            positionals[1]
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "descriptor-sha256 must be an exact lowercase sha256".to_owned())?;
            let arguments = read_bounded_json_file(&positionals[2])?;
            if !arguments.is_object() {
                return Err("tools call arguments must be one JSON object".to_owned());
            }
            let lane = required_option(&options, "--lane")?;
            if !matches!(
                lane.as_str(),
                "read_closed"
                    | "read_open"
                    | "write_closed"
                    | "write_open"
                    | "destructive_closed"
                    | "destructive_open"
            ) {
                return Err("--lane has an unsupported fixed risk lane".to_owned());
            }
            let wait_mode = options
                .get("--wait")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "auto".to_owned());
            if !matches!(wait_mode.as_str(), "auto" | "accepted" | "completed") {
                return Err("--wait must be auto, accepted, or completed".to_owned());
            }
            let requested_timeout_ms = options
                .get("--timeout-ms")
                .and_then(Clone::clone)
                .map(|value| {
                    value
                        .parse::<u32>()
                        .ok()
                        .filter(|value| *value >= 100)
                        .ok_or_else(|| "--timeout-ms must be an integer of at least 100".to_owned())
                })
                .transpose()?;
            let idempotency_key = options.get("--idempotency").and_then(Clone::clone);
            if let Some(value) = &idempotency_key {
                validate_idempotency_key(value)?;
            }
            let mcp_request_id = RequestId::new();
            Ok(Parsed {
                command: "tool.invoke".to_owned(),
                payload: serde_json::json!({
                    "tool_id":positionals[0],
                    "descriptor_hash":positionals[1],
                    "arguments":arguments,
                    "mcp_tool_name":format!("star_tool_call_{lane}"),
                    "mcp_risk_lane":lane,
                    "mcp_request_id":mcp_request_id,
                    "progress_requested":false,
                    "client_info":{"name":"star-cli","version":VERSION},
                    "wait_mode":wait_mode,
                    "requested_timeout_ms":requested_timeout_ms,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "approvals" && second == "resolve" => {
            let (positionals, options) = parse_tail(tail, &["--reason"], &[])?;
            require_positionals(&positionals, 3, "approvals resolve")?;
            star_contracts::ids::ApprovalId::parse(positionals[0].clone())
                .map_err(|_| "approvals resolve requires a valid ApprovalId".to_owned())?;
            positionals[1]
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "scope-sha256 must be an exact lowercase sha256".to_owned())?;
            if !matches!(positionals[2].as_str(), "approve" | "deny") {
                return Err("approval decision must be approve or deny".to_owned());
            }
            let reason = options.get("--reason").and_then(Clone::clone);
            if reason
                .as_ref()
                .is_some_and(|value| value.contains('\0') || value.chars().count() > 1_000)
            {
                return Err("--reason must contain at most 1000 non-NUL characters".to_owned());
            }
            Ok(Parsed {
                command: "approval.resolve".to_owned(),
                payload: serde_json::json!({
                    "approval_id":positionals[0],
                    "scope_hash":positionals[1],
                    "decision":positionals[2],
                    "reason":reason,
                    "conditions":{},
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "operations" && second == "get" => {
            let (positionals, options) = parse_tail(tail, &["--after-sequence", "--wait-ms"], &[])?;
            require_positionals(&positionals, 1, "operations get")?;
            star_contracts::ids::OperationId::parse(positionals[0].clone())
                .map_err(|_| "operations get requires a valid OperationId".to_owned())?;
            let after_sequence = options
                .get("--after-sequence")
                .and_then(Clone::clone)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--after-sequence must be an integer".to_owned())
                })
                .transpose()?
                .unwrap_or(0);
            let wait_ms = options
                .get("--wait-ms")
                .and_then(Clone::clone)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|value| *value <= 30_000)
                        .ok_or_else(|| {
                            "--wait-ms must be an integer from 0 through 30000".to_owned()
                        })
                })
                .transpose()?
                .unwrap_or(0);
            Ok(Parsed {
                command: "operation.get".to_owned(),
                payload: serde_json::json!({
                    "operation_id":positionals[0],
                    "after_sequence":after_sequence,
                    "wait_ms":wait_ms,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "operations" && second == "cancel" => {
            let (positionals, options) = parse_tail(tail, &["--reason"], &[])?;
            require_positionals(&positionals, 1, "operations cancel")?;
            star_contracts::ids::OperationId::parse(positionals[0].clone())
                .map_err(|_| "operations cancel requires a valid OperationId".to_owned())?;
            let reason = options
                .get("--reason")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "cancelled_by_cli".to_owned());
            if reason.trim().is_empty() || reason.contains('\0') || reason.chars().count() > 1_000 {
                return Err("--reason must contain 1 through 1000 non-NUL characters".to_owned());
            }
            Ok(Parsed {
                command: "operation.cancel".to_owned(),
                payload: serde_json::json!({"operation_id":positionals[0],"reason":reason}),
                json,
            })
        }
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
            let (positionals, options) = parse_tail(tail, &["--idempotency"], &[])?;
            require_positionals(&positionals, 1, "project register")?;
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
                command: "project.register".to_owned(),
                payload: serde_json::json!({
                    "project_key":positionals[0],
                    "idempotency_key":idempotency_key,
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
        [first, second, tail @ ..] if first == "project" && second == "discover" => {
            let (positionals, options) = parse_tail(tail, &["--idempotency"], &[])?;
            if positionals.len() > 64 {
                return Err("project discover accepts at most 64 explicit roots".to_owned());
            }
            if positionals.is_empty() {
                if options.contains_key("--idempotency") {
                    return Err("--idempotency requires at least one explicit root".to_owned());
                }
                return Ok(Parsed {
                    command: "project.discover".to_owned(),
                    payload: serde_json::json!({}),
                    json,
                });
            }
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            validate_idempotency_key(&idempotency_key)?;
            let roots = positionals
                .iter()
                .map(|root| absolute_path(root))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Parsed {
                command: "project.discover".to_owned(),
                payload: serde_json::json!({
                    "roots":roots,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "project" && second == "checkout" && third == "attach" =>
        {
            let (positionals, options) = parse_tail(tail, &["--idempotency"], &[])?;
            require_positionals(&positionals, 1, "project checkout attach")?;
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            validate_idempotency_key(&idempotency_key)?;
            Ok(Parsed {
                command: "project.checkout.attach".to_owned(),
                payload: serde_json::json!({
                    "root":absolute_path(&positionals[0])?,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "project" && second == "checkout" && third == "list" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "project checkout list")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "project checkout list requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "project.checkout.list".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "project" && second == "checkout" && third == "show" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "project checkout show")?;
            star_contracts::ids::CheckoutId::parse(positionals[0].clone())
                .map_err(|_| "project checkout show requires a valid CheckoutId".to_owned())?;
            Ok(Parsed {
                command: "project.checkout.show".to_owned(),
                payload: serde_json::json!({"checkout_id":positionals[0]}),
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
        [first, second, tail @ ..] if first == "planning" && second == "create" => {
            let (positionals, options) = parse_tail(tail, &["--idempotency"], &[])?;
            require_positionals(&positionals, 1, "planning create")?;
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
                command: "planning.create".to_owned(),
                payload: serde_json::json!({
                    "task_file":positionals[0],
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "planning" && ["get", "show"].contains(&second.as_str()) =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "planning get|show")?;
            validate_task_spec_id(&positionals[0])?;
            Ok(Parsed {
                command: "planning.get".to_owned(),
                payload: serde_json::json!({"task_spec_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "planning" && ["status", "history"].contains(&second.as_str()) =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, &format!("planning {second}"))?;
            validate_task_spec_id(&positionals[0])?;
            Ok(Parsed {
                command: format!("planning.{second}"),
                payload: serde_json::json!({"task_spec_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "planning" && second == "scope" && third == "revise" =>
        {
            let (positionals, options) = parse_tail(tail, &["--reason", "--idempotency"], &[])?;
            require_positionals(&positionals, 2, "planning scope revise")?;
            validate_task_spec_id(&positionals[0])?;
            let reason = required_option(&options, "--reason")?;
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            validate_idempotency_key(&idempotency_key)?;
            Ok(Parsed {
                command: "planning.scope.revise".to_owned(),
                payload: serde_json::json!({
                    "task_spec_id":positionals[0],
                    "task_file":positionals[1],
                    "reason":reason,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "planning" && second == "impact" && third == "inspect" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "planning impact inspect")?;
            validate_task_spec_id(&positionals[0])?;
            Ok(Parsed {
                command: "planning.impact.inspect".to_owned(),
                payload: serde_json::json!({"task_spec_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "planning" && second == "affected-checks" && third == "show" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "planning affected-checks show")?;
            validate_task_spec_id(&positionals[0])?;
            Ok(Parsed {
                command: "planning.affected-checks.show".to_owned(),
                payload: serde_json::json!({"task_spec_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "planning" && ["override", "waiver"].contains(&second.as_str()) =>
        {
            let value_options = if second == "override" {
                vec!["--kind", "--reason", "--idempotency"]
            } else {
                vec!["--reason", "--idempotency"]
            };
            let (positionals, options) = parse_tail(tail, &value_options, &[])?;
            require_positionals(&positionals, 2, &format!("planning {second}"))?;
            validate_task_spec_id(&positionals[0])?;
            let family = positionals[1].trim();
            if family.is_empty() || family.len() > 128 || family.contains('\0') {
                return Err("planning check family is invalid".to_owned());
            }
            let kind = if second == "waiver" {
                "omit".to_owned()
            } else {
                required_option(&options, "--kind")?
            };
            if !matches!(kind.as_str(), "add" | "promote" | "omit") {
                return Err("--kind must be add, promote, or omit".to_owned());
            }
            let reason = required_option(&options, "--reason")?;
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            validate_idempotency_key(&idempotency_key)?;
            Ok(Parsed {
                command: "planning.override".to_owned(),
                payload: serde_json::json!({
                    "task_spec_id":positionals[0],
                    "family":family,
                    "kind":kind,
                    "reason":reason,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "planning" && ["invalidate", "replan"].contains(&second.as_str()) =>
        {
            let (positionals, options) = parse_tail(tail, &["--reason", "--idempotency"], &[])?;
            require_positionals(&positionals, 1, &format!("planning {second}"))?;
            validate_task_spec_id(&positionals[0])?;
            let reason = required_option(&options, "--reason")?;
            let idempotency_key = options
                .get("--idempotency")
                .and_then(Clone::clone)
                .unwrap_or_else(|| RequestId::new().as_str().to_owned());
            validate_idempotency_key(&idempotency_key)?;
            Ok(Parsed {
                command: format!("planning.{second}"),
                payload: serde_json::json!({
                    "task_spec_id":positionals[0],
                    "reason":reason,
                    "idempotency_key":idempotency_key,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "validation" && ["preflight", "run-plan"].contains(&second.as_str()) =>
        {
            let (positionals, options) = parse_tail(tail, &["--claims", "--guard-evidence"], &[])?;
            require_positionals(&positionals, 1, &format!("validation {second}"))?;
            validate_task_spec_id(&positionals[0])?;
            let completion_claims =
                if let Some(path) = options.get("--claims").and_then(Clone::clone) {
                    let value = read_bounded_json_file(&path)?;
                    let claims = serde_json::from_value::<
                        Vec<star_contracts::evidence_v2::CompletionClaimV2>,
                    >(value)
                    .map_err(|_| "--claims must contain a CompletionClaim JSON array".to_owned())?;
                    if claims.len() > 256 {
                        return Err("--claims cannot contain more than 256 claims".to_owned());
                    }
                    claims
                        .into_iter()
                        .map(star_contracts::evidence_v2::CompletionClaimV2::seal)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| "--claims contains an invalid CompletionClaim".to_owned())?
                } else {
                    vec![]
                };
            let validator_guard_evidence =
                if let Some(path) = options.get("--guard-evidence").and_then(Clone::clone) {
                    let value = read_bounded_json_file(&path)?;
                    let evidence = serde_json::from_value::<
                        star_contracts::validator_guard::ValidatorGuardEvidenceV2,
                    >(value)
                    .map_err(|_| {
                        "--guard-evidence must contain ValidatorGuardEvidenceV2 JSON".to_owned()
                    })?;
                    Some(evidence.seal().map_err(|_| {
                        "--guard-evidence contains invalid validator guard evidence".to_owned()
                    })?)
                } else {
                    None
                };
            Ok(Parsed {
                command: format!("validation.{second}"),
                payload: serde_json::json!({
                    "task_spec_id":positionals[0],
                    "completion_claims":completion_claims,
                    "validator_guard_evidence":validator_guard_evidence,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "validation" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "validation status")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "validation status requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "validation.status".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "diagnostic" && ["list", "show"].contains(&second.as_str()) =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            let expected = if second == "list" { 1 } else { 2 };
            require_positionals(&positionals, expected, &format!("diagnostic {second}"))?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "diagnostic command requires a valid ProjectId".to_owned())?;
            let payload = if second == "show" {
                star_contracts::ids::DiagnosticId::parse(positionals[1].clone())
                    .map_err(|_| "diagnostic show requires a valid DiagnosticId".to_owned())?;
                serde_json::json!({
                    "project_id":positionals[0],
                    "diagnostic_id":positionals[1],
                })
            } else {
                serde_json::json!({"project_id":positionals[0]})
            };
            Ok(Parsed {
                command: format!("diagnostic.{second}"),
                payload,
                json,
            })
        }
        [first, second, tail @ ..]
            if ["baseline", "suppression"].contains(&first.as_str()) && second == "inspect" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, &format!("{first} inspect"))?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "decision inspect requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: format!("{first}.inspect"),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "gate" && second == "show" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "gate show")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "gate show requires a valid ProjectId".to_owned())?;
            star_contracts::ids::GateId::parse(positionals[1].clone())
                .map_err(|_| "gate show requires a valid GateId".to_owned())?;
            Ok(Parsed {
                command: "gate.show".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "gate_id":positionals[1],
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "evidence" && second == "bundle" && third == "export" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "evidence bundle export")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "evidence export requires a valid ProjectId".to_owned())?;
            star_contracts::ids::EvidenceBundleId::parse(positionals[1].clone())
                .map_err(|_| "evidence export requires a valid EvidenceBundleId".to_owned())?;
            Ok(Parsed {
                command: "evidence.bundle.export".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "evidence_bundle_id":positionals[1],
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "review-pack" && second == "export" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "review-pack export")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "review-pack export requires a valid ProjectId".to_owned())?;
            star_contracts::ids::ReviewPackId::parse(positionals[1].clone())
                .map_err(|_| "review-pack export requires a valid ReviewPackId".to_owned())?;
            Ok(Parsed {
                command: "review-pack.export".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "review_pack_id":positionals[1],
                }),
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
            let (positionals, options) = parse_tail(tail, &["--idempotency", "--mode"], &[])?;
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
            let mode = options
                .get("--mode")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "incremental".to_owned());
            if !matches!(mode.as_str(), "full" | "incremental") {
                return Err("--mode must be full or incremental".to_owned());
            }
            Ok(Parsed {
                command: "scan.run".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "idempotency_key":idempotency_key,
                    "mode":mode,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "index status")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index status requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "index.status".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "files" => {
            let (positionals, options) = parse_tail(tail, &[], &["--allow-stale"])?;
            if !(1..=2).contains(&positionals.len()) {
                return Err("index files requires a ProjectId and optional query".to_owned());
            }
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index files requires a valid ProjectId".to_owned())?;
            if positionals
                .get(1)
                .is_some_and(|query| query.trim().is_empty() || query.chars().count() > 256)
            {
                return Err("file query must contain 1 through 256 characters".to_owned());
            }
            Ok(Parsed {
                command: "index.files".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "query":positionals.get(1),
                    "require_current":!options.contains_key("--allow-stale"),
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "search" => {
            let (positionals, options) = parse_tail(tail, &["--tier"], &["--allow-stale"])?;
            require_positionals(&positionals, 2, "index search")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index search requires a valid ProjectId".to_owned())?;
            let tier = options
                .get("--tier")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "text".to_owned());
            if !matches!(tier.as_str(), "text" | "syntax" | "semantic") {
                return Err("--tier must be text, syntax, or semantic".to_owned());
            }
            if positionals[1].trim().is_empty() || positionals[1].chars().count() > 256 {
                return Err("index query must contain 1 through 256 characters".to_owned());
            }
            Ok(Parsed {
                command: "index.search".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "query":positionals[1],
                    "tier":tier,
                    "require_current":!options.contains_key("--allow-stale"),
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "definitions" => {
            let (positionals, options) = parse_tail(tail, &[], &["--allow-stale"])?;
            require_positionals(&positionals, 2, "index definitions")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index definitions requires a valid ProjectId".to_owned())?;
            if positionals[1].trim().is_empty() || positionals[1].chars().count() > 256 {
                return Err("definition query must contain 1 through 256 characters".to_owned());
            }
            Ok(Parsed {
                command: "index.definitions".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "query":positionals[1],
                    "require_current":!options.contains_key("--allow-stale"),
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "references" => {
            let (positionals, options) = parse_tail(tail, &[], &["--allow-stale"])?;
            require_positionals(&positionals, 2, "index references")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index references requires a valid ProjectId".to_owned())?;
            star_contracts::ids::SymbolId::parse(positionals[1].clone())
                .map_err(|_| "index references requires a valid SymbolId".to_owned())?;
            Ok(Parsed {
                command: "index.references".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "symbol_id":positionals[1],
                    "require_current":!options.contains_key("--allow-stale"),
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "index" && second == "hardcoding" => {
            let (positionals, options) = parse_tail(tail, &[], &["--allow-stale"])?;
            require_positionals(&positionals, 1, "index hardcoding")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "index hardcoding requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "index.hardcoding".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "require_current":!options.contains_key("--allow-stale"),
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "registry" && matches!(second.as_str(), "list" | "status") =>
        {
            let (positionals, options) = parse_tail(tail, &["--manifest"], &[])?;
            require_positionals(&positionals, 1, &format!("registry {second}"))?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| format!("registry {second} requires a valid ProjectId"))?;
            let manifest_path = options
                .get("--manifest")
                .and_then(Clone::clone)
                .unwrap_or_else(|| ".star-control/registry/manifest.toml".to_owned());
            star_contracts::management::ProjectPathRef::parse(manifest_path.clone())
                .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
            Ok(Parsed {
                command: format!("registry.{second}"),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":manifest_path,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "registry" && second == "show" => {
            let (positionals, options) = parse_tail(tail, &["--manifest"], &[])?;
            require_positionals(&positionals, 2, "registry show")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "registry show requires a valid ProjectId".to_owned())?;
            star_contracts::managed_registry::ManagedDeclarationId::parse(positionals[1].clone())
                .map_err(|_| "registry show requires a valid ManagedDeclarationId".to_owned())?;
            let manifest_path = options
                .get("--manifest")
                .and_then(Clone::clone)
                .unwrap_or_else(|| ".star-control/registry/manifest.toml".to_owned());
            star_contracts::management::ProjectPathRef::parse(manifest_path.clone())
                .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
            Ok(Parsed {
                command: "registry.show".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "declaration_id":positionals[1],
                    "manifest_path":manifest_path,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "registry" && second == "candidate" && third == "inspect" =>
        {
            let (positionals, options) = parse_tail(tail, &["--manifest"], &[])?;
            require_positionals(&positionals, 1, "registry candidate inspect")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "registry candidate inspect requires a valid ProjectId".to_owned())?;
            let manifest_path = options
                .get("--manifest")
                .and_then(Clone::clone)
                .unwrap_or_else(|| ".star-control/registry/manifest.toml".to_owned());
            star_contracts::management::ProjectPathRef::parse(manifest_path.clone())
                .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
            Ok(Parsed {
                command: "registry.candidate.inspect".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":manifest_path,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "registry" && second == "candidate" && third == "classify" =>
        {
            let (positionals, options) = parse_tail(tail, &["--manifest", "--reason"], &[])?;
            require_positionals(&positionals, 3, "registry candidate classify")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "registry candidate classify requires a valid ProjectId".to_owned())?;
            serde_json::from_value::<
                star_contracts::managed_registry::ManagedDeclarationClassification,
            >(serde_json::Value::String(positionals[2].clone()))
            .map_err(|_| {
                "classification must be managed_declaration, candidate, or local_implementation_constant"
                    .to_owned()
            })?;
            let reason = required_option(&options, "--reason")?;
            if reason.trim().is_empty() || reason.chars().count() > 2_048 {
                return Err("--reason must contain 1 through 2048 characters".to_owned());
            }
            let manifest_path = options
                .get("--manifest")
                .and_then(Clone::clone)
                .unwrap_or_else(|| ".star-control/registry/manifest.toml".to_owned());
            star_contracts::management::ProjectPathRef::parse(manifest_path.clone())
                .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
            Ok(Parsed {
                command: "registry.candidate.classify".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "candidate_id":positionals[1],
                    "classification":positionals[2],
                    "reason":reason,
                    "manifest_path":manifest_path,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "registry" && second == "declaration" && third == "plan" =>
        {
            let (positionals, options) = parse_tail(
                tail,
                &[
                    "--manifest",
                    "--declaration",
                    "--desired",
                    "--reason",
                    "--consumers",
                ],
                &[],
            )?;
            require_positionals(&positionals, 2, "registry declaration plan")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "registry declaration plan requires a valid ProjectId".to_owned())?;
            serde_json::from_value::<
                star_contracts::managed_registry::ManagedDeclarationChangeKind,
            >(serde_json::Value::String(positionals[1].clone()))
            .map_err(|_| "registry declaration plan has an invalid change-kind".to_owned())?;
            let declaration_id = options.get("--declaration").and_then(Clone::clone);
            if let Some(value) = &declaration_id {
                star_contracts::managed_registry::ManagedDeclarationId::parse(value.clone())
                    .map_err(|_| {
                        "--declaration requires a valid ManagedDeclarationId".to_owned()
                    })?;
            }
            let desired_fields = read_bounded_json_file(&required_option(&options, "--desired")?)?;
            serde_json::from_value::<star_contracts::managed_registry::ManagedDesiredFields>(
                desired_fields.clone(),
            )
            .map_err(|_| "--desired must contain ManagedDesiredFields JSON".to_owned())?;
            let requested_consumer_scope =
                if let Some(path) = options.get("--consumers").and_then(Clone::clone) {
                    let value = read_bounded_json_file(&path)?;
                    serde_json::from_value::<Vec<star_contracts::ids::ProjectId>>(value)
                        .map_err(|_| "--consumers must contain a ProjectId JSON array".to_owned())?
                } else {
                    Vec::new()
                };
            let reason = required_option(&options, "--reason")?;
            if reason.trim().is_empty() || reason.chars().count() > 2_048 {
                return Err("--reason must contain 1 through 2048 characters".to_owned());
            }
            let manifest_path = options
                .get("--manifest")
                .and_then(Clone::clone)
                .unwrap_or_else(|| ".star-control/registry/manifest.toml".to_owned());
            star_contracts::management::ProjectPathRef::parse(manifest_path.clone())
                .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
            Ok(Parsed {
                command: "registry.declaration.plan".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "change_kind":positionals[1],
                    "declaration_id":declaration_id,
                    "desired_fields":desired_fields,
                    "reason":reason,
                    "requested_consumer_scope":requested_consumer_scope,
                    "manifest_path":manifest_path,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "contract" && second == "snapshot" => {
            let (positionals, options) = parse_tail(
                tail,
                &[
                    "--role",
                    "--source-revision",
                    "--manifest",
                    "--registry-snapshot",
                    "--revision",
                ],
                &[],
            )?;
            require_positionals(&positionals, 2, "contract snapshot")?;
            validate_project_id(&positionals[0], "contract snapshot")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            let role = required_option(&options, "--role")?;
            if !matches!(role.as_str(), "baseline" | "current") {
                return Err("--role must be baseline or current".to_owned());
            }
            let source_revision = options.get("--source-revision").and_then(Clone::clone);
            if role == "baseline" && source_revision.is_none() {
                return Err("baseline snapshot requires --source-revision".to_owned());
            }
            if role == "current" && source_revision.is_some() {
                return Err("current snapshot does not accept --source-revision".to_owned());
            }
            let manifest_path = development_manifest_path(&options)?;
            Ok(Parsed {
                command: "contract.snapshot".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":manifest_path,
                    "snapshot_id":positionals[1],
                    "role":role,
                    "source_revision":source_revision,
                    "registry_snapshot_ref":options.get("--registry-snapshot").and_then(Clone::clone),
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "contract" && second == "compare" => {
            let (positionals, options) = parse_tail(tail, &["--manifest", "--revision"], &[])?;
            require_positionals(&positionals, 4, "contract compare")?;
            validate_project_id(&positionals[0], "contract compare")?;
            for value in &positionals[1..] {
                validate_development_id(value, "record identifier")?;
            }
            Ok(Parsed {
                command: "contract.compare".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":development_manifest_path(&options)?,
                    "report_id":positionals[1],
                    "baseline_snapshot_id":positionals[2],
                    "current_snapshot_id":positionals[3],
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "docs" && second == "check" => {
            let (positionals, options) =
                parse_tail(tail, &["--registrations", "--manifest", "--revision"], &[])?;
            require_positionals(&positionals, 2, "docs check")?;
            validate_project_id(&positionals[0], "docs check")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            let registrations =
                read_bounded_json_file(&required_option(&options, "--registrations")?)?;
            require_json_object_keys(&registrations, &["commands", "config_keys"])?;
            let commands = registrations
                .get("commands")
                .and_then(serde_json::Value::as_array)
                .ok_or("registrations.commands must be an array")?
                .clone();
            let config_keys = registrations
                .get("config_keys")
                .and_then(serde_json::Value::as_array)
                .ok_or("registrations.config_keys must be an array")?
                .clone();
            Ok(Parsed {
                command: "docs.check".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":development_manifest_path(&options)?,
                    "snapshot_id":positionals[1],
                    "registered_commands":commands,
                    "registered_config_keys":config_keys,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "config" && second == "trace" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 2, "config trace")?;
            validate_project_id(&positionals[0], "config trace")?;
            validate_development_id(&positionals[1], "trace-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(
                &input,
                &[
                    "key_ref",
                    "lifecycle",
                    "declaration_ref",
                    "readers",
                    "overrides",
                ],
            )?;
            Ok(Parsed {
                command: "config.trace".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "trace_id":positionals[1],
                    "key_ref":input.get("key_ref"),
                    "lifecycle":input.get("lifecycle"),
                    "declaration_ref":input.get("declaration_ref"),
                    "readers":input.get("readers"),
                    "overrides":input.get("overrides"),
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "environment" && second == "fingerprint" => {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 2, "environment fingerprint")?;
            validate_project_id(&positionals[0], "environment fingerprint")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            Ok(Parsed {
                command: "environment.fingerprint".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "snapshot_id":positionals[1],
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "project" && second == "doctor" => {
            parse_project_doctor_command(tail, false, json)
        }
        [first, second, third, tail @ ..]
            if first == "clean-room" && second == "specification" && third == "publish" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 2, "clean-room specification publish")?;
            validate_project_id(&positionals[0], "clean-room specification publish")?;
            let specification = read_bounded_json_file(&positionals[1])?;
            serde_json::from_value::<star_contracts::development_v2::CleanRoomSpecification>(
                specification.clone(),
            )
            .map_err(|_| "clean-room specification JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "clean-room.specification.publish".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "specification":specification,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "clean-room" && second == "readiness" => {
            parse_project_doctor_command(tail, true, json)
        }
        [first, second, tail @ ..] if first == "dependency-security" && second == "input" => {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 3, "dependency-security input")?;
            validate_project_id(&positionals[0], "dependency-security input")?;
            validate_development_id(&positionals[1], "manifest-id")?;
            validate_development_id(&positionals[2], "environment-snapshot-id")?;
            Ok(Parsed {
                command: "dependency-security.input".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_id":positionals[1],
                    "environment_snapshot_id":positionals[2],
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "development" && second == "effect" && third == "record" =>
        {
            let (positionals, options) = parse_tail(
                tail,
                &[
                    "--arguments",
                    "--approval",
                    "--permission",
                    "--gate",
                    "--revision",
                ],
                &[],
            )?;
            require_positionals(&positionals, 6, "development effect record")?;
            validate_project_id(&positionals[0], "development effect record")?;
            validate_development_id(&positionals[1], "effect-receipt-id")?;
            if !matches!(
                positionals[2].as_str(),
                "security_refresh"
                    | "debugger_capture"
                    | "license_scan"
                    | "dependency_prepare"
                    | "dependency_apply"
                    | "updater_apply"
                    | "migration_execute"
                    | "performance_run"
                    | "language_cutover"
                    | "remote_recovery"
            ) {
                return Err("development effect record has an unsupported effect kind".to_owned());
            }
            if positionals[3].trim().is_empty()
                || positionals[3].chars().count() > 512
                || positionals[3].contains('\0')
            {
                return Err("subject-ref must contain 1 through 512 non-NUL characters".to_owned());
            }
            positionals[4]
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "subject-sha256 must be an exact lowercase sha256".to_owned())?;
            star_contracts::ids::OperationId::parse(positionals[5].clone())
                .map_err(|_| "operation-id must be a valid OperationId".to_owned())?;
            let bound_arguments =
                read_bounded_json_file(&required_option(&options, "--arguments")?)?;
            if !bound_arguments.is_object() {
                return Err("--arguments must name one bounded JSON object".to_owned());
            }
            Ok(Parsed {
                command: "development.effect.record".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "receipt_id":positionals[1],
                    "effect_kind":positionals[2],
                    "exact_subject_ref":positionals[3],
                    "exact_subject_fingerprint":positionals[4],
                    "operation_id":positionals[5],
                    "bound_arguments":bound_arguments,
                    "approval_ref":options.get("--approval").and_then(Clone::clone),
                    "permission_decision_ref":options.get("--permission").and_then(Clone::clone),
                    "gate_decision_ref":options.get("--gate").and_then(Clone::clone),
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "development" && second == "record" && third == "show" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 2, "development record show")?;
            validate_development_id(&positionals[0], "record-kind")?;
            validate_development_id(&positionals[1], "record-id")?;
            let revision = options
                .get("--revision")
                .and_then(Clone::clone)
                .map(|value| parse_positive_revision(&value))
                .transpose()?;
            Ok(Parsed {
                command: "development.record.show".to_owned(),
                payload: serde_json::json!({
                    "record_kind":positionals[0],
                    "record_id":positionals[1],
                    "revision":revision,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "development" && second == "record" && third == "list" =>
        {
            let (positionals, options) = parse_tail(tail, &["--project"], &[])?;
            require_positionals(&positionals, 1, "development record list")?;
            validate_development_id(&positionals[0], "record-kind")?;
            let project_id = options.get("--project").and_then(Clone::clone);
            if let Some(project_id) = &project_id {
                validate_project_id(project_id, "development record list")?;
            }
            Ok(Parsed {
                command: "development.record.list".to_owned(),
                payload: serde_json::json!({
                    "record_kind":positionals[0],
                    "project_id":project_id,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "failures" && second == "inspect" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 2, "failures inspect")?;
            validate_project_id(&positionals[0], "failures inspect")?;
            validate_development_id(&positionals[1], "failure-record-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(
                &input,
                &[
                    "failure_record_id",
                    "occurrence_id",
                    "diagnostic_refs",
                    "finding_refs",
                    "subject_binding",
                    "failure_kind",
                    "producer_code",
                    "raw_message",
                    "logical_owner",
                    "signature",
                    "causality_role",
                    "root_candidate_refs",
                    "cascade_parent_refs",
                    "invocation",
                    "environment_compatibility_class",
                    "environment_fingerprint",
                    "input_refs",
                    "input_fingerprint",
                    "seed",
                    "manifest_fingerprint",
                    "stdout_ref",
                    "stderr_ref",
                    "artifact_refs",
                    "observed_at",
                    "attempt_id",
                    "verification_state",
                ],
            )?;
            Ok(Parsed {
                command: "failures.inspect".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "failure_record_id":positionals[1],
                    "input":input,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "failures" && second == "reproduce" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 3, "failures reproduce")?;
            validate_project_id(&positionals[0], "failures reproduce")?;
            validate_development_id(&positionals[1], "reproduction-pack-id")?;
            validate_development_id(&positionals[2], "failure-record-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(
                &input,
                &[
                    "reproduction_pack_id",
                    "dirty_state",
                    "manifest_refs",
                    "expected_result",
                    "observed_result",
                    "attempts",
                    "artifacts",
                    "limitations",
                ],
            )?;
            Ok(Parsed {
                command: "failures.reproduce".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "reproduction_pack_id":positionals[1],
                    "failure_record_id":positionals[2],
                    "input":input,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "failures" && second == "compare" => {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 2, "failures compare")?;
            validate_project_id(&positionals[0], "failures compare")?;
            let record = read_bounded_json_file(&positionals[1])?;
            serde_json::from_value::<star_contracts::maintenance_v2::RegressionRecord>(
                record.clone(),
            )
            .map_err(|_| "regression record JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "failures.compare".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "record":record,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "failures" && second == "recovery-plan" => {
            parse_m7_recovery_plan(
                tail,
                "failures.recovery-plan",
                "failures recovery-plan",
                json,
            )
        }
        [first, second, tail @ ..] if first == "security" && second == "inspect" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 3, "security inspect")?;
            validate_project_id(&positionals[0], "security inspect")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            validate_development_id(&positionals[2], "effect-receipt-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(
                &input,
                &[
                    "source",
                    "retrieved_at",
                    "valid_until",
                    "evaluation_time",
                    "source_artifact_ref",
                    "source_sha256",
                    "observations",
                    "available",
                ],
            )?;
            Ok(Parsed {
                command: "security.inspect".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "snapshot_id":positionals[1],
                    "effect_receipt_id":positionals[2],
                    "input":input,
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "security" && second == "release-manifest" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 3, "security release-manifest")?;
            validate_project_id(&positionals[0], "security release-manifest")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            validate_development_id(&positionals[2], "dependency-snapshot-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(&input, &["external_snapshot_ids", "observations"])?;
            Ok(Parsed {
                command: "security.release-manifest".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "snapshot_id":positionals[1],
                    "dependency_snapshot_id":positionals[2],
                    "external_snapshot_ids":input.get("external_snapshot_ids"),
                    "observations":input.get("observations"),
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "deps" && second == "scan" => {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 2, "deps scan")?;
            validate_project_id(&positionals[0], "deps scan")?;
            validate_development_id(&positionals[1], "snapshot-id")?;
            Ok(Parsed {
                command: "deps.scan".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "snapshot_id":positionals[1],
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "deps" && matches!(second.as_str(), "candidates" | "prepare") =>
        {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 3, "deps candidates|prepare")?;
            validate_project_id(&positionals[0], "deps candidates|prepare")?;
            validate_development_id(&positionals[1], "plan-id")?;
            validate_development_id(&positionals[2], "dependency-snapshot-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(
                &input,
                &[
                    "candidate",
                    "expected_manifest_paths",
                    "expected_lockfile_paths",
                ],
            )?;
            serde_json::from_value::<star_contracts::maintenance_v2::UpdateCandidate>(
                input
                    .get("candidate")
                    .cloned()
                    .ok_or("dependency candidate is missing")?,
            )
            .map_err(|_| "dependency candidate JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: format!("deps.{second}"),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "plan_id":positionals[1],
                    "dependency_snapshot_id":positionals[2],
                    "candidate":input.get("candidate"),
                    "expected_manifest_paths":input.get("expected_manifest_paths"),
                    "expected_lockfile_paths":input.get("expected_lockfile_paths"),
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "deps" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "deps status")?;
            validate_project_id(&positionals[0], "deps status")?;
            validate_development_id(&positionals[1], "plan-id")?;
            Ok(Parsed {
                command: "deps.status".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "plan_id":positionals[1],
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "deps" && second == "rollback-plan" => {
            parse_m7_recovery_plan(tail, "deps.rollback-plan", "deps rollback-plan", json)
        }
        [first, second, tail @ ..] if first == "maintenance" && second == "radar" => {
            let (positionals, options) = parse_tail(tail, &["--input", "--revision"], &[])?;
            require_positionals(&positionals, 1, "maintenance radar")?;
            validate_development_id(&positionals[0], "snapshot-id")?;
            let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
            require_json_object_keys(&input, &["evaluation_time", "valid_until", "items"])?;
            Ok(Parsed {
                command: "maintenance.radar".to_owned(),
                payload: serde_json::json!({
                    "snapshot_id":positionals[0],
                    "evaluation_time":input.get("evaluation_time"),
                    "valid_until":input.get("valid_until"),
                    "items":input.get("items"),
                    "revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "migration" && second == "inspect" => {
            let (positionals, options) = parse_tail(tail, &["--manifest"], &[])?;
            require_positionals(&positionals, 2, "migration inspect")?;
            validate_project_id(&positionals[0], "migration inspect")?;
            validate_development_id(&positionals[1], "target-id")?;
            Ok(Parsed {
                command: "migration.inspect".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":migration_manifest_path(&options)?,
                    "target_id":positionals[1],
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "migration" && second == "plan" => {
            let (positionals, options) = parse_tail(tail, &["--manifest", "--revision"], &[])?;
            require_positionals(&positionals, 2, "migration plan")?;
            validate_project_id(&positionals[0], "migration plan")?;
            let input = read_bounded_json_file(&positionals[1])?;
            if !input.is_object() {
                return Err("migration plan input must be a JSON object".to_owned());
            }
            Ok(Parsed {
                command: "migration.plan".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "manifest_path":migration_manifest_path(&options)?,
                    "input":input,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "migration" && second == "checkpoint" => {
            parse_m8_project_record::<star_contracts::migration_v2::MigrationCheckpointV2>(
                tail,
                "migration.checkpoint",
                "migration checkpoint",
                "checkpoint",
                true,
                json,
            )
        }
        [first, second, tail @ ..]
            if first == "migration"
                && matches!(
                    second.as_str(),
                    "dry-run"
                        | "backup"
                        | "rehearse"
                        | "execute"
                        | "resume"
                        | "validate"
                        | "rollback"
                ) =>
        {
            let (positionals, options) = parse_tail(tail, &["--fingerprint", "--revision"], &[])?;
            require_positionals(&positionals, 3, "migration phase")?;
            validate_project_id(&positionals[0], "migration phase")?;
            validate_development_id(&positionals[1], "plan-id")?;
            let fingerprint = required_option(&options, "--fingerprint")?;
            fingerprint
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| {
                    "--fingerprint must be the exact lowercase migration plan sha256".to_owned()
                })?;
            let attempt = read_bounded_json_file(&positionals[2])?;
            serde_json::from_value::<star_contracts::migration_v2::MigrationAttempt>(
                attempt.clone(),
            )
            .map_err(|_| "migration attempt JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: format!("migration.{second}"),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "plan_id":positionals[1],
                    "approved_plan_fingerprint":fingerprint,
                    "attempt":attempt,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "migration" && second == "validation-report" => {
            parse_m8_project_record::<star_contracts::migration_v2::MigrationValidationReport>(
                tail,
                "migration.validation-report",
                "migration validation-report",
                "report",
                false,
                json,
            )
        }
        [first, second, tail @ ..] if first == "migration" && second == "restore-verify" => {
            parse_m8_project_record::<star_contracts::migration_v2::RestoreVerificationRecord>(
                tail,
                "migration.restore-verify",
                "migration restore-verify",
                "record",
                false,
                json,
            )
        }
        [first, second, tail @ ..] if first == "migration" && second == "status" => {
            parse_m8_project_status(tail, "migration.status", "migration status", json)
        }
        [first, second, tail @ ..] if first == "migration" && second == "handoff" => {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 1, "migration handoff")?;
            let handoff = read_bounded_json_file(&positionals[0])?;
            serde_json::from_value::<star_contracts::migration_v2::CrossProjectMigrationHandoff>(
                handoff.clone(),
            )
            .map_err(|_| "migration handoff JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "migration.handoff".to_owned(),
                payload: serde_json::json!({
                    "handoff":handoff,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "performance" && second == "plan" => {
            parse_m8_project_record::<star_contracts::migration_v2::PerformanceWorkloadSpec>(
                tail,
                "performance.plan",
                "performance plan",
                "specification",
                false,
                json,
            )
        }
        [first, second, tail @ ..] if first == "performance" && second == "run" => {
            parse_m8_project_record::<star_contracts::migration_v2::PerformanceRun>(
                tail,
                "performance.run",
                "performance run",
                "run",
                true,
                json,
            )
        }
        [first, second, tail @ ..] if first == "performance" && second == "compare" => {
            let (positionals, options) = parse_tail(
                tail,
                &["--baseline-runs", "--candidate-runs", "--revision"],
                &[],
            )?;
            require_positionals(&positionals, 3, "performance compare")?;
            validate_project_id(&positionals[0], "performance compare")?;
            validate_development_id(&positionals[1], "workload-id")?;
            validate_development_id(&positionals[2], "comparison-id")?;
            let baseline_run_ids =
                read_m8_string_array(&required_option(&options, "--baseline-runs")?)?;
            let candidate_run_ids =
                read_m8_string_array(&required_option(&options, "--candidate-runs")?)?;
            Ok(Parsed {
                command: "performance.compare".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "workload_id":positionals[1],
                    "comparison_id":positionals[2],
                    "baseline_run_ids":baseline_run_ids,
                    "candidate_run_ids":candidate_run_ids,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "language-migration" && second == "plan" => {
            parse_m8_project_record::<star_contracts::migration_v2::LanguageMigrationPlan>(
                tail,
                "language-migration.plan",
                "language-migration plan",
                "plan",
                false,
                json,
            )
        }
        [first, second, tail @ ..] if first == "language-migration" && second == "equivalence" => {
            parse_m8_project_record::<star_contracts::migration_v2::EquivalenceReport>(
                tail,
                "language-migration.equivalence",
                "language-migration equivalence",
                "report",
                true,
                json,
            )
        }
        [first, second, tail @ ..] if first == "language-migration" && second == "cutover" => {
            let (positionals, options) = parse_tail(tail, &["--fingerprint"], &[])?;
            require_positionals(&positionals, 4, "language-migration cutover")?;
            validate_project_id(&positionals[0], "language-migration cutover")?;
            validate_development_id(&positionals[1], "plan-id")?;
            validate_development_id(&positionals[2], "equivalence-report-id")?;
            validate_development_id(&positionals[3], "effect-receipt-id")?;
            let fingerprint = required_option(&options, "--fingerprint")?;
            fingerprint
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| {
                    "--fingerprint must be the exact lowercase language migration plan sha256"
                        .to_owned()
                })?;
            Ok(Parsed {
                command: "language-migration.cutover".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "plan_id":positionals[1],
                    "equivalence_report_id":positionals[2],
                    "effect_receipt_id":positionals[3],
                    "approved_plan_fingerprint":fingerprint,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "language-migration" && second == "status" => {
            parse_m8_project_status(
                tail,
                "language-migration.status",
                "language-migration status",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "goal" && third == "publish" =>
        {
            parse_m9_single_document::<star_contracts::coordination_v2::MultiProjectGoal>(
                tail,
                "change-bundle.goal.publish",
                "change-bundle goal publish",
                "goal",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "participant" && third == "publish" =>
        {
            parse_m9_single_document::<star_contracts::coordination_v2::ChangeBundleParticipantV2>(
                tail,
                "change-bundle.participant.publish",
                "change-bundle participant publish",
                "participant",
                json,
            )
        }
        [first, second, tail @ ..]
            if first == "change-bundle"
                && matches!(second.as_str(), "plan" | "hold" | "resume") =>
        {
            let (positionals, options) = parse_tail(tail, &["--participants", "--revision"], &[])?;
            require_positionals(&positionals, 2, "change-bundle plan|hold|resume")?;
            validate_development_id(&positionals[0], "goal-id")?;
            let bundle = read_bounded_json_file(&positionals[1])?;
            serde_json::from_value::<star_contracts::coordination_v2::CrossRepoChangeBundle>(
                bundle.clone(),
            )
            .map_err(|_| "change bundle JSON has an invalid shape".to_owned())?;
            let participant_ids =
                read_m8_string_array(&required_option(&options, "--participants")?)?;
            Ok(Parsed {
                command: format!("change-bundle.{second}"),
                payload: serde_json::json!({
                    "goal_id":positionals[0],
                    "bundle":bundle,
                    "participant_ids":participant_ids,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "change-bundle"
                && matches!(second.as_str(), "show" | "status" | "conflicts") =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "change-bundle show|status|conflicts")?;
            validate_development_id(&positionals[0], "bundle-id")?;
            Ok(Parsed {
                command: format!("change-bundle.{second}"),
                payload: serde_json::json!({"bundle_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "change-bundle" && second == "preflight" => {
            let (positionals, options) =
                parse_tail(tail, &["--subjects", "--ordered-pairs", "--revision"], &[])?;
            require_positionals(&positionals, 2, "change-bundle preflight")?;
            validate_development_id(&positionals[0], "bundle-id")?;
            validate_development_id(&positionals[1], "analysis-id")?;
            let subjects = read_bounded_json_file(&required_option(&options, "--subjects")?)?;
            serde_json::from_value::<Vec<star_contracts::coordination_v2::OverlapSubject>>(
                subjects.clone(),
            )
            .map_err(|_| "overlap subjects JSON has an invalid shape".to_owned())?;
            let ordered_pairs =
                read_bounded_json_file(&required_option(&options, "--ordered-pairs")?)?;
            serde_json::from_value::<Vec<[String; 2]>>(ordered_pairs.clone())
                .map_err(|_| "ordered pairs JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "change-bundle.preflight".to_owned(),
                payload: serde_json::json!({
                    "bundle_id":positionals[0],
                    "analysis_id":positionals[1],
                    "subjects":subjects,
                    "ordered_pairs":ordered_pairs,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "worktree" && third == "plan" =>
        {
            parse_m9_project_document::<star_contracts::coordination_v2::WorktreeRecord>(
                tail,
                "change-bundle.worktree.plan",
                "change-bundle worktree plan",
                "record",
                json,
            )
        }
        [first, second, tail @ ..] if first == "change-bundle" && second == "apply" => {
            let (positionals, options) = parse_tail(
                tail,
                &["--patch-applications", "--migration-attempts", "--revision"],
                &[],
            )?;
            require_positionals(&positionals, 2, "change-bundle apply")?;
            validate_project_id(&positionals[0], "change-bundle apply")?;
            let participant = read_bounded_json_file(&positionals[1])?;
            serde_json::from_value::<star_contracts::coordination_v2::ChangeBundleParticipantV2>(
                participant.clone(),
            )
            .map_err(|_| "change bundle participant JSON has an invalid shape".to_owned())?;
            let patch_application_ids = read_m9_string_array_allow_empty(&required_option(
                &options,
                "--patch-applications",
            )?)?;
            let migration_attempt_ids = read_m9_string_array_allow_empty(&required_option(
                &options,
                "--migration-attempts",
            )?)?;
            if patch_application_ids.is_empty() && migration_attempt_ids.is_empty() {
                return Err("change-bundle apply requires at least one effect record".to_owned());
            }
            Ok(Parsed {
                command: "change-bundle.apply".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],"participant":participant,
                    "patch_application_ids":patch_application_ids,
                    "migration_attempt_ids":migration_attempt_ids,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "change-bundle" && second == "validate" => {
            parse_m9_project_document::<star_contracts::coordination_v2::ChangeBundleParticipantV2>(
                tail,
                "change-bundle.validate",
                "change-bundle validate",
                "participant",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "worktree" && third == "create" =>
        {
            let (positionals, options) = parse_tail(
                tail,
                &["--branch", "--permission", "--gate", "--approve"],
                &[],
            )?;
            require_positionals(&positionals, 2, "change-bundle worktree create")?;
            validate_project_id(&positionals[0], "change-bundle worktree create")?;
            validate_development_id(&positionals[1], "worktree-id")?;
            let approved = required_sha256_option(&options, "--approve")?;
            Ok(Parsed {
                command: "change-bundle.worktree.create".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "worktree_id":positionals[1],
                    "branch_ref":required_option(&options,"--branch")?,
                    "permission_decision_ref":required_option(&options,"--permission")?,
                    "gate_decision_ref":required_option(&options,"--gate")?,
                    "approved_record_fingerprint":approved,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "merge" && third == "plan" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 3, "change-bundle merge plan")?;
            validate_project_id(&positionals[0], "change-bundle merge plan")?;
            validate_development_id(&positionals[2], "overlap-analysis-id")?;
            let plan = read_bounded_json_file(&positionals[1])?;
            serde_json::from_value::<star_contracts::coordination_v2::MergePlanV2>(plan.clone())
                .map_err(|_| "merge plan JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "change-bundle.merge.plan".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],"plan":plan,
                    "overlap_analysis_id":positionals[2],
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "merge" && third == "enqueue" =>
        {
            parse_m9_project_document::<star_contracts::coordination_v2::MergeQueueRecord>(
                tail,
                "change-bundle.merge.enqueue",
                "change-bundle merge enqueue",
                "queue",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "merge" && third == "run" =>
        {
            let (positionals, options) = parse_tail(
                tail,
                &["--permission", "--gate", "--approve", "--revision"],
                &[],
            )?;
            require_positionals(&positionals, 4, "change-bundle merge run")?;
            validate_project_id(&positionals[0], "change-bundle merge run")?;
            validate_development_id(&positionals[1], "merge-plan-id")?;
            validate_git_oid_cli(&positionals[2], "input-commit-oid")?;
            let result = read_bounded_json_file(&positionals[3])?;
            serde_json::from_value::<star_contracts::coordination_v2::ProjectMergeResult>(
                result.clone(),
            )
            .map_err(|_| "project merge result JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "change-bundle.merge.run".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],"merge_plan_id":positionals[1],
                    "input_commit_oid":positionals[2],"result":result,
                    "permission_decision_ref":required_option(&options,"--permission")?,
                    "gate_decision_ref":required_option(&options,"--gate")?,
                    "approved_plan_fingerprint":required_sha256_option(&options,"--approve")?,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "merge" && third == "result" =>
        {
            parse_m9_project_document::<star_contracts::coordination_v2::ProjectMergeResult>(
                tail,
                "change-bundle.merge.result",
                "change-bundle merge result",
                "result",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "conflict" && third == "publish" =>
        {
            parse_m9_project_document::<star_contracts::coordination_v2::MergeConflictRecord>(
                tail,
                "change-bundle.conflict.publish",
                "change-bundle conflict publish",
                "conflict",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "remote" && third == "refresh" =>
        {
            let (positionals, options) =
                parse_tail(tail, &["--captured-at", "--valid-until", "--revision"], &[])?;
            require_positionals(&positionals, 3, "change-bundle remote refresh")?;
            validate_project_id(&positionals[0], "change-bundle remote refresh")?;
            validate_development_id(&positionals[1], "remote-name")?;
            validate_development_id(&positionals[2], "snapshot-id")?;
            Ok(Parsed {
                command: "change-bundle.remote.snapshot".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],"remote_name":positionals[1],
                    "snapshot_id":positionals[2],
                    "captured_at":required_option(&options,"--captured-at")?,
                    "valid_until":required_option(&options,"--valid-until")?,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, fourth, tail @ ..]
            if first == "change-bundle"
                && second == "remote"
                && third == "operation"
                && matches!(fourth.as_str(), "prepare" | "observe") =>
        {
            parse_m9_single_document::<star_contracts::coordination_v2::RemoteOperationRecord>(
                tail,
                &format!("change-bundle.remote.operation.{fourth}"),
                "change-bundle remote operation",
                "operation",
                json,
            )
        }
        [first, second, third, fourth, tail @ ..]
            if first == "change-bundle"
                && second == "remote"
                && third == "operation"
                && fourth == "apply" =>
        {
            let (positionals, options) = parse_tail(tail, &["--request-fingerprint"], &[])?;
            require_positionals(&positionals, 1, "change-bundle remote operation apply")?;
            validate_development_id(&positionals[0], "remote-operation-id")?;
            Ok(Parsed {
                command: "change-bundle.remote.operation.apply".to_owned(),
                payload: serde_json::json!({
                    "remote_operation_id":positionals[0],
                    "request_fingerprint":required_sha256_option(&options,"--request-fingerprint")?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "release-handoff" && third == "plan" =>
        {
            parse_m9_single_document::<star_contracts::coordination_v2::ChangeBundleReleaseHandoff>(
                tail,
                "change-bundle.release-handoff.plan",
                "change-bundle release-handoff plan",
                "handoff",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "recovery" && third == "plan" =>
        {
            parse_m9_project_document::<star_contracts::maintenance_v2::RecoveryPlanV2>(
                tail,
                "change-bundle.recovery.plan",
                "change-bundle recovery plan",
                "plan",
                json,
            )
        }
        [first, second, third, tail @ ..]
            if first == "change-bundle" && second == "recovery" && third == "apply" =>
        {
            let (positionals, options) = parse_tail(
                tail,
                &["--approve", "--permission", "--gate", "--receipt"],
                &[],
            )?;
            require_positionals(&positionals, 2, "change-bundle recovery apply")?;
            validate_project_id(&positionals[0], "change-bundle recovery apply")?;
            validate_development_id(&positionals[1], "recovery-plan-id")?;
            Ok(Parsed {
                command: "change-bundle.recovery.apply".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],"recovery_plan_id":positionals[1],
                    "approved_plan_fingerprint":required_sha256_option(&options,"--approve")?,
                    "permission_decision_ref":required_option(&options,"--permission")?,
                    "gate_decision_ref":required_option(&options,"--gate")?,
                    "effect_receipt_id":required_option(&options,"--receipt")?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "release" && second == "candidate" && third == "create" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "release candidate create")?;
            validate_project_id(&positionals[0], "release candidate create")?;
            let candidate = read_bounded_json_file(&positionals[1])?;
            candidate
                .as_object()
                .ok_or_else(|| "candidate JSON must be an object".to_owned())?;
            Ok(Parsed {
                command: "release.candidate.create".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "candidate":candidate,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "release" && second == "artifacts" && third == "verify" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 3, "release artifacts verify")?;
            validate_project_id(&positionals[0], "release artifacts verify")?;
            validate_development_id(&positionals[1], "release-manifest-id")?;
            let artifacts = read_bounded_json_file(&positionals[2])?;
            artifacts
                .as_array()
                .ok_or_else(|| "artifacts JSON must be an array".to_owned())?;
            Ok(Parsed {
                command: "release.artifacts.verify".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "release_manifest_id":positionals[1],
                    "artifacts":artifacts,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "release" && second == "verification" && third == "record" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 3, "release verification record")?;
            validate_project_id(&positionals[0], "release verification record")?;
            validate_development_id(&positionals[1], "release-manifest-id")?;
            let verification = read_bounded_json_file(&positionals[2])?;
            verification
                .as_object()
                .ok_or_else(|| "verification JSON must be an object".to_owned())?;
            Ok(Parsed {
                command: "release.verification.record".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "release_manifest_id":positionals[1],
                    "verification":verification,
                }),
                json,
            })
        }
        [first, second, tail @ ..]
            if first == "release" && matches!(second.as_str(), "promote" | "show" | "status") =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "release promote|show|status")?;
            validate_development_id(&positionals[0], "release-manifest-id")?;
            Ok(Parsed {
                command: format!("release.{second}"),
                payload: serde_json::json!({"release_manifest_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "release" && second == "lifecycle" && third == "publish" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 3, "release lifecycle publish")?;
            validate_project_id(&positionals[0], "release lifecycle publish")?;
            validate_development_id(&positionals[1], "lifecycle-id")?;
            let evidence = read_bounded_json_file(&positionals[2])?;
            evidence
                .as_object()
                .ok_or_else(|| "lifecycle evidence JSON must be an object".to_owned())?;
            Ok(Parsed {
                command: "release.lifecycle.publish".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "lifecycle_id":positionals[1],
                    "evidence":evidence,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "release"
                && second == "publish"
                && matches!(third.as_str(), "prepare" | "authorize" | "apply") =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            let expected = if third == "apply" { 1 } else { 2 };
            require_positionals(&positionals, expected, "release publish")?;
            validate_development_id(&positionals[0], "release-manifest-id")?;
            let payload = match third.as_str() {
                "prepare" => {
                    validate_development_id(&positionals[1], "before-snapshot-id")?;
                    serde_json::json!({
                        "release_manifest_id":positionals[0],
                        "before_snapshot_ref":positionals[1],
                    })
                }
                "authorize" => {
                    star_contracts::ids::ApprovalId::parse(positionals[1].clone()).map_err(
                        |_| "release publish authorize requires a valid ApprovalId".to_owned(),
                    )?;
                    serde_json::json!({
                        "release_manifest_id":positionals[0],
                        "approval_id":positionals[1],
                    })
                }
                "apply" => serde_json::json!({
                    "release_manifest_id":positionals[0],
                }),
                _ => unreachable!(),
            };
            Ok(Parsed {
                command: format!("release.publish.{third}"),
                payload,
                json,
            })
        }
        [first, second, tail @ ..] if first == "evaluation" && second == "run" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 2, "evaluation run")?;
            validate_project_id(&positionals[0], "evaluation run")?;
            let input = read_bounded_json_file(&positionals[1])?;
            input
                .as_object()
                .ok_or_else(|| "evaluation input JSON must be an object".to_owned())?;
            Ok(Parsed {
                command: "evaluation.run".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0],"input":input}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "evaluation" && second == "show" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "evaluation show")?;
            validate_development_id(&positionals[0], "evaluation-run-id")?;
            Ok(Parsed {
                command: "evaluation.show".to_owned(),
                payload: serde_json::json!({"evaluation_run_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "evaluation" && second == "catalog" && third == "publish" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
            require_positionals(&positionals, 1, "evaluation catalog publish")?;
            let item = read_bounded_json_file(&positionals[0])?;
            serde_json::from_value::<star_contracts::release_v2::EvaluationCatalogItem>(
                item.clone(),
            )
            .map_err(|_| "evaluation catalog item JSON has an invalid shape".to_owned())?;
            Ok(Parsed {
                command: "evaluation.catalog.publish".to_owned(),
                payload: serde_json::json!({
                    "item":item,
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "evaluation" && second == "catalog" && third == "transition" =>
        {
            let (positionals, options) = parse_tail(tail, &["--revision"], &["--trial-candidate"])?;
            require_positionals(&positionals, 2, "evaluation catalog transition")?;
            if positionals[0].is_empty()
                || positionals[0].chars().count() > 384
                || positionals[0].contains('\0')
            {
                return Err("evaluation catalog record-id is invalid".to_owned());
            }
            if !matches!(
                positionals[1].as_str(),
                "deprecated" | "retired" | "rejected"
            ) {
                return Err("next lifecycle must be deprecated, retired, or rejected".to_owned());
            }
            Ok(Parsed {
                command: "evaluation.catalog.transition".to_owned(),
                payload: serde_json::json!({
                    "record_id":positionals[0],
                    "next":positionals[1],
                    "trial_candidate":options.contains_key("--trial-candidate"),
                    "record_revision":development_revision(&options)?,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "profile" && second == "list" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "profile list")?;
            Ok(Parsed {
                command: "profile.list".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "profile" && second == "show" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "profile show")?;
            validate_profile_id(&positionals[0])?;
            Ok(Parsed {
                command: "profile.show".to_owned(),
                payload: serde_json::json!({"profile_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "profile" && second == "resolve" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            if positionals.is_empty() || positionals.len() > 16 {
                return Err("profile resolve requires 1 through 16 profile IDs".to_owned());
            }
            for profile_id in &positionals {
                validate_profile_id(profile_id)?;
            }
            if positionals.iter().collect::<BTreeSet<_>>().len() != positionals.len() {
                return Err("profile resolve does not accept duplicate profile IDs".to_owned());
            }
            Ok(Parsed {
                command: "profile.resolve".to_owned(),
                payload: serde_json::json!({"profile_ids":positionals}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "graph" && second == "neighbors" => {
            let (positionals, options) = parse_tail(tail, &[], &["--allow-stale"])?;
            require_positionals(&positionals, 2, "graph neighbors")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "graph neighbors requires a valid ProjectId".to_owned())?;
            if positionals[1].trim().is_empty() || positionals[1].chars().count() > 512 {
                return Err("entity key must contain 1 through 512 characters".to_owned());
            }
            Ok(Parsed {
                command: "graph.neighbors".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "entity_key":positionals[1],
                    "require_current":!options.contains_key("--allow-stale"),
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
        [first, second, third, tail @ ..]
            if first == "style" && second == "rust" && third == "inspect" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "style rust inspect")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "style rust inspect requires a valid ProjectId".to_owned())?;
            Ok(Parsed {
                command: "style.rust.inspect".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "style"
                && second == "rust"
                && matches!(third.as_str(), "check" | "prepare" | "auto-apply") =>
        {
            let (positionals, options) = parse_tail(tail, &["--scope", "--package"], &[])?;
            require_positionals(&positionals, 1, "style rust check|prepare|auto-apply")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone()).map_err(|_| {
                "style rust check|prepare|auto-apply requires a valid ProjectId".to_owned()
            })?;
            let scope = options
                .get("--scope")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "workspace".to_owned());
            if third != "check" && !options.contains_key("--scope") {
                return Err("--scope is required for prepare and auto-apply".to_owned());
            }
            let package = options.get("--package").and_then(Clone::clone);
            match (scope.as_str(), package.as_deref()) {
                ("workspace", None) => {}
                ("package", Some(value))
                    if !value.is_empty()
                        && value.len() <= 512
                        && !value.starts_with('-')
                        && !value.contains('\0')
                        && !value.chars().any(char::is_whitespace) => {}
                ("package", None) => {
                    return Err("--package is required when --scope package is used".to_owned());
                }
                ("workspace", Some(_)) => {
                    return Err("--package is valid only with --scope package".to_owned());
                }
                _ => return Err("--scope must be workspace or package".to_owned()),
            }
            let command = match third.as_str() {
                "check" => "style.rust.check",
                "prepare" => "style.rust.prepare",
                "auto-apply" => "style.rust.auto-apply",
                _ => unreachable!(),
            };
            Ok(Parsed {
                command: command.to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "scope":scope,
                    "package":package,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "recipe" && second == "list" => {
            let (positionals, options) = parse_tail(tail, &["--language", "--rewrite-kind"], &[])?;
            require_positionals(&positionals, 0, "recipe list")?;
            let rewrite_kind = options
                .get("--rewrite-kind")
                .and_then(Clone::clone)
                .map(|value| {
                    serde_json::from_value::<star_contracts::patch_v2::RewriteAssuranceV2>(
                        serde_json::Value::String(value),
                    )
                    .map_err(|_| {
                        "--rewrite-kind must be text_exact, syntax_aware, symbol_aware, or codegen_assured"
                            .to_owned()
                    })
                })
                .transpose()?;
            Ok(Parsed {
                command: "recipe.list".to_owned(),
                payload: serde_json::json!({
                    "language":options.get("--language").and_then(Clone::clone),
                    "rewrite_kind":rewrite_kind,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "recipe" && second == "describe" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "recipe describe")?;
            if !positionals[0].contains('@')
                || positionals[0].len() > 256
                || positionals[0].contains('\0')
            {
                return Err("recipe describe requires recipe-id@semver".to_owned());
            }
            Ok(Parsed {
                command: "recipe.describe".to_owned(),
                payload: serde_json::json!({"recipe_spec":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "recipe" && second == "validate" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "recipe validate")?;
            let recipe = serde_json::from_value::<star_contracts::patch_v2::ChangeRecipeV2>(
                read_bounded_json_file(&positionals[0])?,
            )
            .map_err(|_| "recipe validate requires a ChangeRecipeV2 JSON file".to_owned())?
            .seal()
            .map_err(|_| "recipe JSON violates ChangeRecipeV2 invariants".to_owned())?;
            Ok(Parsed {
                command: "recipe.validate".to_owned(),
                payload: serde_json::json!({"recipe":recipe}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "change" && second == "prepare" => {
            let (positionals, options) =
                parse_tail(tail, &["--selector", "--parameters", "--workspace"], &[])?;
            require_positionals(&positionals, 3, "change prepare")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone())
                .map_err(|_| "change prepare requires a valid ProjectId".to_owned())?;
            star_contracts::ids::CheckoutId::parse(positionals[1].clone())
                .map_err(|_| "change prepare requires a valid CheckoutId".to_owned())?;
            if !positionals[2].contains('@') || positionals[2].len() > 256 {
                return Err("change prepare requires recipe-id@semver".to_owned());
            }
            let selector = serde_json::from_value::<star_contracts::patch_v2::TargetSelector>(
                read_bounded_json_file(&required_option(&options, "--selector")?)?,
            )
            .map_err(|_| "--selector must contain TargetSelector JSON".to_owned())?;
            selector
                .validate()
                .map_err(|_| "--selector violates TargetSelector invariants".to_owned())?;
            let parameters = read_bounded_json_file(&required_option(&options, "--parameters")?)?;
            if !parameters.is_object() {
                return Err("--parameters must contain a JSON object".to_owned());
            }
            let workspace = options
                .get("--workspace")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "current".to_owned());
            if !matches!(workspace.as_str(), "current" | "isolated") {
                return Err("--workspace must be current or isolated".to_owned());
            }
            Ok(Parsed {
                command: "change.prepare".to_owned(),
                payload: serde_json::json!({
                    "project_id":positionals[0],
                    "checkout_id":positionals[1],
                    "recipe_spec":positionals[2],
                    "target_selector":selector,
                    "parameters":parameters,
                    "worktree_strategy":workspace,
                }),
                json,
            })
        }
        [first, second, tail @ ..] if first == "patch" && second == "show" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "patch show")?;
            star_contracts::ids::PatchSetId::parse(positionals[0].clone())
                .map_err(|_| "patch show requires a valid PatchSetId".to_owned())?;
            Ok(Parsed {
                command: "patch.show".to_owned(),
                payload: serde_json::json!({"patch_set_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "patch" && second == "status" => {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "patch status")?;
            star_contracts::ids::PatchApplicationId::parse(positionals[0].clone())
                .map_err(|_| "patch status requires a valid PatchApplicationId".to_owned())?;
            Ok(Parsed {
                command: "patch.status".to_owned(),
                payload: serde_json::json!({"patch_application_id":positionals[0]}),
                json,
            })
        }
        [first, second, tail @ ..] if first == "patch" && second == "recover" => {
            let (positionals, options) = parse_tail(tail, &["--strategy"], &[])?;
            require_positionals(&positionals, 1, "patch recover")?;
            star_contracts::ids::PatchApplicationId::parse(positionals[0].clone())
                .map_err(|_| "patch recover requires a valid PatchApplicationId".to_owned())?;
            let strategy = match required_option(&options, "--strategy")?.as_str() {
                "reverse-patch" => "reverse_patch",
                "discard-isolated" => "discard_isolated",
                _ => {
                    return Err("--strategy must be reverse-patch or discard-isolated".to_owned());
                }
            };
            Ok(Parsed {
                command: "patch.recover".to_owned(),
                payload: serde_json::json!({
                    "patch_application_id":positionals[0],
                    "strategy":strategy,
                }),
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
            let (positionals, options) = parse_tail(
                tail,
                &[
                    "--approve",
                    "--fingerprint",
                    "--manual-approval",
                    "--guard-evidence",
                ],
                &[],
            )?;
            match positionals.as_slice() {
                [patch_set_id] => {
                    star_contracts::ids::PatchSetId::parse(patch_set_id.clone())
                        .map_err(|_| "patch apply requires a valid PatchSetId".to_owned())?;
                    if options.get("--approve").and_then(Clone::clone).is_some() {
                        return Err("v2 patch apply uses --fingerprint, not --approve".to_owned());
                    }
                    let fingerprint = required_option(&options, "--fingerprint")?;
                    fingerprint
                        .parse::<star_contracts::Sha256Hash>()
                        .map_err(|_| {
                            "--fingerprint must be the exact lowercase patch sha256".to_owned()
                        })?;
                    let validator_guard_evidence = if let Some(path) =
                        options.get("--guard-evidence").and_then(Clone::clone)
                    {
                        let value = read_bounded_json_file(&path)?;
                        let evidence = serde_json::from_value::<
                            star_contracts::validator_guard::ValidatorGuardEvidenceV2,
                        >(value)
                        .map_err(|_| {
                            "--guard-evidence must contain ValidatorGuardEvidenceV2 JSON".to_owned()
                        })?;
                        Some(evidence.seal().map_err(|_| {
                            "--guard-evidence contains invalid validator guard evidence".to_owned()
                        })?)
                    } else {
                        None
                    };
                    Ok(Parsed {
                        command: "patch.apply-v2".to_owned(),
                        payload: serde_json::json!({
                            "patch_set_id":patch_set_id,
                            "approved_patch_fingerprint":fingerprint,
                            "manual_approval_id":options
                                .get("--manual-approval")
                                .and_then(Clone::clone),
                            "validator_guard_evidence":validator_guard_evidence,
                        }),
                        json,
                    })
                }
                [project_id, patch_set_id] => {
                    if options
                        .get("--fingerprint")
                        .and_then(Clone::clone)
                        .is_some()
                        || options
                            .get("--manual-approval")
                            .and_then(Clone::clone)
                            .is_some()
                        || options
                            .get("--guard-evidence")
                            .and_then(Clone::clone)
                            .is_some()
                    {
                        return Err(
                            "legacy patch apply accepts only --approve; use one positional PatchSetId for v2"
                                .to_owned(),
                        );
                    }
                    star_contracts::ids::ProjectId::parse(project_id.clone())
                        .map_err(|_| "patch apply requires a valid ProjectId".to_owned())?;
                    star_contracts::ids::PatchSetId::parse(patch_set_id.clone())
                        .map_err(|_| "patch apply requires a valid PatchSetId".to_owned())?;
                    let approval = required_option(&options, "--approve")?;
                    approval
                        .parse::<star_contracts::Sha256Hash>()
                        .map_err(|_| {
                            "--approve must be the exact lowercase patch sha256".to_owned()
                        })?;
                    Ok(Parsed {
                        command: "patch.apply".to_owned(),
                        payload: serde_json::json!({
                            "project_id":project_id,
                            "patch_set_id":patch_set_id,
                            "approved_patch_fingerprint":approval,
                        }),
                        json,
                    })
                }
                _ => Err(
                    "patch apply expects either <patch-set-id> or <project-id> <patch-set-id>"
                        .to_owned(),
                ),
            }
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
            if first == "management"
                && matches!(second.as_str(), "backup" | "restore")
                && third == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, &format!("management {second} plan"))?;
            Ok(Parsed {
                command: format!("management.{second}.plan"),
                payload: serde_json::json!({"backup_root":absolute_path(&positionals[0])?}),
                json,
            })
        }
        [first, second, third, tail @ ..]
            if first == "management"
                && matches!(second.as_str(), "backup" | "restore")
                && third == "apply" =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(&positionals, 2, &format!("management {second} apply"))?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact recovery plan sha256".to_owned())?;
            Ok(Parsed {
                command: format!("management.{second}.apply"),
                payload: serde_json::json!({
                    "backup_root":absolute_path(&positionals[0])?,
                    "plan":read_bounded_json_file(&positionals[1])?,
                    "approved_plan_fingerprint":approval,
                }),
                json,
            })
        }
        [first, second, direction, action, tail @ ..]
            if first == "management"
                && second == "local-state"
                && matches!(direction.as_str(), "export" | "import")
                && action == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            let expected = if direction == "export" { 2 } else { 1 };
            require_positionals(
                &positionals,
                expected,
                &format!("management local-state {direction} plan"),
            )?;
            let payload = if direction == "export" {
                star_contracts::ids::ProjectId::parse(positionals[0].clone()).map_err(|_| {
                    "management local-state export requires a valid ProjectId".to_owned()
                })?;
                serde_json::json!({
                    "project_id":positionals[0],
                    "destination":absolute_path(&positionals[1])?,
                })
            } else {
                serde_json::json!({"source":absolute_path(&positionals[0])?})
            };
            Ok(Parsed {
                command: format!("management.local-state.{direction}.plan"),
                payload,
                json,
            })
        }
        [first, second, direction, action, tail @ ..]
            if first == "management"
                && second == "local-state"
                && matches!(direction.as_str(), "export" | "import")
                && action == "apply" =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(
                &positionals,
                2,
                &format!("management local-state {direction} apply"),
            )?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact local-state plan sha256".to_owned())?;
            let path_key = if direction == "export" {
                "destination"
            } else {
                "source"
            };
            Ok(Parsed {
                command: format!("management.local-state.{direction}.apply"),
                payload: serde_json::json!({
                    (path_key):absolute_path(&positionals[0])?,
                    "plan":read_bounded_json_file(&positionals[1])?,
                    "approved_plan_fingerprint":approval,
                }),
                json,
            })
        }
        [first, second, third, fourth, tail @ ..]
            if first == "management"
                && second == "migrate"
                && third == "project-v1-v2"
                && fourth == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 0, "management migrate project-v1-v2 plan")?;
            Ok(Parsed {
                command: "management.migrate.project-v1-v2.plan".to_owned(),
                payload: serde_json::json!({}),
                json,
            })
        }
        [first, second, third, fourth, tail @ ..]
            if first == "management"
                && second == "migrate"
                && third == "project-v1-v2"
                && matches!(fourth.as_str(), "apply" | "rollback") =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(
                &positionals,
                1,
                "management migrate project-v1-v2 apply|rollback",
            )?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact migration or backup sha256".to_owned())?;
            let plan = read_bounded_json_file(&positionals[0])?;
            let (command, approval_key) = if fourth == "apply" {
                (
                    "management.migrate.project-v1-v2.apply",
                    "approved_plan_fingerprint",
                )
            } else {
                (
                    "management.migrate.project-v1-v2.rollback",
                    "approved_backup_fingerprint",
                )
            };
            Ok(Parsed {
                command: command.to_owned(),
                payload: serde_json::json!({
                    "plan":plan,
                    (approval_key):approval,
                }),
                json,
            })
        }
        [first, second, third, fourth, tail @ ..]
            if first == "management"
                && second == "migrate"
                && third == "patch-v1-v2"
                && fourth == "plan" =>
        {
            let (positionals, _) = parse_tail(tail, &[], &[])?;
            require_positionals(&positionals, 1, "management migrate patch-v1-v2 plan")?;
            star_contracts::ids::ProjectId::parse(positionals[0].clone()).map_err(|_| {
                "management migrate patch-v1-v2 plan requires a valid ProjectId".to_owned()
            })?;
            Ok(Parsed {
                command: "management.migrate.patch-v1-v2.plan".to_owned(),
                payload: serde_json::json!({"project_id":positionals[0]}),
                json,
            })
        }
        [first, second, third, fourth, tail @ ..]
            if first == "management"
                && second == "migrate"
                && third == "patch-v1-v2"
                && matches!(fourth.as_str(), "apply" | "rollback") =>
        {
            let (positionals, options) = parse_tail(tail, &["--approve"], &[])?;
            require_positionals(
                &positionals,
                1,
                "management migrate patch-v1-v2 apply|rollback",
            )?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| {
                    "--approve must be the exact patch migration plan sha256".to_owned()
                })?;
            let plan =
                serde_json::from_value::<star_contracts::patch_v2::PatchV1ToV2MigrationPlan>(
                    read_bounded_json_file(&positionals[0])?,
                )
                .map_err(|_| "plan JSON must be PatchV1ToV2MigrationPlan".to_owned())?;
            let sealed = plan
                .clone()
                .seal()
                .map_err(|_| "patch migration plan violates invariants".to_owned())?;
            if sealed != plan {
                return Err("patch migration plan is not canonical".to_owned());
            }
            Ok(Parsed {
                command: format!("management.migrate.patch-v1-v2.{fourth}"),
                payload: serde_json::json!({
                    "plan":plan,
                    "approved_plan_fingerprint":approval,
                }),
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
            require_positionals(&positionals, 1, "management rebuild apply")?;
            let approval = required_option(&options, "--approve")?;
            approval
                .parse::<star_contracts::Sha256Hash>()
                .map_err(|_| "--approve must be the exact source rebuild plan sha256".to_owned())?;
            Ok(Parsed {
                command: "management.rebuild.apply".to_owned(),
                payload: serde_json::json!({
                    "plan":read_bounded_json_file(&positionals[0])?,
                    "approved_plan_fingerprint":approval,
                }),
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

fn parse_project_doctor_command(
    tail: &[String],
    clean_room_only: bool,
    json: bool,
) -> Result<Parsed, String> {
    let (positionals, options) = parse_tail(tail, &["--input", "--manifest", "--revision"], &[])?;
    let label = if clean_room_only {
        "clean-room readiness"
    } else {
        "project doctor"
    };
    require_positionals(&positionals, 3, label)?;
    validate_project_id(&positionals[0], label)?;
    validate_development_id(&positionals[1], "report-id")?;
    validate_development_id(&positionals[2], "environment-snapshot-id")?;
    let input = read_bounded_json_file(&required_option(&options, "--input")?)?;
    require_json_object_keys(&input, &["clean_room_specification_id", "registered_tasks"])?;
    if !input
        .get("registered_tasks")
        .is_some_and(serde_json::Value::is_array)
    {
        return Err("doctor input registered_tasks must be an array".to_owned());
    }
    if clean_room_only
        && input
            .get("clean_room_specification_id")
            .is_none_or(serde_json::Value::is_null)
    {
        return Err("clean-room readiness requires clean_room_specification_id".to_owned());
    }
    Ok(Parsed {
        command: if clean_room_only {
            "clean-room.readiness".to_owned()
        } else {
            "project.doctor".to_owned()
        },
        payload: serde_json::json!({
            "project_id":positionals[0],
            "manifest_path":development_manifest_path(&options)?,
            "environment_snapshot_id":positionals[2],
            "report_id":positionals[1],
            "clean_room_specification_id":input.get("clean_room_specification_id"),
            "registered_tasks":input.get("registered_tasks"),
            "revision":development_revision(&options)?,
        }),
        json,
    })
}

fn parse_m7_recovery_plan(
    tail: &[String],
    command: &str,
    label: &str,
    json: bool,
) -> Result<Parsed, String> {
    let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
    require_positionals(&positionals, 2, label)?;
    validate_project_id(&positionals[0], label)?;
    let plan = read_bounded_json_file(&positionals[1])?;
    serde_json::from_value::<star_contracts::maintenance_v2::RecoveryPlanV2>(plan.clone())
        .map_err(|_| "recovery plan JSON has an invalid shape".to_owned())?;
    Ok(Parsed {
        command: command.to_owned(),
        payload: serde_json::json!({
            "project_id":positionals[0],
            "plan":plan,
            "revision":development_revision(&options)?,
        }),
        json,
    })
}

fn parse_m8_project_record<T>(
    tail: &[String],
    command: &str,
    label: &str,
    document_key: &str,
    has_parent_id: bool,
    json: bool,
) -> Result<Parsed, String>
where
    T: serde::de::DeserializeOwned,
{
    let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
    let expected = if has_parent_id { 3 } else { 2 };
    require_positionals(&positionals, expected, label)?;
    validate_project_id(&positionals[0], label)?;
    let document_index = if has_parent_id { 2 } else { 1 };
    if has_parent_id {
        validate_development_id(&positionals[1], "parent-id")?;
    }
    let document = read_bounded_json_file(&positionals[document_index])?;
    serde_json::from_value::<T>(document.clone())
        .map_err(|_| format!("{label} JSON has an invalid shape"))?;
    let mut payload = serde_json::Map::from_iter([
        (
            "project_id".to_owned(),
            serde_json::Value::String(positionals[0].clone()),
        ),
        (document_key.to_owned(), document),
        (
            "record_revision".to_owned(),
            serde_json::Value::Number(development_revision(&options)?.into()),
        ),
    ]);
    if has_parent_id {
        let parent_key = if command == "performance.run" {
            "workload_id"
        } else {
            "plan_id"
        };
        payload.insert(
            parent_key.to_owned(),
            serde_json::Value::String(positionals[1].clone()),
        );
    }
    Ok(Parsed {
        command: command.to_owned(),
        payload: serde_json::Value::Object(payload),
        json,
    })
}

fn parse_m8_project_status(
    tail: &[String],
    command: &str,
    label: &str,
    json: bool,
) -> Result<Parsed, String> {
    let (positionals, _) = parse_tail(tail, &[], &[])?;
    require_positionals(&positionals, 2, label)?;
    validate_project_id(&positionals[0], label)?;
    validate_development_id(&positionals[1], "plan-id")?;
    Ok(Parsed {
        command: command.to_owned(),
        payload: serde_json::json!({
            "project_id":positionals[0],
            "plan_id":positionals[1],
        }),
        json,
    })
}

fn read_m8_string_array(path: &str) -> Result<Vec<String>, String> {
    let values = serde_json::from_value::<Vec<String>>(read_bounded_json_file(path)?)
        .map_err(|_| "run id input must be a JSON string array".to_owned())?;
    if values.is_empty()
        || values.len() > 1_024
        || values
            .iter()
            .any(|value| validate_development_id(value, "run-id").is_err())
    {
        return Err("run id input must contain 1 through 1024 valid ids".to_owned());
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err("run id input must not contain duplicates".to_owned());
    }
    Ok(values)
}

fn read_m9_string_array_allow_empty(path: &str) -> Result<Vec<String>, String> {
    let values = serde_json::from_value::<Vec<String>>(read_bounded_json_file(path)?)
        .map_err(|_| "record id input must be a JSON string array".to_owned())?;
    if values.len() > 1_024
        || values
            .iter()
            .any(|value| validate_development_id(value, "record-id").is_err())
    {
        return Err("record id input must contain at most 1024 valid ids".to_owned());
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err("record id input must not contain duplicates".to_owned());
    }
    Ok(values)
}

fn migration_manifest_path(options: &BTreeMap<String, Option<String>>) -> Result<String, String> {
    let path = options
        .get("--manifest")
        .and_then(Clone::clone)
        .unwrap_or_else(|| ".star-control/migrations.toml".to_owned());
    star_contracts::management::ProjectPathRef::parse(path.clone())
        .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
    Ok(path)
}

fn parse_m9_single_document<T>(
    tail: &[String],
    command: &str,
    label: &str,
    document_key: &str,
    json: bool,
) -> Result<Parsed, String>
where
    T: serde::de::DeserializeOwned,
{
    let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
    require_positionals(&positionals, 1, label)?;
    let document = read_bounded_json_file(&positionals[0])?;
    serde_json::from_value::<T>(document.clone())
        .map_err(|_| format!("{label} JSON has an invalid shape"))?;
    let payload = serde_json::Map::from_iter([
        (document_key.to_owned(), document),
        (
            "record_revision".to_owned(),
            serde_json::Value::Number(development_revision(&options)?.into()),
        ),
    ]);
    Ok(Parsed {
        command: command.to_owned(),
        payload: serde_json::Value::Object(payload),
        json,
    })
}

fn parse_m9_project_document<T>(
    tail: &[String],
    command: &str,
    label: &str,
    document_key: &str,
    json: bool,
) -> Result<Parsed, String>
where
    T: serde::de::DeserializeOwned,
{
    let (positionals, options) = parse_tail(tail, &["--revision"], &[])?;
    require_positionals(&positionals, 2, label)?;
    validate_project_id(&positionals[0], label)?;
    let document = read_bounded_json_file(&positionals[1])?;
    serde_json::from_value::<T>(document.clone())
        .map_err(|_| format!("{label} JSON has an invalid shape"))?;
    let payload = serde_json::Map::from_iter([
        (
            "project_id".to_owned(),
            serde_json::Value::String(positionals[0].clone()),
        ),
        (document_key.to_owned(), document),
        (
            "record_revision".to_owned(),
            serde_json::Value::Number(development_revision(&options)?.into()),
        ),
    ]);
    Ok(Parsed {
        command: command.to_owned(),
        payload: serde_json::Value::Object(payload),
        json,
    })
}

fn required_sha256_option(
    options: &BTreeMap<String, Option<String>>,
    name: &str,
) -> Result<String, String> {
    let value = required_option(options, name)?;
    value
        .parse::<star_contracts::Sha256Hash>()
        .map_err(|_| format!("{name} must be an exact lowercase sha256"))?;
    Ok(value)
}

fn validate_git_oid_cli(value: &str, label: &str) -> Result<(), String> {
    if matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(format!("{label} must be a lowercase Git object id"))
    }
}

fn validate_project_id(value: &str, command: &str) -> Result<(), String> {
    star_contracts::ids::ProjectId::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| format!("{command} requires a valid ProjectId"))
}

fn validate_profile_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Err(
            "profile-id must use 1 through 96 lowercase ASCII letters, digits, or underscores"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

fn validate_development_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
    {
        Err(format!(
            "{label} must use 1 through 256 ASCII identifier characters"
        ))
    } else {
        Ok(())
    }
}

fn development_manifest_path(options: &BTreeMap<String, Option<String>>) -> Result<String, String> {
    let path = options
        .get("--manifest")
        .and_then(Clone::clone)
        .unwrap_or_else(|| ".star-control/contracts.toml".to_owned());
    star_contracts::management::ProjectPathRef::parse(path.clone())
        .map_err(|_| "--manifest must be a safe project-relative path".to_owned())?;
    Ok(path)
}

fn development_revision(options: &BTreeMap<String, Option<String>>) -> Result<u64, String> {
    options
        .get("--revision")
        .and_then(Clone::clone)
        .map(|value| parse_positive_revision(&value))
        .transpose()
        .map(|revision| revision.unwrap_or(1))
}

fn parse_positive_revision(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "--revision must be a positive integer".to_owned())
}

fn require_json_object_keys(value: &serde_json::Value, expected: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "JSON input must be an object".to_owned())?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err("JSON input has missing or unknown fields".to_owned())
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

fn validate_idempotency_key(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().count() > 128 || value.contains('\0') {
        Err("--idempotency must contain 1 through 128 non-NUL characters".to_owned())
    } else {
        Ok(())
    }
}

fn validate_task_spec_id(value: &str) -> Result<(), String> {
    star_contracts::ids::TaskSpecId::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| "planning command requires a valid TaskSpecId".to_owned())
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

fn read_bounded_json_file(value: &str) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(absolute_path(value)?);
    let metadata =
        std::fs::metadata(&path).map_err(|_| "management plan JSON is unavailable".to_owned())?;
    if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err("management plan JSON must be a file no larger than 4 MiB".to_owned());
    }
    let source = std::fs::read_to_string(path)
        .map_err(|_| "management plan JSON is not valid UTF-8".to_owned())?;
    star_contracts::parse_no_duplicate_keys(&source)
        .map_err(|_| "management plan JSON is invalid or has duplicate keys".to_owned())
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
    fn registered_tool_and_effect_receipt_commands_are_cli_complete() {
        let path = std::env::temp_dir().join(format!(
            "star-cli-effect-{}.json",
            star_contracts::ids::RequestId::new().as_str()
        ));
        let subject = star_contracts::Sha256Hash::digest(b"subject");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "exact_subject_fingerprint":subject,
                "plan_id":"plan-one"
            }))
            .unwrap(),
        )
        .unwrap();
        let descriptor = star_contracts::Sha256Hash::digest(b"descriptor");
        let call = parse(&args(&[
            "tools",
            "call",
            "fixture.effect",
            descriptor.as_str(),
            path.to_str().unwrap(),
            "--lane",
            "write_closed",
            "--wait",
            "completed",
        ]))
        .unwrap();
        assert_eq!(call.command, "tool.invoke");
        assert_eq!(call.payload["mcp_risk_lane"], "write_closed");

        let project = star_contracts::ids::ProjectId::new();
        let operation = star_contracts::ids::OperationId::new();
        let record = parse(&args(&[
            "development",
            "effect",
            "record",
            project.as_str(),
            "effect-one",
            "language_cutover",
            "language_migration_plan:plan-one",
            subject.as_str(),
            operation.as_str(),
            "--arguments",
            path.to_str().unwrap(),
            "--approval",
            "approval-one",
            "--permission",
            "permission-one",
            "--gate",
            "gate-one",
        ]))
        .unwrap();
        assert_eq!(record.command, "development.effect.record");
        assert_eq!(record.payload["operation_id"], operation.as_str());

        let approval = star_contracts::ids::ApprovalId::new();
        let resolve = parse(&args(&[
            "approvals",
            "resolve",
            approval.as_str(),
            descriptor.as_str(),
            "approve",
        ]))
        .unwrap();
        assert_eq!(resolve.command, "approval.resolve");
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
        let symbol = star_contracts::ids::SymbolId::new();
        let task_spec = star_contracts::ids::TaskSpecId::new();
        let fingerprint = star_contracts::Sha256Hash::digest(b"patch").to_string();
        let cases = [
            (args(&["doctor"]), "doctor.run"),
            (
                args(&["project", "register", "star-control"]),
                "project.register",
            ),
            (args(&["project", "discover"]), "project.discover"),
            (args(&["project", "list"]), "project.list"),
            (
                args(&["project", "status", "star-control"]),
                "project.status",
            ),
            (
                args(&["planning", "create", ".star-control/task.json"]),
                "planning.create",
            ),
            (
                vec!["planning".into(), "get".into(), task_spec.to_string()],
                "planning.get",
            ),
            (
                vec!["planning".into(), "status".into(), task_spec.to_string()],
                "planning.status",
            ),
            (
                vec!["planning".into(), "history".into(), task_spec.to_string()],
                "planning.history",
            ),
            (
                vec![
                    "planning".into(),
                    "scope".into(),
                    "revise".into(),
                    task_spec.to_string(),
                    ".star-control/task.json".into(),
                    "--reason".into(),
                    "scope-adjusted".into(),
                ],
                "planning.scope.revise",
            ),
            (
                vec![
                    "planning".into(),
                    "impact".into(),
                    "inspect".into(),
                    task_spec.to_string(),
                ],
                "planning.impact.inspect",
            ),
            (
                vec![
                    "planning".into(),
                    "affected-checks".into(),
                    "show".into(),
                    task_spec.to_string(),
                ],
                "planning.affected-checks.show",
            ),
            (
                vec![
                    "planning".into(),
                    "override".into(),
                    task_spec.to_string(),
                    "lint".into(),
                    "--kind".into(),
                    "promote".into(),
                    "--reason".into(),
                    "required".into(),
                ],
                "planning.override",
            ),
            (
                vec![
                    "planning".into(),
                    "waiver".into(),
                    task_spec.to_string(),
                    "lint".into(),
                    "--reason".into(),
                    "approved".into(),
                ],
                "planning.override",
            ),
            (
                vec![
                    "planning".into(),
                    "invalidate".into(),
                    task_spec.to_string(),
                    "--reason".into(),
                    "source-changed".into(),
                ],
                "planning.invalidate",
            ),
            (
                vec![
                    "planning".into(),
                    "replan".into(),
                    task_spec.to_string(),
                    "--reason".into(),
                    "refresh".into(),
                ],
                "planning.replan",
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
                vec!["index".into(), "status".into(), project.to_string()],
                "index.status",
            ),
            (
                vec![
                    "index".into(),
                    "search".into(),
                    project.to_string(),
                    "main".into(),
                    "--tier".into(),
                    "text".into(),
                ],
                "index.search",
            ),
            (
                vec![
                    "index".into(),
                    "definitions".into(),
                    project.to_string(),
                    "main".into(),
                ],
                "index.definitions",
            ),
            (
                vec![
                    "index".into(),
                    "references".into(),
                    project.to_string(),
                    symbol.to_string(),
                ],
                "index.references",
            ),
            (
                vec![
                    "graph".into(),
                    "neighbors".into(),
                    project.to_string(),
                    "source:fixture".into(),
                ],
                "graph.neighbors",
            ),
            (
                vec![
                    "style".into(),
                    "rust".into(),
                    "inspect".into(),
                    project.to_string(),
                ],
                "style.rust.inspect",
            ),
            (
                vec![
                    "style".into(),
                    "rust".into(),
                    "check".into(),
                    project.to_string(),
                ],
                "style.rust.check",
            ),
            (
                vec![
                    "style".into(),
                    "rust".into(),
                    "prepare".into(),
                    project.to_string(),
                    "--scope".into(),
                    "workspace".into(),
                ],
                "style.rust.prepare",
            ),
            (
                vec![
                    "style".into(),
                    "rust".into(),
                    "auto-apply".into(),
                    project.to_string(),
                    "--scope".into(),
                    "package".into(),
                    "--package".into(),
                    "star-application".into(),
                ],
                "style.rust.auto-apply",
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
                vec![
                    "management".into(),
                    "backup".into(),
                    "plan".into(),
                    std::env::temp_dir()
                        .join("star-control-backup")
                        .to_string_lossy()
                        .into_owned(),
                ],
                "management.backup.plan",
            ),
            (
                vec![
                    "management".into(),
                    "restore".into(),
                    "plan".into(),
                    std::env::temp_dir()
                        .join("star-control-backup")
                        .to_string_lossy()
                        .into_owned(),
                ],
                "management.restore.plan",
            ),
            (
                vec![
                    "management".into(),
                    "local-state".into(),
                    "export".into(),
                    "plan".into(),
                    project.to_string(),
                    std::env::temp_dir()
                        .join("star-control-local-state.json")
                        .to_string_lossy()
                        .into_owned(),
                ],
                "management.local-state.export.plan",
            ),
            (
                vec![
                    "management".into(),
                    "local-state".into(),
                    "import".into(),
                    "plan".into(),
                    std::env::temp_dir()
                        .join("star-control-local-state.json")
                        .to_string_lossy()
                        .into_owned(),
                ],
                "management.local-state.import.plan",
            ),
            (
                args(&["management", "migrate", "project-v1-v2", "plan"]),
                "management.migrate.project-v1-v2.plan",
            ),
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
        ];
        for (arguments, command) in cases {
            assert_eq!(parse(&arguments).unwrap().command, command);
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let backup_root = std::env::temp_dir()
            .join("star-control-backup")
            .to_string_lossy()
            .into_owned();
        for (operation, fixture, expected) in [
            (
                "backup",
                "specs/fixtures/management/v1/management-backup-plan/minimal.json",
                "management.backup.apply",
            ),
            (
                "restore",
                "specs/fixtures/management/v1/management-restore-plan/minimal.json",
                "management.restore.apply",
            ),
        ] {
            let parsed = parse(&[
                "management".into(),
                operation.into(),
                "apply".into(),
                backup_root.clone(),
                root.join(fixture).to_string_lossy().into_owned(),
                "--approve".into(),
                star_contracts::Sha256Hash::digest(operation.as_bytes()).to_string(),
            ])
            .unwrap();
            assert_eq!(parsed.command, expected);
            assert_eq!(parsed.payload["backup_root"], backup_root);
            assert!(parsed.payload["plan"].is_object());
        }
        let rebuild = parse(&[
            "management".into(),
            "rebuild".into(),
            "apply".into(),
            root.join("specs/fixtures/management/v1/management-rebuild-plan/minimal.json")
                .to_string_lossy()
                .into_owned(),
            "--approve".into(),
            star_contracts::Sha256Hash::digest(b"rebuild").to_string(),
        ])
        .unwrap();
        assert_eq!(rebuild.command, "management.rebuild.apply");
        assert!(rebuild.payload["plan"].is_object());
        for (direction, fixture, expected, path_key) in [
            (
                "export",
                "specs/fixtures/management/v1/management-local-state-export-plan/minimal.json",
                "management.local-state.export.apply",
                "destination",
            ),
            (
                "import",
                "specs/fixtures/management/v1/management-local-state-import-plan/minimal.json",
                "management.local-state.import.apply",
                "source",
            ),
        ] {
            let document = std::env::temp_dir()
                .join("star-control-local-state.json")
                .to_string_lossy()
                .into_owned();
            let parsed = parse(&[
                "management".into(),
                "local-state".into(),
                direction.into(),
                "apply".into(),
                document.clone(),
                root.join(fixture).to_string_lossy().into_owned(),
                "--approve".into(),
                star_contracts::Sha256Hash::digest(direction.as_bytes()).to_string(),
            ])
            .unwrap();
            assert_eq!(parsed.command, expected);
            assert_eq!(parsed.payload[path_key], document);
            assert!(parsed.payload["plan"].is_object());
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
        assert!(
            parse(&[
                "style".into(),
                "rust".into(),
                "prepare".into(),
                project.to_string(),
            ])
            .unwrap_err()
            .contains("--scope is required")
        );
        assert!(
            parse(&[
                "style".into(),
                "rust".into(),
                "prepare".into(),
                project.to_string(),
                "--scope".into(),
                "package".into(),
            ])
            .unwrap_err()
            .contains("--package is required")
        );
    }

    #[test]
    fn m3_validation_and_evidence_commands_have_exact_typed_payloads() {
        let task = star_contracts::ids::TaskSpecId::new();
        let project = star_contracts::ids::ProjectId::new();
        let diagnostic = star_contracts::ids::DiagnosticId::new();
        let gate = star_contracts::ids::GateId::new();
        let bundle = star_contracts::ids::EvidenceBundleId::new();
        let review = star_contracts::ids::ReviewPackId::new();
        let cases = [
            (
                args(&["validation", "preflight", task.as_str()]),
                "validation.preflight",
                "task_spec_id",
                task.as_str(),
            ),
            (
                args(&["validation", "run-plan", task.as_str()]),
                "validation.run-plan",
                "task_spec_id",
                task.as_str(),
            ),
            (
                args(&["validation", "status", project.as_str()]),
                "validation.status",
                "project_id",
                project.as_str(),
            ),
            (
                args(&["diagnostic", "list", project.as_str()]),
                "diagnostic.list",
                "project_id",
                project.as_str(),
            ),
            (
                args(&["baseline", "inspect", project.as_str()]),
                "baseline.inspect",
                "project_id",
                project.as_str(),
            ),
            (
                args(&["suppression", "inspect", project.as_str()]),
                "suppression.inspect",
                "project_id",
                project.as_str(),
            ),
        ];
        for (arguments, command, key, value) in cases {
            let parsed = parse(&arguments).unwrap();
            assert_eq!(parsed.command, command);
            assert_eq!(parsed.payload[key], value);
        }
        let show = parse(&args(&[
            "diagnostic",
            "show",
            project.as_str(),
            diagnostic.as_str(),
        ]))
        .unwrap();
        assert_eq!(show.command, "diagnostic.show");
        assert_eq!(show.payload["diagnostic_id"], diagnostic.as_str());
        let gate_show = parse(&args(&["gate", "show", project.as_str(), gate.as_str()])).unwrap();
        assert_eq!(gate_show.command, "gate.show");
        let export = parse(&args(&[
            "evidence",
            "bundle",
            "export",
            project.as_str(),
            bundle.as_str(),
        ]))
        .unwrap();
        assert_eq!(export.command, "evidence.bundle.export");
        let review_export = parse(&args(&[
            "review-pack",
            "export",
            project.as_str(),
            review.as_str(),
        ]))
        .unwrap();
        assert_eq!(review_export.command, "review-pack.export");

        assert!(parse(&args(&["gate", "show", project.as_str(), "gate"])).is_err());
        assert!(parse(&args(&["validation", "preflight", "task"])).is_err());
    }

    #[test]
    fn m4_v2_cli_uses_exact_ids_fingerprints_and_recovery_strategy() {
        let project = star_contracts::ids::ProjectId::new();
        let patch_set = star_contracts::ids::PatchSetId::new();
        let application = star_contracts::ids::PatchApplicationId::new();
        let fingerprint = star_contracts::Sha256Hash::digest(b"patch-v2").to_string();

        let list = parse(&args(&["recipe", "list", "--rewrite-kind", "text_exact"])).unwrap();
        assert_eq!(list.command, "recipe.list");
        assert_eq!(list.payload["rewrite_kind"], "text_exact");

        let show = parse(&["patch".to_owned(), "show".to_owned(), patch_set.to_string()]).unwrap();
        assert_eq!(show.command, "patch.show");

        let apply = parse(&[
            "patch".to_owned(),
            "apply".to_owned(),
            patch_set.to_string(),
            "--fingerprint".to_owned(),
            fingerprint.clone(),
            "--manual-approval".to_owned(),
            "approval-1".to_owned(),
        ])
        .unwrap();
        assert_eq!(apply.command, "patch.apply-v2");
        assert_eq!(apply.payload["approved_patch_fingerprint"], fingerprint);

        let status = parse(&[
            "patch".to_owned(),
            "status".to_owned(),
            application.to_string(),
        ])
        .unwrap();
        assert_eq!(status.command, "patch.status");

        let recover = parse(&[
            "patch".to_owned(),
            "recover".to_owned(),
            application.to_string(),
            "--strategy".to_owned(),
            "reverse-patch".to_owned(),
        ])
        .unwrap();
        assert_eq!(recover.command, "patch.recover");
        assert_eq!(recover.payload["strategy"], "reverse_patch");

        let migration = parse(&[
            "management".to_owned(),
            "migrate".to_owned(),
            "patch-v1-v2".to_owned(),
            "plan".to_owned(),
            project.to_string(),
        ])
        .unwrap();
        assert_eq!(migration.command, "management.migrate.patch-v1-v2.plan");

        assert!(
            parse(&[
                "patch".to_owned(),
                "apply".to_owned(),
                patch_set.to_string(),
                "--approve".to_owned(),
                fingerprint,
            ])
            .is_err()
        );
    }

    #[test]
    fn m5_registry_commands_have_strict_typed_payloads() {
        let project = star_contracts::ids::ProjectId::new();
        let declaration_id = "star.error.invalid";
        let list = parse(&["registry".into(), "list".into(), project.to_string()]).unwrap();
        assert_eq!(list.command, "registry.list");
        assert_eq!(
            list.payload["manifest_path"],
            ".star-control/registry/manifest.toml"
        );

        let show = parse(&[
            "registry".into(),
            "show".into(),
            project.to_string(),
            declaration_id.into(),
        ])
        .unwrap();
        assert_eq!(show.command, "registry.show");
        assert_eq!(show.payload["declaration_id"], declaration_id);

        let classify = parse(&[
            "registry".into(),
            "candidate".into(),
            "classify".into(),
            project.to_string(),
            "mrc_candidate".into(),
            "local_implementation_constant".into(),
            "--reason".into(),
            "implementation-only constant".into(),
        ])
        .unwrap();
        assert_eq!(classify.command, "registry.candidate.classify");
        assert_eq!(
            classify.payload["classification"],
            "local_implementation_constant"
        );

        let desired_path = std::env::temp_dir().join(format!(
            "star-registry-desired-{}-{}.json",
            std::process::id(),
            project
        ));
        std::fs::write(
            &desired_path,
            br#"{"kind":"update_description","description":"updated"}"#,
        )
        .unwrap();
        let plan = parse(&[
            "registry".into(),
            "declaration".into(),
            "plan".into(),
            project.to_string(),
            "update_description".into(),
            "--declaration".into(),
            declaration_id.into(),
            "--desired".into(),
            desired_path.to_string_lossy().into_owned(),
            "--reason".into(),
            "clarify public contract".into(),
        ])
        .unwrap();
        assert_eq!(plan.command, "registry.declaration.plan");
        assert_eq!(plan.payload["change_kind"], "update_description");
        assert_eq!(plan.payload["declaration_id"], declaration_id);
        assert_eq!(
            plan.payload["requested_consumer_scope"],
            serde_json::json!([])
        );

        assert!(
            parse(&[
                "registry".into(),
                "show".into(),
                project.to_string(),
                "INVALID ID".into(),
            ])
            .unwrap_err()
            .contains("ManagedDeclarationId")
        );
    }

    #[test]
    fn m6_product_commands_emit_strict_controller_payloads() {
        let project = star_contracts::ids::ProjectId::new();
        let snapshot = parse(&[
            "contract".into(),
            "snapshot".into(),
            project.to_string(),
            "baseline-one".into(),
            "--role".into(),
            "baseline".into(),
            "--source-revision".into(),
            "HEAD~1".into(),
        ])
        .unwrap();
        assert_eq!(snapshot.command, "contract.snapshot");
        assert_eq!(snapshot.payload["role"], "baseline");
        assert_eq!(snapshot.payload["revision"], 1);

        let environment = parse(&[
            "environment".into(),
            "fingerprint".into(),
            project.to_string(),
            "environment-one".into(),
            "--revision".into(),
            "2".into(),
        ])
        .unwrap();
        assert_eq!(environment.command, "environment.fingerprint");
        assert_eq!(environment.payload["revision"], 2);

        let registrations_path = std::env::temp_dir().join(format!(
            "star-doc-registrations-{}-{}.json",
            std::process::id(),
            project
        ));
        std::fs::write(
            &registrations_path,
            br#"{"commands":["star run"],"config_keys":["star.timeout"]}"#,
        )
        .unwrap();
        let docs = parse(&[
            "docs".into(),
            "check".into(),
            project.to_string(),
            "docs-one".into(),
            "--registrations".into(),
            registrations_path.to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert_eq!(docs.command, "docs.check");
        assert_eq!(docs.payload["registered_commands"][0], "star run");

        let show = parse(&[
            "development".into(),
            "record".into(),
            "show".into(),
            "compatibility_report".into(),
            "report-one".into(),
        ])
        .unwrap();
        assert_eq!(show.command, "development.record.show");
        assert!(show.payload["revision"].is_null());

        assert!(
            parse(&[
                "contract".into(),
                "snapshot".into(),
                project.to_string(),
                "bad id".into(),
                "--role".into(),
                "current".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn m7_dependency_and_status_commands_emit_bounded_payloads() {
        let project = star_contracts::ids::ProjectId::new();
        let scan = parse(&[
            "deps".into(),
            "scan".into(),
            project.to_string(),
            "dependencies-one".into(),
        ])
        .unwrap();
        assert_eq!(scan.command, "deps.scan");
        assert_eq!(scan.payload["revision"], 1);

        let status = parse(&[
            "deps".into(),
            "status".into(),
            project.to_string(),
            "update-one".into(),
        ])
        .unwrap();
        assert_eq!(status.command, "deps.status");

        assert!(
            parse(&[
                "deps".into(),
                "scan".into(),
                project.to_string(),
                "dependencies-one".into(),
                "--revision".into(),
                "0".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn m8_read_only_and_effect_commands_emit_separate_strict_payloads() {
        let project = star_contracts::ids::ProjectId::new();
        let inspect = parse(&[
            "migration".into(),
            "inspect".into(),
            project.to_string(),
            "database-primary".into(),
        ])
        .unwrap();
        assert_eq!(inspect.command, "migration.inspect");
        assert_eq!(
            inspect.payload["manifest_path"],
            ".star-control/migrations.toml"
        );

        let status = parse(&[
            "language-migration".into(),
            "status".into(),
            project.to_string(),
            "language-plan-one".into(),
        ])
        .unwrap();
        assert_eq!(status.command, "language-migration.status");
        assert_eq!(status.payload["plan_id"], "language-plan-one");

        assert!(
            parse(&[
                "language-migration".into(),
                "cutover".into(),
                project.to_string(),
                "language-plan-one".into(),
                "equivalence-one".into(),
                "effect-receipt-one".into(),
                "--fingerprint".into(),
                "not-a-sha".into(),
            ])
            .unwrap_err()
            .contains("sha256")
        );
    }

    #[test]
    fn m9_status_and_remote_effect_commands_keep_their_approval_boundary() {
        let status =
            parse(&["change-bundle".into(), "status".into(), "bundle-one".into()]).unwrap();
        assert_eq!(status.command, "change-bundle.status");
        assert_eq!(status.payload["bundle_id"], "bundle-one");

        let fingerprint = format!("sha256:{}", "a".repeat(64));
        let remote = parse(&[
            "change-bundle".into(),
            "remote".into(),
            "operation".into(),
            "apply".into(),
            "remote-operation-one".into(),
            "--request-fingerprint".into(),
            fingerprint.clone(),
        ])
        .unwrap();
        assert_eq!(remote.command, "change-bundle.remote.operation.apply");
        assert_eq!(remote.payload["request_fingerprint"], fingerprint);

        assert!(
            parse(&[
                "change-bundle".into(),
                "remote".into(),
                "operation".into(),
                "apply".into(),
                "remote-operation-one".into(),
                "--request-fingerprint".into(),
                "invalid".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn m10_release_and_evaluation_commands_emit_typed_controller_requests() {
        let release = parse(&[
            "release".into(),
            "status".into(),
            "rel_01KY0000000000000000000000".into(),
        ])
        .unwrap();
        assert_eq!(release.command, "release.status");
        assert_eq!(
            release.payload["release_manifest_id"],
            "rel_01KY0000000000000000000000"
        );

        let transition = parse(&[
            "evaluation".into(),
            "catalog".into(),
            "transition".into(),
            "rust-style@1.0.0".into(),
            "rejected".into(),
            "--trial-candidate".into(),
            "--revision".into(),
            "2".into(),
        ])
        .unwrap();
        assert_eq!(transition.command, "evaluation.catalog.transition");
        assert_eq!(transition.payload["trial_candidate"], true);
        assert_eq!(transition.payload["record_revision"], 2);

        let publish = parse(&[
            "release".into(),
            "publish".into(),
            "prepare".into(),
            "rel_01KY0000000000000000000000".into(),
            "remote-before".into(),
        ])
        .unwrap();
        assert_eq!(publish.command, "release.publish.prepare");
        assert_eq!(publish.payload["before_snapshot_ref"], "remote-before");
    }

    #[test]
    fn profile_commands_emit_strict_controller_requests() {
        let list = parse(&["profile".into(), "list".into()]).unwrap();
        assert_eq!(list.command, "profile.list");
        assert_eq!(list.payload, serde_json::json!({}));

        let show = parse(&[
            "profile".into(),
            "show".into(),
            "rust_style_auto_fix".into(),
        ])
        .unwrap();
        assert_eq!(show.command, "profile.show");
        assert_eq!(show.payload["profile_id"], "rust_style_auto_fix");

        let resolve = parse(&[
            "profile".into(),
            "resolve".into(),
            "security_supply_chain".into(),
            "rust_style_auto_fix".into(),
        ])
        .unwrap();
        assert_eq!(resolve.command, "profile.resolve");
        assert_eq!(resolve.payload["profile_ids"].as_array().unwrap().len(), 2);
        assert!(
            parse(&[
                "profile".into(),
                "resolve".into(),
                "rust_style_auto_fix".into(),
                "rust_style_auto_fix".into(),
            ])
            .is_err()
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
