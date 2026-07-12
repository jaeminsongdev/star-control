//! Fail-closed extraction of the user policy profile used by the Tool Registry.
//!
//! Only the frozen MCP/Registry subset is interpreted here.  Broader
//! Star-Control configuration remains outside this crate, but every accepted
//! key in these sections is type checked and v1 security invariants are
//! enforced instead of being silently ignored.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use star_contracts::{Sha256Hash, canonical::canonical_sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UserPolicyProfile {
    #[default]
    SafeDefault,
    PersonalAuto,
}

#[derive(Debug, Error)]
pub enum PolicyProfileError {
    #[error("user config I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("user config TOML is invalid")]
    InvalidToml,
    #[error("user config schema or policy profile is unsupported")]
    Unsupported,
    #[error("user config contains an unknown top-level key")]
    UnknownTopLevel,
}

const TOP_LEVEL_KEYS: &[&str] = &[
    "schema_version",
    "policy_profile",
    "default_work_profile",
    "required_policy_profile",
    "controller",
    "codex",
    "routing",
    "permissions",
    "budgets",
    "validation",
    "vcs",
    "remote",
    "state",
    "catalog",
    "tool_registry",
    "mcp_gateway",
    "logging",
    "ipc",
];

const TOOL_REGISTRY_KEYS: &[&str] = &[
    "enabled",
    "user_root",
    "locations",
    "project_enabled",
    "project_trust",
    "user_trust",
    "allow_path_lookup",
    "allowed_process_protocols",
    "allowed_isolation_profiles",
    "default_isolation",
    "require_trusted_desktop_code_trust",
    "live_reload",
    "watch_files",
    "demand_scan",
    "reload_debounce_ms",
    "stable_file_window_ms",
    "stable_file_timeout_ms",
    "persist_last_known_good",
    "user_default_update_policy",
    "allow_follow_path_user",
    "project_update_policy",
    "verify_executable_identity_each_call",
    "max_packages",
    "max_tools",
    "max_actions_per_package",
    "max_watch_roots",
    "max_manifest_bytes",
    "max_schema_bytes",
    "max_action_schema_bytes",
    "max_schema_depth",
    "invalid_optional_package",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserToolRegistryConfig {
    pub config_revision: Sha256Hash,
    pub enabled: bool,
    pub user_root: Option<PathBuf>,
    pub locations: BTreeMap<String, PathBuf>,
    pub project_enabled: bool,
    pub watch_files: bool,
    pub allowed_process_protocols: Vec<String>,
    pub allowed_isolation_profiles: Vec<String>,
    pub require_trusted_desktop_code_trust: bool,
    pub reload_debounce_ms: u64,
    pub stable_file_window_ms: u64,
    pub stable_file_timeout_ms: u64,
    pub persist_last_known_good: bool,
    pub allow_follow_path_user: bool,
    pub max_packages: usize,
    pub max_tools: usize,
    pub max_actions_per_package: usize,
    pub max_watch_roots: usize,
    pub max_manifest_bytes: u64,
    pub max_schema_bytes: u64,
    pub max_action_schema_bytes: usize,
    pub max_schema_depth: usize,
}

impl Default for UserToolRegistryConfig {
    fn default() -> Self {
        Self {
            config_revision: Sha256Hash::digest(b"star.user-config.default.v1"),
            enabled: true,
            user_root: None,
            locations: BTreeMap::new(),
            project_enabled: true,
            watch_files: true,
            allowed_process_protocols: vec!["argv_v1".to_owned(), "star_json_stdio_v1".to_owned()],
            allowed_isolation_profiles: vec![
                "appcontainer_adapter".to_owned(),
                "trusted_desktop".to_owned(),
            ],
            require_trusted_desktop_code_trust: true,
            reload_debounce_ms: 250,
            stable_file_window_ms: 250,
            stable_file_timeout_ms: 5_000,
            persist_last_known_good: true,
            allow_follow_path_user: true,
            max_packages: 128,
            max_tools: 512,
            max_actions_per_package: 64,
            max_watch_roots: 128,
            max_manifest_bytes: 1_048_576,
            max_schema_bytes: 4_194_304,
            max_action_schema_bytes: 1_048_576,
            max_schema_depth: 64,
        }
    }
}

fn boolean(table: &toml::Table, key: &str, default: bool) -> Result<bool, PolicyProfileError> {
    table
        .get(key)
        .map(|value| value.as_bool().ok_or(PolicyProfileError::Unsupported))
        .unwrap_or(Ok(default))
}

fn bounded_integer(
    table: &toml::Table,
    key: &str,
    default: u64,
    maximum: u64,
) -> Result<u64, PolicyProfileError> {
    let value = table
        .get(key)
        .map(|value| value.as_integer().ok_or(PolicyProfileError::Unsupported))
        .unwrap_or(Ok(default as i64))?;
    let value = u64::try_from(value).map_err(|_| PolicyProfileError::Unsupported)?;
    (value > 0 && value <= maximum)
        .then_some(value)
        .ok_or(PolicyProfileError::Unsupported)
}

fn exact_integer(table: &toml::Table, key: &str, expected: u64) -> Result<(), PolicyProfileError> {
    if table.get(key).is_some_and(|value| {
        value
            .as_integer()
            .and_then(|value| u64::try_from(value).ok())
            != Some(expected)
    }) {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(())
}

fn exact_string(table: &toml::Table, key: &str, expected: &str) -> Result<(), PolicyProfileError> {
    if table
        .get(key)
        .is_some_and(|value| value.as_str() != Some(expected))
    {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(())
}

fn string_set(
    table: &toml::Table,
    key: &str,
    default: &[&str],
    allowed: &[&str],
) -> Result<Vec<String>, PolicyProfileError> {
    let Some(value) = table.get(key) else {
        return Ok(default.iter().map(|value| (*value).to_owned()).collect());
    };
    let values = value.as_array().ok_or(PolicyProfileError::Unsupported)?;
    if values.is_empty() || values.len() > allowed.len() {
        return Err(PolicyProfileError::Unsupported);
    }
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        let value = value.as_str().ok_or(PolicyProfileError::Unsupported)?;
        if !allowed.contains(&value) || output.iter().any(|current| current == value) {
            return Err(PolicyProfileError::Unsupported);
        }
        output.push(value.to_owned());
    }
    output.sort();
    Ok(output)
}

#[cfg(windows)]
pub fn safe_user_config_path(path: &Path) -> bool {
    use std::{os::windows::fs::MetadataExt, path::Prefix};
    use windows::{
        Win32::{Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::DRIVE_FIXED},
        core::HSTRING,
    };
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    if !path.is_absolute()
        || path
            .as_os_str()
            .to_string_lossy()
            .chars()
            .any(|character| character == '\0')
    {
        return false;
    }
    let drive = match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter),
            _ => None,
        },
        _ => None,
    };
    let Some(drive) = drive else {
        return false;
    };
    let root = HSTRING::from(format!("{}:\\", char::from(drive)));
    if unsafe { GetDriveTypeW(&root) } != DRIVE_FIXED {
        return false;
    }
    let mut current = PathBuf::from(format!("{}:\\", char::from(drive)));
    for component in path.components().skip(2) {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 => {
                return false;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(_) => return false,
        }
    }
    true
}

#[cfg(not(windows))]
pub fn safe_user_config_path(path: &Path) -> bool {
    path.is_absolute()
}

fn validate_fixed_v1_sections(table: &toml::Table) -> Result<(), PolicyProfileError> {
    if let Some(gateway) = table.get("mcp_gateway") {
        let gateway = gateway.as_table().ok_or(PolicyProfileError::Unsupported)?;
        const KEYS: &[&str] = &[
            "contract_version",
            "max_message_bytes",
            "sync_budget_ms",
            "accepted_dispatch_ms",
            "progress_per_second",
        ];
        if gateway.keys().any(|key| !KEYS.contains(&key.as_str()))
            || gateway
                .get("contract_version")
                .is_some_and(|value| value.as_integer() != Some(1))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        // The thin Gateway cannot read TOML by contract, and protocol v1 has
        // no negotiated settings payload. Accepting a different value here
        // would therefore create a dangerous accepted-but-ignored setting.
        exact_integer(gateway, "max_message_bytes", 8_388_608)?;
        exact_integer(gateway, "sync_budget_ms", 30_000)?;
        exact_integer(gateway, "accepted_dispatch_ms", 5_000)?;
        exact_integer(gateway, "progress_per_second", 4)?;
    }
    if let Some(ipc) = table.get("ipc") {
        let ipc = ipc.as_table().ok_or(PolicyProfileError::Unsupported)?;
        const KEYS: &[&str] = &["connect_timeout_ms", "max_frame_bytes", "auth_required"];
        if ipc.keys().any(|key| !KEYS.contains(&key.as_str()))
            || ipc
                .get("auth_required")
                .is_some_and(|value| value.as_bool() != Some(true))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        exact_integer(ipc, "connect_timeout_ms", 5_000)?;
        exact_integer(ipc, "max_frame_bytes", 8_388_608)?;
    }
    Ok(())
}

fn load_table(appdata: &Path) -> Result<Option<toml::Table>, PolicyProfileError> {
    let path = appdata.join("Star-Control").join("config.toml");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
        .map_err(|_| PolicyProfileError::InvalidToml)?;
    let value: toml::Value = toml::from_str(text).map_err(|_| PolicyProfileError::InvalidToml)?;
    let table = value
        .as_table()
        .cloned()
        .ok_or(PolicyProfileError::InvalidToml)?;
    if table
        .keys()
        .any(|key| !TOP_LEVEL_KEYS.contains(&key.as_str()))
    {
        return Err(PolicyProfileError::UnknownTopLevel);
    }
    if table
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        != Some(1)
    {
        return Err(PolicyProfileError::Unsupported);
    }
    validate_fixed_v1_sections(&table)?;
    Ok(Some(table))
}

impl UserPolicyProfile {
    pub fn load(appdata: &Path) -> Result<Self, PolicyProfileError> {
        let Some(table) = load_table(appdata)? else {
            return Ok(Self::SafeDefault);
        };
        match table
            .get("policy_profile")
            .and_then(toml::Value::as_str)
            .unwrap_or("star.policy-profile.safe-default")
        {
            "safe_default" | "star.policy-profile.safe-default" => Ok(Self::SafeDefault),
            "personal_auto" | "star.policy-profile.personal-auto" => Ok(Self::PersonalAuto),
            _ => Err(PolicyProfileError::Unsupported),
        }
    }
}

impl UserToolRegistryConfig {
    pub fn load(appdata: &Path) -> Result<Self, PolicyProfileError> {
        let Some(table) = load_table(appdata)? else {
            return Ok(Self::default());
        };
        let config_revision = canonical_sha256(
            &serde_json::to_value(&table).map_err(|_| PolicyProfileError::Unsupported)?,
        )
        .map_err(|_| PolicyProfileError::Unsupported)?;
        let Some(registry) = table.get("tool_registry") else {
            return Ok(Self::default());
        };
        let registry = registry.as_table().ok_or(PolicyProfileError::Unsupported)?;
        if registry
            .keys()
            .any(|key| !TOOL_REGISTRY_KEYS.contains(&key.as_str()))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        let defaults = Self::default();
        if registry
            .get("allow_path_lookup")
            .is_some_and(|value| value.as_bool() != Some(false))
            || registry
                .get("live_reload")
                .is_some_and(|value| value.as_bool() != Some(true))
            || registry
                .get("demand_scan")
                .is_some_and(|value| value.as_bool() != Some(true))
            || registry
                .get("verify_executable_identity_each_call")
                .is_some_and(|value| value.as_bool() != Some(true))
            || registry
                .get("require_trusted_desktop_code_trust")
                .is_some_and(|value| value.as_bool() != Some(true))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        exact_string(registry, "project_update_policy", "pinned_hash")?;
        exact_string(registry, "project_trust", "explicit")?;
        exact_string(registry, "user_trust", "policy_profile")?;
        exact_string(registry, "default_isolation", "policy_profile")?;
        exact_string(registry, "user_default_update_policy", "pinned_hash")?;
        exact_string(registry, "invalid_optional_package", "keep_last_known_good")?;
        let user_root = registry
            .get("user_root")
            .map(|value| {
                value
                    .as_str()
                    .map(PathBuf::from)
                    .filter(|path| safe_user_config_path(path))
                    .ok_or(PolicyProfileError::Unsupported)
            })
            .transpose()?;
        let mut locations = BTreeMap::new();
        if let Some(values) = registry.get("locations") {
            let values = values.as_table().ok_or(PolicyProfileError::Unsupported)?;
            if values.len() > 64 {
                return Err(PolicyProfileError::Unsupported);
            }
            let valid_id =
                regex::Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").expect("static location ID regex");
            for (id, value) in values {
                let path = value
                    .as_str()
                    .map(PathBuf::from)
                    .filter(|path| safe_user_config_path(path))
                    .ok_or(PolicyProfileError::Unsupported)?;
                if !valid_id.is_match(id) {
                    return Err(PolicyProfileError::Unsupported);
                }
                locations.insert(id.clone(), path);
            }
        }
        Ok(Self {
            config_revision,
            enabled: boolean(registry, "enabled", defaults.enabled)?,
            user_root,
            locations,
            project_enabled: boolean(registry, "project_enabled", defaults.project_enabled)?,
            watch_files: boolean(registry, "watch_files", defaults.watch_files)?,
            allowed_process_protocols: string_set(
                registry,
                "allowed_process_protocols",
                &["star_json_stdio_v1", "argv_v1"],
                &["star_json_stdio_v1", "argv_v1"],
            )?,
            allowed_isolation_profiles: string_set(
                registry,
                "allowed_isolation_profiles",
                &["appcontainer_adapter", "trusted_desktop"],
                &["appcontainer_adapter", "trusted_desktop"],
            )?,
            require_trusted_desktop_code_trust: boolean(
                registry,
                "require_trusted_desktop_code_trust",
                defaults.require_trusted_desktop_code_trust,
            )?,
            reload_debounce_ms: bounded_integer(
                registry,
                "reload_debounce_ms",
                defaults.reload_debounce_ms,
                defaults.reload_debounce_ms,
            )?,
            stable_file_window_ms: bounded_integer(
                registry,
                "stable_file_window_ms",
                defaults.stable_file_window_ms,
                defaults.stable_file_window_ms,
            )?,
            stable_file_timeout_ms: bounded_integer(
                registry,
                "stable_file_timeout_ms",
                defaults.stable_file_timeout_ms,
                defaults.stable_file_timeout_ms,
            )?,
            persist_last_known_good: boolean(
                registry,
                "persist_last_known_good",
                defaults.persist_last_known_good,
            )?,
            allow_follow_path_user: boolean(
                registry,
                "allow_follow_path_user",
                defaults.allow_follow_path_user,
            )?,
            max_packages: bounded_integer(
                registry,
                "max_packages",
                defaults.max_packages as u64,
                defaults.max_packages as u64,
            )? as usize,
            max_tools: bounded_integer(
                registry,
                "max_tools",
                defaults.max_tools as u64,
                defaults.max_tools as u64,
            )? as usize,
            max_actions_per_package: bounded_integer(
                registry,
                "max_actions_per_package",
                defaults.max_actions_per_package as u64,
                defaults.max_actions_per_package as u64,
            )? as usize,
            max_watch_roots: bounded_integer(
                registry,
                "max_watch_roots",
                defaults.max_watch_roots as u64,
                defaults.max_watch_roots as u64,
            )? as usize,
            max_manifest_bytes: bounded_integer(
                registry,
                "max_manifest_bytes",
                defaults.max_manifest_bytes,
                defaults.max_manifest_bytes,
            )?,
            max_schema_bytes: bounded_integer(
                registry,
                "max_schema_bytes",
                defaults.max_schema_bytes,
                defaults.max_schema_bytes,
            )?,
            max_action_schema_bytes: bounded_integer(
                registry,
                "max_action_schema_bytes",
                defaults.max_action_schema_bytes as u64,
                defaults.max_action_schema_bytes as u64,
            )? as usize,
            max_schema_depth: bounded_integer(
                registry,
                "max_schema_depth",
                defaults.max_schema_depth as u64,
                defaults.max_schema_depth as u64,
            )? as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("star-policy-{name}-{}", star_ipc::nonce()))
    }

    fn write(root: &Path, value: &str) {
        let directory = root.join("Star-Control");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("config.toml"), value).unwrap();
    }

    #[test]
    fn missing_config_is_safe_default() {
        assert_eq!(
            UserPolicyProfile::load(&root("missing")).unwrap(),
            UserPolicyProfile::SafeDefault
        );
    }

    #[test]
    fn personal_auto_requires_an_exact_supported_user_profile() {
        let directory = root("personal");
        write(
            &directory,
            "\u{feff}schema_version = 1\npolicy_profile = \"star.policy-profile.personal-auto\"\n",
        );
        assert_eq!(
            UserPolicyProfile::load(&directory).unwrap(),
            UserPolicyProfile::PersonalAuto
        );

        let configured = root("configured");
        write(
            &configured,
            "schema_version = 1\npolicy_profile = \"personal_auto\"\n[tool_registry]\nuser_trust = \"policy_profile\"\n[tool_registry.locations]\nexample = \"C:\\\\Tools\\\\example.exe\"\n",
        );
        assert_eq!(
            UserPolicyProfile::load(&configured).unwrap(),
            UserPolicyProfile::PersonalAuto
        );
        assert!(
            UserToolRegistryConfig::load(&configured)
                .unwrap()
                .locations
                .contains_key("example")
        );

        let unknown = root("unknown");
        write(&unknown, "schema_version = 1\n[unknown]\nvalue = true\n");
        assert!(matches!(
            UserPolicyProfile::load(&unknown),
            Err(PolicyProfileError::UnknownTopLevel)
        ));

        let duplicate = root("duplicate");
        write(
            &duplicate,
            "schema_version = 1\npolicy_profile = \"safe_default\"\npolicy_profile = \"personal_auto\"\n",
        );
        assert!(matches!(
            UserPolicyProfile::load(&duplicate),
            Err(PolicyProfileError::InvalidToml)
        ));
    }

    #[test]
    // matrix: MCP-S017
    fn frozen_mcp_and_ipc_security_invariants_fail_closed() {
        for (name, body) in [
            ("path-lookup", "[tool_registry]\nallow_path_lookup = true\n"),
            ("live-reload", "[tool_registry]\nlive_reload = false\n"),
            ("demand-scan", "[tool_registry]\ndemand_scan = false\n"),
            (
                "identity-check",
                "[tool_registry]\nverify_executable_identity_each_call = false\n",
            ),
            (
                "desktop-code-trust",
                "[tool_registry]\nrequire_trusted_desktop_code_trust = false\n",
            ),
            (
                "project-policy",
                "[tool_registry]\nproject_update_policy = \"follow_path\"\n",
            ),
            ("gateway-version", "[mcp_gateway]\ncontract_version = 2\n"),
            (
                "gateway-message-limit",
                "[mcp_gateway]\nmax_message_bytes = 4194304\n",
            ),
            ("ipc-frame-limit", "[ipc]\nmax_frame_bytes = 4194304\n"),
            ("ipc-auth", "[ipc]\nauth_required = false\n"),
        ] {
            let directory = root(name);
            write(&directory, &format!("schema_version = 1\n{body}"));
            assert!(
                UserToolRegistryConfig::load(&directory).is_err(),
                "{name} must not be accepted"
            );
        }
    }

    #[test]
    fn bounded_registry_policy_values_are_parsed_instead_of_silently_ignored() {
        let directory = root("bounded-values");
        write(
            &directory,
            "schema_version = 1\n[tool_registry]\nenabled = false\nproject_enabled = false\nwatch_files = false\nallow_follow_path_user = false\nmax_packages = 8\nmax_tools = 16\nmax_actions_per_package = 4\nmax_watch_roots = 6\nmax_manifest_bytes = 65536\nmax_schema_bytes = 131072\nmax_action_schema_bytes = 32768\nmax_schema_depth = 8\nstable_file_window_ms = 100\nstable_file_timeout_ms = 1000\nallowed_process_protocols = [\"star_json_stdio_v1\"]\nallowed_isolation_profiles = [\"appcontainer_adapter\"]\n",
        );
        let config = UserToolRegistryConfig::load(&directory).unwrap();
        assert!(!config.enabled);
        assert!(!config.project_enabled);
        assert!(!config.watch_files);
        assert!(!config.allow_follow_path_user);
        assert_eq!(config.max_packages, 8);
        assert_eq!(config.max_tools, 16);
        assert_eq!(config.max_actions_per_package, 4);
        assert_eq!(config.max_watch_roots, 6);
        assert_eq!(config.max_manifest_bytes, 65_536);
        assert_eq!(config.max_schema_bytes, 131_072);
        assert_eq!(config.max_action_schema_bytes, 32_768);
        assert_eq!(config.max_schema_depth, 8);
        assert_eq!(config.stable_file_window_ms, 100);
        assert_eq!(config.stable_file_timeout_ms, 1_000);
        assert_eq!(config.allowed_process_protocols, ["star_json_stdio_v1"]);
        assert_eq!(config.allowed_isolation_profiles, ["appcontainer_adapter"]);
    }
}
