//! Fail-closed extraction of the user policy profile used by the Tool Registry.
//!
//! Only the frozen MCP/Registry subset is interpreted here.  Broader
//! Star-Control configuration remains outside this crate, but every accepted
//! key in these sections is type checked and v1 security invariants are
//! enforced instead of being silently ignored.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    io::Read,
    path::{Path, PathBuf},
};

use star_contracts::{
    Sha256Hash,
    canonical::canonical_sha256,
    config_v1::{
        ConfigLayerV1, ConfigMergeStrategyV1, ConfigOverrideV1, ConfigSourceKindV1,
        ConfigSourceRefV1, ConfigValueV1, EffectiveConfigEntryV1, EffectiveConfigV1,
    },
    index::IndexTier,
};
use star_planning::PlanningPolicy;
use star_project::{ScanPolicy, index::IndexPolicy, valid_project_relative_glob};
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
    "project_discovery",
    "scan",
    "index",
    "index_cache",
    "change_planning",
    "contract_management",
    "docs_validation",
    "doctor",
    "failure_reproduction",
    "security_supply_chain",
    "dependency_maintenance",
    "maintenance_radar",
    "migration",
    "performance_build",
    "language_platform_migration",
    "release",
    "evaluation",
    "rust_style",
    "management",
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

#[derive(Clone, Debug)]
pub struct UserExecutionConfig {
    pub effective: EffectiveConfigV1,
    pub scan_incremental: bool,
    pub scan_policy: ScanPolicy,
    pub index_policy: IndexPolicy,
    pub index_cache_enabled: bool,
    pub index_cache_max_total_bytes: u64,
    pub index_cache_retention_days: u64,
    pub planning_policy: PlanningPolicy,
}

const CONTROLLER_KEYS: &[&str] = &[
    "auto_start",
    "shutdown_grace_ms",
    "command_timeout_ms",
    "recovery_on_start",
];

const CODEX_KEYS: &[&str] = &[
    "mcp_required",
    "capability_max_age_ms",
    "app_server_start_timeout_ms",
    "require_entry_check",
    "allow_managed_ultra",
];

const ROUTING_KEYS: &[&str] = &[
    "default_model_role",
    "default_reasoning_effort",
    "plan_reasoning_effort",
    "allowed_model_roles",
    "allowed_execution_modes",
    "unsupported_choice",
    "retry_limit",
    "escalation_limit",
    "max_parallel_codex",
];

const PERMISSION_KEYS: &[&str] = &[
    "default_action",
    "approval_ttl_ms",
    "reuse_approval",
    "require_scope_hash",
    "actions",
];

const PERMISSION_ACTION_KEYS: &[&str] = &[
    "local_read",
    "local_write",
    "local_delete",
    "local_mass_move",
    "process_run",
    "dependency_change",
    "system_change",
    "secret_access",
    "network_read",
    "network_download",
    "external_write",
    "account_change",
    "plan_execute",
    "git_commit",
    "git_merge",
    "git_push",
    "pull_request",
    "release_publish",
    "paid_action",
];

const BUDGET_KEYS: &[&str] = &[
    "goal_wall_time_ms",
    "stage_wall_time_ms",
    "goal_paid_action_limit",
    "stage_attempt_limit",
    "max_artifact_bytes",
    "monetary_limit",
];

const CATALOG_KEYS: &[&str] = &["user_roots", "project_enabled", "require_trust"];
const LOGGING_KEYS: &[&str] = &["level", "include_raw_output"];

const VALIDATION_KEYS: &[&str] = &[
    "required_phases",
    "fail_on",
    "command_timeout_ms",
    "allow_manual_evidence",
    "require_independent_review_for",
    "max_log_bytes",
    "checks_add",
    "checks_remove",
    "baseline_mode",
    "require_current_evidence",
    "allow_ratchet_satisfaction",
    "suppression_requires_expiry",
    "allow_permanent_suppressions",
    "required_flaky_action",
    "cli_only_semantic_review",
    "max_parallel_checks",
];

const PROJECT_DISCOVERY_KEYS: &[&str] = &[
    "roots_add",
    "detect_nested_repositories",
    "detect_linked_worktrees",
    "detect_workspaces",
    "detect_non_git",
    "follow_symlinks",
    "max_depth",
    "max_directories",
    "exclude_paths_add",
    "search_ignored_subtrees",
];

const SCAN_KEYS: &[&str] = &[
    "incremental",
    "include_untracked",
    "include_ignored",
    "follow_symlinks",
    "binary_mode",
    "max_file_bytes",
    "max_files",
    "max_total_bytes",
    "max_parallel_files",
    "require_complete_for_gate",
    "rule_error_policy",
    "include_paths",
    "exclude_paths_add",
    "classification_rules_add",
    "rule_sets_add",
    "rule_sets_remove",
    "hardcoding_rules_enabled",
    "hardcoding_include_tests",
    "hardcoding_include_fixtures",
    "hardcoding_include_docs_examples",
    "hardcoding_include_generated",
    "hardcoding_include_vendor",
];

const INDEX_KEYS: &[&str] = &[
    "required_tier",
    "max_tier",
    "fallback_to_lower_tier",
    "max_symbols",
    "max_references",
    "max_graph_edges",
    "cross_project_edges",
];

const INDEX_CACHE_KEYS: &[&str] = &[
    "enabled",
    "max_total_bytes",
    "retention_days",
    "reuse_partial",
    "store_source_bytes",
];

const CHANGE_PLANNING_KEYS: &[&str] = &[
    "require_current_inputs",
    "max_graph_depth",
    "max_graph_nodes",
    "max_graph_edges",
    "max_downstream_projects",
    "max_check_candidates",
    "allow_cross_project_read",
    "allow_previous_success_reuse",
    "require_user_acceptance_for_change_scope_expansion",
];

const CONTRACT_MANAGEMENT_KEYS: &[&str] = &[
    "require_explicit_baseline",
    "require_complete_consumers",
    "require_companion_changes",
    "breaking_requires_migration_guide",
    "deprecation_window",
    "unknown_semantic_action",
    "public_surface_expansion",
];

const DOCS_VALIDATION_KEYS: &[&str] = &[
    "require_local_links",
    "require_registered_commands",
    "require_config_schema",
    "require_generated_provenance",
    "allow_safe_example_execution",
];

const DOCTOR_KEYS: &[&str] = &[
    "read_only",
    "network_action",
    "package_action",
    "system_setting_action",
    "collect_environment_values",
    "probe_timeout_ms",
    "max_output_bytes",
];

const FAILURE_REPRODUCTION_KEYS: &[&str] = &[
    "max_rerun_attempts",
    "require_structured_args",
    "require_before_after",
    "external_condition",
    "default_artifact_role",
    "unsafe_artifact",
    "debugger_action",
];

const SECURITY_SUPPLY_CHAIN_KEYS: &[&str] = &[
    "require_source_provenance",
    "require_freshness",
    "unknown_freshness_action",
    "network_refresh",
    "default_max_age_hours",
    "default_report_artifacts",
];

const DEPENDENCY_MAINTENANCE_KEYS: &[&str] = &[
    "default_stop",
    "lockfile_owner",
    "preview_workspace",
    "network_action",
    "download_action",
    "change_action",
    "preserve_before_lockfile",
    "require_actual_diff_replan",
];

const MAINTENANCE_RADAR_KEYS: &[&str] = &[
    "sort_policy",
    "include_expiring_suppressions",
    "allow_ai_priority",
];

const MIGRATION_KEYS: &[&str] = &[
    "default_strategy",
    "require_dry_run",
    "require_consistent_backup",
    "require_restore_rehearsal",
    "require_migration_rehearsal",
    "unknown_field_action",
    "live_execute_action",
    "destructive_action",
    "rollback_action",
    "max_resume_attempts",
    "max_additional_rehearsals",
];

const PERFORMANCE_BUILD_KEYS: &[&str] = &[
    "enabled_by_default",
    "require_declared_workload",
    "default_warmup_runs",
    "default_measurement_runs",
    "minimum_measurement_runs",
    "max_additional_runs",
    "outlier_policy",
    "missing_measurement_action",
    "require_exact_environment",
    "profiler_action",
];

const LANGUAGE_PLATFORM_MIGRATION_KEYS: &[&str] = &[
    "require_behavior_contract",
    "compile_only_equivalence",
    "unknown_semantics_action",
    "compatibility_window",
    "cutover_action",
    "unsupported_platform_action",
    "allow_full_auto_translation_claim",
];

const RELEASE_KEYS: &[&str] = &[
    "promotion_mode",
    "require_clean_windows",
    "require_native_x64_runtime",
    "arm64_support_tier",
    "arm64_runtime_verification",
    "require_explicit_remote_action_approval",
    "publish_action",
    "deploy_action",
    "withdraw_action",
    "rollback_action",
    "max_parallel_target_jobs",
];

const EVALUATION_KEYS: &[&str] = &[
    "default_mode",
    "separate_cli_codex_contexts",
    "provider_verified_cost_only",
    "max_attempts_per_case",
    "incomparable_action",
];

const RUST_STYLE_KEYS: &[&str] = &[
    "required_profile_ref",
    "auto_apply_grant_refs",
    "max_preview_retention",
    "network_action",
    "unpinned_apply_action",
    "partial_coverage_apply_action",
];

const MANAGEMENT_KEYS: &[&str] = &[
    "integrity_check_on_unclean_start",
    "allow_read_only_recovery",
    "auto_migrate_rebuildable",
    "backup_before_migration",
    "keep_latest_successful_scans",
    "incomplete_staging_retention_days",
    "scan_detail_retention_days",
    "resolved_finding_retention_days",
    "local_decision_retention_days",
    "migration_backup_min_count",
    "suppression_default_expiry_days",
    "baseline_activation",
];

const VCS_KEYS: &[&str] = &[
    "use_worktree",
    "merge_strategy",
    "protected_branches",
    "worktree_root",
    "max_parallel_projects",
    "max_active_worktrees",
    "max_parallel_mutations_per_repository",
    "max_parallel_local_merges",
    "max_merge_queue_entries",
    "worktree_disk_limit_bytes",
];

const REMOTE_KEYS: &[&str] = &[
    "allowed_hosts",
    "require_clean_target",
    "personal_auto_write_scopes",
    "max_parallel_writes",
];

const STATE_KEYS: &[&str] = &[
    "artifact_root",
    "checkpoint_interval_ms",
    "completed_retention_days",
    "failed_retention_days",
    "redaction_rules_add",
    "cleanup_trigger",
];

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

fn validate_known_section_shapes(table: &toml::Table) -> Result<(), PolicyProfileError> {
    for (name, keys) in [
        ("controller", CONTROLLER_KEYS),
        ("codex", CODEX_KEYS),
        ("routing", ROUTING_KEYS),
        ("permissions", PERMISSION_KEYS),
        ("budgets", BUDGET_KEYS),
        ("validation", VALIDATION_KEYS),
        ("project_discovery", PROJECT_DISCOVERY_KEYS),
        ("scan", SCAN_KEYS),
        ("index", INDEX_KEYS),
        ("index_cache", INDEX_CACHE_KEYS),
        ("change_planning", CHANGE_PLANNING_KEYS),
        ("contract_management", CONTRACT_MANAGEMENT_KEYS),
        ("docs_validation", DOCS_VALIDATION_KEYS),
        ("doctor", DOCTOR_KEYS),
        ("failure_reproduction", FAILURE_REPRODUCTION_KEYS),
        ("security_supply_chain", SECURITY_SUPPLY_CHAIN_KEYS),
        ("dependency_maintenance", DEPENDENCY_MAINTENANCE_KEYS),
        ("maintenance_radar", MAINTENANCE_RADAR_KEYS),
        ("migration", MIGRATION_KEYS),
        ("performance_build", PERFORMANCE_BUILD_KEYS),
        (
            "language_platform_migration",
            LANGUAGE_PLATFORM_MIGRATION_KEYS,
        ),
        ("release", RELEASE_KEYS),
        ("evaluation", EVALUATION_KEYS),
        ("rust_style", RUST_STYLE_KEYS),
        ("management", MANAGEMENT_KEYS),
        ("vcs", VCS_KEYS),
        ("remote", REMOTE_KEYS),
        ("state", STATE_KEYS),
        ("catalog", CATALOG_KEYS),
        ("tool_registry", TOOL_REGISTRY_KEYS),
        ("logging", LOGGING_KEYS),
    ] {
        let _ = section(table, name, keys)?;
    }
    if let Some(actions) = table
        .get("permissions")
        .and_then(toml::Value::as_table)
        .and_then(|permissions| permissions.get("actions"))
    {
        let actions = actions.as_table().ok_or(PolicyProfileError::Unsupported)?;
        if actions
            .keys()
            .any(|key| !PERMISSION_ACTION_KEYS.contains(&key.as_str()))
        {
            return Err(PolicyProfileError::Unsupported);
        }
    }
    Ok(())
}

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

fn read_optional_config(path: &Path) -> Result<Option<Vec<u8>>, PolicyProfileError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_CONFIG_BYTES || !safe_user_config_path(path) {
        return Err(PolicyProfileError::Unsupported);
    }
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(Some(bytes))
}

fn parse_config_table(bytes: &[u8]) -> Result<toml::Table, PolicyProfileError> {
    let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
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
    validate_known_section_shapes(&table)?;
    Ok(table)
}

fn load_table(appdata: &Path) -> Result<Option<toml::Table>, PolicyProfileError> {
    let path = appdata.join("Star-Control").join("config.toml");
    read_optional_config(&path)?
        .map(|bytes| parse_config_table(&bytes))
        .transpose()
}

fn normalized_policy_profile_id(value: &str) -> Result<&'static str, PolicyProfileError> {
    match value {
        "safe_default" | "star.policy-profile.safe-default" => {
            Ok("star.policy-profile.safe-default")
        }
        "personal_auto" | "star.policy-profile.personal-auto" => {
            Ok("star.policy-profile.personal-auto")
        }
        _ => Err(PolicyProfileError::Unsupported),
    }
}

fn collect_leaf_keys(prefix: &str, table: &toml::Table, output: &mut BTreeSet<String>) {
    for (key, value) in table {
        let key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(table) = value.as_table() {
            collect_leaf_keys(&key, table, output);
        } else {
            output.insert(key);
        }
    }
}

fn project_config_path(project_directory: &Path) -> Result<Option<PathBuf>, PolicyProfileError> {
    for directory in project_directory.ancestors() {
        let path = directory.join(".star-control").join("config.toml");
        match fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let project_manifest = directory.join(".star-control").join("project.toml");
        let git_boundary = directory.join(".git");
        if fs::symlink_metadata(project_manifest).is_ok()
            || fs::symlink_metadata(git_boundary).is_ok()
        {
            break;
        }
    }
    Ok(None)
}

fn load_project_layer(
    project_directory: &Path,
) -> Result<Option<ConfigLayerV1>, PolicyProfileError> {
    let Some(path) = project_config_path(project_directory)? else {
        return Ok(None);
    };
    let Some(bytes) = read_optional_config(&path)? else {
        return Ok(None);
    };
    let table = parse_config_table(&bytes)?;
    if table.contains_key("policy_profile") {
        return Err(PolicyProfileError::Unsupported);
    }
    let required_profile = table
        .get("required_policy_profile")
        .map(|value| {
            value
                .as_str()
                .ok_or(PolicyProfileError::Unsupported)
                .and_then(normalized_policy_profile_id)
        })
        .transpose()?;
    let source_fingerprint = canonical_sha256(
        &serde_json::to_value(&table).map_err(|_| PolicyProfileError::Unsupported)?,
    )
    .map_err(|_| PolicyProfileError::Unsupported)?;
    let mut normalized = table.clone();
    normalized.remove("required_policy_profile");
    if let Some(required_profile) = required_profile {
        normalized.insert(
            "policy_profile".to_owned(),
            toml::Value::String(required_profile.to_owned()),
        );
    }
    let parsed = UserExecutionConfig::resolve(Some(&normalized))?;
    let mut declared = BTreeSet::new();
    collect_leaf_keys("", &table, &mut declared);
    declared.remove("schema_version");
    if declared.remove("required_policy_profile") {
        declared.insert("policy_profile".to_owned());
    }
    let mut overrides = Vec::with_capacity(declared.len());
    for key in declared {
        let entry = parsed
            .effective
            .entries
            .iter()
            .find(|entry| {
                entry.key == key
                    && entry
                        .provenance
                        .iter()
                        .any(|source| source.source_kind == ConfigSourceKindV1::User)
            })
            .ok_or(PolicyProfileError::Unsupported)?;
        overrides.push(ConfigOverrideV1 {
            key,
            value: entry.value.clone(),
        });
    }
    if overrides.is_empty() {
        return Ok(None);
    }
    Ok(Some(ConfigLayerV1 {
        source: ConfigSourceRefV1 {
            source_kind: ConfigSourceKindV1::Project,
            source_id: "project-config".to_owned(),
            source_fingerprint,
        },
        overrides,
    }))
}

impl UserPolicyProfile {
    pub fn from_effective(config: &EffectiveConfigV1) -> Result<Self, PolicyProfileError> {
        match config.string("policy_profile") {
            Some("star.policy-profile.safe-default") => Ok(Self::SafeDefault),
            Some("star.policy-profile.personal-auto") => Ok(Self::PersonalAuto),
            _ => Err(PolicyProfileError::Unsupported),
        }
    }

    pub fn load(appdata: &Path) -> Result<Self, PolicyProfileError> {
        let Some(table) = load_table(appdata)? else {
            return Ok(Self::SafeDefault);
        };
        match normalized_policy_profile_id(
            table
                .get("policy_profile")
                .and_then(toml::Value::as_str)
                .unwrap_or("star.policy-profile.safe-default"),
        )? {
            "star.policy-profile.safe-default" => Ok(Self::SafeDefault),
            "star.policy-profile.personal-auto" => Ok(Self::PersonalAuto),
            _ => unreachable!("policy profile normalization is exhaustive"),
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

fn section<'a>(
    table: &'a toml::Table,
    name: &str,
    keys: &[&str],
) -> Result<Option<&'a toml::Table>, PolicyProfileError> {
    let Some(value) = table.get(name) else {
        return Ok(None);
    };
    let section = value.as_table().ok_or(PolicyProfileError::Unsupported)?;
    if section.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(Some(section))
}

fn exact_boolean(table: &toml::Table, key: &str, expected: bool) -> Result<(), PolicyProfileError> {
    if table
        .get(key)
        .is_some_and(|value| value.as_bool() != Some(expected))
    {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(())
}

fn enum_string(
    table: &toml::Table,
    key: &str,
    default: &str,
    allowed: &[&str],
) -> Result<String, PolicyProfileError> {
    let value = table
        .get(key)
        .map(|value| value.as_str().ok_or(PolicyProfileError::Unsupported))
        .unwrap_or(Ok(default))?;
    allowed
        .contains(&value)
        .then(|| value.to_owned())
        .ok_or(PolicyProfileError::Unsupported)
}

fn integer_range(
    table: &toml::Table,
    key: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, PolicyProfileError> {
    let value = table
        .get(key)
        .map(|value| value.as_integer().ok_or(PolicyProfileError::Unsupported))
        .unwrap_or(Ok(default as i64))?;
    let value = u64::try_from(value).map_err(|_| PolicyProfileError::Unsupported)?;
    (minimum..=maximum)
        .contains(&value)
        .then_some(value)
        .ok_or(PolicyProfileError::Unsupported)
}

fn optional_bounded_integer(
    table: &toml::Table,
    key: &str,
    maximum: u64,
) -> Result<Option<u64>, PolicyProfileError> {
    table
        .get(key)
        .map(|value| {
            let value = value
                .as_integer()
                .and_then(|value| u64::try_from(value).ok())
                .filter(|value| *value > 0 && *value <= maximum)
                .ok_or(PolicyProfileError::Unsupported)?;
            Ok(value)
        })
        .transpose()
}

fn string_values(
    table: &toml::Table,
    key: &str,
    defaults: &[&str],
    maximum: usize,
) -> Result<Vec<String>, PolicyProfileError> {
    let Some(value) = table.get(key) else {
        let mut output = defaults
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        output.sort();
        output.dedup();
        return Ok(output);
    };
    let values = value.as_array().ok_or(PolicyProfileError::Unsupported)?;
    if values.len() > maximum {
        return Err(PolicyProfileError::Unsupported);
    }
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .as_str()
            .filter(|value| !value.trim().is_empty() && value.len() <= 256 && !value.contains('\0'))
            .ok_or(PolicyProfileError::Unsupported)?;
        output.push(value.to_owned());
    }
    output.sort();
    if output.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(output)
}

fn exact_empty_array(table: &toml::Table, key: &str) -> Result<(), PolicyProfileError> {
    if table
        .get(key)
        .is_some_and(|value| value.as_array().is_none_or(|values| !values.is_empty()))
    {
        return Err(PolicyProfileError::Unsupported);
    }
    Ok(())
}

fn effective_entry(
    key: &str,
    value: ConfigValueV1,
    merge_strategy: ConfigMergeStrategyV1,
    user_revision: Option<&Sha256Hash>,
    user_declared: bool,
) -> EffectiveConfigEntryV1 {
    let mut provenance = vec![ConfigSourceRefV1 {
        source_kind: ConfigSourceKindV1::BuiltIn,
        source_id: "star.product-default.v1".to_owned(),
        source_fingerprint: Sha256Hash::digest(b"star.product-default.v1"),
    }];
    if user_declared {
        provenance.push(ConfigSourceRefV1 {
            source_kind: ConfigSourceKindV1::User,
            source_id: "user-config".to_owned(),
            source_fingerprint: user_revision
                .cloned()
                .unwrap_or_else(|| Sha256Hash::digest(b"star.user-config.missing")),
        });
    }
    EffectiveConfigEntryV1 {
        key: key.to_owned(),
        value,
        merge_strategy,
        provenance,
    }
}

fn push_effective_entry(
    entries: &mut Vec<EffectiveConfigEntryV1>,
    root: &toml::Table,
    user_revision: Option<&Sha256Hash>,
    section: &str,
    key: &str,
    value: ConfigValueV1,
    merge_strategy: ConfigMergeStrategyV1,
) {
    entries.push(effective_entry(
        &format!("{section}.{key}"),
        value,
        merge_strategy,
        user_revision,
        root.get(section)
            .and_then(toml::Value::as_table)
            .is_some_and(|table| table.contains_key(key)),
    ));
}

fn exact_absent(table: &toml::Table, key: &str) -> Result<(), PolicyProfileError> {
    (!table.contains_key(key))
        .then_some(())
        .ok_or(PolicyProfileError::Unsupported)
}

fn supported_work_profile(value: &str) -> bool {
    matches!(
        value,
        "ai_development_validation"
            | "api_contract_change"
            | "architecture_quality"
            | "change_planning"
            | "ci_release_deploy"
            | "data_config_db_migration"
            | "debug_recovery"
            | "dependency_upgrade"
            | "docs_config_environment"
            | "language_platform_migration"
            | "performance_build"
            | "project_understanding"
            | "refactor_codemod"
            | "rust_style_auto_fix"
            | "security_supply_chain"
            | "test_correctness"
    )
}

fn common_policy_entries(
    root: &toml::Table,
    empty: &toml::Table,
    user_revision: Option<&Sha256Hash>,
    policy_profile_id: &str,
) -> Result<Vec<EffectiveConfigEntryV1>, PolicyProfileError> {
    exact_absent(root, "required_policy_profile")?;
    let default_work_profile = root
        .get("default_work_profile")
        .map(|value| {
            value
                .as_str()
                .filter(|value| supported_work_profile(value))
                .map(str::to_owned)
                .ok_or(PolicyProfileError::Unsupported)
        })
        .transpose()?;

    let controller = section(root, "controller", CONTROLLER_KEYS)?.unwrap_or(empty);
    let codex = section(root, "codex", CODEX_KEYS)?.unwrap_or(empty);
    let routing = section(root, "routing", ROUTING_KEYS)?.unwrap_or(empty);
    let permissions = section(root, "permissions", PERMISSION_KEYS)?.unwrap_or(empty);
    let budgets = section(root, "budgets", BUDGET_KEYS)?.unwrap_or(empty);
    let catalog = section(root, "catalog", CATALOG_KEYS)?.unwrap_or(empty);
    let logging = section(root, "logging", LOGGING_KEYS)?.unwrap_or(empty);

    let actions = match permissions.get("actions") {
        Some(value) => {
            let table = value.as_table().ok_or(PolicyProfileError::Unsupported)?;
            if table
                .keys()
                .any(|key| !PERMISSION_ACTION_KEYS.contains(&key.as_str()))
            {
                return Err(PolicyProfileError::Unsupported);
            }
            table
        }
        None => empty,
    };

    // These process/Gateway settings do not have a negotiated v1 runtime
    // payload. They are accepted only at the shipped value so a user typo or
    // unsupported override can never look effective while being ignored.
    for (key, expected) in [("auto_start", true), ("recovery_on_start", true)] {
        exact_boolean(controller, key, expected)?;
    }
    exact_integer(controller, "shutdown_grace_ms", 10_000)?;
    exact_integer(controller, "command_timeout_ms", 300_000)?;

    for (key, expected) in [
        ("mcp_required", true),
        ("require_entry_check", true),
        ("allow_managed_ultra", true),
    ] {
        exact_boolean(codex, key, expected)?;
    }
    exact_integer(codex, "capability_max_age_ms", 900_000)?;
    exact_integer(codex, "app_server_start_timeout_ms", 30_000)?;

    for (key, expected) in [
        ("default_model_role", "terra"),
        ("default_reasoning_effort", "medium"),
        ("plan_reasoning_effort", "high"),
        ("unsupported_choice", "explain_and_fallback"),
    ] {
        exact_string(routing, key, expected)?;
    }
    let allowed_roles =
        string_values(routing, "allowed_model_roles", &["luna", "sol", "terra"], 3)?;
    if allowed_roles != ["luna", "sol", "terra"] {
        return Err(PolicyProfileError::Unsupported);
    }
    let allowed_modes = string_values(
        routing,
        "allowed_execution_modes",
        &["single", "max", "ultra"],
        3,
    )?;
    if allowed_modes != ["max", "single", "ultra"] {
        return Err(PolicyProfileError::Unsupported);
    }
    exact_integer(routing, "retry_limit", 1)?;
    exact_integer(routing, "escalation_limit", 2)?;
    exact_integer(routing, "max_parallel_codex", 3)?;

    exact_string(permissions, "default_action", "prompt")?;
    let approval_ttl_ms = bounded_integer(permissions, "approval_ttl_ms", 1_800_000, 1_800_000)?;
    exact_boolean(permissions, "reuse_approval", false)?;
    exact_boolean(permissions, "require_scope_hash", true)?;
    let personal_auto = policy_profile_id == "star.policy-profile.personal-auto";
    let automatic_actions = if personal_auto {
        &[
            "local_read",
            "local_write",
            "local_delete",
            "local_mass_move",
            "process_run",
            "dependency_change",
            "system_change",
            "secret_access",
            "network_read",
            "network_download",
            "plan_execute",
            "git_commit",
            "git_merge",
        ][..]
    } else {
        &["local_read", "local_write", "process_run", "network_read"][..]
    };
    for action in PERMISSION_ACTION_KEYS {
        let expected = if automatic_actions.contains(action) {
            "auto"
        } else {
            "prompt"
        };
        exact_string(actions, action, expected)?;
    }

    let goal_wall_time_ms = optional_bounded_integer(budgets, "goal_wall_time_ms", u64::MAX)?;
    let stage_wall_time_ms = optional_bounded_integer(budgets, "stage_wall_time_ms", u64::MAX)?;
    exact_absent(budgets, "goal_paid_action_limit")?;
    exact_integer(budgets, "stage_attempt_limit", 2)?;
    let max_artifact_bytes =
        bounded_integer(budgets, "max_artifact_bytes", 1_073_741_824, 1_073_741_824)?;
    exact_absent(budgets, "monetary_limit")?;

    exact_empty_array(catalog, "user_roots")?;
    exact_boolean(catalog, "project_enabled", true)?;
    exact_boolean(catalog, "require_trust", true)?;
    exact_string(logging, "level", "info")?;
    exact_boolean(logging, "include_raw_output", false)?;

    let mut entries = vec![
        effective_entry(
            "schema_version",
            ConfigValueV1::Integer(1),
            ConfigMergeStrategyV1::Immutable,
            user_revision,
            root.contains_key("schema_version"),
        ),
        effective_entry(
            "policy_profile",
            ConfigValueV1::String(policy_profile_id.to_owned()),
            ConfigMergeStrategyV1::MostRestrictive,
            user_revision,
            root.contains_key("policy_profile"),
        ),
        effective_entry(
            "default_work_profile",
            ConfigValueV1::StringSet(default_work_profile.into_iter().collect()),
            ConfigMergeStrategyV1::Replace,
            user_revision,
            root.contains_key("default_work_profile"),
        ),
    ];
    let mut add =
        |section: &str, key: &str, value: ConfigValueV1, strategy: ConfigMergeStrategyV1| {
            push_effective_entry(
                &mut entries,
                root,
                user_revision,
                section,
                key,
                value,
                strategy,
            );
        };

    for (key, value, strategy) in [
        (
            "auto_start",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::FalseWins,
        ),
        (
            "shutdown_grace_ms",
            ConfigValueV1::Integer(10_000),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "command_timeout_ms",
            ConfigValueV1::Integer(300_000),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "recovery_on_start",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins,
        ),
    ] {
        add("controller", key, value, strategy);
    }
    for (key, value, strategy) in [
        (
            "mcp_required",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins,
        ),
        (
            "capability_max_age_ms",
            ConfigValueV1::Integer(900_000),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "app_server_start_timeout_ms",
            ConfigValueV1::Integer(30_000),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "require_entry_check",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins,
        ),
        (
            "allow_managed_ultra",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::FalseWins,
        ),
    ] {
        add("codex", key, value, strategy);
    }
    for (key, value, strategy) in [
        (
            "default_model_role",
            ConfigValueV1::String("terra".to_owned()),
            ConfigMergeStrategyV1::Replace,
        ),
        (
            "default_reasoning_effort",
            ConfigValueV1::String("medium".to_owned()),
            ConfigMergeStrategyV1::Intersection,
        ),
        (
            "plan_reasoning_effort",
            ConfigValueV1::String("high".to_owned()),
            ConfigMergeStrategyV1::Intersection,
        ),
        (
            "allowed_model_roles",
            ConfigValueV1::StringSet(allowed_roles),
            ConfigMergeStrategyV1::Intersection,
        ),
        (
            "allowed_execution_modes",
            ConfigValueV1::StringSet(allowed_modes),
            ConfigMergeStrategyV1::Intersection,
        ),
        (
            "unsupported_choice",
            ConfigValueV1::String("explain_and_fallback".to_owned()),
            ConfigMergeStrategyV1::Replace,
        ),
        (
            "retry_limit",
            ConfigValueV1::Integer(1),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "escalation_limit",
            ConfigValueV1::Integer(2),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
        (
            "max_parallel_codex",
            ConfigValueV1::Integer(3),
            ConfigMergeStrategyV1::MinimumLimit,
        ),
    ] {
        add("routing", key, value, strategy);
    }
    add(
        "permissions",
        "default_action",
        ConfigValueV1::String("prompt".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    add(
        "permissions",
        "approval_ttl_ms",
        ConfigValueV1::Integer(approval_ttl_ms),
        ConfigMergeStrategyV1::MinimumLimit,
    );
    add(
        "permissions",
        "reuse_approval",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::FalseWins,
    );
    add(
        "permissions",
        "require_scope_hash",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::TrueWins,
    );
    for action in PERMISSION_ACTION_KEYS {
        let value = if automatic_actions.contains(action) {
            "auto"
        } else {
            "prompt"
        };
        entries.push(effective_entry(
            &format!("permissions.actions.{action}"),
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::MostRestrictive,
            user_revision,
            actions.contains_key(*action),
        ));
    }
    let mut add =
        |section: &str, key: &str, value: ConfigValueV1, strategy: ConfigMergeStrategyV1| {
            push_effective_entry(
                &mut entries,
                root,
                user_revision,
                section,
                key,
                value,
                strategy,
            );
        };
    for (key, value) in [
        ("goal_wall_time_ms", goal_wall_time_ms),
        ("stage_wall_time_ms", stage_wall_time_ms),
    ] {
        add(
            "budgets",
            key,
            ConfigValueV1::Json(
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
            ),
            ConfigMergeStrategyV1::MinimumLimit,
        );
    }
    add(
        "budgets",
        "stage_attempt_limit",
        ConfigValueV1::Integer(2),
        ConfigMergeStrategyV1::MinimumLimit,
    );
    add(
        "budgets",
        "max_artifact_bytes",
        ConfigValueV1::Integer(max_artifact_bytes),
        ConfigMergeStrategyV1::MinimumLimit,
    );
    add(
        "catalog",
        "user_roots",
        ConfigValueV1::StringSet(Vec::new()),
        ConfigMergeStrategyV1::Union,
    );
    add(
        "catalog",
        "project_enabled",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::FalseWins,
    );
    add(
        "catalog",
        "require_trust",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::TrueWins,
    );
    add(
        "logging",
        "level",
        ConfigValueV1::String("info".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    add(
        "logging",
        "include_raw_output",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::FalseWins,
    );
    Ok(entries)
}

fn later_stage_policy_entries(
    root: &toml::Table,
    empty: &toml::Table,
    user_revision: Option<&Sha256Hash>,
) -> Result<Vec<EffectiveConfigEntryV1>, PolicyProfileError> {
    let contract = section(root, "contract_management", CONTRACT_MANAGEMENT_KEYS)?.unwrap_or(empty);
    let docs = section(root, "docs_validation", DOCS_VALIDATION_KEYS)?.unwrap_or(empty);
    let doctor = section(root, "doctor", DOCTOR_KEYS)?.unwrap_or(empty);
    let failure =
        section(root, "failure_reproduction", FAILURE_REPRODUCTION_KEYS)?.unwrap_or(empty);
    let security =
        section(root, "security_supply_chain", SECURITY_SUPPLY_CHAIN_KEYS)?.unwrap_or(empty);
    let dependency =
        section(root, "dependency_maintenance", DEPENDENCY_MAINTENANCE_KEYS)?.unwrap_or(empty);
    let radar = section(root, "maintenance_radar", MAINTENANCE_RADAR_KEYS)?.unwrap_or(empty);
    let migration = section(root, "migration", MIGRATION_KEYS)?.unwrap_or(empty);
    let performance = section(root, "performance_build", PERFORMANCE_BUILD_KEYS)?.unwrap_or(empty);
    let language = section(
        root,
        "language_platform_migration",
        LANGUAGE_PLATFORM_MIGRATION_KEYS,
    )?
    .unwrap_or(empty);
    let release = section(root, "release", RELEASE_KEYS)?.unwrap_or(empty);
    let evaluation = section(root, "evaluation", EVALUATION_KEYS)?.unwrap_or(empty);
    let rust_style = section(root, "rust_style", RUST_STYLE_KEYS)?.unwrap_or(empty);
    let management = section(root, "management", MANAGEMENT_KEYS)?.unwrap_or(empty);
    let vcs = section(root, "vcs", VCS_KEYS)?.unwrap_or(empty);
    let remote = section(root, "remote", REMOTE_KEYS)?.unwrap_or(empty);
    let state = section(root, "state", STATE_KEYS)?.unwrap_or(empty);
    let mut entries = Vec::new();
    let mut add =
        |section: &str, key: &str, value: ConfigValueV1, strategy: ConfigMergeStrategyV1| {
            push_effective_entry(
                &mut entries,
                root,
                user_revision,
                section,
                key,
                value,
                strategy,
            );
        };

    for key in [
        "require_explicit_baseline",
        "require_complete_consumers",
        "require_companion_changes",
        "breaking_requires_migration_guide",
    ] {
        exact_boolean(contract, key, true)?;
        add(
            "contract_management",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins,
        );
    }
    for (key, value) in [
        ("deprecation_window", "finite_required"),
        ("unknown_semantic_action", "human_review"),
        ("public_surface_expansion", "declared_only"),
    ] {
        exact_string(contract, key, value)?;
        add(
            "contract_management",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }

    for key in [
        "require_local_links",
        "require_registered_commands",
        "require_config_schema",
        "require_generated_provenance",
    ] {
        exact_boolean(docs, key, true)?;
        add(
            "docs_validation",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins,
        );
    }
    exact_boolean(docs, "allow_safe_example_execution", false)?;
    add(
        "docs_validation",
        "allow_safe_example_execution",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::FalseWins,
    );

    exact_boolean(doctor, "read_only", true)?;
    exact_boolean(doctor, "collect_environment_values", false)?;
    add(
        "doctor",
        "read_only",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::Immutable,
    );
    add(
        "doctor",
        "collect_environment_values",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::Immutable,
    );
    for key in ["network_action", "package_action", "system_setting_action"] {
        exact_string(doctor, key, "diagnose_only")?;
        add(
            "doctor",
            key,
            ConfigValueV1::String("diagnose_only".to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for (key, value) in [
        ("probe_timeout_ms", 30_000),
        ("max_output_bytes", 1_048_576),
    ] {
        exact_integer(doctor, key, value)?;
        add(
            "doctor",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MinimumLimit,
        );
    }

    let reruns = bounded_integer(failure, "max_rerun_attempts", 3, 3)?;
    add(
        "failure_reproduction",
        "max_rerun_attempts",
        ConfigValueV1::Integer(reruns),
        ConfigMergeStrategyV1::MinimumLimit,
    );
    for key in ["require_structured_args", "require_before_after"] {
        exact_boolean(failure, key, true)?;
        add(
            "failure_reproduction",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for (key, value) in [
        ("external_condition", "record_unverified"),
        ("default_artifact_role", "general_log"),
        ("unsafe_artifact", "quarantine_or_drop"),
    ] {
        exact_string(failure, key, value)?;
        add(
            "failure_reproduction",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    exact_string(failure, "debugger_action", "prompt")?;
    add(
        "failure_reproduction",
        "debugger_action",
        ConfigValueV1::String("prompt".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );

    for key in ["require_source_provenance", "require_freshness"] {
        exact_boolean(security, key, true)?;
        add(
            "security_supply_chain",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for (key, value) in [
        ("unknown_freshness_action", "block_required_check"),
        ("default_report_artifacts", "redacted_only"),
    ] {
        exact_string(security, key, value)?;
        add(
            "security_supply_chain",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    exact_string(security, "network_refresh", "prompt")?;
    let max_age = bounded_integer(security, "default_max_age_hours", 168, 168)?;
    add(
        "security_supply_chain",
        "network_refresh",
        ConfigValueV1::String("prompt".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    add(
        "security_supply_chain",
        "default_max_age_hours",
        ConfigValueV1::Integer(max_age),
        ConfigMergeStrategyV1::MinimumLimit,
    );

    for (key, value) in [
        ("default_stop", "awaiting_apply_approval"),
        ("lockfile_owner", "package_manager"),
        ("preview_workspace", "isolated"),
    ] {
        exact_string(dependency, key, value)?;
        add(
            "dependency_maintenance",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for key in ["network_action", "download_action", "change_action"] {
        exact_string(dependency, key, "prompt")?;
        add(
            "dependency_maintenance",
            key,
            ConfigValueV1::String("prompt".to_owned()),
            ConfigMergeStrategyV1::MostRestrictive,
        );
    }
    for key in ["preserve_before_lockfile", "require_actual_diff_replan"] {
        exact_boolean(dependency, key, true)?;
        add(
            "dependency_maintenance",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    exact_string(radar, "sort_policy", "risk_freshness_evidence_v1")?;
    exact_boolean(radar, "include_expiring_suppressions", true)?;
    exact_boolean(radar, "allow_ai_priority", false)?;
    add(
        "maintenance_radar",
        "sort_policy",
        ConfigValueV1::String("risk_freshness_evidence_v1".to_owned()),
        ConfigMergeStrategyV1::Immutable,
    );
    add(
        "maintenance_radar",
        "include_expiring_suppressions",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::TrueWins,
    );
    add(
        "maintenance_radar",
        "allow_ai_priority",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::Immutable,
    );

    let migration_strategy = enum_string(
        migration,
        "default_strategy",
        "side_by_side",
        &["side_by_side", "atomic_replace", "transactional_in_place"],
    )?;
    add(
        "migration",
        "default_strategy",
        ConfigValueV1::String(migration_strategy),
        ConfigMergeStrategyV1::Replace,
    );
    for key in [
        "require_dry_run",
        "require_consistent_backup",
        "require_restore_rehearsal",
        "require_migration_rehearsal",
    ] {
        exact_boolean(migration, key, true)?;
        add(
            "migration",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    exact_string(migration, "unknown_field_action", "block_unless_preserved")?;
    add(
        "migration",
        "unknown_field_action",
        ConfigValueV1::String("block_unless_preserved".to_owned()),
        ConfigMergeStrategyV1::Immutable,
    );
    for key in ["live_execute_action", "rollback_action"] {
        let value = enum_string(migration, key, "prompt", &["prompt", "deny"])?;
        add(
            "migration",
            key,
            ConfigValueV1::String(value),
            ConfigMergeStrategyV1::MostRestrictive,
        );
    }
    exact_string(migration, "destructive_action", "prompt")?;
    add(
        "migration",
        "destructive_action",
        ConfigValueV1::String("prompt".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    for (key, default) in [("max_resume_attempts", 3), ("max_additional_rehearsals", 2)] {
        let value = bounded_integer(migration, key, default, default)?;
        add(
            "migration",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MinimumLimit,
        );
    }

    exact_boolean(performance, "enabled_by_default", false)?;
    exact_boolean(performance, "require_declared_workload", true)?;
    add(
        "performance_build",
        "enabled_by_default",
        ConfigValueV1::Boolean(false),
        ConfigMergeStrategyV1::Immutable,
    );
    add(
        "performance_build",
        "require_declared_workload",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::Immutable,
    );
    let warmups = integer_range(performance, "default_warmup_runs", 1, 0, 100)?;
    let measurements = integer_range(performance, "default_measurement_runs", 5, 3, 100)?;
    exact_integer(performance, "minimum_measurement_runs", 3)?;
    let additional = bounded_integer(performance, "max_additional_runs", 5, 5)?;
    for (key, value, strategy) in [
        (
            "default_warmup_runs",
            warmups,
            ConfigMergeStrategyV1::Replace,
        ),
        (
            "default_measurement_runs",
            measurements,
            ConfigMergeStrategyV1::Replace,
        ),
        (
            "minimum_measurement_runs",
            3,
            ConfigMergeStrategyV1::Immutable,
        ),
        (
            "max_additional_runs",
            additional,
            ConfigMergeStrategyV1::MinimumLimit,
        ),
    ] {
        add(
            "performance_build",
            key,
            ConfigValueV1::Integer(value),
            strategy,
        );
    }
    for (key, value) in [
        ("outlier_policy", "predeclared_report_both"),
        ("missing_measurement_action", "inconclusive"),
    ] {
        exact_string(performance, key, value)?;
        add(
            "performance_build",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    exact_boolean(performance, "require_exact_environment", true)?;
    add(
        "performance_build",
        "require_exact_environment",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::Immutable,
    );
    exact_string(performance, "profiler_action", "prompt")?;
    add(
        "performance_build",
        "profiler_action",
        ConfigValueV1::String("prompt".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );

    for (key, expected) in [
        ("require_behavior_contract", true),
        ("compile_only_equivalence", false),
        ("allow_full_auto_translation_claim", false),
    ] {
        exact_boolean(language, key, expected)?;
        add(
            "language_platform_migration",
            key,
            ConfigValueV1::Boolean(expected),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for (key, value) in [
        ("unknown_semantics_action", "human_review"),
        ("compatibility_window", "finite_required"),
        ("unsupported_platform_action", "record_unverified"),
    ] {
        exact_string(language, key, value)?;
        add(
            "language_platform_migration",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    let cutover = enum_string(language, "cutover_action", "prompt", &["prompt", "deny"])?;
    add(
        "language_platform_migration",
        "cutover_action",
        ConfigValueV1::String(cutover),
        ConfigMergeStrategyV1::MostRestrictive,
    );

    for (key, value) in [
        ("promotion_mode", "build_once"),
        ("arm64_support_tier", "preview"),
        ("arm64_runtime_verification", "native_unverified"),
    ] {
        exact_string(release, key, value)?;
        add(
            "release",
            key,
            ConfigValueV1::String(value.to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    for key in [
        "require_clean_windows",
        "require_native_x64_runtime",
        "require_explicit_remote_action_approval",
    ] {
        exact_boolean(release, key, true)?;
        add(
            "release",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    let publish_action = enum_string(release, "publish_action", "prompt", &["prompt", "deny"])?;
    add(
        "release",
        "publish_action",
        ConfigValueV1::String(publish_action),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    for key in ["deploy_action", "withdraw_action", "rollback_action"] {
        exact_string(release, key, "prompt")?;
        add(
            "release",
            key,
            ConfigValueV1::String("prompt".to_owned()),
            ConfigMergeStrategyV1::MostRestrictive,
        );
    }
    let release_jobs = bounded_integer(release, "max_parallel_target_jobs", 1, 1)?;
    add(
        "release",
        "max_parallel_target_jobs",
        ConfigValueV1::Integer(release_jobs),
        ConfigMergeStrategyV1::MinimumLimit,
    );

    let evaluation_mode = enum_string(
        evaluation,
        "default_mode",
        "shadow",
        &["offline", "replay", "shadow"],
    )?;
    add(
        "evaluation",
        "default_mode",
        ConfigValueV1::String(evaluation_mode),
        ConfigMergeStrategyV1::Replace,
    );
    for key in ["separate_cli_codex_contexts", "provider_verified_cost_only"] {
        exact_boolean(evaluation, key, true)?;
        add(
            "evaluation",
            key,
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable,
        );
    }
    let case_attempts = bounded_integer(evaluation, "max_attempts_per_case", 3, 3)?;
    exact_string(evaluation, "incomparable_action", "needs_review")?;
    add(
        "evaluation",
        "max_attempts_per_case",
        ConfigValueV1::Integer(case_attempts),
        ConfigMergeStrategyV1::MinimumLimit,
    );
    add(
        "evaluation",
        "incomparable_action",
        ConfigValueV1::String("needs_review".to_owned()),
        ConfigMergeStrategyV1::Immutable,
    );

    exact_absent(rust_style, "required_profile_ref")?;
    exact_empty_array(rust_style, "auto_apply_grant_refs")?;
    exact_absent(rust_style, "max_preview_retention")?;
    for key in [
        "network_action",
        "unpinned_apply_action",
        "partial_coverage_apply_action",
    ] {
        exact_string(rust_style, key, "deny")?;
        add(
            "rust_style",
            key,
            ConfigValueV1::String("deny".to_owned()),
            ConfigMergeStrategyV1::Immutable,
        );
    }

    exact_string(management, "integrity_check_on_unclean_start", "full")?;
    exact_boolean(management, "allow_read_only_recovery", true)?;
    exact_boolean(management, "backup_before_migration", true)?;
    exact_string(management, "baseline_activation", "explicit_review")?;
    add(
        "management",
        "integrity_check_on_unclean_start",
        ConfigValueV1::String("full".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    add(
        "management",
        "allow_read_only_recovery",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::Immutable,
    );
    add(
        "management",
        "backup_before_migration",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::Immutable,
    );
    add(
        "management",
        "baseline_activation",
        ConfigValueV1::String("explicit_review".to_owned()),
        ConfigMergeStrategyV1::Immutable,
    );
    exact_boolean(management, "auto_migrate_rebuildable", true)?;
    add(
        "management",
        "auto_migrate_rebuildable",
        ConfigValueV1::Boolean(true),
        ConfigMergeStrategyV1::FalseWins,
    );
    for (key, default) in [
        ("keep_latest_successful_scans", 2),
        ("incomplete_staging_retention_days", 7),
        ("scan_detail_retention_days", 90),
    ] {
        let value = integer_range(management, key, default, default, 3_650)?;
        add(
            "management",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MaximumFloor,
        );
    }
    for (key, value) in [
        ("resolved_finding_retention_days", 180),
        ("local_decision_retention_days", 180),
        ("migration_backup_min_count", 2),
    ] {
        exact_integer(management, key, value)?;
        add(
            "management",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MaximumFloor,
        );
    }
    exact_integer(management, "suppression_default_expiry_days", 90)?;
    add(
        "management",
        "suppression_default_expiry_days",
        ConfigValueV1::Integer(90),
        ConfigMergeStrategyV1::MinimumLimit,
    );

    let use_worktree = boolean(vcs, "use_worktree", true)?;
    let merge_strategy = enum_string(
        vcs,
        "merge_strategy",
        "review_then_merge",
        &["review_then_merge", "manual", "never"],
    )?;
    exact_empty_array(vcs, "protected_branches")?;
    exact_absent(vcs, "worktree_root")?;
    add(
        "vcs",
        "use_worktree",
        ConfigValueV1::Boolean(use_worktree),
        ConfigMergeStrategyV1::FalseWins,
    );
    add(
        "vcs",
        "merge_strategy",
        ConfigValueV1::String(merge_strategy),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    add(
        "vcs",
        "protected_branches",
        ConfigValueV1::StringSet(Vec::new()),
        ConfigMergeStrategyV1::Union,
    );
    for (key, default) in [
        ("max_parallel_projects", 2),
        ("max_active_worktrees", 4),
        ("max_parallel_mutations_per_repository", 1),
        ("max_parallel_local_merges", 1),
        ("max_merge_queue_entries", 64),
    ] {
        let value = bounded_integer(vcs, key, default, default)?;
        add(
            "vcs",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MinimumLimit,
        );
    }
    let disk_limit = vcs
        .get("worktree_disk_limit_bytes")
        .map(|_| integer_range(vcs, "worktree_disk_limit_bytes", 1, 1, u64::MAX))
        .transpose()?;
    add(
        "vcs",
        "worktree_disk_limit_bytes",
        ConfigValueV1::Json(
            disk_limit
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
        ),
        ConfigMergeStrategyV1::MinimumLimit,
    );

    let allowed_hosts = string_values(remote, "allowed_hosts", &[], 128)?;
    let require_clean_target = boolean(remote, "require_clean_target", true)?;
    exact_empty_array(remote, "personal_auto_write_scopes")?;
    let remote_writes = bounded_integer(remote, "max_parallel_writes", 1, 1)?;
    add(
        "remote",
        "allowed_hosts",
        ConfigValueV1::StringSet(allowed_hosts),
        ConfigMergeStrategyV1::Intersection,
    );
    add(
        "remote",
        "require_clean_target",
        ConfigValueV1::Boolean(require_clean_target),
        ConfigMergeStrategyV1::TrueWins,
    );
    add(
        "remote",
        "max_parallel_writes",
        ConfigValueV1::Integer(remote_writes),
        ConfigMergeStrategyV1::MinimumLimit,
    );

    exact_string(state, "artifact_root", ".ai-runs/star-control")?;
    exact_empty_array(state, "redaction_rules_add")?;
    exact_string(state, "cleanup_trigger", "startup_and_manual")?;
    add(
        "state",
        "artifact_root",
        ConfigValueV1::String(".ai-runs/star-control".to_owned()),
        ConfigMergeStrategyV1::Immutable,
    );
    for (key, value) in [
        ("checkpoint_interval_ms", 300_000),
        ("completed_retention_days", 90),
        ("failed_retention_days", 180),
    ] {
        exact_integer(state, key, value)?;
        add(
            "state",
            key,
            ConfigValueV1::Integer(value),
            ConfigMergeStrategyV1::MinimumLimit,
        );
    }
    add(
        "state",
        "cleanup_trigger",
        ConfigValueV1::String("startup_and_manual".to_owned()),
        ConfigMergeStrategyV1::MostRestrictive,
    );
    Ok(entries)
}

impl Default for UserExecutionConfig {
    fn default() -> Self {
        Self::resolve(None).expect("built-in execution configuration must be valid")
    }
}

impl UserExecutionConfig {
    pub fn load(appdata: &Path) -> Result<Self, PolicyProfileError> {
        let table = load_table(appdata)?;
        Self::resolve(table.as_ref())
    }

    pub fn load_for_project_and_layers(
        appdata: &Path,
        project_root: &Path,
        layers: impl IntoIterator<Item = ConfigLayerV1>,
    ) -> Result<Self, PolicyProfileError> {
        let mut config = Self::load(appdata)?;
        if let Some(project) = load_project_layer(project_root)? {
            config = config.apply_layer(project)?;
        }
        for layer in layers {
            config = config.apply_layer(layer)?;
        }
        Ok(config)
    }

    pub fn apply_layer(self, layer: ConfigLayerV1) -> Result<Self, PolicyProfileError> {
        let effective = self
            .effective
            .apply_layer(layer)
            .map_err(|_| PolicyProfileError::Unsupported)?;
        Self::from_effective(effective)
    }

    fn from_effective(effective: EffectiveConfigV1) -> Result<Self, PolicyProfileError> {
        effective
            .verify()
            .map_err(|_| PolicyProfileError::Unsupported)?;
        let boolean = |key: &str| {
            effective
                .boolean(key)
                .ok_or(PolicyProfileError::Unsupported)
        };
        let integer = |key: &str| {
            effective
                .integer(key)
                .ok_or(PolicyProfileError::Unsupported)
        };
        let usize_value =
            |key: &str| usize::try_from(integer(key)?).map_err(|_| PolicyProfileError::Unsupported);
        let string = |key: &str| {
            effective
                .string(key)
                .map(str::to_owned)
                .ok_or(PolicyProfileError::Unsupported)
        };
        let string_set = |key: &str| {
            effective
                .string_set(key)
                .map(<[String]>::to_vec)
                .ok_or(PolicyProfileError::Unsupported)
        };
        let parse_tier = |value: &str| match value {
            "text" => Ok(IndexTier::Text),
            "syntax" => Ok(IndexTier::Syntax),
            "semantic" => Ok(IndexTier::Semantic),
            _ => Err(PolicyProfileError::Unsupported),
        };

        let default_work_profile = string_set("default_work_profile")?;
        if default_work_profile.len() > 1
            || default_work_profile
                .iter()
                .any(|value| !supported_work_profile(value))
        {
            return Err(PolicyProfileError::Unsupported);
        }

        let include_path_patterns = string_set("scan.include_paths")?;
        let exclude_path_patterns = string_set("scan.exclude_paths_add")?;
        if include_path_patterns
            .iter()
            .chain(exclude_path_patterns.iter())
            .any(|pattern| !valid_project_relative_glob(pattern))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        let scan_policy = ScanPolicy {
            include_untracked: boolean("scan.include_untracked")?,
            include_ignored: boolean("scan.include_ignored")?,
            follow_symlinks: boolean("scan.follow_symlinks")?,
            binary_mode: string("scan.binary_mode")?,
            max_file_bytes: integer("scan.max_file_bytes")?,
            max_files: usize_value("scan.max_files")?,
            max_total_bytes: integer("scan.max_total_bytes")?,
            max_parallel_files: usize_value("scan.max_parallel_files")?,
            include_path_patterns,
            exclude_path_patterns,
            excluded_relative_roots: Vec::new(),
        };
        let required_tier = parse_tier(&string("index.required_tier")?)?;
        let max_tier = parse_tier(&string("index.max_tier")?)?;
        if required_tier > max_tier {
            return Err(PolicyProfileError::Unsupported);
        }
        let index_policy = IndexPolicy {
            required_tier,
            max_tier,
            fallback_to_lower_tier: boolean("index.fallback_to_lower_tier")?,
            max_symbols: usize_value("index.max_symbols")?,
            max_references: usize_value("index.max_references")?,
            max_graph_edges: usize_value("index.max_graph_edges")?,
            cross_project_edges: boolean("index.cross_project_edges")?,
            hardcoding_rules_enabled: boolean("scan.hardcoding_rules_enabled")?,
            hardcoding_include_tests: boolean("scan.hardcoding_include_tests")?,
            hardcoding_include_fixtures: boolean("scan.hardcoding_include_fixtures")?,
            hardcoding_include_docs_examples: boolean("scan.hardcoding_include_docs_examples")?,
            hardcoding_include_generated: boolean("scan.hardcoding_include_generated")?,
            hardcoding_include_vendor: boolean("scan.hardcoding_include_vendor")?,
            ..IndexPolicy::default()
        };
        let planning_policy = PlanningPolicy {
            effective_config_fingerprint: effective.config_fingerprint.clone(),
            max_depth: u32::try_from(integer("change_planning.max_graph_depth")?)
                .map_err(|_| PolicyProfileError::Unsupported)?,
            max_nodes: usize_value("change_planning.max_graph_nodes")?,
            max_edges: usize_value("change_planning.max_graph_edges")?,
            max_downstream_projects: usize_value("change_planning.max_downstream_projects")?,
            max_check_candidates: usize_value("change_planning.max_check_candidates")?,
            max_parallel_checks: u32::try_from(integer("validation.max_parallel_checks")?)
                .map_err(|_| PolicyProfileError::Unsupported)?,
            command_timeout_ms: integer("validation.command_timeout_ms")?,
            max_log_bytes: integer("validation.max_log_bytes")?,
            max_artifact_bytes: integer("budgets.max_artifact_bytes")?,
            allow_cross_project_read: boolean("change_planning.allow_cross_project_read")?,
            allow_previous_success_reuse: boolean("change_planning.allow_previous_success_reuse")?,
        };
        Ok(Self {
            scan_incremental: boolean("scan.incremental")?,
            scan_policy,
            index_policy,
            index_cache_enabled: boolean("index_cache.enabled")?,
            index_cache_max_total_bytes: integer("index_cache.max_total_bytes")?,
            index_cache_retention_days: integer("index_cache.retention_days")?,
            planning_policy,
            effective,
        })
    }

    fn resolve(table: Option<&toml::Table>) -> Result<Self, PolicyProfileError> {
        let empty = toml::Table::new();
        let root = table.unwrap_or(&empty);
        let user_revision = table
            .map(|table| {
                canonical_sha256(
                    &serde_json::to_value(table).map_err(|_| PolicyProfileError::Unsupported)?,
                )
                .map_err(|_| PolicyProfileError::Unsupported)
            })
            .transpose()?;
        let policy_profile_id = match root
            .get("policy_profile")
            .and_then(toml::Value::as_str)
            .unwrap_or("star.policy-profile.safe-default")
        {
            "safe_default" | "star.policy-profile.safe-default" => {
                "star.policy-profile.safe-default"
            }
            "personal_auto" | "star.policy-profile.personal-auto" => {
                "star.policy-profile.personal-auto"
            }
            _ => return Err(PolicyProfileError::Unsupported),
        };
        let validation = section(root, "validation", VALIDATION_KEYS)?.unwrap_or(&empty);
        let discovery =
            section(root, "project_discovery", PROJECT_DISCOVERY_KEYS)?.unwrap_or(&empty);
        let scan = section(root, "scan", SCAN_KEYS)?.unwrap_or(&empty);
        let index = section(root, "index", INDEX_KEYS)?.unwrap_or(&empty);
        let index_cache = section(root, "index_cache", INDEX_CACHE_KEYS)?.unwrap_or(&empty);
        let planning = section(root, "change_planning", CHANGE_PLANNING_KEYS)?.unwrap_or(&empty);

        // M3 settings whose alternative behavior is not representable by the
        // current runner are accepted only at the enforced product value. This
        // prevents an accepted-but-ignored weakening while max_parallel_checks
        // is materialized into the M2 ValidationPlan budget below.
        for (key, expected) in [
            ("allow_manual_evidence", true),
            ("require_current_evidence", true),
            ("allow_ratchet_satisfaction", true),
            ("suppression_requires_expiry", true),
            ("allow_permanent_suppressions", false),
        ] {
            exact_boolean(validation, key, expected)?;
        }
        exact_string(validation, "baseline_mode", "ratchet_new_and_worsened")?;
        exact_string(validation, "required_flaky_action", "human_review")?;
        exact_string(validation, "cli_only_semantic_review", "human_review")?;
        exact_empty_array(validation, "checks_add")?;
        exact_empty_array(validation, "checks_remove")?;
        let required_phases = string_values(validation, "required_phases", &["stage", "goal"], 4)?;
        if required_phases != ["goal", "stage"] {
            return Err(PolicyProfileError::Unsupported);
        }
        let review_risks = string_values(
            validation,
            "require_independent_review_for",
            &["high", "critical"],
            4,
        )?;
        if review_risks != ["critical", "high"] {
            return Err(PolicyProfileError::Unsupported);
        }
        let fail_on = enum_string(validation, "fail_on", "error", &["error"])?;
        let validation_timeout =
            bounded_integer(validation, "command_timeout_ms", 600_000, 600_000)?;
        let max_log_bytes = bounded_integer(validation, "max_log_bytes", 10_485_760, 10_485_760)?;
        let max_parallel_checks = bounded_integer(validation, "max_parallel_checks", 4, 4)? as u32;

        // M1 discovery is currently explicit-root only. Non-default recursive
        // discovery choices are rejected until they can affect ownership.
        for (key, expected) in [
            ("detect_nested_repositories", true),
            ("detect_linked_worktrees", true),
            ("detect_workspaces", true),
            ("detect_non_git", true),
            ("follow_symlinks", false),
            ("search_ignored_subtrees", false),
        ] {
            exact_boolean(discovery, key, expected)?;
        }
        exact_empty_array(discovery, "roots_add")?;
        exact_empty_array(discovery, "exclude_paths_add")?;
        exact_integer(discovery, "max_depth", 16)?;
        exact_integer(discovery, "max_directories", 100_000)?;

        let scan_incremental = boolean(scan, "incremental", true)?;
        let include_path_patterns = string_values(scan, "include_paths", &[], 256)?;
        let mut exclude_path_patterns = string_values(scan, "exclude_paths_add", &[], 256)?;
        exclude_path_patterns.extend([".ai-runs/**".to_owned(), ".git/**".to_owned()]);
        exclude_path_patterns.sort();
        exclude_path_patterns.dedup();
        if include_path_patterns
            .iter()
            .chain(exclude_path_patterns.iter())
            .any(|pattern| !valid_project_relative_glob(pattern))
        {
            return Err(PolicyProfileError::Unsupported);
        }
        let scan_policy = ScanPolicy {
            include_untracked: boolean(scan, "include_untracked", true)?,
            include_ignored: boolean(scan, "include_ignored", false)?,
            follow_symlinks: boolean(scan, "follow_symlinks", false)?,
            binary_mode: enum_string(
                scan,
                "binary_mode",
                "metadata_only",
                &["skip", "metadata_only"],
            )?,
            max_file_bytes: bounded_integer(scan, "max_file_bytes", 16_777_216, 16_777_216)?,
            max_files: bounded_integer(scan, "max_files", 200_000, 200_000)? as usize,
            max_total_bytes: bounded_integer(
                scan,
                "max_total_bytes",
                8_589_934_592,
                8_589_934_592,
            )?,
            max_parallel_files: bounded_integer(scan, "max_parallel_files", 4, 4)? as usize,
            include_path_patterns,
            exclude_path_patterns,
            excluded_relative_roots: Vec::new(),
        };
        exact_boolean(scan, "require_complete_for_gate", true)?;
        exact_string(scan, "rule_error_policy", "mark_incomplete")?;
        exact_empty_array(scan, "classification_rules_add")?;
        exact_empty_array(scan, "rule_sets_add")?;
        exact_empty_array(scan, "rule_sets_remove")?;

        let required_tier = enum_string(
            index,
            "required_tier",
            "text",
            &["text", "syntax", "semantic"],
        )?;
        let max_tier = enum_string(
            index,
            "max_tier",
            "semantic",
            &["text", "syntax", "semantic"],
        )?;
        let parse_tier = |value: &str| match value {
            "text" => Ok(IndexTier::Text),
            "syntax" => Ok(IndexTier::Syntax),
            "semantic" => Ok(IndexTier::Semantic),
            _ => Err(PolicyProfileError::Unsupported),
        };
        let required_tier = parse_tier(&required_tier)?;
        let max_tier = parse_tier(&max_tier)?;
        if required_tier > max_tier {
            return Err(PolicyProfileError::Unsupported);
        }
        let index_policy = IndexPolicy {
            required_tier,
            max_tier,
            fallback_to_lower_tier: boolean(index, "fallback_to_lower_tier", true)?,
            max_symbols: bounded_integer(index, "max_symbols", 5_000_000, 5_000_000)? as usize,
            max_references: bounded_integer(index, "max_references", 20_000_000, 20_000_000)?
                as usize,
            max_graph_edges: bounded_integer(index, "max_graph_edges", 25_000_000, 25_000_000)?
                as usize,
            cross_project_edges: boolean(index, "cross_project_edges", true)?,
            hardcoding_rules_enabled: boolean(scan, "hardcoding_rules_enabled", true)?,
            hardcoding_include_tests: boolean(scan, "hardcoding_include_tests", false)?,
            hardcoding_include_fixtures: boolean(scan, "hardcoding_include_fixtures", false)?,
            hardcoding_include_docs_examples: boolean(
                scan,
                "hardcoding_include_docs_examples",
                false,
            )?,
            hardcoding_include_generated: boolean(scan, "hardcoding_include_generated", false)?,
            hardcoding_include_vendor: boolean(scan, "hardcoding_include_vendor", false)?,
            ..IndexPolicy::default()
        };
        let index_cache_enabled = boolean(index_cache, "enabled", true)?;
        let index_cache_max_total_bytes =
            bounded_integer(index_cache, "max_total_bytes", 2_147_483_648, 2_147_483_648)?;
        let index_cache_retention_days = bounded_integer(index_cache, "retention_days", 30, 30)?;
        exact_boolean(index_cache, "reuse_partial", false)?;
        exact_boolean(index_cache, "store_source_bytes", false)?;

        exact_boolean(planning, "require_current_inputs", true)?;
        exact_boolean(
            planning,
            "require_user_acceptance_for_change_scope_expansion",
            true,
        )?;
        let mut planning_policy = PlanningPolicy {
            effective_config_fingerprint: Sha256Hash::digest(b"star.effective-config.pending"),
            max_depth: bounded_integer(planning, "max_graph_depth", 8, 8)? as u32,
            max_nodes: bounded_integer(planning, "max_graph_nodes", 100_000, 100_000)? as usize,
            max_edges: bounded_integer(planning, "max_graph_edges", 500_000, 500_000)? as usize,
            max_downstream_projects: bounded_integer(planning, "max_downstream_projects", 64, 64)?
                as usize,
            max_check_candidates: bounded_integer(planning, "max_check_candidates", 2_048, 2_048)?
                as usize,
            max_parallel_checks,
            command_timeout_ms: validation_timeout,
            max_log_bytes,
            max_artifact_bytes: 1_073_741_824,
            allow_cross_project_read: boolean(planning, "allow_cross_project_read", true)?,
            allow_previous_success_reuse: boolean(planning, "allow_previous_success_reuse", true)?,
        };

        let mut entries =
            common_policy_entries(root, &empty, user_revision.as_ref(), policy_profile_id)?;
        macro_rules! add {
            ($section:expr, $name:literal, $value:expr, $merge:expr) => {{
                entries.push(effective_entry(
                    concat!($section, ".", $name),
                    $value,
                    $merge,
                    user_revision.as_ref(),
                    root.get($section)
                        .and_then(toml::Value::as_table)
                        .is_some_and(|table| table.contains_key($name)),
                ));
            }};
        }
        add!(
            "validation",
            "required_phases",
            ConfigValueV1::StringSet(required_phases),
            ConfigMergeStrategyV1::Union
        );
        add!(
            "validation",
            "fail_on",
            ConfigValueV1::String(fail_on),
            ConfigMergeStrategyV1::MostRestrictive
        );
        add!(
            "validation",
            "command_timeout_ms",
            ConfigValueV1::Integer(validation_timeout),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "validation",
            "max_log_bytes",
            ConfigValueV1::Integer(max_log_bytes),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "validation",
            "max_parallel_checks",
            ConfigValueV1::Integer(u64::from(max_parallel_checks)),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "scan",
            "incremental",
            ConfigValueV1::Boolean(scan_incremental),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "scan",
            "include_untracked",
            ConfigValueV1::Boolean(scan_policy.include_untracked),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "include_ignored",
            ConfigValueV1::Boolean(scan_policy.include_ignored),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "follow_symlinks",
            ConfigValueV1::Boolean(scan_policy.follow_symlinks),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "scan",
            "binary_mode",
            ConfigValueV1::String(scan_policy.binary_mode.clone()),
            ConfigMergeStrategyV1::MostRestrictive
        );
        add!(
            "scan",
            "max_file_bytes",
            ConfigValueV1::Integer(scan_policy.max_file_bytes),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "scan",
            "max_files",
            ConfigValueV1::Integer(scan_policy.max_files as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "scan",
            "max_total_bytes",
            ConfigValueV1::Integer(scan_policy.max_total_bytes),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "scan",
            "max_parallel_files",
            ConfigValueV1::Integer(scan_policy.max_parallel_files as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "scan",
            "include_paths",
            ConfigValueV1::StringSet(scan_policy.include_path_patterns.clone()),
            ConfigMergeStrategyV1::Intersection
        );
        add!(
            "scan",
            "exclude_paths_add",
            ConfigValueV1::StringSet(scan_policy.exclude_path_patterns.clone()),
            ConfigMergeStrategyV1::Union
        );
        add!(
            "scan",
            "hardcoding_rules_enabled",
            ConfigValueV1::Boolean(index_policy.hardcoding_rules_enabled),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "scan",
            "hardcoding_include_tests",
            ConfigValueV1::Boolean(index_policy.hardcoding_include_tests),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "hardcoding_include_fixtures",
            ConfigValueV1::Boolean(index_policy.hardcoding_include_fixtures),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "hardcoding_include_docs_examples",
            ConfigValueV1::Boolean(index_policy.hardcoding_include_docs_examples),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "hardcoding_include_generated",
            ConfigValueV1::Boolean(index_policy.hardcoding_include_generated),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "scan",
            "hardcoding_include_vendor",
            ConfigValueV1::Boolean(index_policy.hardcoding_include_vendor),
            ConfigMergeStrategyV1::ExplicitWidening
        );
        add!(
            "index",
            "required_tier",
            ConfigValueV1::String(format!("{:?}", index_policy.required_tier).to_ascii_lowercase()),
            ConfigMergeStrategyV1::MostRestrictive
        );
        add!(
            "index",
            "max_tier",
            ConfigValueV1::String(format!("{:?}", index_policy.max_tier).to_ascii_lowercase()),
            ConfigMergeStrategyV1::MostRestrictive
        );
        add!(
            "index",
            "fallback_to_lower_tier",
            ConfigValueV1::Boolean(index_policy.fallback_to_lower_tier),
            ConfigMergeStrategyV1::Immutable
        );
        add!(
            "index",
            "max_symbols",
            ConfigValueV1::Integer(index_policy.max_symbols as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "index",
            "max_references",
            ConfigValueV1::Integer(index_policy.max_references as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "index",
            "max_graph_edges",
            ConfigValueV1::Integer(index_policy.max_graph_edges as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "index",
            "cross_project_edges",
            ConfigValueV1::Boolean(index_policy.cross_project_edges),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "index_cache",
            "enabled",
            ConfigValueV1::Boolean(index_cache_enabled),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "index_cache",
            "max_total_bytes",
            ConfigValueV1::Integer(index_cache_max_total_bytes),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "index_cache",
            "retention_days",
            ConfigValueV1::Integer(index_cache_retention_days),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "require_current_inputs",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::TrueWins
        );
        add!(
            "change_planning",
            "max_graph_depth",
            ConfigValueV1::Integer(u64::from(planning_policy.max_depth)),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "max_graph_nodes",
            ConfigValueV1::Integer(planning_policy.max_nodes as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "max_graph_edges",
            ConfigValueV1::Integer(planning_policy.max_edges as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "max_downstream_projects",
            ConfigValueV1::Integer(planning_policy.max_downstream_projects as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "max_check_candidates",
            ConfigValueV1::Integer(planning_policy.max_check_candidates as u64),
            ConfigMergeStrategyV1::MinimumLimit
        );
        add!(
            "change_planning",
            "allow_cross_project_read",
            ConfigValueV1::Boolean(planning_policy.allow_cross_project_read),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "change_planning",
            "allow_previous_success_reuse",
            ConfigValueV1::Boolean(planning_policy.allow_previous_success_reuse),
            ConfigMergeStrategyV1::FalseWins
        );
        add!(
            "change_planning",
            "require_user_acceptance_for_change_scope_expansion",
            ConfigValueV1::Boolean(true),
            ConfigMergeStrategyV1::Immutable
        );
        entries.extend(later_stage_policy_entries(
            root,
            &empty,
            user_revision.as_ref(),
        )?);

        let effective = EffectiveConfigV1::seal(policy_profile_id, entries)
            .map_err(|_| PolicyProfileError::Unsupported)?;
        effective
            .verify()
            .map_err(|_| PolicyProfileError::Unsupported)?;
        planning_policy.effective_config_fingerprint = effective.config_fingerprint.clone();
        planning_policy.max_artifact_bytes = effective
            .integer("budgets.max_artifact_bytes")
            .ok_or(PolicyProfileError::Unsupported)?;
        Ok(Self {
            effective,
            scan_incremental,
            scan_policy,
            index_policy,
            index_cache_enabled,
            index_cache_max_total_bytes,
            index_cache_retention_days,
            planning_policy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        std::env::current_dir()
            .unwrap()
            .join(".ai-runs/star-control/test-policy")
            .join(format!("{name}-{}", star_ipc::nonce()))
    }

    fn write(root: &Path, value: &str) {
        let directory = root.join("Star-Control");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("config.toml"), value).unwrap();
    }

    fn write_project(root: &Path, value: &str) {
        let directory = root.join(".star-control");
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

    #[test]
    fn m1_to_m3_execution_settings_materialize_into_typed_policies() {
        let directory = root("execution-values");
        write(
            &directory,
            "schema_version = 1\n[permissions]\napproval_ttl_ms = 60000\n[budgets]\ngoal_wall_time_ms = 3600000\nstage_wall_time_ms = 600000\nmax_artifact_bytes = 67108864\n[validation]\nmax_parallel_checks = 2\n[scan]\nincremental = false\ninclude_untracked = false\ninclude_paths = [\"crates/**\", \"apps/**\"]\nexclude_paths_add = [\"crates/generated/**\"]\nmax_file_bytes = 1048576\nmax_files = 1000\nmax_total_bytes = 16777216\nmax_parallel_files = 2\nhardcoding_include_tests = true\n[index]\nrequired_tier = \"syntax\"\nmax_tier = \"semantic\"\nmax_symbols = 10000\nmax_references = 20000\nmax_graph_edges = 30000\ncross_project_edges = false\n[index_cache]\nmax_total_bytes = 268435456\nretention_days = 7\n[change_planning]\nmax_graph_depth = 4\nmax_graph_nodes = 10000\nmax_graph_edges = 20000\nmax_downstream_projects = 8\nmax_check_candidates = 128\nallow_cross_project_read = false\nallow_previous_success_reuse = false\n",
        );
        let config = UserExecutionConfig::load(&directory).unwrap();
        config.effective.verify().unwrap();
        assert!(!config.scan_incremental);
        assert!(!config.scan_policy.include_untracked);
        assert_eq!(config.scan_policy.max_file_bytes, 1_048_576);
        assert_eq!(
            config.scan_policy.include_path_patterns,
            ["apps/**", "crates/**"]
        );
        assert!(
            config
                .scan_policy
                .exclude_path_patterns
                .contains(&"crates/generated/**".to_owned())
        );
        assert!(config.index_policy.hardcoding_include_tests);
        assert_eq!(config.index_policy.required_tier, IndexTier::Syntax);
        assert_eq!(config.index_policy.max_symbols, 10_000);
        assert!(!config.index_policy.cross_project_edges);
        assert_eq!(config.index_cache_max_total_bytes, 268_435_456);
        assert_eq!(config.index_cache_retention_days, 7);
        assert_eq!(config.planning_policy.max_depth, 4);
        assert_eq!(config.planning_policy.max_nodes, 10_000);
        assert_eq!(config.planning_policy.max_parallel_checks, 2);
        assert!(!config.planning_policy.allow_cross_project_read);
        assert!(!config.planning_policy.allow_previous_success_reuse);
        assert_eq!(
            config.effective.get("change_planning.max_graph_depth"),
            Some(&ConfigValueV1::Integer(4))
        );
        assert_eq!(
            config.effective.integer("permissions.approval_ttl_ms"),
            Some(60_000)
        );
        assert_eq!(
            config.effective.integer("budgets.max_artifact_bytes"),
            Some(67_108_864)
        );
        assert_eq!(
            config.effective.config_fingerprint.as_str(),
            "sha256:9327669046808f91508844a789026749a9f59fb1d3d4812054d21f25b9f3c8f1"
        );
    }

    #[test]
    fn execution_settings_reject_unknown_keys_and_unwired_policy_weakening() {
        let unknown = root("execution-unknown");
        write(
            &unknown,
            "schema_version = 1\n[change_planning]\nunknown = true\n",
        );
        assert!(matches!(
            UserExecutionConfig::load(&unknown),
            Err(PolicyProfileError::Unsupported)
        ));

        let weakening = root("execution-weakening");
        write(
            &weakening,
            "schema_version = 1\n[validation]\nrequire_current_evidence = false\n",
        );
        assert!(matches!(
            UserExecutionConfig::load(&weakening),
            Err(PolicyProfileError::Unsupported)
        ));

        let common_unknown = root("common-unknown");
        write(
            &common_unknown,
            "schema_version = 1\n[permissions]\naccepted_but_ignored = true\n",
        );
        assert!(matches!(
            UserExecutionConfig::load(&common_unknown),
            Err(PolicyProfileError::Unsupported)
        ));

        let unsupported_common_override = root("unsupported-common-override");
        write(
            &unsupported_common_override,
            "schema_version = 1\n[routing]\ndefault_model_role = \"sol\"\n",
        );
        assert!(matches!(
            UserExecutionConfig::load(&unsupported_common_override),
            Err(PolicyProfileError::Unsupported)
        ));

        for (name, body) in [
            ("doctor-probe", "[doctor]\nprobe_timeout_ms = 1000\n"),
            (
                "failure-debugger",
                "[failure_reproduction]\ndebugger_action = \"deny\"\n",
            ),
            (
                "security-refresh",
                "[security_supply_chain]\nnetwork_refresh = \"deny\"\n",
            ),
            (
                "dependency-download",
                "[dependency_maintenance]\ndownload_action = \"deny\"\n",
            ),
            (
                "migration-destructive",
                "[migration]\ndestructive_action = \"deny\"\n",
            ),
            (
                "performance-profiler",
                "[performance_build]\nprofiler_action = \"deny\"\n",
            ),
            ("release-deploy", "[release]\ndeploy_action = \"deny\"\n"),
            (
                "management-auto-migrate",
                "[management]\nauto_migrate_rebuildable = false\n",
            ),
            (
                "management-resolved-retention",
                "[management]\nresolved_finding_retention_days = 365\n",
            ),
            (
                "management-suppression-expiry",
                "[management]\nsuppression_default_expiry_days = 30\n",
            ),
            (
                "vcs-protected-branches",
                "[vcs]\nprotected_branches = [\"main\"]\n",
            ),
            ("state-cleanup", "[state]\ncleanup_trigger = \"manual\"\n"),
        ] {
            let directory = root(name);
            write(&directory, &format!("schema_version = 1\n{body}"));
            assert!(
                matches!(
                    UserExecutionConfig::load(&directory),
                    Err(PolicyProfileError::Unsupported)
                ),
                "{name} must not be accepted until a runtime consumer exists"
            );
        }
    }

    #[test]
    fn project_goal_and_command_layers_materialize_in_order_and_reject_widening() {
        let appdata = root("layered-appdata");
        write(
            &appdata,
            "schema_version = 1\npolicy_profile = \"personal_auto\"\n[scan]\nmax_files = 1000\n",
        );
        let project = root("layered-project");
        fs::create_dir_all(&project).unwrap();
        write_project(
            &project,
            "schema_version = 1\nrequired_policy_profile = \"safe_default\"\ndefault_work_profile = \"debug_recovery\"\n[scan]\nmax_files = 500\ninclude_untracked = false\ninclude_paths = [\"crates/**\"]\n",
        );
        let goal_source = ConfigSourceRefV1 {
            source_kind: ConfigSourceKindV1::Goal,
            source_id: "goal:fixture".to_owned(),
            source_fingerprint: Sha256Hash::digest(b"goal-fixture"),
        };
        let command_source = ConfigSourceRefV1 {
            source_kind: ConfigSourceKindV1::Command,
            source_id: "command:fixture".to_owned(),
            source_fingerprint: Sha256Hash::digest(b"command-fixture"),
        };
        let config = UserExecutionConfig::load_for_project_and_layers(
            &appdata,
            &project,
            [
                ConfigLayerV1 {
                    source: goal_source.clone(),
                    overrides: vec![ConfigOverrideV1 {
                        key: "scan.max_files".to_owned(),
                        value: ConfigValueV1::Integer(250),
                    }],
                },
                ConfigLayerV1 {
                    source: command_source.clone(),
                    overrides: vec![ConfigOverrideV1 {
                        key: "scan.max_files".to_owned(),
                        value: ConfigValueV1::Integer(125),
                    }],
                },
            ],
        )
        .unwrap();
        assert_eq!(
            UserPolicyProfile::from_effective(&config.effective).unwrap(),
            UserPolicyProfile::SafeDefault
        );
        assert_eq!(config.scan_policy.max_files, 125);
        assert!(!config.scan_policy.include_untracked);
        assert_eq!(config.scan_policy.include_path_patterns, ["crates/**"]);
        assert_eq!(
            config.effective.string_set("default_work_profile"),
            Some(["debug_recovery".to_owned()].as_slice())
        );
        let max_files = config
            .effective
            .entries
            .iter()
            .find(|entry| entry.key == "scan.max_files")
            .unwrap();
        assert!(max_files.provenance.contains(&goal_source));
        assert!(max_files.provenance.contains(&command_source));
        assert!(
            max_files
                .provenance
                .iter()
                .any(|source| source.source_kind == ConfigSourceKindV1::Project)
        );
        assert!(matches!(
            UserExecutionConfig::load_for_project_and_layers(
                &appdata,
                &project,
                [ConfigLayerV1 {
                    source: command_source.clone(),
                    overrides: vec![ConfigOverrideV1 {
                        key: "default_work_profile".to_owned(),
                        value: ConfigValueV1::StringSet(vec!["unknown_profile".to_owned()]),
                    }],
                }],
            ),
            Err(PolicyProfileError::Unsupported)
        ));
        for (key, value) in [
            (
                "routing.default_model_role",
                ConfigValueV1::String("unknown".to_owned()),
            ),
            (
                "routing.unsupported_choice",
                ConfigValueV1::String("silently_fallback".to_owned()),
            ),
            (
                "migration.default_strategy",
                ConfigValueV1::String("overwrite_live".to_owned()),
            ),
            (
                "performance_build.default_warmup_runs",
                ConfigValueV1::Integer(101),
            ),
            (
                "performance_build.default_measurement_runs",
                ConfigValueV1::Integer(2),
            ),
            (
                "evaluation.default_mode",
                ConfigValueV1::String("live".to_owned()),
            ),
        ] {
            assert!(matches!(
                config.clone().apply_layer(ConfigLayerV1 {
                    source: command_source.clone(),
                    overrides: vec![ConfigOverrideV1 {
                        key: key.to_owned(),
                        value,
                    }],
                }),
                Err(PolicyProfileError::Unsupported)
            ));
        }

        let safe_appdata = root("layered-safe-appdata");
        write(&safe_appdata, "schema_version = 1\n");
        let widening_project = root("layered-widening-project");
        fs::create_dir_all(&widening_project).unwrap();
        write_project(
            &widening_project,
            "schema_version = 1\nrequired_policy_profile = \"personal_auto\"\n",
        );
        assert!(matches!(
            UserExecutionConfig::load_for_project_and_layers(
                &safe_appdata,
                &widening_project,
                std::iter::empty::<ConfigLayerV1>(),
            ),
            Err(PolicyProfileError::Unsupported)
        ));

        let forbidden_project = root("layered-forbidden-project");
        fs::create_dir_all(&forbidden_project).unwrap();
        write_project(
            &forbidden_project,
            "schema_version = 1\n[controller]\nauto_start = false\n",
        );
        assert!(matches!(
            UserExecutionConfig::load_for_project_and_layers(
                &safe_appdata,
                &forbidden_project,
                std::iter::empty::<ConfigLayerV1>(),
            ),
            Err(PolicyProfileError::Unsupported)
        ));
    }

    #[test]
    fn nested_working_directory_uses_only_the_nearest_project_config() {
        let appdata = root("nested-project-appdata");
        write(&appdata, "schema_version = 1\n[scan]\nmax_files = 1000\n");
        let outer = root("nested-project-outer");
        fs::create_dir_all(outer.join(".git")).unwrap();
        write_project(&outer, "schema_version = 1\n[scan]\nmax_files = 500\n");
        let outer_child = outer.join("crates/app/src");
        fs::create_dir_all(&outer_child).unwrap();
        let outer_config = UserExecutionConfig::load_for_project_and_layers(
            &appdata,
            &outer_child,
            std::iter::empty::<ConfigLayerV1>(),
        )
        .unwrap();
        assert_eq!(outer_config.scan_policy.max_files, 500);

        let nested = outer.join("vendor/nested");
        fs::create_dir_all(nested.join(".git")).unwrap();
        let nested_child = nested.join("src");
        fs::create_dir_all(&nested_child).unwrap();
        let nested_config = UserExecutionConfig::load_for_project_and_layers(
            &appdata,
            &nested_child,
            std::iter::empty::<ConfigLayerV1>(),
        )
        .unwrap();
        assert_eq!(nested_config.scan_policy.max_files, 1000);
        assert!(
            nested_config
                .effective
                .entries
                .iter()
                .find(|entry| entry.key == "scan.max_files")
                .unwrap()
                .provenance
                .iter()
                .all(|source| source.source_kind != ConfigSourceKindV1::Project)
        );
    }

    #[test]
    fn project_config_applies_without_a_user_config() {
        let appdata = root("project-only-appdata");
        let project = root("project-only-root");
        fs::create_dir_all(project.join(".git")).unwrap();
        write_project(&project, "schema_version = 1\n[scan]\nmax_files = 4096\n");

        let config = UserExecutionConfig::load_for_project_and_layers(
            &appdata,
            &project,
            std::iter::empty::<ConfigLayerV1>(),
        )
        .unwrap();

        assert_eq!(config.scan_policy.max_files, 4096);
        assert!(
            config
                .effective
                .entries
                .iter()
                .find(|entry| entry.key == "scan.max_files")
                .unwrap()
                .provenance
                .iter()
                .any(|source| source.source_kind == ConfigSourceKindV1::Project)
        );
    }
}
