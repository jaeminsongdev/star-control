use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    ProjectId, Sha256Hash,
    ids::WorkspaceSnapshotId,
    management::ProjectPathRef,
    rust_style::{
        RUST_STYLE_COVERAGE_MATRIX_SCHEMA_ID, RUST_STYLE_PIPELINE_ID, RUST_STYLE_PIPELINE_VERSION,
        RUST_STYLE_POLICY_SNAPSHOT_SCHEMA_ID, RUST_TOOLCHAIN_BINDING_SCHEMA_ID, RustAutoPolicy,
        RustAvailabilityState, RustCompleteness, RustCoverageExecution, RustCoveragePhase,
        RustEditionBinding, RustExecutableBinding, RustSourceBinding, RustSourceOwnership,
        RustStyleCoverageCell, RustStyleCoverageMatrix, RustStylePolicySnapshot, RustTargetKind,
        RustTargetState, RustToolchainBinding, RustToolchainPinState, RustToolchainSource,
    },
};
use star_domain::versioned_fingerprint;
use star_execution::rust_style::{
    CargoRustStyleAdapter, RustCargoScope, RustStyleAdapter, RustStyleAdapterError,
    RustStylePatchScope, RustToolOutput, effective_cargo_home, materialize_owned_preview,
    probe_direct_tool_version,
};

use crate::rust_style::{
    RustStyleCandidate, RustStyleCandidateInput, RustStyleWorkflowError,
    prepare_rust_style_candidate,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustStyleScopeKind {
    Workspace,
    Package,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustStyleScope {
    pub kind: RustStyleScopeKind,
    pub package: Option<String>,
}

impl RustStyleScope {
    pub fn workspace() -> Self {
        Self {
            kind: RustStyleScopeKind::Workspace,
            package: None,
        }
    }

    pub fn package(package: String) -> Result<Self, RustStyleRuntimeError> {
        if package.is_empty()
            || package.len() > 512
            || package.starts_with('-')
            || package.contains('\0')
            || package.chars().any(char::is_whitespace)
        {
            return Err(RustStyleRuntimeError::InvalidScope);
        }
        Ok(Self {
            kind: RustStyleScopeKind::Package,
            package: Some(package),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStyleTargetInventory {
    pub target_name: String,
    pub target_kind: RustTargetKind,
    pub source_root: ProjectPathRef,
    pub required_features: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStylePackageInventory {
    pub package_id: String,
    pub package_name: String,
    pub manifest_ref: String,
    pub edition: String,
    pub rust_version: Option<String>,
    pub declared_features: Vec<String>,
    pub targets: Vec<RustStyleTargetInventory>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStyleInspection {
    pub project_id: ProjectId,
    pub scope: RustStyleScope,
    pub binding: RustToolchainBinding,
    pub policy: RustStylePolicySnapshot,
    pub coverage: RustStyleCoverageMatrix,
    pub packages: Vec<RustStylePackageInventory>,
    pub limitations: Vec<String>,
    pub standing_grant_template: Option<serde_json::Value>,
    pub inspection_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustToolRunSummary {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout_sha256: Sha256Hash,
    pub stderr_sha256: Sha256Hash,
    pub command_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStyleCheckResult {
    pub inspection: RustStyleInspection,
    pub rustfmt: RustToolRunSummary,
    pub clippy: RustToolRunSummary,
    pub source_unchanged: bool,
    pub isolation_ref: String,
    pub check_fingerprint: Sha256Hash,
}

pub struct RustStylePreparedRuntime {
    pub inspection: RustStyleInspection,
    pub candidate: RustStyleCandidate,
    pub isolation_ref: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RustStyleRuntimeError {
    #[error("Rust style scope is invalid or ambiguous")]
    InvalidScope,
    #[error("Rust style release Catalog is invalid")]
    InvalidCatalog,
    #[error("Rust project metadata is unavailable or invalid")]
    InvalidMetadata,
    #[error("the pinned Rust 1.96 toolchain is unavailable")]
    ToolchainUnavailable,
    #[error("Rust style isolated runtime root is unsafe")]
    UnsafeRuntimeRoot,
    #[error("Rust style source/config I/O failed")]
    Io,
    #[error("Rust style fingerprint failed")]
    Fingerprint,
    #[error("Rust style adapter failed: {0}")]
    Adapter(#[from] RustStyleAdapterError),
    #[error("Rust style workflow failed: {0}")]
    Workflow(#[from] RustStyleWorkflowError),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStyleCatalogPolicy {
    schema_version: u32,
    profile_ref: String,
    pipeline_ref: String,
    toolchain_release: String,
    coverage_policy_ref: String,
    feature_policy: String,
    max_files: u32,
    max_hunks: u32,
    max_changed_bytes: u64,
    forbidden_operations: Vec<String>,
    built_in_fix_allowlist: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStyleProjectCoveragePolicy {
    schema_version: u32,
    feature_union_compatible: bool,
    packages: Vec<RustStyleProjectPackageFeatures>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStyleProjectPackageFeatures {
    package: String,
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    manifest_path: PathBuf,
    edition: String,
    rust_version: Option<String>,
    features: BTreeMap<String, Vec<String>>,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
    #[serde(default)]
    required_features: Vec<String>,
}

struct ResolvedRustStyleEnvironment {
    inspection: RustStyleInspection,
    cargo_executable: PathBuf,
    cargo_scope: RustCargoScope,
    cargo_features: Vec<String>,
    project_root: PathBuf,
    runtime_root: PathBuf,
}

pub fn inspect_rust_style(
    project_id: &ProjectId,
    project_root: &Path,
    runtime_root: &Path,
    release_policy_path: &Path,
    scope: RustStyleScope,
    auto_policy: RustAutoPolicy,
) -> Result<RustStyleInspection, RustStyleRuntimeError> {
    Ok(resolve_environment(
        project_id,
        project_root,
        runtime_root,
        release_policy_path,
        scope,
        auto_policy,
    )?
    .inspection)
}

pub fn check_rust_style(
    project_id: &ProjectId,
    project_root: &Path,
    runtime_root: &Path,
    release_policy_path: &Path,
    scope: RustStyleScope,
    auto_policy: RustAutoPolicy,
) -> Result<RustStyleCheckResult, RustStyleRuntimeError> {
    let resolved = resolve_environment(
        project_id,
        project_root,
        runtime_root,
        release_policy_path,
        scope,
        auto_policy,
    )?;
    let operation_root = unique_operation_root(
        &resolved.runtime_root,
        &resolved.project_root,
        project_id,
        "check",
    )?;
    let mirror = operation_root.join("mirror");
    materialize_owned_preview(&resolved.project_root, &mirror)?;
    let target = operation_root.join("target");
    let mut adapter = CargoRustStyleAdapter::check_only_configured(
        resolved.cargo_executable,
        mirror,
        target,
        resolved.cargo_scope,
        resolved.cargo_features,
    )?;
    let before = adapter.snapshot()?;
    let rustfmt = adapter.run_rustfmt(true)?;
    let clippy = adapter.run_clippy_check()?;
    let source_unchanged = adapter.snapshot()? == before;
    let isolation_ref = opaque_isolation_ref(project_id, &operation_root)?;
    let rustfmt = summarize_tool_output(rustfmt);
    let clippy = summarize_tool_output(clippy);
    let check_fingerprint = versioned_fingerprint(
        "star.rust-style-check-result",
        1,
        &serde_json::json!({
            "inspection_fingerprint":resolved.inspection.inspection_fingerprint,
            "rustfmt":rustfmt,
            "clippy":clippy,
            "source_unchanged":source_unchanged,
            "isolation_ref":isolation_ref,
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)?;
    Ok(RustStyleCheckResult {
        inspection: resolved.inspection,
        rustfmt,
        clippy,
        source_unchanged,
        isolation_ref,
        check_fingerprint,
    })
}

pub fn prepare_rust_style(
    project_id: &ProjectId,
    base_workspace_snapshot_id: WorkspaceSnapshotId,
    project_root: &Path,
    runtime_root: &Path,
    release_policy_path: &Path,
    scope: RustStyleScope,
    auto_policy: RustAutoPolicy,
) -> Result<RustStylePreparedRuntime, RustStyleRuntimeError> {
    let resolved = resolve_environment(
        project_id,
        project_root,
        runtime_root,
        release_policy_path,
        scope,
        auto_policy,
    )?;
    let operation_root = unique_operation_root(
        &resolved.runtime_root,
        &resolved.project_root,
        project_id,
        "prepare",
    )?;
    let preview_root = operation_root.join("preview");
    let replay_root = operation_root.join("replay");
    materialize_owned_preview(&resolved.project_root, &preview_root)?;
    materialize_owned_preview(&resolved.project_root, &replay_root)?;
    let mut preview = CargoRustStyleAdapter::owned_preview_configured(
        resolved.cargo_executable.clone(),
        preview_root,
        operation_root.join("target-preview"),
        resolved.cargo_scope.clone(),
        resolved.cargo_features.clone(),
    )?;
    let mut replay = CargoRustStyleAdapter::owned_preview_configured(
        resolved.cargo_executable,
        replay_root,
        operation_root.join("target-replay"),
        resolved.cargo_scope,
        resolved.cargo_features,
    )?;
    let candidate = prepare_rust_style_candidate(
        RustStyleCandidateInput {
            project_id: project_id.clone(),
            base_workspace_snapshot_id,
            scope: match &resolved.inspection.scope {
                RustStyleScope {
                    kind: RustStyleScopeKind::Workspace,
                    ..
                } => RustStylePatchScope::Workspace,
                RustStyleScope {
                    kind: RustStyleScopeKind::Package,
                    package: Some(package),
                } => RustStylePatchScope::Package {
                    package: package.clone(),
                },
                _ => return Err(RustStyleRuntimeError::InvalidScope),
            },
            binding: &resolved.inspection.binding,
            policy: &resolved.inspection.policy,
            coverage: &resolved.inspection.coverage,
        },
        &mut preview,
        &mut replay,
    )?;
    Ok(RustStylePreparedRuntime {
        isolation_ref: opaque_isolation_ref(project_id, &operation_root)?,
        inspection: resolved.inspection,
        candidate,
    })
}

fn resolve_environment(
    project_id: &ProjectId,
    project_root: &Path,
    runtime_root: &Path,
    release_policy_path: &Path,
    scope: RustStyleScope,
    auto_policy: RustAutoPolicy,
) -> Result<ResolvedRustStyleEnvironment, RustStyleRuntimeError> {
    let project_root = project_root
        .canonicalize()
        .map_err(|_| RustStyleRuntimeError::Io)?;
    let runtime_root = validate_runtime_root(&project_root, runtime_root)?;
    let (catalog, catalog_hash) = load_catalog_policy(release_policy_path)?;
    let toolchain_path = project_root.join("rust-toolchain.toml");
    let toolchain_bytes =
        fs::read(&toolchain_path).map_err(|_| RustStyleRuntimeError::ToolchainUnavailable)?;
    let toolchain_text = std::str::from_utf8(&toolchain_bytes)
        .map_err(|_| RustStyleRuntimeError::ToolchainUnavailable)?;
    let toolchain_value: toml::Value =
        toml::from_str(toolchain_text).map_err(|_| RustStyleRuntimeError::ToolchainUnavailable)?;
    let toolchain = toolchain_value
        .get("toolchain")
        .and_then(toml::Value::as_table)
        .ok_or(RustStyleRuntimeError::ToolchainUnavailable)?;
    let channel = toolchain
        .get("channel")
        .and_then(toml::Value::as_str)
        .ok_or(RustStyleRuntimeError::ToolchainUnavailable)?;
    if channel != catalog.toolchain_release || channel != "1.96.0" {
        return Err(RustStyleRuntimeError::ToolchainUnavailable);
    }
    let components = toolchain
        .get("components")
        .and_then(toml::Value::as_array)
        .ok_or(RustStyleRuntimeError::ToolchainUnavailable)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(RustStyleRuntimeError::ToolchainUnavailable)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if !components.contains("rustfmt") || !components.contains("clippy") {
        return Err(RustStyleRuntimeError::ToolchainUnavailable);
    }
    let host = host_triple()?;
    let toolchain_bin = installed_toolchain_bin(channel, &host)?;
    let cargo_executable = toolchain_bin.join(executable_name("cargo"));
    let rustc_executable = toolchain_bin.join(executable_name("rustc"));
    let rustfmt_executable = toolchain_bin.join(executable_name("rustfmt"));
    let clippy_executable = toolchain_bin.join(executable_name("clippy-driver"));
    let discovery_operation =
        unique_operation_root(&runtime_root, &project_root, project_id, "inspect")?;
    let discovery_root_candidate = discovery_operation.join("mirror");
    materialize_owned_preview(&project_root, &discovery_root_candidate)?;
    let discovery_root = discovery_root_candidate
        .canonicalize()
        .map_err(|_| RustStyleRuntimeError::Io)?;
    let target_probe = discovery_operation.join("metadata-target");
    let metadata_adapter = CargoRustStyleAdapter::check_only(
        cargo_executable.clone(),
        discovery_root.clone(),
        target_probe,
    )?;
    let metadata_output = metadata_adapter.cargo_metadata()?;
    if !metadata_output.success {
        return Err(RustStyleRuntimeError::InvalidMetadata);
    }
    let metadata: CargoMetadata = serde_json::from_str(&metadata_output.stdout)
        .map_err(|_| RustStyleRuntimeError::InvalidMetadata)?;
    let selected = select_packages(&metadata, &scope)?;
    let (enabled_features, cargo_features, project_coverage_source) =
        load_project_coverage_policy(&project_root, &selected, &scope)?;
    let (packages, scope_paths, coverage_cells, coverage_limitations, cargo_scope) =
        materialize_inventory(&discovery_root, &selected, &scope, &host, &enabled_features)?;

    let cargo = executable_binding("cargo", &cargo_executable, false)?;
    let rustc = executable_binding("rustc", &rustc_executable, true)?;
    let rustfmt = executable_binding("rustfmt", &rustfmt_executable, false)?;
    let clippy_driver = executable_binding("clippy-driver", &clippy_executable, false)?;
    let manifest_refs = selected
        .iter()
        .map(|package| {
            source_binding_from_mirror(&project_root, &discovery_root, &package.manifest_path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut config_bindings = vec![RustSourceBinding {
        source_ref: "rust-toolchain.toml".to_owned(),
        content_sha256: Sha256Hash::digest(&toolchain_bytes),
    }];
    let formatting_sources =
        discover_exclusive_config_sources(&project_root, "rustfmt.toml", ".rustfmt.toml")?;
    let clippy_parameter_sources =
        discover_exclusive_config_sources(&project_root, "clippy.toml", ".clippy.toml")?;
    let cargo_project_config_sources =
        discover_exclusive_config_sources(&project_root, ".cargo/config.toml", ".cargo/config")?;
    let cargo_home = effective_cargo_home()?;
    let cargo_home_config_sources = discover_external_exclusive_config_sources(
        &cargo_home,
        "config.toml",
        "config",
        "cargo-home",
    )?;
    config_bindings.extend(formatting_sources.clone());
    config_bindings.extend(clippy_parameter_sources.clone());
    config_bindings.extend(cargo_project_config_sources);
    config_bindings.extend(cargo_home_config_sources);
    config_bindings.extend(project_coverage_source);
    config_bindings.sort_by(|left, right| left.source_ref.cmp(&right.source_ref));
    config_bindings.dedup_by(|left, right| left.source_ref == right.source_ref);

    let parsing_editions = packages
        .iter()
        .map(|package| RustEditionBinding {
            subject_ref: package.package_id.clone(),
            edition: package.edition.clone(),
            provenance: package.manifest_ref.clone(),
        })
        .collect::<Vec<_>>();
    let style_editions = parsing_editions.clone();
    let msrv_bindings = packages
        .iter()
        .filter_map(|package| {
            package
                .rust_version
                .as_ref()
                .map(|version| RustEditionBinding {
                    subject_ref: package.package_id.clone(),
                    edition: version.clone(),
                    provenance: package.manifest_ref.clone(),
                })
        })
        .collect::<Vec<_>>();
    let mut binding = RustToolchainBinding {
        schema_id: RUST_TOOLCHAIN_BINDING_SCHEMA_ID.to_owned(),
        schema_version: 1,
        contract_version: 1,
        workspace_root_ref: format!("project:{}", project_id.as_str()),
        manifest_refs,
        toolchain_source: RustToolchainSource::RustToolchainToml,
        toolchain_source_ref: "rust-toolchain.toml".to_owned(),
        toolchain_pin_state: RustToolchainPinState::PinnedStable,
        channel: channel.to_owned(),
        release: Some(channel.to_owned()),
        host_triple: host.clone(),
        cargo,
        rustc,
        rustfmt,
        clippy_driver,
        parsing_editions,
        style_editions,
        msrv_bindings,
        host_target: host.clone(),
        requested_target_triples: vec![host.clone()],
        config_bindings,
        component_states: vec![
            RustTargetState {
                target_triple: "rustfmt".to_owned(),
                state: RustAvailabilityState::Available,
            },
            RustTargetState {
                target_triple: "clippy".to_owned(),
                state: RustAvailabilityState::Available,
            },
        ],
        target_states: vec![RustTargetState {
            target_triple: host.clone(),
            state: RustAvailabilityState::Available,
        }],
        completeness: RustCompleteness::Complete,
        limitations: Vec::new(),
        binding_fingerprint: Sha256Hash::digest(b"pending-rust-toolchain-binding"),
    };
    binding.binding_fingerprint = binding_fingerprint(&binding)?;

    let lint_level_sources = selected
        .iter()
        .map(|package| {
            source_binding_from_mirror(&project_root, &discovery_root, &package.manifest_path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fixed_adapter_definition_fingerprint = versioned_fingerprint(
        "star.rust-style-fixed-adapter",
        1,
        &serde_json::json!({
            "pipeline_ref":catalog.pipeline_ref,
            "cargo_selection":"typed_workspace_or_exact_package",
            "feature_policy":catalog.feature_policy,
            "network":"offline",
            "target_dir":"external_owned",
            "mutator":"owned_preview_only",
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)?;
    let mut policy_limitations = Vec::new();
    if !catalog.built_in_fix_allowlist.is_empty() {
        policy_limitations
            .push("builtin_fix_allowlist_must_remain_empty_without_adjudicated_entries".to_owned());
    }
    let mut policy = RustStylePolicySnapshot {
        schema_id: RUST_STYLE_POLICY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        contract_version: 1,
        profile_ref: catalog.profile_ref.clone(),
        profile_definition_hash: catalog_hash,
        pipeline_ref: catalog.pipeline_ref.clone(),
        fixed_adapter_definition_fingerprint,
        formatting_sources,
        lint_level_sources,
        clippy_parameter_sources,
        clippy_fix_allowlist: Vec::new(),
        coverage_policy_ref: catalog.coverage_policy_ref.clone(),
        scope_project_id: project_id.clone(),
        scope_packages: packages
            .iter()
            .map(|package| package.package_id.clone())
            .collect(),
        scope_paths,
        auto_policy,
        standing_grant_ref: (auto_policy == RustAutoPolicy::PersonalAuto)
            .then_some(".star-control/rust-style-auto-grant.json".to_owned()),
        max_files: catalog.max_files,
        max_hunks: catalog.max_hunks,
        max_changed_bytes: catalog.max_changed_bytes,
        forbidden_operations: catalog.forbidden_operations.clone(),
        policy_completeness: if policy_limitations.is_empty() {
            RustCompleteness::Complete
        } else {
            RustCompleteness::Partial
        },
        limitations: policy_limitations,
        policy_fingerprint: Sha256Hash::digest(b"pending-rust-style-policy"),
    };
    policy.policy_fingerprint = policy_fingerprint(&policy)?;

    let required_cell_ids = coverage_cells
        .iter()
        .filter(|cell| cell.execution == RustCoverageExecution::Executed)
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    let cfg_frontier = coverage_limitations
        .iter()
        .filter_map(|limitation| {
            limitation
                .strip_prefix("uncovered_feature:")
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let mut coverage = RustStyleCoverageMatrix {
        schema_id: RUST_STYLE_COVERAGE_MATRIX_SCHEMA_ID.to_owned(),
        schema_version: 1,
        contract_version: 1,
        policy_ref: catalog.coverage_policy_ref,
        cells: coverage_cells,
        required_cell_ids,
        cfg_frontier,
        conflicts: Vec::new(),
        completeness: if coverage_limitations.is_empty() {
            RustCompleteness::Complete
        } else {
            RustCompleteness::Partial
        },
        limitations: coverage_limitations,
        coverage_fingerprint: Sha256Hash::digest(b"pending-rust-style-coverage"),
    };
    coverage.coverage_fingerprint = coverage_fingerprint(&coverage)?;

    let mut limitations = binding.limitations.clone();
    limitations.extend(policy.limitations.clone());
    limitations.extend(coverage.limitations.clone());
    limitations.sort();
    limitations.dedup();
    let standing_grant_template = (auto_policy == RustAutoPolicy::PersonalAuto).then(|| {
        serde_json::json!({
            "schema_version":1,
            "action":"apply_rust_style_patch",
            "project_id":project_id,
            "profile_ref":policy.profile_ref,
            "pipeline_ref":policy.pipeline_ref,
            "toolchain_fingerprint":binding.binding_fingerprint,
            "style_policy_fingerprint":policy.policy_fingerprint,
            "coverage_fingerprint":coverage.coverage_fingerprint,
            "scope_paths":policy.scope_paths,
            "max_files":policy.max_files,
            "max_changed_bytes":policy.max_changed_bytes,
            "expires_at":"<RFC3339-expiry>",
        })
    });
    let inspection_fingerprint = versioned_fingerprint(
        "star.rust-style-inspection",
        1,
        &serde_json::json!({
            "project_id":project_id,
            "scope":scope,
            "binding_fingerprint":binding.binding_fingerprint,
            "policy_fingerprint":policy.policy_fingerprint,
            "coverage_fingerprint":coverage.coverage_fingerprint,
            "packages":packages,
            "limitations":limitations,
            "standing_grant_template":standing_grant_template,
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)?;
    Ok(ResolvedRustStyleEnvironment {
        inspection: RustStyleInspection {
            project_id: project_id.clone(),
            scope,
            binding,
            policy,
            coverage,
            packages,
            limitations,
            standing_grant_template,
            inspection_fingerprint,
        },
        cargo_executable,
        cargo_scope,
        cargo_features,
        project_root,
        runtime_root,
    })
}

fn load_catalog_policy(
    path: &Path,
) -> Result<(RustStyleCatalogPolicy, Sha256Hash), RustStyleRuntimeError> {
    let bytes = fs::read(path).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    let policy: RustStyleCatalogPolicy =
        toml::from_str(text).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    let expected_forbidden = [
        "create",
        "delete",
        "rename",
        "mode_change",
        "non_rust_write",
        "generated_write",
        "vendor_write",
        "out_of_scope_write",
        "cargo_or_lockfile_write",
        "config_or_toolchain_write",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if policy.schema_version != 1
        || policy.profile_ref != "rust_style_auto_fix"
        || policy.pipeline_ref != format!("{RUST_STYLE_PIPELINE_ID}@{RUST_STYLE_PIPELINE_VERSION}")
        || policy.toolchain_release != "1.96.0"
        || policy.coverage_policy_ref != "rust-style-default-feature-host-v1"
        || policy.feature_policy != "project_declared_compatible_union_or_default"
        || policy.max_files == 0
        || policy.max_files > 10_000
        || policy.max_hunks == 0
        || policy.max_hunks > 10_000
        || policy.max_changed_bytes == 0
        || policy.max_changed_bytes > 1_073_741_824
        || policy
            .forbidden_operations
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_forbidden
    {
        return Err(RustStyleRuntimeError::InvalidCatalog);
    }
    Ok((policy, Sha256Hash::digest(&bytes)))
}

fn select_packages<'a>(
    metadata: &'a CargoMetadata,
    scope: &RustStyleScope,
) -> Result<Vec<&'a CargoPackage>, RustStyleRuntimeError> {
    let workspace_members = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut packages = metadata
        .packages
        .iter()
        .filter(|package| workspace_members.contains(package.id.as_str()))
        .collect::<Vec<_>>();
    match scope.kind {
        RustStyleScopeKind::Workspace if scope.package.is_none() => {}
        RustStyleScopeKind::Package => {
            let requested = scope
                .package
                .as_deref()
                .ok_or(RustStyleRuntimeError::InvalidScope)?;
            packages.retain(|package| {
                package.name == requested || stable_package_id(package) == requested
            });
            if packages.len() != 1 {
                return Err(RustStyleRuntimeError::InvalidScope);
            }
        }
        _ => return Err(RustStyleRuntimeError::InvalidScope),
    }
    if packages.is_empty() {
        return Err(RustStyleRuntimeError::InvalidMetadata);
    }
    packages.sort_by_key(|package| stable_package_id(package));
    Ok(packages)
}

type ProjectCoverageResolution = (
    BTreeMap<String, BTreeSet<String>>,
    Vec<String>,
    Vec<RustSourceBinding>,
);

fn load_project_coverage_policy(
    project_root: &Path,
    selected: &[&CargoPackage],
    scope: &RustStyleScope,
) -> Result<ProjectCoverageResolution, RustStyleRuntimeError> {
    let path = project_root.join(".star-control/rust-style.toml");
    if !path.exists() {
        return Ok((BTreeMap::new(), Vec::new(), Vec::new()));
    }
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    let canonical = path
        .canonicalize()
        .map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || !canonical.starts_with(project_root)
    {
        return Err(RustStyleRuntimeError::InvalidCatalog);
    }
    let bytes = fs::read(&canonical).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    if bytes.len() > 256 * 1024 {
        return Err(RustStyleRuntimeError::InvalidCatalog);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    let policy: RustStyleProjectCoveragePolicy =
        toml::from_str(text).map_err(|_| RustStyleRuntimeError::InvalidCatalog)?;
    if policy.schema_version != 1
        || !policy.feature_union_compatible
        || policy.packages.is_empty()
        || policy.packages.len() > 128
    {
        return Err(RustStyleRuntimeError::InvalidCatalog);
    }
    let mut enabled = BTreeMap::<String, BTreeSet<String>>::new();
    let mut cargo_features = Vec::new();
    for declaration in policy.packages {
        let matching = selected
            .iter()
            .filter(|package| {
                package.name == declaration.package
                    || stable_package_id(package) == declaration.package
            })
            .copied()
            .collect::<Vec<_>>();
        if matching.len() != 1 || declaration.features.is_empty() {
            return Err(RustStyleRuntimeError::InvalidCatalog);
        }
        let package = matching[0];
        let package_id = stable_package_id(package);
        let entry = enabled.entry(package_id).or_default();
        for feature in declaration.features {
            if feature == "default"
                || !valid_feature_name(&feature)
                || !package.features.contains_key(&feature)
                || !entry.insert(feature.clone())
            {
                return Err(RustStyleRuntimeError::InvalidCatalog);
            }
            cargo_features.push(match scope.kind {
                RustStyleScopeKind::Workspace => format!("{}/{feature}", package.name),
                RustStyleScopeKind::Package => feature,
            });
        }
    }
    cargo_features.sort();
    cargo_features.dedup();
    Ok((
        enabled,
        cargo_features,
        vec![RustSourceBinding {
            source_ref: ".star-control/rust-style.toml".to_owned(),
            content_sha256: Sha256Hash::digest(&bytes),
        }],
    ))
}

fn valid_feature_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'.'))
}

type InventoryMaterialization = (
    Vec<RustStylePackageInventory>,
    Vec<ProjectPathRef>,
    Vec<RustStyleCoverageCell>,
    Vec<String>,
    RustCargoScope,
);

fn materialize_inventory(
    project_root: &Path,
    selected: &[&CargoPackage],
    scope: &RustStyleScope,
    host: &str,
    enabled_features: &BTreeMap<String, BTreeSet<String>>,
) -> Result<InventoryMaterialization, RustStyleRuntimeError> {
    let mut inventory = Vec::new();
    let mut scope_paths = BTreeSet::new();
    let mut cells = Vec::new();
    let mut limitations = Vec::new();
    for package in selected {
        let package_id = stable_package_id(package);
        let manifest_ref = relative_project_path(project_root, &package.manifest_path)?;
        let default_features = package.features.get("default").cloned().unwrap_or_default();
        let explicitly_enabled = enabled_features
            .get(&package_id)
            .cloned()
            .unwrap_or_default();
        for feature in package
            .features
            .keys()
            .filter(|feature| feature.as_str() != "default")
        {
            if !default_features.iter().any(|default| default == feature)
                && !explicitly_enabled.contains(feature)
            {
                limitations.push(format!("uncovered_feature:{package_id}:{feature}"));
            }
        }
        let mut targets = Vec::new();
        for target in &package.targets {
            let source_path = relative_project_path(project_root, &target.src_path)?;
            let source_root = source_scope_for_target(&source_path)?;
            scope_paths.insert(source_root.clone());
            let target_kind = target_kind(&target.kind)?;
            let required_features_satisfied = target.required_features.iter().all(|feature| {
                default_features.iter().any(|default| default == feature)
                    || explicitly_enabled.contains(feature)
            });
            let execution = if required_features_satisfied {
                RustCoverageExecution::Executed
            } else {
                limitations.push(format!(
                    "required_feature_not_enabled:{package_id}:{}",
                    target.name
                ));
                RustCoverageExecution::Skipped
            };
            let manifest_sha256 = Sha256Hash::digest(
                &fs::read(&package.manifest_path).map_err(|_| RustStyleRuntimeError::Io)?,
            );
            let mut effective_features = default_features.clone();
            effective_features.extend(explicitly_enabled.iter().cloned());
            effective_features.sort();
            effective_features.dedup();
            let feature_set_id = format!(
                "default-{}",
                short_hash(&Sha256Hash::digest(
                    effective_features.join("\n").as_bytes(),
                ))
            );
            let cell_id = format!(
                "rust-cell-{}-{}-{}",
                sanitize_id(&package.name),
                sanitize_id(&target.name),
                short_hash(&manifest_sha256)
            );
            cells.push(RustStyleCoverageCell {
                cell_id,
                workspace_ref: "current-project-workspace".to_owned(),
                package_id: package_id.clone(),
                manifest_sha256,
                target_kind,
                target_name: target.name.clone(),
                source_root: source_root.clone(),
                feature_set_id,
                default_features: true,
                features: effective_features,
                required_features_satisfied,
                host_triple: host.to_owned(),
                target_triple: host.to_owned(),
                cfg_observation_ref: "host-default-cfg-v1".to_owned(),
                ownership: RustSourceOwnership::Handwritten,
                phase: RustCoveragePhase::DiagnosticCheck,
                execution,
                reason: (!required_features_satisfied)
                    .then_some("required_features_not_in_default_set".to_owned()),
            });
            targets.push(RustStyleTargetInventory {
                target_name: target.name.clone(),
                target_kind,
                source_root,
                required_features: target.required_features.clone(),
            });
        }
        targets.sort_by(|left, right| {
            (&left.source_root, &left.target_name).cmp(&(&right.source_root, &right.target_name))
        });
        let mut declared_features = package.features.keys().cloned().collect::<Vec<_>>();
        declared_features.sort();
        inventory.push(RustStylePackageInventory {
            package_id,
            package_name: package.name.clone(),
            manifest_ref,
            edition: package.edition.clone(),
            rust_version: package.rust_version.clone(),
            declared_features,
            targets,
        });
    }
    let cargo_scope = match scope.kind {
        RustStyleScopeKind::Workspace => RustCargoScope::Workspace,
        RustStyleScopeKind::Package => RustCargoScope::Package(
            inventory
                .first()
                .ok_or(RustStyleRuntimeError::InvalidScope)?
                .package_name
                .clone(),
        ),
    };
    cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    limitations.sort();
    limitations.dedup();
    Ok((
        inventory,
        scope_paths.into_iter().collect(),
        cells,
        limitations,
        cargo_scope,
    ))
}

fn stable_package_id(package: &CargoPackage) -> String {
    format!("{}@{}", package.name, package.version)
}

fn source_scope_for_target(path: &str) -> Result<ProjectPathRef, RustStyleRuntimeError> {
    let path = ProjectPathRef::parse(path.to_owned())
        .map_err(|_| RustStyleRuntimeError::InvalidMetadata)?;
    let parent = path
        .as_str()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .filter(|parent| !parent.is_empty())
        .unwrap_or(path.as_str());
    ProjectPathRef::parse(parent.to_owned()).map_err(|_| RustStyleRuntimeError::InvalidMetadata)
}

fn target_kind(kinds: &[String]) -> Result<RustTargetKind, RustStyleRuntimeError> {
    if kinds.iter().any(|kind| kind == "proc-macro") {
        Ok(RustTargetKind::ProcMacro)
    } else if kinds.iter().any(|kind| kind == "custom-build") {
        Ok(RustTargetKind::CustomBuild)
    } else if kinds.iter().any(|kind| kind == "lib" || kind == "rlib") {
        Ok(RustTargetKind::Lib)
    } else if kinds.iter().any(|kind| kind == "bin") {
        Ok(RustTargetKind::Bin)
    } else if kinds.iter().any(|kind| kind == "test") {
        Ok(RustTargetKind::Test)
    } else if kinds.iter().any(|kind| kind == "example") {
        Ok(RustTargetKind::Example)
    } else if kinds.iter().any(|kind| kind == "bench") {
        Ok(RustTargetKind::Bench)
    } else {
        Err(RustStyleRuntimeError::InvalidMetadata)
    }
}

fn executable_binding(
    logical_id: &str,
    path: &Path,
    verbose: bool,
) -> Result<RustExecutableBinding, RustStyleRuntimeError> {
    let bytes = fs::read(path).map_err(|_| RustStyleRuntimeError::ToolchainUnavailable)?;
    let sha256 = Sha256Hash::digest(&bytes);
    let output = probe_direct_tool_version(path, verbose)?;
    if !output.success {
        return Err(RustStyleRuntimeError::ToolchainUnavailable);
    }
    let version = output
        .stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or(RustStyleRuntimeError::ToolchainUnavailable)?
        .to_owned();
    Ok(RustExecutableBinding {
        logical_id: logical_id.to_owned(),
        opaque_file_identity: format!("opaque:{}", short_hash(&sha256)),
        version,
        sha256,
        component_state: RustAvailabilityState::Available,
    })
}

fn source_binding(
    project_root: &Path,
    path: &Path,
) -> Result<RustSourceBinding, RustStyleRuntimeError> {
    Ok(RustSourceBinding {
        source_ref: relative_project_path(project_root, path)?,
        content_sha256: Sha256Hash::digest(&fs::read(path).map_err(|_| RustStyleRuntimeError::Io)?),
    })
}

fn source_binding_from_mirror(
    project_root: &Path,
    mirror_root: &Path,
    mirror_path: &Path,
) -> Result<RustSourceBinding, RustStyleRuntimeError> {
    let relative = relative_project_path(mirror_root, mirror_path)?;
    source_binding(project_root, &project_root.join(relative))
}

fn discover_exclusive_config_sources(
    project_root: &Path,
    primary: &str,
    alternate: &str,
) -> Result<Vec<RustSourceBinding>, RustStyleRuntimeError> {
    let primary_path = project_root.join(primary);
    let alternate_path = project_root.join(alternate);
    if primary_path.is_file() && alternate_path.is_file() {
        return Err(RustStyleRuntimeError::InvalidMetadata);
    }
    [primary_path, alternate_path]
        .into_iter()
        .filter(|path| path.is_file())
        .map(|path| source_binding(project_root, &path))
        .collect()
}

fn discover_external_exclusive_config_sources(
    root: &Path,
    primary: &str,
    alternate: &str,
    logical_root: &str,
) -> Result<Vec<RustSourceBinding>, RustStyleRuntimeError> {
    let primary_path = root.join(primary);
    let alternate_path = root.join(alternate);
    if primary_path.is_file() && alternate_path.is_file() {
        return Err(RustStyleRuntimeError::InvalidMetadata);
    }
    [primary_path, alternate_path]
        .into_iter()
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).map_err(|_| RustStyleRuntimeError::Io)?;
            Ok(RustSourceBinding {
                source_ref: format!(
                    "{logical_root}/{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
                content_sha256: Sha256Hash::digest(&bytes),
            })
        })
        .collect()
}

fn relative_project_path(
    project_root: &Path,
    path: &Path,
) -> Result<String, RustStyleRuntimeError> {
    let canonical = path
        .canonicalize()
        .map_err(|_| RustStyleRuntimeError::InvalidMetadata)?;
    let relative = canonical
        .strip_prefix(project_root)
        .map_err(|_| RustStyleRuntimeError::InvalidMetadata)?
        .to_string_lossy()
        .replace('\\', "/");
    ProjectPathRef::parse(relative.clone()).map_err(|_| RustStyleRuntimeError::InvalidMetadata)?;
    Ok(relative)
}

fn binding_fingerprint(
    binding: &RustToolchainBinding,
) -> Result<Sha256Hash, RustStyleRuntimeError> {
    versioned_fingerprint(
        "star.rust-toolchain-binding",
        1,
        &serde_json::json!({
            "schema_id":binding.schema_id,
            "schema_version":binding.schema_version,
            "contract_version":binding.contract_version,
            "workspace_root_ref":binding.workspace_root_ref,
            "manifest_refs":binding.manifest_refs,
            "toolchain_source":binding.toolchain_source,
            "toolchain_source_ref":binding.toolchain_source_ref,
            "toolchain_pin_state":binding.toolchain_pin_state,
            "channel":binding.channel,
            "release":binding.release,
            "host_triple":binding.host_triple,
            "cargo":binding.cargo,
            "rustc":binding.rustc,
            "rustfmt":binding.rustfmt,
            "clippy_driver":binding.clippy_driver,
            "parsing_editions":binding.parsing_editions,
            "style_editions":binding.style_editions,
            "msrv_bindings":binding.msrv_bindings,
            "host_target":binding.host_target,
            "requested_target_triples":binding.requested_target_triples,
            "config_bindings":binding.config_bindings,
            "component_states":binding.component_states,
            "target_states":binding.target_states,
            "completeness":binding.completeness,
            "limitations":binding.limitations,
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)
}

fn policy_fingerprint(
    policy: &RustStylePolicySnapshot,
) -> Result<Sha256Hash, RustStyleRuntimeError> {
    versioned_fingerprint(
        "star.rust-style-policy-snapshot",
        1,
        &serde_json::json!({
            "schema_id":policy.schema_id,
            "schema_version":policy.schema_version,
            "contract_version":policy.contract_version,
            "profile_ref":policy.profile_ref,
            "profile_definition_hash":policy.profile_definition_hash,
            "pipeline_ref":policy.pipeline_ref,
            "fixed_adapter_definition_fingerprint":policy.fixed_adapter_definition_fingerprint,
            "formatting_sources":policy.formatting_sources,
            "lint_level_sources":policy.lint_level_sources,
            "clippy_parameter_sources":policy.clippy_parameter_sources,
            "clippy_fix_allowlist":policy.clippy_fix_allowlist,
            "coverage_policy_ref":policy.coverage_policy_ref,
            "scope_project_id":policy.scope_project_id,
            "scope_packages":policy.scope_packages,
            "scope_paths":policy.scope_paths,
            "auto_policy":policy.auto_policy,
            "standing_grant_ref":policy.standing_grant_ref,
            "max_files":policy.max_files,
            "max_hunks":policy.max_hunks,
            "max_changed_bytes":policy.max_changed_bytes,
            "forbidden_operations":policy.forbidden_operations,
            "policy_completeness":policy.policy_completeness,
            "limitations":policy.limitations,
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)
}

fn coverage_fingerprint(
    coverage: &RustStyleCoverageMatrix,
) -> Result<Sha256Hash, RustStyleRuntimeError> {
    versioned_fingerprint(
        "star.rust-style-coverage-matrix",
        1,
        &serde_json::json!({
            "schema_id":coverage.schema_id,
            "schema_version":coverage.schema_version,
            "contract_version":coverage.contract_version,
            "policy_ref":coverage.policy_ref,
            "cells":coverage.cells,
            "required_cell_ids":coverage.required_cell_ids,
            "cfg_frontier":coverage.cfg_frontier,
            "conflicts":coverage.conflicts,
            "completeness":coverage.completeness,
            "limitations":coverage.limitations,
        }),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)
}

fn validate_runtime_root(
    project_root: &Path,
    runtime_root: &Path,
) -> Result<PathBuf, RustStyleRuntimeError> {
    if !runtime_root.is_absolute() || runtime_root.as_os_str().to_string_lossy().len() > 160 {
        return Err(RustStyleRuntimeError::UnsafeRuntimeRoot);
    }
    fs::create_dir_all(runtime_root).map_err(|_| RustStyleRuntimeError::Io)?;
    let metadata =
        fs::symlink_metadata(runtime_root).map_err(|_| RustStyleRuntimeError::UnsafeRuntimeRoot)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || runtime_reparse_point(&metadata) {
        return Err(RustStyleRuntimeError::UnsafeRuntimeRoot);
    }
    let canonical = runtime_root
        .canonicalize()
        .map_err(|_| RustStyleRuntimeError::UnsafeRuntimeRoot)?;
    if canonical.starts_with(project_root) || canonical.as_os_str().to_string_lossy().len() > 160 {
        return Err(RustStyleRuntimeError::UnsafeRuntimeRoot);
    }
    Ok(canonical)
}

fn unique_operation_root(
    runtime_root: &Path,
    project_root: &Path,
    project_id: &ProjectId,
    kind: &str,
) -> Result<PathBuf, RustStyleRuntimeError> {
    fs::create_dir_all(runtime_root).map_err(|_| RustStyleRuntimeError::Io)?;
    let nonce = star_contracts::ArtifactId::new();
    let project_key = Sha256Hash::digest(project_id.as_str().as_bytes());
    let nonce_text = nonce.as_str();
    let kind = match kind {
        "inspect" => "i",
        "check" => "c",
        "prepare" => "p",
        _ => return Err(RustStyleRuntimeError::UnsafeRuntimeRoot),
    };
    let root = runtime_root
        .join("p")
        .join(short_hash(&project_key))
        .join(format!("{kind}-{}", &nonce_text[nonce_text.len() - 12..]));
    if root.exists() || root.starts_with(project_root) || project_root.starts_with(&root) {
        return Err(RustStyleRuntimeError::UnsafeRuntimeRoot);
    }
    fs::create_dir_all(&root).map_err(|_| RustStyleRuntimeError::Io)?;
    Ok(root)
}

#[cfg(windows)]
fn runtime_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn runtime_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn opaque_isolation_ref(
    project_id: &ProjectId,
    operation_root: &Path,
) -> Result<String, RustStyleRuntimeError> {
    let leaf = operation_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(RustStyleRuntimeError::UnsafeRuntimeRoot)?;
    let fingerprint = versioned_fingerprint(
        "star.rust-style-isolation-ref",
        1,
        &serde_json::json!({"project_id":project_id,"owned_leaf":leaf}),
    )
    .map_err(|_| RustStyleRuntimeError::Fingerprint)?;
    Ok(format!("rsi_{}", &fingerprint.as_str()[7..31]))
}

fn summarize_tool_output(output: RustToolOutput) -> RustToolRunSummary {
    RustToolRunSummary {
        success: output.success,
        exit_code: output.exit_code,
        stdout_sha256: Sha256Hash::digest(output.stdout.as_bytes()),
        stderr_sha256: Sha256Hash::digest(output.stderr.as_bytes()),
        command_fingerprint: output.command_fingerprint,
    }
}

fn installed_toolchain_bin(channel: &str, host: &str) -> Result<PathBuf, RustStyleRuntimeError> {
    let rustup_root = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|path| path.join(".rustup"))
        })
        .ok_or(RustStyleRuntimeError::ToolchainUnavailable)?;
    let bin = rustup_root
        .join("toolchains")
        .join(format!("{channel}-{host}"))
        .join("bin");
    if !bin.is_dir() {
        return Err(RustStyleRuntimeError::ToolchainUnavailable);
    }
    Ok(bin)
}

fn host_triple() -> Result<String, RustStyleRuntimeError> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Ok("x86_64-pc-windows-msvc".to_owned())
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Ok("aarch64-pc-windows-msvc".to_owned())
    } else {
        Err(RustStyleRuntimeError::ToolchainUnavailable)
    }
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(48)
        .collect()
}

fn short_hash(hash: &Sha256Hash) -> &str {
    &hash.as_str()[7..19]
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_execution::{rollback_applied, rust_style::apply_rust_style_patch};

    #[test]
    fn actual_pinned_workspace_check_prepare_apply_and_rollback_is_isolated() {
        let nonce = star_contracts::ArtifactId::new();
        let nonce_suffix = &nonce.as_str()[nonce.as_str().len() - 12..];
        let project_root =
            std::env::temp_dir().join(format!("rsp-{}-{}", std::process::id(), nonce_suffix));
        let runtime_root =
            std::env::temp_dir().join(format!("rsr-{}-{}", std::process::id(), nonce_suffix));
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::write(
            project_root.join("Cargo.toml"),
            "[package]\nname = \"runtime-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.96\"\n",
        )
        .unwrap();
        fs::write(
            project_root.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"runtime-fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            project_root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.96.0\"\nprofile = \"minimal\"\ncomponents = [\"rustfmt\", \"clippy\"]\n",
        )
        .unwrap();
        let original = b"pub fn answer( )->u32{42}\n";
        fs::write(project_root.join("src/lib.rs"), original).unwrap();
        let policy_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .join("catalog/policies/rust-style.toml");
        let project_id = ProjectId::new();
        let inspection = inspect_rust_style(
            &project_id,
            &project_root,
            &runtime_root,
            &policy_path,
            RustStyleScope::workspace(),
            RustAutoPolicy::SafeDefault,
        )
        .unwrap();
        assert_eq!(inspection.binding.completeness, RustCompleteness::Complete);
        assert_eq!(inspection.coverage.completeness, RustCompleteness::Complete);
        assert!(inspection.limitations.is_empty());
        assert_eq!(fs::read(project_root.join("src/lib.rs")).unwrap(), original);

        let check = check_rust_style(
            &project_id,
            &project_root,
            &runtime_root,
            &policy_path,
            RustStyleScope::workspace(),
            RustAutoPolicy::SafeDefault,
        )
        .unwrap();
        assert!(!check.rustfmt.success);
        assert!(check.clippy.success);
        assert!(check.source_unchanged);
        assert_eq!(fs::read(project_root.join("src/lib.rs")).unwrap(), original);

        let prepared = prepare_rust_style(
            &project_id,
            WorkspaceSnapshotId::new(),
            &project_root,
            &runtime_root,
            &policy_path,
            RustStyleScope::workspace(),
            RustAutoPolicy::SafeDefault,
        )
        .unwrap();
        assert!(prepared.candidate.idempotence_proved);
        let patch_set = prepared.candidate.patch_set.clone().unwrap();
        let approval = patch_set.patch_fingerprint.as_str().to_owned();
        let applied = apply_rust_style_patch(
            patch_set,
            &project_root,
            prepared.candidate.forward_artifact.as_ref().unwrap(),
            &approval,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(project_root.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    42\n}\n"
        );
        let reverted = rollback_applied(applied).unwrap();
        assert_eq!(
            reverted.status,
            star_contracts::management::PatchSetStatus::Reverted
        );
        assert_eq!(fs::read(project_root.join("src/lib.rs")).unwrap(), original);
    }

    #[test]
    fn actual_multicrate_feature_build_script_proc_macro_corpus_is_complete_and_idempotent() {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        let project_root = repository.join("specs/corpus/rust-style/multicrate");
        let nonce = star_contracts::ArtifactId::new();
        let nonce_suffix = &nonce.as_str()[nonce.as_str().len() - 12..];
        let runtime_root =
            std::env::temp_dir().join(format!("rsm-{}-{}", std::process::id(), nonce_suffix));
        let policy_path = repository.join("catalog/policies/rust-style.toml");
        let project_id = ProjectId::new();
        let check = check_rust_style(
            &project_id,
            &project_root,
            &runtime_root,
            &policy_path,
            RustStyleScope::workspace(),
            RustAutoPolicy::SafeDefault,
        )
        .unwrap();
        assert!(check.rustfmt.success);
        assert!(check.clippy.success);
        assert!(check.source_unchanged);
        assert!(check.inspection.limitations.is_empty());
        assert!(
            check
                .inspection
                .packages
                .iter()
                .flat_map(|package| &package.targets)
                .any(|target| target.target_kind == RustTargetKind::CustomBuild)
        );
        assert!(
            check
                .inspection
                .packages
                .iter()
                .flat_map(|package| &package.targets)
                .any(|target| target.target_kind == RustTargetKind::ProcMacro)
        );
        let prepared = prepare_rust_style(
            &project_id,
            WorkspaceSnapshotId::new(),
            &project_root,
            &runtime_root,
            &policy_path,
            RustStyleScope::workspace(),
            RustAutoPolicy::SafeDefault,
        )
        .unwrap();
        assert_eq!(
            prepared.candidate.state,
            crate::rust_style::RustCandidateState::SucceededNoChange
        );
        assert!(prepared.candidate.patch_set.is_none());
        assert!(prepared.candidate.idempotence_proved);
    }
}
