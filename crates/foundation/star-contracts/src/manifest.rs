use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Sha256Hash, fixed_mcp::RiskLane};

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("manifest TOML is invalid: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("manifest exceeds the 1 MiB v1 source limit")]
    SourceTooLarge,
    #[error("format_version {0} is newer than the supported v1 contract")]
    FutureFormatVersion(u32),
    #[error("format_version must be exactly 1")]
    FormatVersion,
    #[error("package_id must match the contract lexical rule")]
    PackageId,
    #[error("package {0} has no action while enabled")]
    EnabledWithoutAction(String),
    #[error("release-only field is not allowed for this source")]
    SourcePolicy,
    #[error("duplicate {kind}: {value}")]
    Duplicate { kind: &'static str, value: String },
    #[error("unknown permission action: {0}")]
    Permission(String),
    #[error("action {0} has an invalid declared risk lane")]
    Lane(String),
    #[error("executable locator or path violates the v1 policy")]
    Locator,
    #[error("version-compatible executable requires a probe")]
    ProbeRequired,
    #[error("manifest value violates a bounded string, enum, or collection rule")]
    Value,
    #[error("manifest environment declaration is invalid")]
    Environment,
    #[error("manifest probe declaration is invalid")]
    Probe,
    #[error("action binding or exit/output contract is invalid")]
    Binding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestSource {
    Release,
    User,
    Project,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolPackageManifest {
    pub format_version: u32,
    pub package_id: String,
    pub package_version: String,
    pub display_name: String,
    pub description: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(default)]
    pub required: bool,
    pub publisher: Option<String>,
    pub homepage: Option<String>,
    #[serde(default = "license_default")]
    pub license: String,
    pub backend_kinds: Vec<BackendKind>,
    #[serde(default)]
    pub replaces: Vec<ReplacementDescriptor>,
    #[serde(default)]
    pub executables: Vec<ExecutableDescriptor>,
    #[serde(default)]
    pub actions: Vec<ActionDescriptor>,
}
fn enabled_default() -> bool {
    true
}
fn license_default() -> String {
    "NOASSERTION".to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Process,
    ControllerCommand,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    PinnedHash,
    VersionCompatible,
    FollowPath,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocatorKind {
    Absolute,
    AnchorRelative,
    LocationRef,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ManifestProtocol {
    ArgvV1,
    StarJsonStdioV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplacementDescriptor {
    pub package_id: String,
    pub version_req: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutableDescriptor {
    pub executable_id: String,
    pub locator_kind: LocatorKind,
    pub path: Option<String>,
    pub anchor: Option<String>,
    pub location_ref: Option<String>,
    pub update_policy: UpdatePolicy,
    pub sha256: Option<Sha256Hash>,
    pub protocol: ManifestProtocol,
    #[serde(default = "interface_version_default")]
    pub interface_version_req: String,
    pub product_version_req: Option<String>,
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default = "minimum_windows_build_default")]
    pub minimum_windows_build: u32,
    #[serde(default = "working_directory_default")]
    pub working_directory: String,
    pub fixed_working_directory: Option<String>,
    #[serde(default = "environment_mode_default")]
    pub environment_mode: String,
    #[serde(default)]
    pub environment_allow: Vec<String>,
    #[serde(default)]
    pub startup_args: Vec<String>,
    #[serde(default = "timeout_default")]
    pub timeout_ms: u32,
    #[serde(default = "stdout_default")]
    pub max_stdout_bytes: u64,
    #[serde(default = "stderr_default")]
    pub max_stderr_bytes: u64,
    pub max_memory_bytes: Option<u64>,
    #[serde(default = "process_default")]
    pub max_processes: u16,
    #[serde(default = "isolation_compatibility_default")]
    pub isolation_compatibility: Vec<String>,
    #[serde(default = "authenticode_default")]
    pub authenticode_policy: String,
    pub authenticode_subject: Option<String>,
    #[serde(default)]
    pub integrity_files: Vec<IntegrityFile>,
    #[serde(default)]
    pub environment_values: Vec<EnvironmentValue>,
    #[serde(default)]
    pub state_directories: Vec<StateDirectory>,
    pub probe: Option<ProbeDescriptor>,
}
fn working_directory_default() -> String {
    "stage_worktree".to_owned()
}
fn interface_version_default() -> String {
    "*".to_owned()
}
fn minimum_windows_build_default() -> u32 {
    26_100
}
fn isolation_compatibility_default() -> Vec<String> {
    vec!["trusted_desktop".to_owned()]
}
fn environment_mode_default() -> String {
    "core".to_owned()
}
fn timeout_default() -> u32 {
    60_000
}
fn stdout_default() -> u64 {
    8 * 1024 * 1024
}
fn stderr_default() -> u64 {
    1024 * 1024
}
fn process_default() -> u16 {
    16
}
fn authenticode_default() -> String {
    "record".to_owned()
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProbeDescriptor {
    pub kind: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub output_format: Option<String>,
    pub version_pattern: Option<String>,
    #[serde(default = "probe_timeout_default")]
    pub timeout_ms: u32,
}
fn probe_timeout_default() -> u32 {
    5_000
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrityFile {
    pub path: String,
    pub sha256: Sha256Hash,
    #[serde(default = "required_default")]
    pub required: bool,
}

fn required_default() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentValue {
    pub name: String,
    pub value: Option<String>,
    pub secret_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateDirectory {
    pub kind: String,
    pub scope: String,
    pub location: String,
    pub environment_name: Option<String>,
    #[serde(default = "retention_default")]
    pub retention: String,
}

fn retention_default() -> String {
    "policy".to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionDescriptor {
    pub tool_id: String,
    pub backend_kind: BackendKind,
    pub backend_ref: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub task_kinds: Vec<String>,
    #[serde(default)]
    pub when_to_use: Vec<String>,
    #[serde(default)]
    pub when_not_to_use: Vec<String>,
    pub permission_actions: Vec<String>,
    pub paid_action: String,
    pub idempotency: String,
    #[serde(default = "execution_default")]
    pub execution_mode: String,
    #[serde(default = "expected_duration_default")]
    pub expected_duration_ms: u32,
    pub cancel_mode: Option<String>,
    pub input_schema_file: Option<String>,
    pub output_schema_file: Option<String>,
    #[serde(default)]
    pub examples: Vec<ManifestExample>,
    #[serde(default)]
    pub parameters: Vec<ParameterDescriptor>,
    #[serde(default)]
    pub argv: Vec<ArgvBinding>,
    pub exit_codes: Option<ExitCodes>,
    pub output: Option<OutputContract>,
    pub concurrency: Option<ConcurrencyContract>,
    pub cancel: Option<CancelContract>,
}
fn execution_default() -> String {
    "waitable".to_owned()
}
fn expected_duration_default() -> u32 {
    1_000
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManifestExample {
    pub name: String,
    pub arguments: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParameterDescriptor {
    pub name: String,
    #[serde(rename = "type")]
    pub parameter_type: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub enum_values: Vec<serde_json::Value>,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub pattern: Option<String>,
    pub path_kind: Option<String>,
    pub must_exist: Option<bool>,
    pub mutually_exclusive_group: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArgvBinding {
    pub kind: String,
    pub value: Option<String>,
    pub input: Option<String>,
    pub flag: Option<String>,
    pub separator: Option<String>,
    pub when_present: Option<bool>,
    pub when_input: Option<String>,
    pub when_equals: Option<serde_json::Value>,
    #[serde(default)]
    pub inputs: Vec<String>,
    pub encoding: Option<String>,
    pub suffix: Option<String>,
    pub content_kind: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExitCodes {
    #[serde(default = "exit_success_default")]
    pub success: Vec<u32>,
    #[serde(default)]
    pub empty: Vec<u32>,
    #[serde(default)]
    pub warning: Vec<u32>,
    #[serde(default)]
    pub retryable: Vec<u32>,
}
fn exit_success_default() -> Vec<u32> {
    vec![0]
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputContract {
    pub format: String,
    #[serde(default = "output_encoding_default")]
    pub encoding: String,
    #[serde(default = "output_encoding_default")]
    pub stderr_encoding: String,
    #[serde(default = "inline_limit_default")]
    pub inline_limit_bytes: u64,
    pub max_items: Option<u32>,
    #[serde(default = "overflow_default")]
    pub overflow: String,
    #[serde(default = "stdout_role_default")]
    pub stdout_role: String,
    #[serde(default = "stderr_role_default")]
    pub stderr_role: String,
    pub artifact_media_type: Option<String>,
}
fn output_encoding_default() -> String {
    "utf8".to_owned()
}
fn inline_limit_default() -> u64 {
    65_536
}
fn overflow_default() -> String {
    "artifact".to_owned()
}
fn stdout_role_default() -> String {
    "data".to_owned()
}
fn stderr_role_default() -> String {
    "log".to_owned()
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConcurrencyContract {
    #[serde(default = "max_parallel_default")]
    pub max_parallel: u16,
    #[serde(default = "exclusive_scope_default")]
    pub exclusive_scope: String,
    #[serde(default)]
    pub lock_key_inputs: Vec<String>,
    #[serde(default = "queue_timeout_default")]
    pub queue_timeout_ms: u32,
}
fn max_parallel_default() -> u16 {
    1
}
fn exclusive_scope_default() -> String {
    "none".to_owned()
}
fn queue_timeout_default() -> u32 {
    30_000
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelContract {
    #[serde(default = "cancel_grace_default")]
    pub grace_ms: u32,
}
fn cancel_grace_default() -> u32 {
    2_000
}

pub const PERMISSION_ACTIONS: &[&str] = &[
    "network_read",
    "network_download",
    "external_write",
    "account_change",
    "git_push",
    "pull_request",
    "release_publish",
    "paid_action",
    "local_delete",
    "local_mass_move",
    "system_change",
    "git_merge",
    "local_write",
    "dependency_change",
    "plan_execute",
    "git_commit",
    "process_run",
    "local_read",
    "secret_access",
];

fn is_global_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && regex::Regex::new(r"^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){1,7}$")
            .expect("static global id regex")
            .is_match(value)
}

fn is_local_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && regex::Regex::new(r"^[a-z][a-z0-9_-]{0,63}$")
            .expect("static local id regex")
            .is_match(value)
}

fn bounded(value: &str, minimum: usize, maximum: usize) -> bool {
    !value.contains('\0') && (minimum..=maximum).contains(&value.chars().count())
}

fn has_duplicate(values: impl IntoIterator<Item = String>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().any(|value| !seen.insert(value))
}

pub fn parse_manifest_v1(
    input: &str,
    source: ManifestSource,
) -> Result<ToolPackageManifest, ManifestError> {
    if input.len() > 1024 * 1024 {
        return Err(ManifestError::SourceTooLarge);
    }
    if input.starts_with('\u{feff}') || input.contains('\0') {
        return Err(ManifestError::Value);
    }
    #[derive(Deserialize)]
    struct Header {
        format_version: u32,
    }
    let header: Header = toml::from_str(input)?;
    if header.format_version > 1 {
        return Err(ManifestError::FutureFormatVersion(header.format_version));
    }
    let manifest: ToolPackageManifest = toml::from_str(input)?;
    validate_manifest_v1(&manifest, source)?;
    Ok(manifest)
}

pub fn validate_manifest_v1(
    manifest: &ToolPackageManifest,
    source: ManifestSource,
) -> Result<(), ManifestError> {
    if manifest.format_version != 1 {
        return Err(ManifestError::FormatVersion);
    }
    semver::Version::parse(&manifest.package_version).map_err(|_| ManifestError::FormatVersion)?;
    if !is_global_id(&manifest.package_id) {
        return Err(ManifestError::PackageId);
    }
    if !bounded(&manifest.display_name, 1, 80)
        || !bounded(&manifest.description, 1, 2_000)
        || manifest
            .publisher
            .as_ref()
            .is_some_and(|value| !bounded(value, 1, 200))
        || manifest.license.is_empty()
        || manifest.homepage.as_ref().is_some_and(|value| {
            !value.starts_with("https://") || value.contains('#') || value.contains('\0')
        })
        || manifest.backend_kinds.is_empty()
        || manifest.backend_kinds.len() > 2
        || manifest.replaces.len() > 16
        || manifest.executables.len() > 16
        || manifest.actions.len() > 64
    {
        return Err(ManifestError::Value);
    }
    if has_duplicate(
        manifest
            .backend_kinds
            .iter()
            .map(|kind| format!("{kind:?}")),
    ) {
        return Err(ManifestError::Duplicate {
            kind: "backend_kind",
            value: manifest.package_id.clone(),
        });
    }
    if manifest.required && source != ManifestSource::Release {
        return Err(ManifestError::SourcePolicy);
    }
    if manifest.enabled && manifest.actions.is_empty() {
        return Err(ManifestError::EnabledWithoutAction(
            manifest.package_id.clone(),
        ));
    }
    for replacement in &manifest.replaces {
        if !is_global_id(&replacement.package_id)
            || replacement.package_id == manifest.package_id
            || semver::VersionReq::parse(&replacement.version_req).is_err()
        {
            return Err(ManifestError::Value);
        }
    }
    let declares_process = manifest.backend_kinds.contains(&BackendKind::Process);
    let declares_controller = manifest
        .backend_kinds
        .contains(&BackendKind::ControllerCommand);
    if declares_process == manifest.executables.is_empty() {
        return Err(ManifestError::Binding);
    }
    let mut executable_ids = BTreeMap::new();
    for executable in &manifest.executables {
        if !is_local_id(&executable.executable_id) {
            return Err(ManifestError::Value);
        }
        if executable_ids
            .insert(executable.executable_id.clone(), ())
            .is_some()
        {
            return Err(ManifestError::Duplicate {
                kind: "executable_id",
                value: executable.executable_id.clone(),
            });
        }
        if matches!(executable.update_policy, UpdatePolicy::PinnedHash)
            && executable.sha256.is_none()
        {
            return Err(ManifestError::SourcePolicy);
        }
        if source == ManifestSource::Project
            && !matches!(executable.update_policy, UpdatePolicy::PinnedHash)
        {
            return Err(ManifestError::SourcePolicy);
        }
        if source == ManifestSource::Release
            && matches!(executable.update_policy, UpdatePolicy::FollowPath)
        {
            return Err(ManifestError::SourcePolicy);
        }
        match executable.locator_kind {
            LocatorKind::Absolute => {
                let path = executable.path.as_deref().ok_or(ManifestError::Locator)?;
                if executable.anchor.is_some()
                    || executable.location_ref.is_some()
                    || !is_safe_absolute_exe(path)
                {
                    return Err(ManifestError::Locator);
                }
            }
            LocatorKind::AnchorRelative => {
                if executable
                    .anchor
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .is_none()
                    || executable
                        .path
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .is_none()
                    || executable.location_ref.is_some()
                    || !matches!(
                        executable.anchor.as_deref(),
                        Some("program_files" | "local_app_data" | "user_tools" | "package_dir")
                    )
                    || !is_safe_relative_path(executable.path.as_deref().unwrap_or_default())
                    || is_forbidden_executable_name(
                        executable
                            .path
                            .as_deref()
                            .unwrap_or_default()
                            .replace('/', "\\")
                            .rsplit('\\')
                            .next()
                            .unwrap_or_default(),
                    )
                {
                    return Err(ManifestError::Locator);
                }
            }
            LocatorKind::LocationRef => {
                if executable
                    .location_ref
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .is_none()
                    || executable.path.is_some()
                    || executable.anchor.is_some()
                    || !is_local_id(executable.location_ref.as_deref().unwrap_or_default())
                    || source == ManifestSource::Project
                {
                    return Err(ManifestError::Locator);
                }
            }
        }
        semver::VersionReq::parse(&executable.interface_version_req)
            .map_err(|_| ManifestError::Probe)?;
        let product_req = executable.product_version_req.as_deref().unwrap_or("*");
        semver::VersionReq::parse(product_req).map_err(|_| ManifestError::Probe)?;
        let requires_probe = matches!(executable.update_policy, UpdatePolicy::VersionCompatible)
            || executable.interface_version_req != "*"
            || product_req != "*";
        if requires_probe && executable.probe.is_none() {
            return Err(ManifestError::ProbeRequired);
        }
        if matches!(executable.update_policy, UpdatePolicy::VersionCompatible)
            && (executable.authenticode_policy != "require_subject"
                || executable
                    .authenticode_subject
                    .as_ref()
                    .is_none_or(|subject| subject.trim().is_empty()))
        {
            return Err(ManifestError::ProbeRequired);
        }
        validate_executable(executable, source)?;
    }
    let mut tool_ids = BTreeMap::new();
    let mut referenced_executables = BTreeSet::new();
    let mut has_process_action = false;
    let mut has_controller_action = false;
    for action in &manifest.actions {
        if !is_global_id(&action.tool_id)
            || !bounded(&action.display_name, 1, 80)
            || !bounded(&action.summary, 1, 240)
            || !bounded(&action.description, 1, 4_000)
            || action.aliases.len() > 16
            || action.tags.len() > 32
            || action.task_kinds.len() > 16
            || action.when_to_use.len() > 8
            || action.when_not_to_use.len() > 8
            || action.examples.len() > 3
            || action.parameters.len() > 128
            || action.argv.len() > 256
        {
            return Err(ManifestError::Value);
        }
        if tool_ids.insert(action.tool_id.clone(), ()).is_some() {
            return Err(ManifestError::Duplicate {
                kind: "tool_id",
                value: action.tool_id.clone(),
            });
        }
        if has_duplicate(action.aliases.iter().cloned())
            || has_duplicate(action.tags.iter().cloned())
            || has_duplicate(action.task_kinds.iter().cloned())
            || action.aliases.iter().any(|value| !bounded(value, 1, 80))
            || action.tags.iter().any(|value| !is_tag(value))
            || action.task_kinds.iter().any(|value| !is_tag(value))
            || action
                .when_to_use
                .iter()
                .chain(&action.when_not_to_use)
                .any(|value| !bounded(value, 1, 240))
        {
            return Err(ManifestError::Value);
        }
        if has_duplicate(action.permission_actions.iter().cloned()) {
            return Err(ManifestError::Duplicate {
                kind: "permission_action",
                value: action.tool_id.clone(),
            });
        }
        for permission in &action.permission_actions {
            if !PERMISSION_ACTIONS.contains(&permission.as_str()) {
                return Err(ManifestError::Permission(permission.clone()));
            }
        }
        let declares_paid_permission = action
            .permission_actions
            .iter()
            .any(|permission| permission == "paid_action");
        match action.paid_action.as_str() {
            "yes" | "unknown" if !declares_paid_permission => {
                return Err(ManifestError::Permission("paid_action".to_owned()));
            }
            "no" if declares_paid_permission => {
                return Err(ManifestError::Permission("paid_action".to_owned()));
            }
            "yes" | "no" | "unknown" => {}
            other => return Err(ManifestError::Permission(other.to_owned())),
        }
        if !matches!(
            action.idempotency.as_str(),
            "read_only" | "idempotent" | "non_idempotent"
        ) {
            return Err(ManifestError::Binding);
        }
        let lane = risk_lane(&action.permission_actions)?;
        if action.idempotency == "read_only"
            && !matches!(lane, RiskLane::ReadClosed | RiskLane::ReadOpen)
        {
            return Err(ManifestError::Lane(action.tool_id.clone()));
        }
        match action.backend_kind {
            BackendKind::ControllerCommand => {
                has_controller_action = true;
                if source != ManifestSource::Release
                    || manifest.package_id != "star.control.core"
                    || !CORE_CONTROLLER_COMMANDS.contains(&action.backend_ref.as_str())
                    || action.cancel_mode.is_some()
                    || action.cancel.is_some()
                {
                    return Err(ManifestError::SourcePolicy);
                }
            }
            BackendKind::Process => {
                has_process_action = true;
                referenced_executables.insert(action.backend_ref.clone());
            }
        }
        if action.expected_duration_ms > 86_400_000
            || !matches!(action.execution_mode.as_str(), "waitable" | "detachable")
            || (action.input_schema_file.is_some() && !action.parameters.is_empty())
            || action
                .input_schema_file
                .as_ref()
                .is_some_and(|path| !is_safe_schema_path(path))
            || action
                .output_schema_file
                .as_ref()
                .is_some_and(|path| !is_safe_schema_path(path))
        {
            return Err(ManifestError::Binding);
        }
        if action.backend_kind == BackendKind::Process {
            if !action
                .permission_actions
                .iter()
                .any(|permission| permission == "process_run")
                || action.output.is_none()
            {
                return Err(ManifestError::Binding);
            }
            let executable = manifest
                .executables
                .iter()
                .find(|executable| executable.executable_id == action.backend_ref)
                .ok_or(ManifestError::Binding)?;
            let json_output = action
                .output
                .as_ref()
                .is_some_and(|output| matches!(output.format.as_str(), "json" | "jsonl"));
            if (json_output || executable.protocol == ManifestProtocol::StarJsonStdioV1)
                && action.output_schema_file.is_none()
            {
                return Err(ManifestError::Binding);
            }
            match executable.protocol {
                ManifestProtocol::ArgvV1
                    if action.exit_codes.is_none()
                        || !matches!(
                            action.cancel_mode.as_deref().unwrap_or("terminate_job"),
                            "terminate_job" | "none"
                        ) =>
                {
                    return Err(ManifestError::Binding);
                }
                ManifestProtocol::StarJsonStdioV1
                    if !action.argv.is_empty()
                        || action.exit_codes.is_some()
                        || !matches!(
                            action.cancel_mode.as_deref().unwrap_or("stdin_frame"),
                            "stdin_frame" | "terminate_job" | "none"
                        ) =>
                {
                    return Err(ManifestError::Binding);
                }
                _ => {}
            }
        }
        validate_action_details(action)?;
        let parameter_names: BTreeMap<_, _> = action
            .parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), ()))
            .collect();
        if parameter_names.len() != action.parameters.len() {
            return Err(ManifestError::Duplicate {
                kind: "parameter",
                value: action.tool_id.clone(),
            });
        }
        for parameter in &action.parameters {
            validate_parameter(parameter, &parameter_names)?;
        }
        let mut stdin_bindings = 0;
        let schema_backed = action.input_schema_file.is_some();
        let mut terminator_seen = false;
        for binding in &action.argv {
            if matches!(binding.kind.as_str(), "stdin_text" | "stdin_json") {
                stdin_bindings += 1;
            }
            if let Some(input) = &binding.input {
                if (!schema_backed && !parameter_names.contains_key(input.as_str()))
                    || (schema_backed && !is_parameter_name(input))
                {
                    return Err(ManifestError::Binding);
                }
            }
            if let Some(input) = &binding.when_input {
                if (!schema_backed && !parameter_names.contains_key(input.as_str()))
                    || (schema_backed && !is_parameter_name(input))
                {
                    return Err(ManifestError::Binding);
                }
            }
            if binding.inputs.iter().any(|input| {
                (!schema_backed && !parameter_names.contains_key(input.as_str()))
                    || (schema_backed && !is_parameter_name(input))
            }) {
                return Err(ManifestError::Binding);
            }
            if schema_backed {
                validate_schema_backed_argv_binding(binding)?;
            } else {
                validate_argv_binding(binding, &action.parameters)?;
                if matches!(binding.kind.as_str(), "positional" | "repeat")
                    && binding.input.as_ref().is_some_and(|name| {
                        action.parameters.iter().any(|parameter| {
                            parameter.name == *name
                                && matches!(
                                    parameter.parameter_type.as_str(),
                                    "project_path" | "project_path_array"
                                )
                        })
                    })
                    && !terminator_seen
                {
                    return Err(ManifestError::Binding);
                }
            }
            terminator_seen |= binding.kind == "terminator";
        }
        if stdin_bindings > 1 {
            return Err(ManifestError::Binding);
        }
        if let Some(exit_codes) = &action.exit_codes {
            let mut seen = BTreeMap::new();
            for code in exit_codes
                .success
                .iter()
                .chain(&exit_codes.empty)
                .chain(&exit_codes.warning)
                .chain(&exit_codes.retryable)
            {
                if seen.insert(*code, ()).is_some() {
                    return Err(ManifestError::Binding);
                }
            }
        }
    }
    const APPCONTAINER_FORBIDDEN_PERMISSIONS: &[&str] = &[
        "network_read",
        "network_download",
        "external_write",
        "account_change",
        "git_push",
        "pull_request",
        "release_publish",
        "paid_action",
        "system_change",
    ];
    for executable in manifest.executables.iter().filter(|executable| {
        executable
            .isolation_compatibility
            .iter()
            .any(|profile| profile == "appcontainer_adapter")
    }) {
        if executable.protocol != ManifestProtocol::StarJsonStdioV1
            || executable.working_directory != "artifact_root"
            || executable
                .state_directories
                .iter()
                .any(|state| state.location == "tool_default")
            || manifest.actions.iter().any(|action| {
                action.backend_kind == BackendKind::Process
                    && action.backend_ref == executable.executable_id
                    && (action.paid_action != "no"
                        || action.permission_actions.iter().any(|permission| {
                            APPCONTAINER_FORBIDDEN_PERMISSIONS.contains(&permission.as_str())
                        })
                        || action.parameters.iter().any(|parameter| {
                            matches!(
                                parameter.parameter_type.as_str(),
                                "project_path" | "project_path_array"
                            ) && parameter.must_exist == Some(false)
                        }))
            })
        {
            return Err(ManifestError::SourcePolicy);
        }
    }
    let disabled_zero_action_draft = !manifest.enabled && manifest.actions.is_empty();
    if !disabled_zero_action_draft {
        if declares_process != has_process_action || declares_controller != has_controller_action {
            return Err(ManifestError::Binding);
        }
        if manifest
            .executables
            .iter()
            .any(|executable| !referenced_executables.contains(&executable.executable_id))
        {
            return Err(ManifestError::Binding);
        }
    }
    Ok(())
}

fn is_safe_absolute_exe(path: &str) -> bool {
    let normalized = path.replace('/', "\\");
    let lower = normalized.to_ascii_lowercase();
    normalized.len() >= 4
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
        && normalized.as_bytes()[2] == b'\\'
        && !lower.starts_with("\\\\")
        && !lower.starts_with("\\\\?\\")
        && !normalized[3..].contains(':')
        && normalized[3..].split('\\').all(is_safe_windows_component)
        && lower.ends_with(".exe")
        && !is_forbidden_executable_name(normalized.rsplit('\\').next().unwrap_or_default())
}

/// Command interpreters and Windows script hosts are never valid external tool
/// images.  Keeping this check in the contracts crate lets the manifest parser,
/// Registry final-path validation, and the launch-time lease enforce the same
/// frozen no-shell boundary.
pub fn is_forbidden_executable_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "wscript.exe"
            | "cscript.exe"
            | "mshta.exe"
            | "sh.exe"
            | "bash.exe"
    )
}

fn is_safe_windows_component(component: &str) -> bool {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.ends_with([' ', '.'])
        || component.chars().any(|character| {
            character < ' ' || matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
        })
    {
        return false;
    }
    let device = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    !matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !(device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
}

const CORE_CONTROLLER_COMMANDS: &[&str] = &[
    "goal.start",
    "goal.answer",
    "plan.get",
    "plan.update",
    "run.continue",
    "goal.status",
    "goal.pause",
    "goal.resume",
    "goal.cancel",
    "evidence.get",
    "merge.status",
    "handoff.get",
    "doctor.run",
];

fn is_tag(value: &str) -> bool {
    let pattern = regex::Regex::new(r"^[a-z][a-z0-9_-]{0,31}$").expect("static regex");
    pattern.is_match(value)
}

fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') || path.contains(':') {
        return false;
    }
    let normalized = path.replace('\\', "/");
    !normalized.starts_with('/') && normalized.split('/').all(is_safe_windows_component)
}

fn is_safe_schema_path(path: &str) -> bool {
    is_safe_relative_path(path) && path.to_ascii_lowercase().ends_with(".json")
}

fn is_environment_name(name: &str) -> bool {
    let pattern = regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{0,127}$").expect("static regex");
    pattern.is_match(name)
}

fn is_reserved_environment(name: &str) -> bool {
    [
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERNAME",
        "USERDOMAIN",
        "PATH",
        "PATHEXT",
        "COMSPEC",
        "PSMODULEPATH",
        "PROMPT",
    ]
    .contains(&name.to_ascii_uppercase().as_str())
}

fn is_safe_absolute_directory(path: &str) -> bool {
    let normalized = path.replace('/', "\\");
    normalized.len() >= 3
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
        && normalized.as_bytes()[2] == b'\\'
        && !normalized[3..].contains(':')
        && !normalized.starts_with("\\\\")
        && !normalized.starts_with("\\\\?\\")
        && normalized[3..]
            .split('\\')
            .filter(|component| !component.is_empty())
            .all(is_safe_windows_component)
}

fn validate_executable(
    executable: &ExecutableDescriptor,
    source: ManifestSource,
) -> Result<(), ManifestError> {
    if !matches!(
        executable.working_directory.as_str(),
        "project_root" | "stage_worktree" | "artifact_root" | "fixed"
    ) || executable.environment_mode != "core"
        || !(100..=86_400_000).contains(&executable.timeout_ms)
        || executable.max_stdout_bytes > 64 * 1024 * 1024
        || executable.max_stderr_bytes > 8 * 1024 * 1024
        || executable.minimum_windows_build == 0
        || executable
            .max_memory_bytes
            .is_some_and(|bytes| bytes < 16 * 1024 * 1024)
        || !(1..=128).contains(&executable.max_processes)
        || executable.environment_allow.len() > 64
        || executable.startup_args.len() > 32
        || executable.integrity_files.len() > 128
        || executable.environment_values.len() > 64
        || executable.state_directories.len() > 16
    {
        return Err(ManifestError::Value);
    }
    match executable.working_directory.as_str() {
        "fixed" => {
            let path = executable
                .fixed_working_directory
                .as_deref()
                .ok_or(ManifestError::Locator)?;
            if source == ManifestSource::Project || !is_safe_absolute_directory(path) {
                return Err(ManifestError::SourcePolicy);
            }
        }
        _ if executable.fixed_working_directory.is_some() => return Err(ManifestError::Locator),
        _ => {}
    }
    if has_duplicate(executable.architectures.iter().cloned())
        || executable
            .architectures
            .iter()
            .any(|arch| !matches!(arch.as_str(), "x86_64" | "aarch64"))
        || has_duplicate(executable.isolation_compatibility.iter().cloned())
        || executable.isolation_compatibility.iter().any(|isolation| {
            !matches!(
                isolation.as_str(),
                "trusted_desktop" | "appcontainer_adapter"
            )
        })
        || executable
            .startup_args
            .iter()
            .any(|value| value.contains('\0'))
    {
        return Err(ManifestError::Value);
    }
    if !matches!(
        executable.authenticode_policy.as_str(),
        "ignore" | "record" | "require_valid" | "require_subject"
    ) || (executable.authenticode_policy == "require_subject")
        != executable
            .authenticode_subject
            .as_ref()
            .is_some_and(|subject| !subject.trim().is_empty())
    {
        return Err(ManifestError::Value);
    }
    if let Some(probe) = &executable.probe {
        validate_probe(probe, executable.protocol)?;
    }
    let mut paths = BTreeSet::new();
    for integrity in &executable.integrity_files {
        if !is_safe_relative_path(&integrity.path)
            || !paths.insert(integrity.path.to_ascii_lowercase())
        {
            return Err(ManifestError::Locator);
        }
    }
    let mut environment_names = BTreeSet::new();
    for name in &executable.environment_allow {
        if !is_environment_name(name)
            || is_reserved_environment(name)
            || !environment_names.insert(name.to_ascii_uppercase())
        {
            return Err(ManifestError::Environment);
        }
    }
    for value in &executable.environment_values {
        if !is_environment_name(&value.name)
            || is_reserved_environment(&value.name)
            || !environment_names.insert(value.name.to_ascii_uppercase())
            || (value.value.is_some() == value.secret_ref.is_some())
            || value
                .value
                .as_ref()
                .is_some_and(|value| value.contains('\0'))
            || value.secret_ref.as_ref().is_some_and(|secret| {
                !(secret.starts_with("env:") || secret.starts_with("windows-credential:"))
                    || secret.len() <= secret.find(':').unwrap_or(0) + 1
            })
        {
            return Err(ManifestError::Environment);
        }
    }
    for state in &executable.state_directories {
        if !matches!(state.kind.as_str(), "config" | "cache" | "data")
            || !matches!(state.scope.as_str(), "operation" | "project" | "user")
            || !matches!(
                state.location.as_str(),
                "controller_temp" | "controller_data" | "tool_default"
            )
            || !matches!(
                state.retention.as_str(),
                "delete_on_success" | "keep_on_failure" | "policy"
            )
        {
            return Err(ManifestError::Environment);
        }
        let requires_name = state.location != "tool_default";
        if requires_name
            != state
                .environment_name
                .as_ref()
                .is_some_and(|name| is_environment_name(name))
        {
            return Err(ManifestError::Environment);
        }
        if let Some(name) = &state.environment_name {
            if is_reserved_environment(name) || !environment_names.insert(name.to_ascii_uppercase())
            {
                return Err(ManifestError::Environment);
            }
        }
    }
    Ok(())
}

fn validate_action_details(action: &ActionDescriptor) -> Result<(), ManifestError> {
    if !bounded(&action.backend_ref, 1, 128)
        || action
            .examples
            .iter()
            .any(|example| !bounded(&example.name, 1, 80) || !example.arguments.is_object())
        || has_duplicate(action.examples.iter().map(|example| example.name.clone()))
    {
        return Err(ManifestError::Binding);
    }

    if let Some(output) = &action.output {
        let binary = output.format == "binary";
        if !matches!(output.format.as_str(), "text" | "json" | "jsonl" | "binary")
            || output.inline_limit_bytes == 0
            || output.inline_limit_bytes > 64 * 1024 * 1024
            || !matches!(output.overflow.as_str(), "artifact" | "error")
            || output.stdout_role != "data"
            || output.stderr_role != "log"
            || (!binary && !matches!(output.encoding.as_str(), "utf8" | "oem" | "utf16le"))
            || (binary && output.encoding != "binary")
            || !matches!(
                output.stderr_encoding.as_str(),
                "utf8" | "oem" | "utf16le" | "binary"
            )
            || (output.format != "jsonl" && output.max_items.is_some())
            || output
                .max_items
                .is_some_and(|value| value == 0 || value > 5_000)
            || output
                .artifact_media_type
                .as_ref()
                .is_some_and(|value| !bounded(value, 1, 255))
        {
            return Err(ManifestError::Binding);
        }
    } else if action.backend_kind == BackendKind::Process {
        return Err(ManifestError::Binding);
    }

    if let Some(concurrency) = &action.concurrency {
        if !(1..=64).contains(&concurrency.max_parallel)
            || !matches!(
                concurrency.exclusive_scope.as_str(),
                "none" | "project" | "worktree" | "custom"
            )
            || concurrency.queue_timeout_ms > 86_400_000
            || concurrency.lock_key_inputs.len() > 16
            || has_duplicate(concurrency.lock_key_inputs.iter().cloned())
            || (concurrency.exclusive_scope == "custom") == concurrency.lock_key_inputs.is_empty()
        {
            return Err(ManifestError::Binding);
        }
    }
    if action
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.grace_ms > 30_000)
    {
        return Err(ManifestError::Binding);
    }
    Ok(())
}

fn is_parameter_name(value: &str) -> bool {
    regex::Regex::new(r"^[a-z][a-z0-9_]{0,63}$")
        .expect("static parameter regex")
        .is_match(value)
}

fn value_matches_parameter(value: &serde_json::Value, parameter_type: &str) -> bool {
    match parameter_type {
        "string" | "decimal_string" | "project_path" | "artifact_ref" | "secret_ref" => {
            value.is_string()
        }
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "enum" => value.is_string() || value.is_number() || value.is_boolean(),
        "string_array" | "project_path_array" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
        "integer_array" => value.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|item| item.as_i64().is_some() || item.as_u64().is_some())
        }),
        _ => false,
    }
}

fn validate_parameter(
    parameter: &ParameterDescriptor,
    parameter_names: &BTreeMap<&str, ()>,
) -> Result<(), ManifestError> {
    let kind = parameter.parameter_type.as_str();
    let is_array = matches!(
        kind,
        "string_array" | "integer_array" | "project_path_array"
    );
    let is_path = matches!(kind, "project_path" | "project_path_array");
    let supports_length = matches!(kind, "string" | "decimal_string") || is_array;
    let supports_number = kind == "integer";
    if !is_parameter_name(&parameter.name)
        || !bounded(&parameter.description, 1, 500)
        || !matches!(
            kind,
            "string"
                | "integer"
                | "decimal_string"
                | "boolean"
                | "enum"
                | "string_array"
                | "integer_array"
                | "project_path"
                | "project_path_array"
                | "artifact_ref"
                | "secret_ref"
        )
        || parameter.enum_values.len() > 128
        || parameter.requires.len() > 16
        || parameter.conflicts_with.len() > 16
        || has_duplicate(parameter.requires.iter().cloned())
        || has_duplicate(parameter.conflicts_with.iter().cloned())
        || parameter
            .requires
            .iter()
            .chain(&parameter.conflicts_with)
            .any(|name| name == &parameter.name || !parameter_names.contains_key(name.as_str()))
        || parameter
            .mutually_exclusive_group
            .as_ref()
            .is_some_and(|group| !is_local_id(group))
        || (!supports_length && (parameter.min_length.is_some() || parameter.max_length.is_some()))
        || (!supports_number && (parameter.minimum.is_some() || parameter.maximum.is_some()))
        || (!matches!(kind, "string" | "decimal_string") && parameter.pattern.is_some())
        || (!is_path && (parameter.path_kind.is_some() || parameter.must_exist.is_some()))
        || (is_path
            && !matches!(
                parameter.path_kind.as_deref(),
                Some("file" | "directory" | "file_or_directory" | "glob")
            ))
        || (kind != "enum" && !parameter.enum_values.is_empty())
        || (kind == "enum" && parameter.enum_values.is_empty())
        || parameter
            .default
            .as_ref()
            .is_some_and(|value| !value_matches_parameter(value, kind))
        || parameter
            .enum_values
            .iter()
            .any(|value| !value_matches_parameter(value, kind))
    {
        return Err(ManifestError::Binding);
    }
    if parameter
        .min_length
        .zip(parameter.max_length)
        .is_some_and(|(min, max)| min > max)
        || parameter
            .minimum
            .zip(parameter.maximum)
            .is_some_and(|(min, max)| min > max)
        || parameter
            .pattern
            .as_ref()
            .is_some_and(|pattern| pattern.len() > 256 || regex::Regex::new(pattern).is_err())
        || (kind == "enum"
            && (has_duplicate(
                parameter
                    .enum_values
                    .iter()
                    .map(serde_json::Value::to_string),
            ) || parameter
                .default
                .as_ref()
                .is_some_and(|value| !parameter.enum_values.contains(value))))
    {
        return Err(ManifestError::Binding);
    }
    Ok(())
}

fn is_safe_flag(value: &str) -> bool {
    (1..=64).contains(&value.chars().count())
        && value.starts_with('-')
        && !value
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '\'' | '\"' | '\0'))
}

fn parameter_by_name<'a>(
    parameters: &'a [ParameterDescriptor],
    name: Option<&str>,
) -> Option<&'a ParameterDescriptor> {
    name.and_then(|name| parameters.iter().find(|parameter| parameter.name == name))
}

fn validate_argv_binding(
    binding: &ArgvBinding,
    parameters: &[ParameterDescriptor],
) -> Result<(), ManifestError> {
    let conditional = match (
        binding.when_present,
        binding.when_input.as_deref(),
        binding.when_equals.as_ref(),
    ) {
        (None, None, None) => true,
        (Some(true), None, None) => true,
        (None, Some(_), Some(value))
            if value.is_string() || value.is_number() || value.is_boolean() =>
        {
            true
        }
        _ => false,
    };
    if !conditional || binding.inputs.len() > 128 || has_duplicate(binding.inputs.iter().cloned()) {
        return Err(ManifestError::Binding);
    }
    let input = parameter_by_name(parameters, binding.input.as_deref());
    let no_condition = binding.when_present.is_none() && binding.when_input.is_none();
    let no_inputs = binding.inputs.is_empty();
    let no_aux =
        binding.encoding.is_none() && binding.suffix.is_none() && binding.content_kind.is_none();
    let plain = binding
        .value
        .as_ref()
        .is_some_and(|value| !value.contains('\0'));
    let flag_ok = binding.flag.as_deref().is_some_and(is_safe_flag);
    let input_kind = input.map(|parameter| parameter.parameter_type.as_str());
    let array_input = matches!(
        input_kind,
        Some("string_array" | "integer_array" | "project_path_array")
    );
    let boolean_input = input_kind == Some("boolean");
    let string_input = matches!(input_kind, Some("string" | "decimal_string"));
    let base_valid = match binding.kind.as_str() {
        "literal" => {
            plain
                && binding.input.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
                && no_condition
        }
        "positional" => {
            input.is_some()
                && binding.value.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
        }
        "option" => {
            input.is_some()
                && flag_ok
                && binding.value.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
        }
        "flag_if_true" | "flag_if_false" => {
            boolean_input
                && flag_ok
                && binding.value.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
                && no_condition
        }
        "repeat" => {
            array_input
                && binding.value.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
                && binding.flag.as_deref().is_none_or(is_safe_flag)
        }
        "joined" => {
            input.is_some()
                && flag_ok
                && binding.value.is_none()
                && matches!(binding.separator.as_deref(), Some("=" | ":"))
                && no_inputs
                && no_aux
        }
        "terminator" => {
            binding.value.as_deref() == Some("--")
                && binding.input.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_inputs
                && no_aux
                && no_condition
        }
        "stdin_text" => {
            string_input
                && binding.value.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_inputs
                && binding.suffix.is_none()
                && binding.content_kind.is_none()
                && binding
                    .encoding
                    .as_deref()
                    .is_none_or(|value| matches!(value, "utf8" | "utf16le"))
                && no_condition
        }
        "stdin_json" => {
            binding.value.is_none()
                && binding.input.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_aux
                && no_condition
                && binding.inputs.iter().all(|name| {
                    parameter_by_name(parameters, Some(name))
                        .is_some_and(|parameter| parameter.parameter_type != "secret_ref")
                })
        }
        "temp_file" => {
            input.is_some()
                && binding.value.is_none()
                && binding.flag.is_none()
                && binding.separator.is_none()
                && no_inputs
                && binding.suffix.as_ref().is_none_or(|suffix| {
                    regex::Regex::new(r"^\.[A-Za-z0-9][A-Za-z0-9._-]{0,15}$")
                        .expect("static suffix regex")
                        .is_match(suffix)
                })
                && matches!(
                    binding.content_kind.as_deref().unwrap_or("text"),
                    "text" | "json" | "base64"
                )
                && binding
                    .encoding
                    .as_deref()
                    .is_none_or(|encoding| matches!(encoding, "utf8" | "utf16le"))
                && !(binding.content_kind.as_deref() == Some("base64")
                    && binding.encoding.is_some())
        }
        _ => false,
    };
    if !base_valid {
        return Err(ManifestError::Binding);
    }
    if let Some(name) = binding.when_input.as_deref() {
        let parameter = parameter_by_name(parameters, Some(name)).ok_or(ManifestError::Binding)?;
        if !value_matches_parameter(
            binding.when_equals.as_ref().expect("validated conditional"),
            &parameter.parameter_type,
        ) {
            return Err(ManifestError::Binding);
        }
    }
    Ok(())
}

fn validate_schema_backed_argv_binding(binding: &ArgvBinding) -> Result<(), ManifestError> {
    let conditional = match (
        binding.when_present,
        binding.when_input.as_deref(),
        binding.when_equals.as_ref(),
    ) {
        (None, None, None) | (Some(true), None, None) => true,
        (None, Some(_), Some(value))
            if value.is_string() || value.is_number() || value.is_boolean() =>
        {
            true
        }
        _ => false,
    };
    let no_condition = binding.when_present.is_none() && binding.when_input.is_none();
    let no_inputs = binding.inputs.is_empty();
    let no_aux =
        binding.encoding.is_none() && binding.suffix.is_none() && binding.content_kind.is_none();
    let plain = binding
        .value
        .as_ref()
        .is_some_and(|value| !value.contains('\0'));
    let flag_ok = binding.flag.as_deref().is_some_and(is_safe_flag);
    let has_input = binding.input.as_deref().is_some_and(is_parameter_name);
    let valid = conditional
        && binding.inputs.len() <= 128
        && !has_duplicate(binding.inputs.iter().cloned())
        && match binding.kind.as_str() {
            "literal" => {
                plain
                    && binding.input.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
                    && no_condition
            }
            "positional" => {
                has_input
                    && binding.value.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
            }
            "option" => {
                has_input
                    && flag_ok
                    && binding.value.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
            }
            "flag_if_true" | "flag_if_false" => {
                has_input
                    && flag_ok
                    && binding.value.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
                    && no_condition
            }
            "repeat" => {
                has_input
                    && binding.value.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
                    && binding.flag.as_deref().is_none_or(is_safe_flag)
            }
            "joined" => {
                has_input
                    && flag_ok
                    && binding.value.is_none()
                    && matches!(binding.separator.as_deref(), Some("=" | ":"))
                    && no_inputs
                    && no_aux
            }
            "terminator" => {
                binding.value.as_deref() == Some("--")
                    && binding.input.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && no_aux
                    && no_condition
            }
            "stdin_text" => {
                has_input
                    && binding.value.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && binding.suffix.is_none()
                    && binding.content_kind.is_none()
                    && binding
                        .encoding
                        .as_deref()
                        .is_none_or(|value| matches!(value, "utf8" | "utf16le"))
                    && no_condition
            }
            "stdin_json" => {
                binding.value.is_none()
                    && binding.input.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_aux
                    && no_condition
            }
            "temp_file" => {
                has_input
                    && binding.value.is_none()
                    && binding.flag.is_none()
                    && binding.separator.is_none()
                    && no_inputs
                    && binding.suffix.as_ref().is_none_or(|suffix| {
                        regex::Regex::new(r"^\.[A-Za-z0-9][A-Za-z0-9._-]{0,15}$")
                            .expect("static suffix regex")
                            .is_match(suffix)
                    })
                    && matches!(
                        binding.content_kind.as_deref().unwrap_or("text"),
                        "text" | "json" | "base64"
                    )
                    && binding
                        .encoding
                        .as_deref()
                        .is_none_or(|encoding| matches!(encoding, "utf8" | "utf16le"))
                    && !(binding.content_kind.as_deref() == Some("base64")
                        && binding.encoding.is_some())
            }
            _ => false,
        };
    valid.then_some(()).ok_or(ManifestError::Binding)
}

fn validate_probe(
    probe: &ProbeDescriptor,
    protocol: ManifestProtocol,
) -> Result<(), ManifestError> {
    if probe.timeout_ms == 0 || probe.timeout_ms > 30_000 {
        return Err(ManifestError::Probe);
    }
    match (probe.kind.as_str(), protocol) {
        ("json_stdio", ManifestProtocol::StarJsonStdioV1) => {
            if !probe.args.is_empty()
                || probe.output_format.is_some()
                || probe.version_pattern.is_some()
            {
                return Err(ManifestError::Probe);
            }
        }
        ("argv", _) => {
            let output_format = probe.output_format.as_deref().ok_or(ManifestError::Probe)?;
            if probe.args.len() > 16 || !matches!(output_format, "json" | "semver_line") {
                return Err(ManifestError::Probe);
            }
            match output_format {
                "semver_line" => {
                    let pattern = probe
                        .version_pattern
                        .as_deref()
                        .ok_or(ManifestError::Probe)?;
                    if pattern.len() > 256
                        || pattern.contains("(?=")
                        || pattern.contains("(?<=")
                        || regex::RegexBuilder::new(pattern)
                            .size_limit(1024 * 1024)
                            .build()
                            .ok()
                            .is_none_or(|compiled| {
                                let names: BTreeSet<_> =
                                    compiled.capture_names().flatten().collect();
                                !names.contains("product")
                                    || names
                                        .iter()
                                        .any(|name| !matches!(*name, "product" | "interface"))
                            })
                    {
                        return Err(ManifestError::Probe);
                    }
                }
                "json" if probe.version_pattern.is_some() => return Err(ManifestError::Probe),
                _ => {}
            }
        }
        _ => return Err(ManifestError::Probe),
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_action_details_legacy_unused(action: &ActionDescriptor) -> Result<(), ManifestError> {
    for example in &action.examples {
        if !bounded(&example.name, 1, 80) || !example.arguments.is_object() {
            return Err(ManifestError::Binding);
        }
    }
    if let Some(output) = &action.output {
        if !matches!(output.format.as_str(), "text" | "json" | "jsonl" | "binary")
            || !matches!(
                output.encoding.as_str(),
                "utf8" | "oem" | "utf16le" | "binary"
            )
            || !matches!(
                output.stderr_encoding.as_str(),
                "utf8" | "oem" | "utf16le" | "binary"
            )
            || !matches!(output.overflow.as_str(), "artifact" | "error")
            || output.stdout_role != "data"
            || output.stderr_role != "log"
            || (output.format == "binary" && output.encoding != "binary")
            || (output.format != "binary" && output.encoding == "binary")
        {
            return Err(ManifestError::Binding);
        }
    }
    if let Some(concurrency) = &action.concurrency {
        if !(1..=64).contains(&concurrency.max_parallel)
            || !matches!(
                concurrency.exclusive_scope.as_str(),
                "none" | "project" | "worktree" | "custom"
            )
            || concurrency.queue_timeout_ms > 86_400_000
            || ((concurrency.exclusive_scope == "custom") == concurrency.lock_key_inputs.is_empty())
        {
            return Err(ManifestError::Binding);
        }
    }
    if action
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.grace_ms > 30_000)
    {
        return Err(ManifestError::Binding);
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_parameter_legacy_unused(
    parameter: &ParameterDescriptor,
    names: &BTreeMap<&str, ()>,
) -> Result<(), ManifestError> {
    let parameter_types = [
        "string",
        "integer",
        "decimal_string",
        "boolean",
        "enum",
        "string_array",
        "integer_array",
        "project_path",
        "project_path_array",
        "artifact_ref",
        "secret_ref",
    ];
    if !is_local_id(&parameter.name)
        || !parameter_types.contains(&parameter.parameter_type.as_str())
        || !bounded(&parameter.description, 1, 500)
        || parameter.enum_values.len() > 128
        || parameter.requires.len() > 16
        || parameter.conflicts_with.len() > 16
        || parameter
            .min_length
            .zip(parameter.max_length)
            .is_some_and(|(min, max)| min > max)
        || parameter
            .minimum
            .zip(parameter.maximum)
            .is_some_and(|(min, max)| min > max)
        || parameter
            .requires
            .iter()
            .chain(&parameter.conflicts_with)
            .any(|name| !names.contains_key(name.as_str()) || name == &parameter.name)
    {
        return Err(ManifestError::Binding);
    }
    let is_path = matches!(
        parameter.parameter_type.as_str(),
        "project_path" | "project_path_array"
    );
    if is_path
        != parameter.path_kind.as_ref().is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "file" | "directory" | "file_or_directory" | "glob"
            )
        })
    {
        return Err(ManifestError::Binding);
    }
    if (parameter.parameter_type == "enum") == parameter.enum_values.is_empty() {
        return Err(ManifestError::Binding);
    }
    if let Some(pattern) = &parameter.pattern {
        if pattern.len() > 256
            || !matches!(
                parameter.parameter_type.as_str(),
                "string" | "decimal_string"
            )
            || regex::RegexBuilder::new(pattern)
                .size_limit(1024 * 1024)
                .build()
                .is_err()
        {
            return Err(ManifestError::Binding);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_argv_binding_legacy_unused(
    binding: &ArgvBinding,
    parameters: &[ParameterDescriptor],
) -> Result<(), ManifestError> {
    let parameter = binding
        .input
        .as_ref()
        .and_then(|name| parameters.iter().find(|parameter| parameter.name == *name));
    let no_extra = |allowed: &[&str]| {
        let fields = [
            ("value", binding.value.is_some()),
            ("input", binding.input.is_some()),
            ("flag", binding.flag.is_some()),
            ("separator", binding.separator.is_some()),
            ("when_present", binding.when_present.is_some()),
            ("when_input", binding.when_input.is_some()),
            ("when_equals", binding.when_equals.is_some()),
            ("inputs", !binding.inputs.is_empty()),
            ("encoding", binding.encoding.is_some()),
            ("suffix", binding.suffix.is_some()),
            ("content_kind", binding.content_kind.is_some()),
        ];
        fields
            .iter()
            .all(|(name, present)| !*present || allowed.contains(name))
    };
    let valid_flag = |value: Option<&String>| {
        value.is_some_and(|value| {
            (1..=64).contains(&value.len())
                && value.starts_with('-')
                && !value.contains([' ', '"', '\'', '\0'])
        })
    };
    let valid = match binding.kind.as_str() {
        "literal" => binding.value.is_some() && no_extra(&["value"]),
        "positional" => {
            parameter.is_some() && no_extra(&["input", "when_present", "when_input", "when_equals"])
        }
        "option" => {
            valid_flag(binding.flag.as_ref())
                && parameter.is_some()
                && no_extra(&["flag", "input", "when_present", "when_input", "when_equals"])
        }
        "flag_if_true" | "flag_if_false" => {
            valid_flag(binding.flag.as_ref())
                && parameter.is_some_and(|parameter| parameter.parameter_type == "boolean")
                && no_extra(&["flag", "input"])
        }
        "repeat" => {
            parameter.is_some_and(|parameter| parameter.parameter_type.ends_with("_array"))
                && binding
                    .flag
                    .as_ref()
                    .is_none_or(|_| valid_flag(binding.flag.as_ref()))
                && no_extra(&["flag", "input"])
        }
        "joined" => {
            valid_flag(binding.flag.as_ref())
                && parameter.is_some()
                && matches!(binding.separator.as_deref(), Some("=" | ":"))
                && no_extra(&[
                    "flag",
                    "input",
                    "separator",
                    "when_present",
                    "when_input",
                    "when_equals",
                ])
        }
        "terminator" => binding.value.as_deref() == Some("--") && no_extra(&["value"]),
        "stdin_text" => {
            parameter.is_some_and(|parameter| parameter.parameter_type == "string")
                && matches!(binding.encoding.as_deref(), None | Some("utf8" | "utf16le"))
                && no_extra(&["input", "encoding"])
        }
        "stdin_json" => no_extra(&["inputs"]),
        "temp_file" => {
            parameter.is_some()
                && binding.suffix.as_ref().is_none_or(|suffix| {
                    regex::Regex::new(r"^\.[A-Za-z0-9][A-Za-z0-9._-]{0,15}$")
                        .expect("static regex")
                        .is_match(suffix)
                })
                && matches!(
                    binding.content_kind.as_deref(),
                    Some("text" | "json" | "base64")
                )
                && no_extra(&["input", "suffix", "encoding", "content_kind"])
        }
        _ => false,
    };
    if !valid
        || (binding.when_present.is_some()
            && (binding.when_input.is_some() || binding.when_equals.is_some()))
        || (binding.when_input.is_some() != binding.when_equals.is_some())
    {
        return Err(ManifestError::Binding);
    }
    Ok(())
}

pub fn risk_lane(actions: &[String]) -> Result<RiskLane, ManifestError> {
    for action in actions {
        if !PERMISSION_ACTIONS.contains(&action.as_str()) {
            return Err(ManifestError::Permission(action.clone()));
        }
    }
    let destructive = [
        "local_delete",
        "local_mass_move",
        "system_change",
        "account_change",
        "git_merge",
        "release_publish",
    ];
    let write = [
        "local_write",
        "dependency_change",
        "external_write",
        "plan_execute",
        "git_commit",
        "git_push",
        "pull_request",
    ];
    let open = [
        "network_read",
        "network_download",
        "external_write",
        "account_change",
        "git_push",
        "pull_request",
        "release_publish",
        "paid_action",
    ];
    let destructive = actions
        .iter()
        .any(|item| destructive.contains(&item.as_str()));
    let write = destructive || actions.iter().any(|item| write.contains(&item.as_str()));
    let open = actions.iter().any(|item| open.contains(&item.as_str()));
    Ok(match (write, destructive, open) {
        (false, false, false) => RiskLane::ReadClosed,
        (false, false, true) => RiskLane::ReadOpen,
        (true, false, false) => RiskLane::WriteClosed,
        (true, true, false) => RiskLane::DestructiveClosed,
        (true, false, true) => RiskLane::WriteOpen,
        (true, true, true) => RiskLane::DestructiveOpen,
        _ => unreachable!("destructive implies write"),
    })
}

pub fn parameter_pattern_matches(pattern: &str, value: &str) -> bool {
    regex::RegexBuilder::new(pattern)
        .size_limit(1024 * 1024)
        .build()
        .is_ok_and(|pattern| pattern.is_match(value))
}

pub fn version_requirement_matches(requirement: &str, version: &str) -> bool {
    semver::VersionReq::parse(requirement)
        .ok()
        .zip(semver::Version::parse(version).ok())
        .is_some_and(|(requirement, version)| requirement.matches(&version))
}
