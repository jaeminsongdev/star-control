//! Read-only tracked-root observation for `validation.plan`.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use star_application::{
    UnitDependency, ValidationCheckDefinition, ValidationPlanningInput, build_validation_plan,
};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        EVIDENCE_CONTRACT_SCHEMA_VERSION, VALIDATION_POLICY_SCHEMA_VERSION, ValidationChangeClass,
        ValidationChangeSource, ValidationChangedFile, ValidationCommand, ValidationPlan,
        ValidationProfile,
    },
};
use star_project::catalog::{
    CatalogAvailability, CatalogIdentityStatus, CatalogProjectRole, CatalogRepositoryKind,
    ProjectCatalogManifest, inspect_project_catalog_entry,
};
use thiserror::Error;

const PROJECT_MANIFEST: &str = ".star-control/project.toml";
const MAX_PROJECT_MANIFEST_BYTES: u64 = 1_048_576;
const MAX_CARGO_MANIFEST_BYTES: u64 = 2_097_152;
const MAX_FINGERPRINT_FILE_BYTES: u64 = 33_554_432;

#[derive(Debug, Error)]
pub enum ValidationPlanningObservationError {
    #[error("project is not an active canonical Git project")]
    ProjectBoundary,
    #[error("project validation manifest is missing or invalid")]
    ProjectManifest,
    #[error("project Git observation failed")]
    GitObservation,
    #[error("project validation input is too large")]
    ObservationLimit,
    #[error("requested unit does not match the observed change set")]
    RequestedUnit,
    #[error("validation plan construction failed")]
    Planning,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectValidationManifest {
    schema_version: u32,
    project_key: String,
    default_profile: ValidationProfile,
    workspace_unit: String,
    validation_entrypoint: String,
    policy_schema_version: u32,
    evidence_schema_version: u32,
    limits: ObservationLimits,
    classification: ClassificationPolicy,
    unit_mappings: Vec<UnitMapping>,
    fingerprints: FingerprintPolicy,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationLimits {
    max_git_output_bytes: usize,
    max_untracked_file_bytes: u64,
    max_untracked_total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassificationPolicy {
    security_prefixes: Vec<String>,
    data_prefixes: Vec<String>,
    workflow_release_prefixes: Vec<String>,
    validator_policy_prefixes: Vec<String>,
    public_contract_prefixes: Vec<String>,
    toolchain_paths: Vec<String>,
    lockfile_paths: Vec<String>,
    documentation_extensions: Vec<String>,
    configuration_extensions: Vec<String>,
    code_extensions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitMapping {
    prefix: String,
    unit: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FingerprintPolicy {
    toolchain_paths: Vec<String>,
    lockfile_paths: Vec<String>,
    validation_script_paths: Vec<String>,
    config_paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ValidationProjectTarget {
    pub project_root: PathBuf,
    pub validation_entrypoint: PathBuf,
}

struct CargoWorkspace {
    units_by_prefix: Vec<(String, String)>,
    unit_names: BTreeSet<String>,
    dependencies: Vec<UnitDependency>,
    complete: bool,
}

pub fn build_project_validation_plan(
    catalog: &ProjectCatalogManifest,
    catalog_root: &Path,
    project_key: &str,
    requested_profile: Option<ValidationProfile>,
    requested_unit: Option<String>,
) -> Result<ValidationPlan, ValidationPlanningObservationError> {
    let (project_root, manifest, manifest_bytes) =
        load_validation_project(catalog, catalog_root, project_key)?;

    let revision = git_text(
        &project_root,
        &["rev-parse", "HEAD"],
        manifest.limits.max_git_output_bytes,
    )?
    .trim()
    .to_owned();
    let staged_diff = git_bytes(
        &project_root,
        &["diff", "--cached", "--binary", "--no-ext-diff"],
        manifest.limits.max_git_output_bytes,
    )?;
    let unstaged_diff = git_bytes(
        &project_root,
        &["diff", "--binary", "--no-ext-diff"],
        manifest.limits.max_git_output_bytes,
    )?;
    let staged_paths = git_paths(
        &project_root,
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--diff-filter=ACDMRTUXB",
        ],
        manifest.limits.max_git_output_bytes,
    )?;
    let unstaged_paths = git_paths(
        &project_root,
        &["diff", "--name-only", "-z", "--diff-filter=ACDMRTUXB"],
        manifest.limits.max_git_output_bytes,
    )?;
    let untracked_paths = git_paths(
        &project_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        manifest.limits.max_git_output_bytes,
    )?;

    let cargo = load_cargo_workspace(&project_root);
    let (untracked_content, untracked_complete) =
        hash_untracked(&project_root, &untracked_paths, &manifest.limits)?;
    let mut changed = merge_changed_paths(
        staged_paths,
        unstaged_paths,
        untracked_paths,
        &manifest,
        &cargo,
    );
    if !untracked_complete {
        for file in &mut changed {
            if file.sources.contains(&ValidationChangeSource::Untracked) {
                file.change_class = ValidationChangeClass::Unknown;
            }
        }
    }
    if let Some(unit) = requested_unit.as_deref() {
        let known = unit == manifest.workspace_unit
            || manifest.unit_mappings.iter().any(|item| item.unit == unit)
            || cargo.unit_names.contains(unit);
        let mismatched = changed
            .iter()
            .filter_map(|file| file.direct_unit.as_deref())
            .any(|observed| observed != unit && observed != manifest.workspace_unit);
        if !known || mismatched {
            return Err(ValidationPlanningObservationError::RequestedUnit);
        }
    }

    let project_manifest = Sha256Hash::digest(&manifest_bytes);
    let (toolchain_files, toolchain_files_complete) =
        hash_files(&project_root, &manifest.fingerprints.toolchain_paths)?;
    let (rustc_identity, rustc_available) = command_identity(&project_root, "rustc", &["-Vv"]);
    let (cargo_identity, cargo_available) = command_identity(&project_root, "cargo", &["-V"]);
    let toolchain = canonical_hash(&serde_json::json!({
        "files":toolchain_files,
        "rustc":rustc_identity,
        "cargo":cargo_identity,
    }))?;
    let (lockfile, lockfile_complete) =
        hash_files(&project_root, &manifest.fingerprints.lockfile_paths)?;
    let (validation_scripts, validation_scripts_complete) = hash_files(
        &project_root,
        &manifest.fingerprints.validation_script_paths,
    )?;
    let (config, config_complete) = hash_files(&project_root, &manifest.fingerprints.config_paths)?;
    let fingerprints_complete = toolchain_files_complete
        && rustc_available
        && cargo_available
        && lockfile_complete
        && validation_scripts_complete
        && config_complete;
    let fingerprints = star_contracts::evidence::ValidationInputFingerprintComponents {
        revision: revision.clone(),
        staged_diff: Sha256Hash::digest(&staged_diff),
        unstaged_diff: Sha256Hash::digest(&unstaged_diff),
        untracked_content,
        toolchain,
        lockfile,
        project_manifest,
        validation_scripts,
        config,
        policy_schema_version: manifest.policy_schema_version,
        evidence_schema_version: manifest.evidence_schema_version,
    };
    let requested_unit_required_profile = requested_unit.as_deref().map(|unit| {
        if unit == "docs" {
            ValidationProfile::Quick
        } else {
            ValidationProfile::Target
        }
    });
    let checks = check_definitions(&changed, &cargo, &fingerprints, requested_unit.as_deref())?;
    let public_graph_complete = changed.iter().all(|file| {
        file.change_class != ValidationChangeClass::PublicContract
            || file.direct_unit.as_ref().is_some_and(|unit| {
                unit == &manifest.workspace_unit || cargo.unit_names.contains(unit)
            })
    });
    build_validation_plan(ValidationPlanningInput {
        project_key: project_key.to_owned(),
        revision,
        requested_profile,
        requested_unit,
        requested_unit_required_profile,
        workspace_unit_id: manifest.workspace_unit,
        changed_files: changed,
        dependencies: cargo.dependencies,
        checks,
        cache_candidates: Vec::new(),
        fingerprints,
        fingerprints_complete,
        impact_complete: cargo.complete && untracked_complete && public_graph_complete,
        repeated_failures: false,
    })
    .map_err(|_| ValidationPlanningObservationError::Planning)
}

pub fn resolve_project_validation_target(
    catalog: &ProjectCatalogManifest,
    catalog_root: &Path,
    project_key: &str,
) -> Result<ValidationProjectTarget, ValidationPlanningObservationError> {
    let (project_root, manifest, _) = load_validation_project(catalog, catalog_root, project_key)?;
    let validation_entrypoint =
        canonical_project_file(&project_root, &manifest.validation_entrypoint)?;
    Ok(ValidationProjectTarget {
        project_root,
        validation_entrypoint,
    })
}

fn load_validation_project(
    catalog: &ProjectCatalogManifest,
    catalog_root: &Path,
    project_key: &str,
) -> Result<(PathBuf, ProjectValidationManifest, Vec<u8>), ValidationPlanningObservationError> {
    let entry = catalog
        .projects
        .iter()
        .find(|entry| entry.project_key == project_key)
        .ok_or(ValidationPlanningObservationError::ProjectBoundary)?;
    let status = inspect_project_catalog_entry(catalog, catalog_root, project_key)
        .ok_or(ValidationPlanningObservationError::ProjectBoundary)?;
    if entry.role != CatalogProjectRole::ActiveCanonical
        || entry.repository_kind != CatalogRepositoryKind::Git
        || status.availability != CatalogAvailability::Available
        || status.identity_status != CatalogIdentityStatus::Match
    {
        return Err(ValidationPlanningObservationError::ProjectBoundary);
    }
    let project_root = catalog_root.join(&entry.relative_path);
    let manifest_bytes =
        read_bounded_project_file(&project_root, PROJECT_MANIFEST, MAX_PROJECT_MANIFEST_BYTES)
            .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    let manifest: ProjectValidationManifest = toml::from_str(manifest_text)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    validate_manifest(&manifest, project_key)?;
    Ok((project_root, manifest, manifest_bytes))
}

fn validate_manifest(
    manifest: &ProjectValidationManifest,
    project_key: &str,
) -> Result<(), ValidationPlanningObservationError> {
    let all_paths = manifest
        .classification
        .security_prefixes
        .iter()
        .chain(manifest.classification.data_prefixes.iter())
        .chain(manifest.classification.workflow_release_prefixes.iter())
        .chain(manifest.classification.validator_policy_prefixes.iter())
        .chain(manifest.classification.public_contract_prefixes.iter())
        .chain(manifest.classification.toolchain_paths.iter())
        .chain(manifest.classification.lockfile_paths.iter())
        .chain(manifest.fingerprints.toolchain_paths.iter())
        .chain(manifest.fingerprints.lockfile_paths.iter())
        .chain(manifest.fingerprints.validation_script_paths.iter())
        .chain(manifest.fingerprints.config_paths.iter())
        .chain(manifest.unit_mappings.iter().map(|item| &item.prefix));
    if manifest.schema_version != 1
        || manifest.project_key != project_key
        || manifest.policy_schema_version != VALIDATION_POLICY_SCHEMA_VERSION
        || manifest.evidence_schema_version != EVIDENCE_CONTRACT_SCHEMA_VERSION
        || manifest.default_profile != ValidationProfile::Target
        || manifest.workspace_unit.trim().is_empty()
        || manifest.validation_entrypoint != "scripts/validate.ps1"
        || !safe_relative_path(&manifest.validation_entrypoint)
        || manifest.limits.max_git_output_bytes == 0
        || manifest.limits.max_untracked_file_bytes == 0
        || manifest.limits.max_untracked_total_bytes == 0
        || all_paths.into_iter().any(|path| !safe_relative_path(path))
        || manifest
            .unit_mappings
            .iter()
            .any(|item| item.unit.trim().is_empty())
    {
        return Err(ValidationPlanningObservationError::ProjectManifest);
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.contains(':')
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn git_bytes(
    root: &Path,
    args: &[&str],
    limit: usize,
) -> Result<Vec<u8>, ValidationPlanningObservationError> {
    let output = hidden_command("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|_| ValidationPlanningObservationError::GitObservation)?;
    if !output.status.success() {
        return Err(ValidationPlanningObservationError::GitObservation);
    }
    if output.stdout.len() > limit || output.stderr.len() > limit {
        return Err(ValidationPlanningObservationError::ObservationLimit);
    }
    Ok(output.stdout)
}

fn git_text(
    root: &Path,
    args: &[&str],
    limit: usize,
) -> Result<String, ValidationPlanningObservationError> {
    String::from_utf8(git_bytes(root, args, limit)?)
        .map_err(|_| ValidationPlanningObservationError::GitObservation)
}

fn git_paths(
    root: &Path,
    args: &[&str],
    limit: usize,
) -> Result<Vec<String>, ValidationPlanningObservationError> {
    let bytes = git_bytes(root, args, limit)?;
    let mut paths = bytes
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
        .map(|item| {
            std::str::from_utf8(item)
                .map(|value| value.replace('\\', "/"))
                .map_err(|_| ValidationPlanningObservationError::GitObservation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if paths.iter().any(|path| !safe_relative_path(path)) {
        return Err(ValidationPlanningObservationError::GitObservation);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn hash_untracked(
    root: &Path,
    paths: &[String],
    limits: &ObservationLimits,
) -> Result<(Sha256Hash, bool), ValidationPlanningObservationError> {
    let mut records = Vec::new();
    let mut total = 0_u64;
    let mut complete = true;
    for path in paths {
        let absolute = root.join(path);
        let metadata = fs::symlink_metadata(&absolute)
            .map_err(|_| ValidationPlanningObservationError::GitObservation)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > limits.max_untracked_file_bytes
            || total.saturating_add(metadata.len()) > limits.max_untracked_total_bytes
        {
            complete = false;
            records.push(serde_json::json!({
                "path":path,
                "state":"content_unavailable",
                "size":metadata.len(),
            }));
            continue;
        }
        let absolute = canonical_project_file(root, path)
            .map_err(|_| ValidationPlanningObservationError::GitObservation)?;
        let remaining_total = limits.max_untracked_total_bytes.saturating_sub(total);
        let read_limit = limits.max_untracked_file_bytes.min(remaining_total);
        let mut bytes = Vec::new();
        fs::File::open(&absolute)
            .map_err(|_| ValidationPlanningObservationError::GitObservation)?
            .take(read_limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| ValidationPlanningObservationError::GitObservation)?;
        if bytes.len() as u64 > read_limit {
            complete = false;
            records.push(serde_json::json!({
                "path":path,
                "state":"content_changed_or_limit_exceeded",
                "size":metadata.len(),
            }));
            continue;
        }
        total += bytes.len() as u64;
        records.push(serde_json::json!({
            "path":path,
            "state":"hashed",
            "size":bytes.len(),
            "sha256":Sha256Hash::digest(&bytes),
        }));
    }
    Ok((
        canonical_hash(&serde_json::Value::Array(records))?,
        complete,
    ))
}

fn merge_changed_paths(
    staged: Vec<String>,
    unstaged: Vec<String>,
    untracked: Vec<String>,
    manifest: &ProjectValidationManifest,
    cargo: &CargoWorkspace,
) -> Vec<ValidationChangedFile> {
    let mut sources: BTreeMap<String, BTreeSet<ValidationChangeSource>> = BTreeMap::new();
    for (paths, source) in [
        (staged, ValidationChangeSource::Staged),
        (unstaged, ValidationChangeSource::Unstaged),
        (untracked, ValidationChangeSource::Untracked),
    ] {
        for path in paths {
            sources.entry(path).or_default().insert(source);
        }
    }
    sources
        .into_iter()
        .map(|(path, sources)| ValidationChangedFile {
            change_class: classify_path(&path, &manifest.classification),
            direct_unit: resolve_unit(&path, manifest, cargo),
            path,
            sources: sources.into_iter().collect(),
        })
        .collect()
}

fn classify_path(path: &str, policy: &ClassificationPolicy) -> ValidationChangeClass {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if starts_with_any(path, &policy.security_prefixes) {
        ValidationChangeClass::Security
    } else if starts_with_any(path, &policy.data_prefixes) {
        ValidationChangeClass::DataMigration
    } else if starts_with_any(path, &policy.workflow_release_prefixes) {
        ValidationChangeClass::WorkflowRelease
    } else if starts_with_any(path, &policy.validator_policy_prefixes) {
        ValidationChangeClass::ValidatorPolicy
    } else if path == "Cargo.toml"
        || path.ends_with("/Cargo.toml")
        || starts_with_any(path, &policy.public_contract_prefixes)
    {
        ValidationChangeClass::PublicContract
    } else if policy.toolchain_paths.iter().any(|item| item == path) {
        ValidationChangeClass::Toolchain
    } else if policy.lockfile_paths.iter().any(|item| item == path) {
        ValidationChangeClass::Lockfile
    } else if policy.documentation_extensions.contains(&extension) {
        ValidationChangeClass::Documentation
    } else if policy.configuration_extensions.contains(&extension) {
        ValidationChangeClass::Configuration
    } else if policy.code_extensions.contains(&extension) {
        ValidationChangeClass::InternalCode
    } else {
        ValidationChangeClass::Unknown
    }
}

fn starts_with_any(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn resolve_unit(
    path: &str,
    manifest: &ProjectValidationManifest,
    cargo: &CargoWorkspace,
) -> Option<String> {
    if let Some((_, unit)) = cargo
        .units_by_prefix
        .iter()
        .find(|(prefix, _)| path == prefix.trim_end_matches('/') || path.starts_with(prefix))
    {
        return Some(unit.clone());
    }
    if let Some(mapping) = manifest
        .unit_mappings
        .iter()
        .filter(|mapping| path.starts_with(&mapping.prefix))
        .max_by_key(|mapping| mapping.prefix.len())
    {
        return Some(mapping.unit.clone());
    }
    if matches!(path, "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml") {
        return Some(manifest.workspace_unit.clone());
    }
    if Path::new(path).extension().and_then(|value| value.to_str()) == Some("md") {
        return Some("docs".to_owned());
    }
    None
}

fn load_cargo_workspace(root: &Path) -> CargoWorkspace {
    let result = (|| {
        let root_bytes =
            read_bounded_project_file(root, "Cargo.toml", MAX_CARGO_MANIFEST_BYTES).ok()?;
        let root_value: toml::Value =
            toml::from_str(std::str::from_utf8(&root_bytes).ok()?).ok()?;
        let members = root_value.get("workspace")?.get("members")?.as_array()?;
        if members.len() > 512 {
            return None;
        }
        let mut units = Vec::new();
        let mut manifests = Vec::new();
        for member in members {
            let relative = member.as_str()?.replace('\\', "/");
            if !safe_relative_path(&relative) {
                return None;
            }
            let manifest_path = format!("{}/Cargo.toml", relative.trim_end_matches('/'));
            let manifest_bytes =
                read_bounded_project_file(root, &manifest_path, MAX_CARGO_MANIFEST_BYTES).ok()?;
            let value: toml::Value =
                toml::from_str(std::str::from_utf8(&manifest_bytes).ok()?).ok()?;
            let name = value.get("package")?.get("name")?.as_str()?.to_owned();
            units.push((format!("{}/", relative.trim_end_matches('/')), name.clone()));
            manifests.push((name, value));
        }
        units.sort_by(|left, right| right.0.len().cmp(&left.0.len()).then(left.0.cmp(&right.0)));
        let names: BTreeSet<_> = manifests.iter().map(|(name, _)| name.clone()).collect();
        let mut dependencies = BTreeSet::new();
        for (consumer, manifest) in &manifests {
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                let Some(table) = manifest.get(table_name).and_then(toml::Value::as_table) else {
                    continue;
                };
                for (key, value) in table {
                    let provider = value
                        .as_table()
                        .and_then(|item| item.get("package"))
                        .and_then(toml::Value::as_str)
                        .unwrap_or(key);
                    if names.contains(provider) {
                        dependencies.insert((provider.to_owned(), consumer.clone()));
                    }
                }
            }
        }
        Some((units, names, dependencies))
    })();
    match result {
        Some((units_by_prefix, unit_names, dependencies)) => CargoWorkspace {
            units_by_prefix,
            unit_names,
            dependencies: dependencies
                .into_iter()
                .map(|(provider_unit_id, consumer_unit_id)| UnitDependency {
                    provider_unit_id,
                    consumer_unit_id,
                })
                .collect(),
            complete: true,
        },
        None => CargoWorkspace {
            units_by_prefix: Vec::new(),
            unit_names: BTreeSet::new(),
            dependencies: Vec::new(),
            complete: false,
        },
    }
}

fn hash_files(
    root: &Path,
    paths: &[String],
) -> Result<(Sha256Hash, bool), ValidationPlanningObservationError> {
    let mut complete = true;
    let mut records = Vec::new();
    for path in paths {
        let bytes = canonical_project_file(root, path)
            .ok()
            .and_then(|absolute| fs::File::open(absolute).ok())
            .and_then(|file| {
                let mut bytes = Vec::new();
                file.take(MAX_FINGERPRINT_FILE_BYTES.saturating_add(1))
                    .read_to_end(&mut bytes)
                    .ok()
                    .filter(|_| bytes.len() as u64 <= MAX_FINGERPRINT_FILE_BYTES)
                    .map(|_| bytes)
            });
        match bytes {
            Some(bytes) => records.push(serde_json::json!({
                "path":path,
                "state":"present",
                "size":bytes.len(),
                "sha256":Sha256Hash::digest(&bytes),
            })),
            None => {
                complete = false;
                records.push(serde_json::json!({"path":path,"state":"unavailable"}));
            }
        }
    }
    Ok((
        canonical_hash(&serde_json::Value::Array(records))?,
        complete,
    ))
}

fn canonical_project_file(
    root: &Path,
    relative: &str,
) -> Result<PathBuf, ValidationPlanningObservationError> {
    if !safe_relative_path(relative) {
        return Err(ValidationPlanningObservationError::ProjectManifest);
    }
    let canonical_root =
        fs::canonicalize(root).map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    let joined = root.join(relative);
    let metadata = fs::symlink_metadata(&joined)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ValidationPlanningObservationError::ProjectManifest);
    }
    let canonical = fs::canonicalize(joined)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ValidationPlanningObservationError::ProjectManifest);
    }
    Ok(canonical)
}

fn read_bounded_project_file(
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, ValidationPlanningObservationError> {
    let path = canonical_project_file(root, relative)?;
    let metadata =
        fs::metadata(&path).map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    if metadata.len() > max_bytes {
        return Err(ValidationPlanningObservationError::ObservationLimit);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ValidationPlanningObservationError::ProjectManifest)?;
    if bytes.len() as u64 > max_bytes {
        return Err(ValidationPlanningObservationError::ObservationLimit);
    }
    Ok(bytes)
}

fn command_identity(root: &Path, executable: &str, args: &[&str]) -> (serde_json::Value, bool) {
    match hidden_command(executable)
        .args(args)
        .current_dir(root)
        .output()
    {
        Ok(output) if output.status.success() && output.stdout.len() <= 1_048_576 => (
            serde_json::json!({
                "status":"available",
                "stdout":String::from_utf8_lossy(&output.stdout).trim(),
            }),
            true,
        ),
        _ => (serde_json::json!({"status":"unavailable"}), false),
    }
}

fn hidden_command(executable: &str) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    command
}

fn canonical_hash(
    value: &serde_json::Value,
) -> Result<Sha256Hash, ValidationPlanningObservationError> {
    canonical_sha256(value).map_err(|_| ValidationPlanningObservationError::Planning)
}

fn check_definitions(
    changed: &[ValidationChangedFile],
    cargo: &CargoWorkspace,
    fingerprints: &star_contracts::evidence::ValidationInputFingerprintComponents,
    requested_unit: Option<&str>,
) -> Result<Vec<ValidationCheckDefinition>, ValidationPlanningObservationError> {
    let paths_file = format!(
        "target/validation-plan/{}/changed-paths.txt",
        fingerprints
            .fingerprint()
            .map_err(|_| ValidationPlanningObservationError::Planning)?
            .as_str()
            .trim_start_matches("sha256:")
    );
    let mut definitions = Vec::new();
    for profile in [
        ValidationProfile::Quick,
        ValidationProfile::Target,
        ValidationProfile::Full,
        ValidationProfile::Release,
    ] {
        definitions.extend(base_checks(profile, &paths_file));
    }
    let mut affected_packages: BTreeSet<_> = changed
        .iter()
        .filter_map(|file| file.direct_unit.as_ref())
        .filter(|unit| cargo.unit_names.contains(*unit))
        .cloned()
        .collect();
    if let Some(unit) = requested_unit.filter(|unit| cargo.unit_names.contains(*unit)) {
        affected_packages.insert(unit.to_owned());
    }
    let affected_packages: Vec<_> = affected_packages.into_iter().collect();
    definitions.extend(target_cargo_checks(&affected_packages));
    definitions.extend(full_cargo_checks(ValidationProfile::Full));
    definitions.extend(full_cargo_checks(ValidationProfile::Release));
    definitions.push(check(
        ValidationProfile::Release,
        "cargo-release-build",
        "workspace",
        "cargo",
        &["build", "--workspace", "--release", "--locked"],
        "RELEASE requires a clean release build; external platform gates remain an uncertainty.",
    ));
    Ok(definitions)
}

fn base_checks(profile: ValidationProfile, paths_file: &str) -> Vec<ValidationCheckDefinition> {
    vec![
        check_owned(
            profile,
            "static-files",
            "project",
            "python",
            vec![
                "-X".to_owned(),
                "utf8".to_owned(),
                "scripts/validation/check_files.py".to_owned(),
                "--root".to_owned(),
                ".".to_owned(),
                "--paths-file".to_owned(),
                paths_file.to_owned(),
            ],
            "Parse and validate exactly the changed paths materialized for this plan.",
        ),
        check(
            profile,
            "diff-staged",
            "project",
            "git",
            &["diff", "--cached", "--check"],
            "Reject staged whitespace errors.",
        ),
        check(
            profile,
            "diff-worktree",
            "project",
            "git",
            &["diff", "--check"],
            "Reject worktree whitespace errors.",
        ),
    ]
}

fn target_cargo_checks(packages: &[String]) -> Vec<ValidationCheckDefinition> {
    let mut package_args = Vec::new();
    for package in packages {
        package_args.extend(["-p".to_owned(), package.clone()]);
    }
    let unit = if packages.is_empty() {
        "workspace"
    } else {
        "affected-units"
    };
    let mut fmt = vec!["fmt".to_owned()];
    fmt.extend(package_args.clone());
    fmt.extend(["--".to_owned(), "--check".to_owned()]);
    let mut check_args = vec!["check".to_owned()];
    check_args.extend(package_args.clone());
    check_args.extend(["--all-targets".to_owned(), "--locked".to_owned()]);
    let mut test = vec!["test".to_owned()];
    test.extend(package_args.clone());
    test.push("--locked".to_owned());
    let mut clippy = vec!["clippy".to_owned()];
    clippy.extend(package_args);
    clippy.extend([
        "--all-targets".to_owned(),
        "--locked".to_owned(),
        "--".to_owned(),
        "-D".to_owned(),
        "warnings".to_owned(),
    ]);
    [
        ("cargo-fmt", fmt),
        ("cargo-check", check_args),
        ("cargo-test", test),
        ("cargo-clippy", clippy),
    ]
    .into_iter()
    .map(|(id, args)| {
        check_owned(
            ValidationProfile::Target,
            id,
            unit,
            "cargo",
            args,
            "TARGET validates the directly affected Rust packages.",
        )
    })
    .collect()
}

fn full_cargo_checks(profile: ValidationProfile) -> Vec<ValidationCheckDefinition> {
    vec![
        check(
            profile,
            "cargo-fmt",
            "workspace",
            "cargo",
            &["fmt", "--all", "--", "--check"],
            "FULL validates workspace formatting.",
        ),
        check(
            profile,
            "cargo-check",
            "workspace",
            "cargo",
            &["check", "--workspace", "--all-targets", "--locked"],
            "FULL checks all workspace targets.",
        ),
        check(
            profile,
            "cargo-test",
            "workspace",
            "cargo",
            &["test", "--workspace", "--locked"],
            "FULL tests the workspace.",
        ),
        check(
            profile,
            "cargo-clippy",
            "workspace",
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
            "FULL lints all workspace targets and features.",
        ),
        check(
            profile,
            "schema-check",
            "contracts",
            "cargo",
            &["run", "--locked", "-p", "star-schema-gen", "--", "--check"],
            "FULL checks generated public Schemas.",
        ),
        check(
            profile,
            "mcp-matrix",
            "contracts",
            "cargo",
            &["run", "--locked", "-p", "star-matrix-check"],
            "FULL checks MCP conformance coverage.",
        ),
    ]
}

fn check(
    profile: ValidationProfile,
    id: &str,
    unit: &str,
    executable: &str,
    args: &[&str],
    reason: &str,
) -> ValidationCheckDefinition {
    check_owned(
        profile,
        id,
        unit,
        executable,
        args.iter().map(|value| (*value).to_owned()).collect(),
        reason,
    )
}

fn check_owned(
    profile: ValidationProfile,
    id: &str,
    unit: &str,
    executable: &str,
    args: Vec<String>,
    reason: &str,
) -> ValidationCheckDefinition {
    ValidationCheckDefinition {
        profile,
        check_id: id.to_owned(),
        unit_id: unit.to_owned(),
        command: ValidationCommand {
            executable: executable.to_owned(),
            args,
            working_directory: ".".to_owned(),
            expected_exit_codes: BTreeSet::from([0]),
        },
        selection_reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_readable_contract_paths_are_not_downgraded_by_extension() {
        let source = include_str!("../../../.star-control/project.toml");
        let manifest: ProjectValidationManifest = toml::from_str(source).unwrap();
        assert_eq!(
            classify_path("docs/contracts/example.md", &manifest.classification),
            ValidationChangeClass::PublicContract
        );
        assert_eq!(
            classify_path("specs/example.json", &manifest.classification),
            ValidationChangeClass::PublicContract
        );
        assert_eq!(
            classify_path(
                "crates/control/star-project/Cargo.toml",
                &manifest.classification
            ),
            ValidationChangeClass::PublicContract
        );
        assert_eq!(
            classify_path("README.md", &manifest.classification),
            ValidationChangeClass::Documentation
        );
    }

    #[test]
    fn project_manifest_has_closed_current_versions_and_safe_paths() {
        let source = include_str!("../../../.star-control/project.toml");
        let manifest: ProjectValidationManifest = toml::from_str(source).unwrap();
        assert!(validate_manifest(&manifest, "star-control").is_ok());
        assert!(!safe_relative_path("../escape"));
        assert!(!safe_relative_path("D:/escape"));
    }
}
