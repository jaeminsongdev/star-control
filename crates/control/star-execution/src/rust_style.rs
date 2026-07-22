use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    management::{FileOperationKind, PatchSet, PatchSetStatus, ProjectPathRef},
    parse_no_duplicate_keys,
    rust_style::{RustAutoPolicy, RustSourceOwnership},
};
use star_domain::versioned_fingerprint;
use star_validation::rust_style::RustFileSnapshot;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug)]
pub struct RustToolOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub command_fingerprint: Sha256Hash,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RustStyleAdapterError {
    #[error("adapter path or ownership is invalid")]
    InvalidPath,
    #[error("owned preview marker is missing or invalid")]
    NotOwned,
    #[error("source snapshot I/O failed")]
    Io,
    #[error("fixed tool process could not be started")]
    Spawn,
    #[error("tool output was not valid UTF-8")]
    InvalidOutput,
    #[error("fixed command fingerprint failed")]
    Fingerprint,
}

pub trait RustStyleAdapter {
    fn snapshot(&self) -> Result<Vec<RustFileSnapshot>, RustStyleAdapterError>;
    fn materialize_exact(
        &mut self,
        files: &[RustFileSnapshot],
    ) -> Result<(), RustStyleAdapterError>;
    fn run_rustfmt(&mut self, check: bool) -> Result<RustToolOutput, RustStyleAdapterError>;
    fn run_clippy_check(&mut self) -> Result<RustToolOutput, RustStyleAdapterError>;
    fn run_clippy_fix(
        &mut self,
        exact_lint_ids: &[String],
    ) -> Result<RustToolOutput, RustStyleAdapterError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RustCargoScope {
    Workspace,
    Package(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RustStylePatchScope {
    Workspace,
    Package { package: String },
}

impl RustStylePatchScope {
    fn is_valid(&self) -> bool {
        match self {
            Self::Workspace => true,
            Self::Package { package } => valid_package_spec(package),
        }
    }
}

impl RustCargoScope {
    fn cargo_selection_args(&self) -> Result<Vec<String>, RustStyleAdapterError> {
        match self {
            Self::Workspace => Ok(vec!["--workspace".to_owned()]),
            Self::Package(package) if valid_package_spec(package) => {
                Ok(vec!["--package".to_owned(), package.clone()])
            }
            Self::Package(_) => Err(RustStyleAdapterError::InvalidPath),
        }
    }

    fn rustfmt_selection_args(&self) -> Result<Vec<String>, RustStyleAdapterError> {
        match self {
            Self::Workspace => Ok(vec!["--all".to_owned()]),
            Self::Package(package) if valid_package_spec(package) => {
                Ok(vec!["--package".to_owned(), package.clone()])
            }
            Self::Package(_) => Err(RustStyleAdapterError::InvalidPath),
        }
    }
}

pub struct CargoRustStyleAdapter {
    cargo_executable: PathBuf,
    cargo_home: PathBuf,
    toolchain_bin: PathBuf,
    workspace_root: PathBuf,
    target_dir: PathBuf,
    mutation_allowed: bool,
    scope: RustCargoScope,
    features: Vec<String>,
}

impl CargoRustStyleAdapter {
    pub fn check_only(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::check_only_scoped(
            cargo_executable,
            workspace_root,
            target_dir,
            RustCargoScope::Workspace,
        )
    }

    pub fn check_only_scoped(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
        scope: RustCargoScope,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::check_only_configured(
            cargo_executable,
            workspace_root,
            target_dir,
            scope,
            Vec::new(),
        )
    }

    pub fn check_only_configured(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
        scope: RustCargoScope,
        features: Vec<String>,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::new(
            cargo_executable,
            workspace_root,
            target_dir,
            false,
            scope,
            features,
        )
    }

    pub fn owned_preview(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::owned_preview_scoped(
            cargo_executable,
            workspace_root,
            target_dir,
            RustCargoScope::Workspace,
        )
    }

    pub fn owned_preview_scoped(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
        scope: RustCargoScope,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::owned_preview_configured(
            cargo_executable,
            workspace_root,
            target_dir,
            scope,
            Vec::new(),
        )
    }

    pub fn owned_preview_configured(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
        scope: RustCargoScope,
        features: Vec<String>,
    ) -> Result<Self, RustStyleAdapterError> {
        Self::new(
            cargo_executable,
            workspace_root,
            target_dir,
            true,
            scope,
            features,
        )
    }

    fn new(
        cargo_executable: PathBuf,
        workspace_root: PathBuf,
        target_dir: PathBuf,
        mutation_allowed: bool,
        scope: RustCargoScope,
        mut features: Vec<String>,
    ) -> Result<Self, RustStyleAdapterError> {
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|_| RustStyleAdapterError::InvalidPath)?;
        let toolchain_bin = cargo_executable
            .parent()
            .ok_or(RustStyleAdapterError::InvalidPath)?
            .to_path_buf();
        let cargo_home = effective_cargo_home()?;
        if !workspace_root.join("Cargo.toml").is_file()
            || !cargo_executable.is_file()
            || !toolchain_bin.join(executable_name("rustc")).is_file()
            || !toolchain_bin.join(executable_name("rustfmt")).is_file()
            || !toolchain_bin.join(executable_name("cargo-fmt")).is_file()
            || !toolchain_bin
                .join(executable_name("clippy-driver"))
                .is_file()
            || !toolchain_bin
                .join(executable_name("cargo-clippy"))
                .is_file()
            || !target_dir.is_absolute()
            || target_dir.starts_with(&workspace_root)
        {
            return Err(RustStyleAdapterError::InvalidPath);
        }
        if mutation_allowed {
            validate_owned_marker(&workspace_root)?;
        }
        features.sort();
        features.dedup();
        if features.iter().any(|feature| !valid_feature_spec(feature)) {
            return Err(RustStyleAdapterError::InvalidPath);
        }
        Ok(Self {
            cargo_executable,
            cargo_home,
            toolchain_bin,
            workspace_root,
            target_dir,
            mutation_allowed,
            scope,
            features,
        })
    }

    fn run_fixed(&self, args: &[String]) -> Result<RustToolOutput, RustStyleAdapterError> {
        let mut command = Command::new(&self.cargo_executable);
        command
            .args(args)
            .current_dir(&self.workspace_root)
            .env("CARGO_HOME", &self.cargo_home)
            .env("CARGO_TARGET_DIR", &self.target_dir)
            .env("CARGO_NET_OFFLINE", "true")
            .env("RUSTC", self.toolchain_bin.join(executable_name("rustc")))
            .env(
                "PATH",
                prepend_process_path(&self.toolchain_bin)
                    .ok_or(RustStyleAdapterError::InvalidPath)?,
            )
            .env_remove("RUSTFLAGS")
            .env_remove("RUSTDOCFLAGS")
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC")
            .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            .env_remove("CARGO_BUILD_TARGET")
            .env_remove("CARGO_BUILD_RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0");
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let output = command.output().map_err(|_| RustStyleAdapterError::Spawn)?;
        let stdout =
            String::from_utf8(output.stdout).map_err(|_| RustStyleAdapterError::InvalidOutput)?;
        let stderr =
            String::from_utf8(output.stderr).map_err(|_| RustStyleAdapterError::InvalidOutput)?;
        let command_fingerprint = versioned_fingerprint(
            "star.rust-style-fixed-command",
            1,
            &serde_json::json!({
                "cargo_sha256":Sha256Hash::digest(
                    &fs::read(&self.cargo_executable).map_err(|_| RustStyleAdapterError::Io)?
                ),
                "args":args,
                "cwd_role":"owned_preview_or_check_mirror",
                "cargo_target_dir_role":"external_owned_target",
                "network":"offline",
            }),
        )
        .map_err(|_| RustStyleAdapterError::Fingerprint)?;
        Ok(RustToolOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout,
            stderr,
            command_fingerprint,
        })
    }

    pub fn cargo_metadata(&self) -> Result<RustToolOutput, RustStyleAdapterError> {
        self.run_fixed(&[
            "metadata".to_owned(),
            "--format-version".to_owned(),
            "1".to_owned(),
            "--no-deps".to_owned(),
            "--offline".to_owned(),
        ])
    }
}

pub fn effective_cargo_home() -> Result<PathBuf, RustStyleAdapterError> {
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|profile| profile.join(".cargo"))
        })
        .ok_or(RustStyleAdapterError::InvalidPath)?;
    if !cargo_home.is_absolute() || !cargo_home.is_dir() {
        return Err(RustStyleAdapterError::InvalidPath);
    }
    cargo_home
        .canonicalize()
        .map_err(|_| RustStyleAdapterError::InvalidPath)
}

pub fn probe_direct_tool_version(
    executable: &Path,
    verbose: bool,
) -> Result<RustToolOutput, RustStyleAdapterError> {
    if !executable.is_file() {
        return Err(RustStyleAdapterError::InvalidPath);
    }
    let toolchain_bin = executable
        .parent()
        .ok_or(RustStyleAdapterError::InvalidPath)?;
    let args = if verbose {
        vec!["-vV".to_owned()]
    } else {
        vec!["--version".to_owned()]
    };
    let mut command = Command::new(executable);
    command
        .args(&args)
        .env(
            "PATH",
            prepend_process_path(toolchain_bin).ok_or(RustStyleAdapterError::InvalidPath)?,
        )
        .env_remove("RUSTUP_TOOLCHAIN");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().map_err(|_| RustStyleAdapterError::Spawn)?;
    let stdout =
        String::from_utf8(output.stdout).map_err(|_| RustStyleAdapterError::InvalidOutput)?;
    let stderr =
        String::from_utf8(output.stderr).map_err(|_| RustStyleAdapterError::InvalidOutput)?;
    let command_fingerprint = versioned_fingerprint(
        "star.rust-style-tool-version-probe",
        1,
        &serde_json::json!({
            "executable_sha256":Sha256Hash::digest(
                &fs::read(executable).map_err(|_| RustStyleAdapterError::Io)?
            ),
            "args":args,
        }),
    )
    .map_err(|_| RustStyleAdapterError::Fingerprint)?;
    Ok(RustToolOutput {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        command_fingerprint,
    })
}

pub fn materialize_owned_preview(
    source_root: &Path,
    destination_root: &Path,
) -> Result<(), RustStyleAdapterError> {
    let source_root = source_root
        .canonicalize()
        .map_err(|_| RustStyleAdapterError::InvalidPath)?;
    if !source_root.join("Cargo.toml").is_file()
        || source_root
            .join(".star-control-owned-preview.json")
            .exists()
        || !destination_root.is_absolute()
        || destination_root.exists()
        || destination_root.starts_with(&source_root)
    {
        return Err(RustStyleAdapterError::InvalidPath);
    }
    fs::create_dir_all(destination_root).map_err(|_| RustStyleAdapterError::Io)?;
    copy_owned_tree(&source_root, &source_root, destination_root)?;
    fs::write(
        destination_root.join(".star-control-owned-preview.json"),
        b"{\n  \"schema_version\": 1,\n  \"owner\": \"star-control\",\n  \"purpose\": \"rust-style-preview\"\n}\n",
    )
    .map_err(|_| RustStyleAdapterError::Io)?;
    Ok(())
}

fn copy_owned_tree(
    source_root: &Path,
    source_directory: &Path,
    destination_root: &Path,
) -> Result<(), RustStyleAdapterError> {
    let mut entries = fs::read_dir(source_directory)
        .map_err(|_| RustStyleAdapterError::Io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RustStyleAdapterError::Io)?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| RustStyleAdapterError::Io)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(RustStyleAdapterError::InvalidPath);
        }
        let name = entry.file_name();
        if metadata.is_dir()
            && matches!(
                name.to_str(),
                Some(".git" | "target" | "dist" | ".ai-runs" | "--check")
            )
        {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source_root)
            .map_err(|_| RustStyleAdapterError::InvalidPath)?
            .to_path_buf();
        let destination = destination_root.join(relative);
        if metadata.is_dir() {
            fs::create_dir_all(&destination).map_err(|_| RustStyleAdapterError::Io)?;
            copy_owned_tree(source_root, &entry.path(), destination_root)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), destination).map_err(|_| RustStyleAdapterError::Io)?;
        } else {
            return Err(RustStyleAdapterError::InvalidPath);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

impl RustStyleAdapter for CargoRustStyleAdapter {
    fn snapshot(&self) -> Result<Vec<RustFileSnapshot>, RustStyleAdapterError> {
        snapshot_tree(&self.workspace_root)
    }

    fn materialize_exact(
        &mut self,
        files: &[RustFileSnapshot],
    ) -> Result<(), RustStyleAdapterError> {
        if !self.mutation_allowed {
            return Err(RustStyleAdapterError::NotOwned);
        }
        let current = self.snapshot()?;
        if current.iter().map(|file| &file.path).collect::<Vec<_>>()
            != files.iter().map(|file| &file.path).collect::<Vec<_>>()
        {
            return Err(RustStyleAdapterError::InvalidPath);
        }
        for file in files {
            let target = resolve_relative(&self.workspace_root, file.path.as_str())?;
            fs::write(target, &file.bytes).map_err(|_| RustStyleAdapterError::Io)?;
        }
        Ok(())
    }

    fn run_rustfmt(&mut self, check: bool) -> Result<RustToolOutput, RustStyleAdapterError> {
        if !check && !self.mutation_allowed {
            return Err(RustStyleAdapterError::NotOwned);
        }
        let mut args = vec!["fmt".to_owned()];
        args.extend(self.scope.rustfmt_selection_args()?);
        if check {
            args.extend(["--".to_owned(), "--check".to_owned()]);
        }
        self.run_fixed(&args)
    }

    fn run_clippy_check(&mut self) -> Result<RustToolOutput, RustStyleAdapterError> {
        let mut args = vec!["clippy".to_owned()];
        args.extend(self.scope.cargo_selection_args()?);
        append_feature_args(&mut args, &self.features);
        args.extend([
            "--all-targets".to_owned(),
            "--offline".to_owned(),
            "--message-format=json".to_owned(),
            "--no-deps".to_owned(),
        ]);
        let mut output = self.run_fixed(&args)?;
        output.stdout = normalize_clippy_json_stdout(&output.stdout, &self.workspace_root)?;
        Ok(output)
    }

    fn run_clippy_fix(
        &mut self,
        exact_lint_ids: &[String],
    ) -> Result<RustToolOutput, RustStyleAdapterError> {
        if !self.mutation_allowed || exact_lint_ids.is_empty() {
            return Err(RustStyleAdapterError::NotOwned);
        }
        let mut lint_ids = exact_lint_ids.to_vec();
        lint_ids.sort();
        lint_ids.dedup();
        if lint_ids.iter().any(|lint| !exact_lint_id(lint)) {
            return Err(RustStyleAdapterError::InvalidPath);
        }
        let mut args = vec!["clippy".to_owned(), "--fix".to_owned()];
        args.extend(self.scope.cargo_selection_args()?);
        append_feature_args(&mut args, &self.features);
        args.extend([
            "--all-targets".to_owned(),
            "--offline".to_owned(),
            "--allow-dirty".to_owned(),
            "--message-format=json".to_owned(),
            "--no-deps".to_owned(),
            "--".to_owned(),
        ]);
        for lint in lint_ids {
            args.push("-W".to_owned());
            args.push(lint);
        }
        let mut output = self.run_fixed(&args)?;
        output.stdout = normalize_clippy_json_stdout(&output.stdout, &self.workspace_root)?;
        Ok(output)
    }
}

fn normalize_clippy_json_stdout(
    stdout: &str,
    workspace_root: &Path,
) -> Result<String, RustStyleAdapterError> {
    let mut lines = Vec::new();
    for line in stdout.lines() {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(line) else {
            lines.push(line.to_owned());
            continue;
        };
        normalize_json_file_names(&mut value, workspace_root)?;
        lines
            .push(serde_json::to_string(&value).map_err(|_| RustStyleAdapterError::InvalidOutput)?);
    }
    if stdout.ends_with('\n') {
        Ok(format!("{}\n", lines.join("\n")))
    } else {
        Ok(lines.join("\n"))
    }
}

fn normalize_json_file_names(
    value: &mut serde_json::Value,
    workspace_root: &Path,
) -> Result<(), RustStyleAdapterError> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(file_name) = object.get_mut("file_name")
                && let Some(raw) = file_name.as_str()
            {
                let path = PathBuf::from(raw);
                if path.is_absolute() {
                    let relative = path
                        .strip_prefix(workspace_root)
                        .map_err(|_| RustStyleAdapterError::InvalidOutput)?
                        .to_string_lossy()
                        .replace('\\', "/");
                    ProjectPathRef::parse(relative.clone())
                        .map_err(|_| RustStyleAdapterError::InvalidOutput)?;
                    *file_name = serde_json::Value::String(relative);
                }
            }
            for nested in object.values_mut() {
                normalize_json_file_names(nested, workspace_root)?;
            }
        }
        serde_json::Value::Array(values) => {
            for nested in values {
                normalize_json_file_names(nested, workspace_root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStylePatchFile {
    path: ProjectPathRef,
    before_sha256: Sha256Hash,
    after_sha256: Sha256Hash,
    after_utf8: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStyleForwardPatchArtifact {
    schema_id: String,
    schema_version: u32,
    pipeline_ref: String,
    toolchain_fingerprint: Sha256Hash,
    policy_fingerprint: Sha256Hash,
    coverage_fingerprint: Sha256Hash,
    fixed_adapter_fingerprint: Sha256Hash,
    scope: RustStylePatchScope,
    auto_policy: RustAutoPolicy,
    steps: Vec<Sha256Hash>,
    files: Vec<RustStylePatchFile>,
    idempotence: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustStylePatchBinding {
    pub toolchain_fingerprint: Sha256Hash,
    pub policy_fingerprint: Sha256Hash,
    pub coverage_fingerprint: Sha256Hash,
    pub fixed_adapter_fingerprint: Sha256Hash,
    pub scope: RustStylePatchScope,
    pub auto_policy: RustAutoPolicy,
}

pub fn rust_style_patch_binding(
    value: &serde_json::Value,
) -> Result<RustStylePatchBinding, RustStyleAdapterError> {
    let artifact = serde_json::from_value::<RustStyleForwardPatchArtifact>(value.clone())
        .map_err(|_| RustStyleAdapterError::InvalidOutput)?;
    if artifact.schema_id != "star.rust-style-forward-patch"
        || artifact.schema_version != 1
        || artifact.pipeline_ref != "rust_style_v1@1"
        || artifact.idempotence != "proved"
        || artifact.steps.is_empty()
        || artifact.files.is_empty()
        || !artifact.scope.is_valid()
    {
        return Err(RustStyleAdapterError::InvalidOutput);
    }
    Ok(RustStylePatchBinding {
        toolchain_fingerprint: artifact.toolchain_fingerprint,
        policy_fingerprint: artifact.policy_fingerprint,
        coverage_fingerprint: artifact.coverage_fingerprint,
        fixed_adapter_fingerprint: artifact.fixed_adapter_fingerprint,
        scope: artifact.scope,
        auto_policy: artifact.auto_policy,
    })
}

pub fn is_rust_style_patch_artifact(value: &serde_json::Value) -> bool {
    value.get("schema_id").and_then(serde_json::Value::as_str)
        == Some("star.rust-style-forward-patch")
}

pub fn apply_rust_style_patch(
    mut patch_set: PatchSet,
    project_root: &Path,
    artifact_value: &serde_json::Value,
    approved_patch_fingerprint: &str,
) -> Result<super::AppliedPatch, Box<super::ApplyFailure>> {
    if patch_set.status != PatchSetStatus::Proposed
        || patch_set.patch_fingerprint.as_str() != approved_patch_fingerprint
    {
        return Err(super::failure(
            patch_set,
            false,
            "PATCH_APPROVAL_OR_STATE_MISMATCH",
        ));
    }
    let artifact =
        match serde_json::from_value::<RustStyleForwardPatchArtifact>(artifact_value.clone()) {
            Ok(artifact)
                if artifact.schema_id == "star.rust-style-forward-patch"
                    && artifact.schema_version == 1
                    && artifact.pipeline_ref == "rust_style_v1@1"
                    && artifact.idempotence == "proved"
                    && !artifact.steps.is_empty()
                    && !artifact.files.is_empty()
                    && artifact.scope.is_valid() =>
            {
                artifact
            }
            _ => return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
        };
    let artifact_bytes = match serde_json::to_vec_pretty(artifact_value) {
        Ok(bytes) => bytes,
        Err(_) => return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
    };
    if patch_set.patch_artifact_refs.len() != 1
        || patch_set.rollback_artifact_refs.len() != 1
        || patch_set.patch_artifact_refs[0].sha256 != Sha256Hash::digest(&artifact_bytes)
        || patch_set.expected_result_fingerprint.is_none()
        || artifact.files.len() != patch_set.operations.len()
    {
        return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_INVALID"));
    }
    let expected_patch_fingerprint = match versioned_fingerprint(
        "star.rust-style-patch-set",
        1,
        &serde_json::json!({
            "project_id":patch_set.project_id,
            "base_workspace_snapshot_id":patch_set.base_workspace_snapshot_id,
            "change_plan_id":patch_set.change_plan_id,
            "operations":patch_set.operations,
            "forward_artifact_sha256":patch_set.patch_artifact_refs[0].sha256,
            "reverse_artifact_sha256":patch_set.rollback_artifact_refs[0].sha256,
            "expected_after_fingerprint":patch_set.expected_result_fingerprint,
        }),
    ) {
        Ok(fingerprint) => fingerprint,
        Err(_) => return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
    };
    if expected_patch_fingerprint != patch_set.patch_fingerprint {
        return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
    }

    let mut seen = BTreeSet::new();
    let mut prepared: Vec<(PathBuf, Vec<u8>, Vec<u8>, Sha256Hash)> = Vec::new();
    for file in &artifact.files {
        if !seen.insert(file.path.clone()) || !file.path.as_str().ends_with(".rs") {
            return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
        }
        let Some(operation) = patch_set
            .operations
            .iter()
            .find(|operation| operation.path == file.path)
        else {
            return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
        };
        let expected_operation = match versioned_fingerprint(
            "star.rust-style-patch-operation",
            1,
            &serde_json::json!({
                "kind":"modify",
                "path":file.path,
                "before_sha256":file.before_sha256,
                "after_sha256":file.after_sha256,
            }),
        ) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
        };
        if operation.kind != FileOperationKind::Modify
            || operation.rename_from.is_some()
            || operation.before_mode.is_some()
            || operation.after_mode.is_some()
            || operation.before_sha256.as_ref() != Some(&file.before_sha256)
            || operation.after_sha256.as_ref() != Some(&file.after_sha256)
            || operation.operation_fingerprint != expected_operation
        {
            return Err(super::failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
        }
        let path = match super::resolve_safe_file(project_root, &file.path) {
            Ok(path) => path,
            Err(_) => return Err(super::failure(patch_set, false, "PATCH_PATH_UNSAFE")),
        };
        let before = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return Err(super::failure(patch_set, false, "PATCH_READ_FAILED")),
        };
        if Sha256Hash::digest(&before) != file.before_sha256 {
            return Err(super::failure(
                patch_set,
                false,
                "PATCH_TARGET_DIRTY_OR_STALE",
            ));
        }
        let after = file.after_utf8.as_bytes().to_vec();
        if Sha256Hash::digest(&after) != file.after_sha256 || before == after {
            return Err(super::failure(
                patch_set,
                false,
                "PATCH_RESULT_HASH_MISMATCH",
            ));
        }
        prepared.push((path, before, after, file.after_sha256.clone()));
    }

    let mut originals: Vec<(PathBuf, Vec<u8>, Sha256Hash)> = Vec::new();
    for (path, before, after, after_hash) in prepared {
        let before_hash = Sha256Hash::digest(&before);
        let still_current = fs::read(&path)
            .ok()
            .is_some_and(|bytes| Sha256Hash::digest(&bytes) == before_hash);
        if !still_current {
            let partial = !super::rollback_originals(&originals);
            patch_set.status = if partial {
                PatchSetStatus::PartiallyApplied
            } else {
                PatchSetStatus::Failed
            };
            return Err(super::failure(
                patch_set,
                partial,
                "PATCH_TARGET_DIRTY_OR_STALE",
            ));
        }
        if super::replace_file_atomic(&path, &after).is_err() {
            let partial = !super::rollback_originals(&originals);
            patch_set.status = if partial {
                PatchSetStatus::PartiallyApplied
            } else {
                PatchSetStatus::Failed
            };
            return Err(super::failure(patch_set, partial, "PATCH_APPLY_FAILED"));
        }
        originals.push((path, before, after_hash));
    }
    patch_set.status = PatchSetStatus::Applied;
    Ok(super::AppliedPatch {
        patch_set,
        originals,
    })
}

fn snapshot_tree(root: &Path) -> Result<Vec<RustFileSnapshot>, RustStyleAdapterError> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|_| RustStyleAdapterError::Io)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| RustStyleAdapterError::Io)?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let file_type = entry.file_type().map_err(|_| RustStyleAdapterError::Io)?;
            if file_type.is_symlink() {
                return Err(RustStyleAdapterError::InvalidPath);
            }
            let name = entry.file_name();
            if file_type.is_dir() {
                if name != ".git" && name != "target" {
                    pending.push(entry.path());
                }
                continue;
            }
            if !file_type.is_file() {
                return Err(RustStyleAdapterError::InvalidPath);
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| RustStyleAdapterError::InvalidPath)?
                .to_string_lossy()
                .replace('\\', "/");
            let path = star_contracts::management::ProjectPathRef::parse(relative)
                .map_err(|_| RustStyleAdapterError::InvalidPath)?;
            let ownership = classify_ownership(path.as_str());
            files.push(RustFileSnapshot {
                path,
                bytes: fs::read(entry.path()).map_err(|_| RustStyleAdapterError::Io)?,
                ownership,
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn classify_ownership(path: &str) -> RustSourceOwnership {
    if path
        .split('/')
        .any(|segment| matches!(segment, "vendor" | "third_party"))
    {
        RustSourceOwnership::Vendor
    } else if path
        .split('/')
        .any(|segment| matches!(segment, "generated" | "gen" | "out"))
    {
        RustSourceOwnership::Generated
    } else {
        RustSourceOwnership::Handwritten
    }
}

fn validate_owned_marker(root: &Path) -> Result<(), RustStyleAdapterError> {
    let marker = fs::read_to_string(root.join(".star-control-owned-preview.json"))
        .map_err(|_| RustStyleAdapterError::NotOwned)?;
    let value = parse_no_duplicate_keys(&marker).map_err(|_| RustStyleAdapterError::NotOwned)?;
    if value
        != serde_json::json!({
            "schema_version":1,
            "owner":"star-control",
            "purpose":"rust-style-preview"
        })
    {
        return Err(RustStyleAdapterError::NotOwned);
    }
    Ok(())
}

fn resolve_relative(root: &Path, relative: &str) -> Result<PathBuf, RustStyleAdapterError> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.contains(':')
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(RustStyleAdapterError::InvalidPath);
    }
    Ok(root.join(relative))
}

fn exact_lint_id(value: &str) -> bool {
    let Some(name) = value.strip_prefix("clippy::") else {
        return false;
    };
    !name.is_empty()
        && ![
            "all",
            "correctness",
            "style",
            "pedantic",
            "restriction",
            "nursery",
            "cargo",
        ]
        .contains(&name)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_package_spec(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.contains('\0')
        && !value.starts_with('-')
        && !value.chars().any(char::is_whitespace)
}

fn valid_feature_spec(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'/' | b'.')
        })
}

fn append_feature_args(args: &mut Vec<String>, features: &[String]) {
    if !features.is_empty() {
        args.push("--features".to_owned());
        args.push(features.join(","));
    }
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn prepend_process_path(toolchain_bin: &Path) -> Option<std::ffi::OsString> {
    let mut paths = vec![toolchain_bin.to_path_buf()];
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(paths).ok()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn actual_rustfmt_and_clippy_check_multicrate_corpus_without_source_write() {
        let cargo = resolve_cargo();
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        let workspace = repository.join("specs/corpus/rust-style/multicrate");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target = std::env::temp_dir().join(format!(
            "star-rust-style-corpus-target-{}-{nonce}",
            std::process::id()
        ));
        let mut adapter = CargoRustStyleAdapter::check_only(cargo, workspace, target).unwrap();
        let before = adapter.snapshot().unwrap();
        let fmt = adapter.run_rustfmt(true).unwrap();
        assert!(fmt.success, "{}", fmt.stderr);
        let clippy = adapter.run_clippy_check().unwrap();
        assert!(clippy.success, "{}", clippy.stderr);
        assert_eq!(adapter.snapshot().unwrap(), before);
        assert!(before.iter().any(|file| {
            file.path.as_str() == "generated/out.rs"
                && file.ownership == RustSourceOwnership::Generated
        }));
        assert!(before.iter().any(|file| {
            file.path.as_str() == "vendor/vendored.rs"
                && file.ownership == RustSourceOwnership::Vendor
        }));
    }

    fn resolve_cargo() -> PathBuf {
        if let Some(path) = std::env::var_os("CARGO") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return path;
            }
        }
        let output = Command::new("where.exe")
            .arg("cargo.exe")
            .output()
            .expect("where.exe must resolve cargo for the adapter smoke test");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        PathBuf::from(stdout.lines().next().unwrap().trim())
    }
}
