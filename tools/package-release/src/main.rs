use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use chrono::Utc;
use star_contracts::{
    Sha256Hash, canonical_sha256,
    installation::{
        INSTALLATION_SCHEMA_VERSION, PackageSigningState, RELEASE_FILE_MANIFEST_SCHEMA_ID,
        RUNTIME_GENERATION_MANIFEST_SCHEMA_ID, ReleaseFileEntry, ReleaseFileManifest,
        RuntimeGenerationManifest, RuntimeGenerationRef, TargetArchitecture,
    },
    parse_no_duplicate_keys,
};
use star_controller::authenticode::{AuthenticodeStatus, verify_authenticode};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "star-package-release stage --architecture x64|arm64 --binary-dir <dir> --output <dir> --source-revision <value>\n\
star-package-release reseal --architecture x64|arm64 --stage <dist-stage-dir> --source-revision <value>\n\
star-package-release seal-signed --architecture x64|arm64 --stage <dist-stage-dir> --source-revision <value>\n\
star-package-release verify --architecture x64|arm64 --stage <dir>";

#[derive(Debug)]
enum Action {
    Stage {
        architecture: TargetArchitecture,
        binary_dir: PathBuf,
        output: PathBuf,
        source_revision: String,
    },
    Reseal {
        architecture: TargetArchitecture,
        stage: PathBuf,
        source_revision: String,
    },
    SealSigned {
        architecture: TargetArchitecture,
        stage: PathBuf,
        source_revision: String,
    },
    Verify {
        architecture: TargetArchitecture,
        stage: PathBuf,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> DynResult<()> {
    let action = parse(std::env::args().skip(1).collect())?;
    match action {
        Action::Stage {
            architecture,
            binary_dir,
            output,
            source_revision,
        } => {
            stage_release(
                &workspace_root(),
                &binary_dir,
                &output,
                architecture,
                &source_revision,
                PackageSigningState::UnsignedLocal,
            )?;
            let manifest = verify_stage(&output, architecture)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "stage": output,
                    "product_version": manifest.product_version,
                    "target_architecture": manifest.target_architecture,
                    "file_count": manifest.files.len(),
                    "set_sha256": manifest.set_sha256,
                }))?
            );
        }
        Action::Reseal {
            architecture,
            stage,
            source_revision,
        } => {
            let manifest = reseal_release_stage(
                &stage,
                architecture,
                &source_revision,
                PackageSigningState::UnsignedLocal,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "stage": stage,
                    "product_version": manifest.product_version,
                    "target_architecture": manifest.target_architecture,
                    "file_count": manifest.files.len(),
                    "set_sha256": manifest.set_sha256,
                    "resealed": true,
                }))?
            );
        }
        Action::SealSigned {
            architecture,
            stage,
            source_revision,
        } => {
            let manifest = reseal_release_stage(
                &stage,
                architecture,
                &source_revision,
                PackageSigningState::Signed,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "stage": stage,
                    "product_version": manifest.product_version,
                    "target_architecture": manifest.target_architecture,
                    "file_count": manifest.files.len(),
                    "set_sha256": manifest.set_sha256,
                    "signing": manifest.signing,
                    "resealed": true,
                }))?
            );
        }
        Action::Verify {
            architecture,
            stage,
        } => {
            let manifest = verify_stage(&stage, architecture)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "verified": true,
                    "stage": stage,
                    "target_architecture": manifest.target_architecture,
                    "file_count": manifest.files.len(),
                    "set_sha256": manifest.set_sha256,
                    "source_revision": manifest.source_revision,
                    "signing": manifest.signing,
                }))?
            );
        }
    }
    Ok(())
}

fn parse(args: Vec<String>) -> DynResult<Action> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err(HELP.into());
    };
    let mut options = BTreeMap::<String, Option<String>>::new();
    let mut index = 1;
    while index < args.len() {
        let name = &args[index];
        if !name.starts_with("--") || options.contains_key(name) {
            return Err(format!("unknown or duplicate argument: {name}\n{HELP}").into());
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("{name} requires a value"))?;
        options.insert(name.clone(), Some(value.clone()));
        index += 2;
    }
    let value = |name: &str| -> DynResult<String> {
        options
            .get(name)
            .and_then(Clone::clone)
            .ok_or_else(|| format!("missing {name}").into())
    };
    let architecture = value("--architecture")?
        .parse::<TargetArchitecture>()
        .map_err(|error| error.to_owned())?;
    match action {
        "stage" => {
            let allowed = [
                "--architecture",
                "--binary-dir",
                "--output",
                "--source-revision",
            ];
            reject_unknown(&options, &allowed)?;
            let source_revision = value("--source-revision")?;
            if source_revision.trim().is_empty() || source_revision.len() > 256 {
                return Err("--source-revision must be 1..256 characters".into());
            }
            Ok(Action::Stage {
                architecture,
                binary_dir: value("--binary-dir")?.into(),
                output: value("--output")?.into(),
                source_revision,
            })
        }
        "reseal" | "seal-signed" => {
            reject_unknown(
                &options,
                &["--architecture", "--stage", "--source-revision"],
            )?;
            let source_revision = value("--source-revision")?;
            if source_revision.trim().is_empty() || source_revision.len() > 256 {
                return Err("--source-revision must be 1..256 characters".into());
            }
            if action == "seal-signed"
                && (!matches!(source_revision.len(), 40 | 64)
                    || source_revision
                        .bytes()
                        .any(|byte| !byte.is_ascii_hexdigit()))
            {
                return Err("seal-signed requires an exact 40 or 64 hex source revision".into());
            }
            let stage = value("--stage")?.into();
            if action == "seal-signed" {
                Ok(Action::SealSigned {
                    architecture,
                    stage,
                    source_revision,
                })
            } else {
                Ok(Action::Reseal {
                    architecture,
                    stage,
                    source_revision,
                })
            }
        }
        "verify" => {
            reject_unknown(&options, &["--architecture", "--stage"])?;
            Ok(Action::Verify {
                architecture,
                stage: value("--stage")?.into(),
            })
        }
        _ => Err(HELP.into()),
    }
}

fn reject_unknown(options: &BTreeMap<String, Option<String>>, allowed: &[&str]) -> DynResult<()> {
    if let Some(name) = options
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        Err(format!("unknown option: {name}").into())
    } else {
        Ok(())
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("package tool lives under workspace/tools")
        .to_path_buf()
}

fn stage_release(
    workspace: &Path,
    binary_dir: &Path,
    output: &Path,
    architecture: TargetArchitecture,
    source_revision: &str,
    signing: PackageSigningState,
) -> DynResult<()> {
    require_new_or_empty_directory(output)?;
    fs::create_dir_all(output)?;

    for name in [
        "star.exe",
        "star-controller.exe",
        "star-mcp.exe",
        "star-updater.exe",
    ] {
        copy_file(&binary_dir.join(name), &output.join(name))?;
        verify_pe_architecture(&output.join(name), architecture)?;
    }
    copy_tree(&workspace.join("catalog"), &output.join("catalog"))?;
    copy_tree(
        &workspace.join("specs/schemas/v1"),
        &output.join("schemas/v1"),
    )?;
    copy_tree(
        &workspace.join("specs/examples/valid"),
        &output.join("examples/tool-packages"),
    )?;
    copy_tree(
        &workspace.join("integrations/codex-plugin-template"),
        &output.join("integrations/codex-plugin-template"),
    )?;
    copy_file(
        &workspace.join("LICENSE"),
        &output.join("legal/LICENSE.txt"),
    )?;
    let migrations = workspace.join("migrations");
    if migrations.is_dir() {
        copy_tree(&migrations, &output.join("migrations"))?;
    }

    stage_runtime_generation(
        workspace,
        binary_dir,
        output,
        architecture,
        source_revision,
        signing,
    )?;

    let files = collect_release_entries(output)?;
    let set_sha256 = canonical_sha256(&serde_json::to_value(&files)?)?;
    let manifest = ReleaseFileManifest {
        schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: INSTALLATION_SCHEMA_VERSION,
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        target_architecture: architecture,
        created_at: Utc::now(),
        source_revision: source_revision.to_owned(),
        files,
        generated_files: vec!["star-control-install.v1.json".to_owned()],
        set_sha256,
        signing,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    write_new_file(&output.join("release-manifest.json"), &bytes)?;
    Ok(())
}

/// Re-seals an already staged package after an allowed package-side
/// transformation without granting mutation access to an installed product
/// root. `Signed` is accepted only after every staged executable passes the
/// offline Authenticode policy. The stage must be below this repository's
/// `dist/stage` root and retain the current package identity.
fn reseal_release_stage(
    stage: &Path,
    expected_architecture: TargetArchitecture,
    source_revision: &str,
    signing: PackageSigningState,
) -> DynResult<ReleaseFileManifest> {
    let stage = stage.canonicalize()?;
    let allowed_root = workspace_root().join("dist").join("stage").canonicalize()?;
    if !stage.starts_with(&allowed_root) || stage == allowed_root {
        return Err(format!(
            "reseal stage must be a descendant of the package dist/stage root: {}",
            stage.display()
        )
        .into());
    }
    let manifest_path = stage.join("release-manifest.json");
    let current_text = fs::read_to_string(&manifest_path)?;
    let current: ReleaseFileManifest =
        serde_json::from_value(parse_no_duplicate_keys(&current_text)?)?;
    if current.schema_id != RELEASE_FILE_MANIFEST_SCHEMA_ID
        || current.schema_version != INSTALLATION_SCHEMA_VERSION
        || current.product_version != env!("CARGO_PKG_VERSION")
        || current.target_architecture != expected_architecture
        || current.generated_files != ["star-control-install.v1.json"]
        || current.signing != PackageSigningState::UnsignedLocal
    {
        return Err("reseal stage has an incompatible release manifest".into());
    }
    if signing == PackageSigningState::Signed {
        verify_inventory_matches_manifest(&stage, &current, expected_architecture)?;
        verify_signed_executables(&stage)?;
    }
    reseal_runtime_generation(&stage, expected_architecture, source_revision, signing)?;
    let files = collect_release_entries(&stage)?;
    let set_sha256 = canonical_sha256(&serde_json::to_value(&files)?)?;
    let manifest = ReleaseFileManifest {
        schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: INSTALLATION_SCHEMA_VERSION,
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        target_architecture: expected_architecture,
        created_at: Utc::now(),
        source_revision: source_revision.to_owned(),
        files,
        generated_files: vec!["star-control-install.v1.json".to_owned()],
        set_sha256,
        signing,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(&manifest_path, bytes)?;
    verify_stage(&stage, expected_architecture)?;
    Ok(manifest)
}

fn reseal_runtime_generation(
    stage: &Path,
    expected_architecture: TargetArchitecture,
    source_revision: &str,
    signing: PackageSigningState,
) -> DynResult<()> {
    let generations = stage.join("runtime").join("generations");
    let directories = fs::read_dir(&generations)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    if directories.len() != 1 {
        return Err("stage must contain exactly one Runtime Generation".into());
    }
    let root = directories[0].path();
    let manifest_path = root.join("runtime-generation.v1.json");
    let manifest_text = fs::read_to_string(&manifest_path)?;
    let mut generation: RuntimeGenerationManifest =
        serde_json::from_value(parse_no_duplicate_keys(&manifest_text)?)?;
    if generation.schema_id != RUNTIME_GENERATION_MANIFEST_SCHEMA_ID
        || generation.schema_version != 1
        || generation.generation.generation_id != directories[0].file_name().to_string_lossy()
        || generation.target_architecture != expected_architecture
    {
        return Err("Runtime Generation cannot be resealed".into());
    }
    let controller_path = root.join("star-controller.exe");
    let cli_path = root.join("star-cli-runtime.exe");
    verify_pe_architecture(&controller_path, expected_architecture)?;
    verify_pe_architecture(&cli_path, expected_architecture)?;
    let controller_sha256 = Sha256Hash::digest_reader(fs::File::open(&controller_path)?)?;

    let mut runtime_files = Vec::new();
    for relative in collect_relative_files(&root)? {
        if matches!(
            relative.as_str(),
            "runtime-release-manifest.json" | "runtime-generation.v1.json"
        ) {
            continue;
        }
        let path = root.join(relative.replace('/', "\\"));
        let metadata = fs::metadata(&path)?;
        runtime_files.push(ReleaseFileEntry {
            path: relative,
            size: metadata.len(),
            sha256: Sha256Hash::digest_reader(fs::File::open(path)?)?,
        });
    }
    runtime_files.sort_by(|left, right| left.path.cmp(&right.path));
    let runtime_set_sha256 = canonical_sha256(&serde_json::to_value(&runtime_files)?)?;
    let generation_id = runtime_generation_id(&runtime_set_sha256);
    let current_generation_name = directories[0].file_name();
    let current_generation_id = current_generation_name.to_string_lossy();
    let destination =
        (current_generation_id != generation_id).then(|| generations.join(&generation_id));
    if destination.as_ref().is_some_and(|path| path.exists()) {
        return Err(format!("Runtime Generation already exists: {generation_id}").into());
    }
    let runtime_release = ReleaseFileManifest {
        schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: INSTALLATION_SCHEMA_VERSION,
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        target_architecture: expected_architecture,
        created_at: Utc::now(),
        source_revision: source_revision.to_owned(),
        files: runtime_files,
        generated_files: vec!["runtime-generation.v1.json".to_owned()],
        set_sha256: runtime_set_sha256,
        signing,
    };
    let mut runtime_release_bytes = serde_json::to_vec_pretty(&runtime_release)?;
    runtime_release_bytes.push(b'\n');
    fs::write(
        root.join("runtime-release-manifest.json"),
        &runtime_release_bytes,
    )?;

    generation.generation.generation_id = generation_id.clone();
    generation.generation.release_manifest_sha256 = Sha256Hash::digest(&runtime_release_bytes);
    generation.product_version = env!("CARGO_PKG_VERSION").to_owned();
    generation.target_architecture = expected_architecture;
    generation.controller_sha256 = controller_sha256;
    let mut generation_bytes = serde_json::to_vec_pretty(&generation)?;
    generation_bytes.push(b'\n');
    fs::write(manifest_path, generation_bytes)?;
    if let Some(destination) = destination {
        fs::rename(root, destination)?;
    }
    Ok(())
}

fn verify_signed_executables(stage: &Path) -> DynResult<()> {
    let mut executables = Vec::new();
    collect_executables(stage, stage, &mut executables)?;
    if executables.is_empty() {
        return Err("signed release stage contains no executables".into());
    }
    for executable in executables {
        let bytes = fs::read(&executable)?;
        let digest = Sha256Hash::digest(&bytes);
        let evidence = verify_authenticode(&executable, &digest, "require_valid", None)
            .map_err(|_| format!("Authenticode verification failed: {}", executable.display()))?;
        if evidence.status != AuthenticodeStatus::Valid {
            return Err(format!(
                "Authenticode verification was not valid: {}",
                executable.display()
            )
            .into());
        }
    }
    Ok(())
}

fn verify_inventory_matches_manifest(
    stage: &Path,
    manifest: &ReleaseFileManifest,
    expected_architecture: TargetArchitecture,
) -> DynResult<()> {
    let mut expected = BTreeSet::new();
    let mut casefolded = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for entry in &manifest.files {
        if !valid_relative_path(&entry.path)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !expected.insert(entry.path.clone())
            || !casefolded.insert(entry.path.to_ascii_lowercase())
        {
            return Err("release manifest inventory paths are not canonical and unique".into());
        }
        previous = Some(&entry.path);
    }
    let actual = collect_relative_files(stage)?
        .into_iter()
        .filter(|path| path != "release-manifest.json")
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("signed reseal cannot add or remove staged files".into());
    }
    for name in [
        "star.exe",
        "star-controller.exe",
        "star-mcp.exe",
        "star-updater.exe",
    ] {
        if !expected.contains(name) {
            return Err(format!("signed reseal is missing required runtime: {name}").into());
        }
        verify_pe_architecture(&stage.join(name), expected_architecture)?;
    }
    Ok(())
}

fn collect_executables(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> DynResult<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!("release stage contains a symlink: {}", path.display()).into());
        }
        if metadata.is_dir() {
            collect_executables(root, &path, output)?;
        } else if metadata.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            if !path.starts_with(root) {
                return Err("executable escaped the release stage".into());
            }
            output.push(path);
        }
    }
    Ok(())
}

fn stage_runtime_generation(
    workspace: &Path,
    binary_dir: &Path,
    output: &Path,
    architecture: TargetArchitecture,
    source_revision: &str,
    signing: PackageSigningState,
) -> DynResult<()> {
    let runtime_root = output.join("runtime");
    let staging = runtime_root.join("generation-staging");
    fs::create_dir_all(&staging)?;
    copy_file(
        &binary_dir.join("star-controller.exe"),
        &staging.join("star-controller.exe"),
    )?;
    copy_file(
        &binary_dir.join("star.exe"),
        &staging.join("star-cli-runtime.exe"),
    )?;
    copy_tree(&workspace.join("catalog"), &staging.join("catalog"))?;
    copy_tree(
        &workspace.join("specs/schemas/v1"),
        &staging.join("schemas/v1"),
    )?;
    let runtime_files = collect_release_entries(&staging)?;
    let runtime_set_sha256 = canonical_sha256(&serde_json::to_value(&runtime_files)?)?;
    let generation_id = runtime_generation_id(&runtime_set_sha256);
    let generations = runtime_root.join("generations");
    let generation = generations.join(&generation_id);
    fs::create_dir_all(&generations)?;
    if generation.exists() {
        return Err(format!("Runtime Generation already exists: {generation_id}").into());
    }
    fs::rename(&staging, &generation)?;
    let runtime_release = ReleaseFileManifest {
        schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: INSTALLATION_SCHEMA_VERSION,
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        target_architecture: architecture,
        created_at: Utc::now(),
        source_revision: source_revision.to_owned(),
        files: runtime_files,
        generated_files: vec!["runtime-generation.v1.json".to_owned()],
        set_sha256: runtime_set_sha256,
        signing,
    };
    let mut runtime_release_bytes = serde_json::to_vec_pretty(&runtime_release)?;
    runtime_release_bytes.push(b'\n');
    let runtime_release_hash = Sha256Hash::digest(&runtime_release_bytes);
    write_new_file(
        &generation.join("runtime-release-manifest.json"),
        &runtime_release_bytes,
    )?;
    let controller_sha256 =
        Sha256Hash::digest_reader(fs::File::open(generation.join("star-controller.exe"))?)?;
    let runtime_manifest = RuntimeGenerationManifest {
        schema_id: RUNTIME_GENERATION_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        generation: RuntimeGenerationRef {
            generation_id,
            runtime_root: ".".to_owned(),
            release_manifest_sha256: runtime_release_hash,
        },
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        target_architecture: architecture,
        controller_path: "star-controller.exe".to_owned(),
        controller_sha256,
        cli_runtime_path: "star-cli-runtime.exe".to_owned(),
        catalog_path: "catalog".to_owned(),
        schemas_root: "schemas/v1".to_owned(),
        bridge_contract_version: 2,
    };
    let mut runtime_manifest_bytes = serde_json::to_vec_pretty(&runtime_manifest)?;
    runtime_manifest_bytes.push(b'\n');
    write_new_file(
        &generation.join("runtime-generation.v1.json"),
        &runtime_manifest_bytes,
    )?;
    Ok(())
}

fn runtime_generation_id(runtime_set_sha256: &Sha256Hash) -> String {
    format!("rt_{}", &runtime_set_sha256.as_str()[7..23])
}

fn verify_stage(
    stage: &Path,
    expected_architecture: TargetArchitecture,
) -> DynResult<ReleaseFileManifest> {
    let stage = stage.canonicalize()?;
    let bytes = fs::read(stage.join("release-manifest.json"))?;
    if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
        return Err("release-manifest.json is outside its size bound".into());
    }
    let text = std::str::from_utf8(&bytes)?;
    let value = parse_no_duplicate_keys(text)?;
    let manifest: ReleaseFileManifest = serde_json::from_value(value)?;
    if manifest.schema_id != RELEASE_FILE_MANIFEST_SCHEMA_ID
        || manifest.schema_version != INSTALLATION_SCHEMA_VERSION
        || manifest.product_version != env!("CARGO_PKG_VERSION")
        || semver::Version::parse(&manifest.product_version).is_err()
        || manifest.target_architecture != expected_architecture
        || manifest.source_revision.trim().is_empty()
        || manifest.source_revision.len() > 256
        || manifest.generated_files != ["star-control-install.v1.json"]
        || manifest.files.is_empty()
    {
        return Err("release-file manifest contract mismatch".into());
    }
    let mut expected_paths = BTreeSet::new();
    let mut casefolded_paths = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for entry in &manifest.files {
        if !valid_relative_path(&entry.path)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !expected_paths.insert(entry.path.clone())
            || !casefolded_paths.insert(entry.path.to_ascii_lowercase())
        {
            return Err("release-file manifest paths are not canonical and unique".into());
        }
        previous = Some(&entry.path);
        let path = stage.join(entry.path.replace('/', "\\"));
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.size
        {
            return Err(format!("release file metadata mismatch: {}", entry.path).into());
        }
        let hash = Sha256Hash::digest_reader(fs::File::open(&path)?)?;
        if hash != entry.sha256 {
            return Err(format!("release file hash mismatch: {}", entry.path).into());
        }
    }
    let actual_paths = collect_relative_files(&stage)?
        .into_iter()
        .filter(|path| path != "release-manifest.json")
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return Err("stage contains missing or unmanifested files".into());
    }
    let set_sha256 = canonical_sha256(&serde_json::to_value(&manifest.files)?)?;
    if set_sha256 != manifest.set_sha256 {
        return Err("release file set digest mismatch".into());
    }
    for name in [
        "star.exe",
        "star-controller.exe",
        "star-mcp.exe",
        "star-updater.exe",
    ] {
        if !expected_paths.contains(name) {
            return Err(format!("required runtime is missing: {name}").into());
        }
        verify_pe_architecture(&stage.join(name), expected_architecture)?;
    }
    if manifest.signing == PackageSigningState::Signed {
        verify_signed_executables(&stage)?;
    }
    verify_runtime_generation(
        &stage,
        expected_architecture,
        manifest.signing,
        &manifest.source_revision,
    )?;
    Ok(manifest)
}

fn verify_runtime_generation(
    stage: &Path,
    expected_architecture: TargetArchitecture,
    expected_signing: PackageSigningState,
    expected_source_revision: &str,
) -> DynResult<()> {
    let generations = stage.join("runtime").join("generations");
    let directories = fs::read_dir(&generations)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    if directories.len() != 1 {
        return Err("stage must contain exactly one Runtime Generation".into());
    }
    let root = directories[0].path();
    let generation_bytes = fs::read(root.join("runtime-generation.v1.json"))?;
    let generation: RuntimeGenerationManifest = serde_json::from_value(parse_no_duplicate_keys(
        std::str::from_utf8(&generation_bytes)?,
    )?)?;
    if generation.schema_id != RUNTIME_GENERATION_MANIFEST_SCHEMA_ID
        || generation.schema_version != 1
        || generation.generation.generation_id != directories[0].file_name().to_string_lossy()
        || generation.generation.runtime_root != "."
        || generation.target_architecture != expected_architecture
        || generation.bridge_contract_version != 2
        || generation.controller_path != "star-controller.exe"
        || generation.cli_runtime_path != "star-cli-runtime.exe"
        || generation.catalog_path != "catalog"
        || generation.schemas_root != "schemas/v1"
    {
        return Err("Runtime Generation manifest contract mismatch".into());
    }
    let runtime_release_path = root.join("runtime-release-manifest.json");
    let runtime_release_bytes = fs::read(&runtime_release_path)?;
    if Sha256Hash::digest(&runtime_release_bytes) != generation.generation.release_manifest_sha256 {
        return Err("Runtime Generation release manifest hash mismatch".into());
    }
    let runtime_release: ReleaseFileManifest = serde_json::from_value(parse_no_duplicate_keys(
        std::str::from_utf8(&runtime_release_bytes)?,
    )?)?;
    if runtime_release.schema_id != RELEASE_FILE_MANIFEST_SCHEMA_ID
        || runtime_release.schema_version != INSTALLATION_SCHEMA_VERSION
        || runtime_release.product_version != env!("CARGO_PKG_VERSION")
        || runtime_release.target_architecture != expected_architecture
        || runtime_release.signing != expected_signing
        || runtime_release.source_revision != expected_source_revision
        || runtime_release.generated_files != ["runtime-generation.v1.json"]
    {
        return Err("Runtime Generation release contract mismatch".into());
    }
    let mut expected = BTreeSet::new();
    let mut casefolded = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for entry in &runtime_release.files {
        if !valid_relative_path(&entry.path)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !expected.insert(entry.path.clone())
            || !casefolded.insert(entry.path.to_ascii_lowercase())
        {
            return Err("Runtime Generation file paths are not canonical and unique".into());
        }
        previous = Some(&entry.path);
    }
    let actual = collect_relative_files(&root)?
        .into_iter()
        .filter(|path| {
            path != "runtime-release-manifest.json" && path != "runtime-generation.v1.json"
        })
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("Runtime Generation contains missing or unmanifested files".into());
    }
    for entry in &runtime_release.files {
        let path = root.join(entry.path.replace('/', "\\"));
        if !valid_relative_path(&entry.path)
            || Sha256Hash::digest_reader(fs::File::open(&path)?)? != entry.sha256
        {
            return Err(format!("Runtime Generation file hash mismatch: {}", entry.path).into());
        }
    }
    let set_sha256 = canonical_sha256(&serde_json::to_value(&runtime_release.files)?)?;
    if set_sha256 != runtime_release.set_sha256 {
        return Err("Runtime Generation file set digest mismatch".into());
    }
    if generation.generation.generation_id != runtime_generation_id(&set_sha256) {
        return Err("Runtime Generation ID is not content-addressed".into());
    }
    if Sha256Hash::digest_reader(fs::File::open(root.join("star-controller.exe"))?)?
        != generation.controller_sha256
    {
        return Err("Runtime Generation Controller hash mismatch".into());
    }
    verify_pe_architecture(&root.join("star-controller.exe"), expected_architecture)?;
    verify_pe_architecture(&root.join("star-cli-runtime.exe"), expected_architecture)?;
    Ok(())
}

fn collect_release_entries(root: &Path) -> DynResult<Vec<ReleaseFileEntry>> {
    let mut entries = Vec::new();
    for relative in collect_relative_files(root)? {
        if relative == "release-manifest.json" || relative == "star-control-install.v1.json" {
            continue;
        }
        let path = root.join(relative.replace('/', "\\"));
        let metadata = fs::metadata(&path)?;
        entries.push(ReleaseFileEntry {
            path: relative,
            size: metadata.len(),
            sha256: Sha256Hash::digest_reader(fs::File::open(path)?)?,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_relative_files(root: &Path) -> DynResult<Vec<String>> {
    let mut files = Vec::new();
    collect_relative_files_inner(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_relative_files_inner(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> DynResult<()> {
    let metadata = fs::symlink_metadata(current)?;
    if metadata.file_type().is_symlink() {
        return Err(format!("symlink is not packageable: {}", current.display()).into());
    }
    if metadata.is_file() {
        let relative = current.strip_prefix(root)?;
        let value = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if !valid_relative_path(&value) {
            return Err(format!("invalid package path: {value}").into());
        }
        files.push(value);
    } else if metadata.is_dir() {
        let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            collect_relative_files_inner(root, &child.path(), files)?;
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> DynResult<()> {
    if !source.is_dir() {
        return Err(format!("package source directory is missing: {}", source.display()).into());
    }
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(format!("package source cannot be a symlink: {}", source.display()).into());
    }
    fs::create_dir_all(destination)?;
    let mut children = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let metadata = child.file_type()?;
        if metadata.is_symlink() {
            return Err(format!(
                "package source cannot contain symlinks: {}",
                child.path().display()
            )
            .into());
        }
        if metadata.is_dir() {
            copy_tree(&child.path(), &destination.join(child.file_name()))?;
        } else if metadata.is_file() {
            copy_file(&child.path(), &destination.join(child.file_name()))?;
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> DynResult<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("package source is not a regular file: {}", source.display()).into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> DynResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn require_new_or_empty_directory(path: &Path) -> DynResult<()> {
    if path.exists() && (!path.is_dir() || fs::read_dir(path)?.next().is_some()) {
        return Err(format!(
            "stage output must be absent or empty; refusing to overwrite: {}",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn verify_pe_architecture(path: &Path, architecture: TargetArchitecture) -> DynResult<()> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    if !(0x100..=512 * 1024 * 1024).contains(&length) {
        return Err(format!("runtime PE size is invalid: {}", path.display()).into());
    }
    let mut header = vec![0_u8; usize::try_from(length.min(1024 * 1024))?];
    file.read_exact(&mut header)?;
    if header.get(..2) != Some(b"MZ") || header.len() < 0x40 {
        return Err(format!("runtime is not a PE image: {}", path.display()).into());
    }
    let offset = u32::from_le_bytes(header[0x3c..0x40].try_into()?) as usize;
    if offset.checked_add(6).is_none_or(|end| end > header.len())
        || header.get(offset..offset + 4) != Some(b"PE\0\0")
    {
        return Err(format!("runtime PE header is invalid: {}", path.display()).into());
    }
    let machine = u16::from_le_bytes(header[offset + 4..offset + 6].try_into()?);
    let expected = match architecture {
        TargetArchitecture::X64 => 0x8664,
        TargetArchitecture::Arm64 => 0xaa64,
    };
    if machine != expected {
        return Err(format!(
            "runtime PE architecture mismatch: {} (expected {expected:#06x}, got {machine:#06x})",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_is_closed_and_requires_source_identity() {
        assert!(parse(vec!["stage".to_owned()]).is_err());
        assert!(
            parse(
                [
                    "verify",
                    "--architecture",
                    "x64",
                    "--stage",
                    "D:\\stage",
                    "--signed",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect()
            )
            .is_err()
        );
        assert!(matches!(
            parse(
                [
                    "seal-signed",
                    "--architecture",
                    "x64",
                    "--stage",
                    r"D:\dist\stage\0.1.0\x64",
                    "--source-revision",
                    "0123456789abcdef0123456789abcdef01234567",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            )
            .unwrap(),
            Action::SealSigned { .. }
        ));
        assert!(
            parse(
                [
                    "seal-signed",
                    "--architecture",
                    "x64",
                    "--stage",
                    r"D:\dist\stage\0.1.0\x64",
                    "--source-revision",
                    "dirty:0123456789abcdef0123456789abcdef01234567",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect()
            )
            .is_err()
        );
    }

    #[test]
    fn relative_paths_reject_windows_escape_forms() {
        for value in ["", "../x", "a/../x", "C:/x", "a\\x", "a//x"] {
            assert!(!valid_relative_path(value), "{value}");
        }
        assert!(valid_relative_path("schemas/v1/example.schema.json"));
    }

    #[test]
    fn current_test_binary_has_the_host_pe_machine() {
        let expected = match std::env::consts::ARCH {
            "x86_64" => TargetArchitecture::X64,
            "aarch64" => TargetArchitecture::Arm64,
            other => panic!("unsupported test architecture {other}"),
        };
        verify_pe_architecture(&std::env::current_exe().unwrap(), expected).unwrap();
    }

    #[test]
    fn runtime_generation_identity_tracks_stage_and_reseal_payload_bytes() {
        let root = std::env::temp_dir().join(format!(
            "star-package-release-runtime-id-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let binary_dir = root.join("bin");
        fs::create_dir_all(&binary_dir).unwrap();
        let current = std::env::current_exe().unwrap();
        fs::copy(&current, binary_dir.join("star.exe")).unwrap();
        fs::copy(&current, binary_dir.join("star-controller.exe")).unwrap();
        let architecture = match std::env::consts::ARCH {
            "x86_64" => TargetArchitecture::X64,
            "aarch64" => TargetArchitecture::Arm64,
            other => panic!("unsupported test architecture {other}"),
        };
        let source_revision = "0123456789abcdef0123456789abcdef01234567";
        let first = root.join("first");
        stage_runtime_generation(
            &workspace_root(),
            &binary_dir,
            &first,
            architecture,
            source_revision,
            PackageSigningState::UnsignedLocal,
        )
        .unwrap();
        let first_generation = fs::read_dir(first.join("runtime/generations"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();

        fs::OpenOptions::new()
            .append(true)
            .open(
                first
                    .join("runtime/generations")
                    .join(&first_generation)
                    .join("star-cli-runtime.exe"),
            )
            .unwrap()
            .write_all(b"payload-change")
            .unwrap();
        reseal_runtime_generation(
            &first,
            architecture,
            source_revision,
            PackageSigningState::UnsignedLocal,
        )
        .unwrap();
        verify_runtime_generation(
            &first,
            architecture,
            PackageSigningState::UnsignedLocal,
            source_revision,
        )
        .unwrap();
        let resealed_generation = fs::read_dir(first.join("runtime/generations"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();

        fs::OpenOptions::new()
            .append(true)
            .open(binary_dir.join("star.exe"))
            .unwrap()
            .write_all(b"payload-change")
            .unwrap();
        let second = root.join("second");
        stage_runtime_generation(
            &workspace_root(),
            &binary_dir,
            &second,
            architecture,
            source_revision,
            PackageSigningState::UnsignedLocal,
        )
        .unwrap();
        let second_generation = fs::read_dir(second.join("runtime/generations"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();

        assert_ne!(first_generation, resealed_generation);
        assert_eq!(resealed_generation, second_generation);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_seal_rejects_an_unsigned_or_invalid_executable() {
        let root = std::env::temp_dir().join(format!(
            "star-package-release-unsigned-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let executable = root.join(format!(
            "fixture-{}.exe",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::write(executable, b"MZ unsigned fixture").unwrap();
        assert!(verify_signed_executables(&root).is_err());
    }

    #[test]
    fn signed_reseal_rejects_inventory_drift_before_rehashing() {
        let root = std::env::temp_dir().join(format!(
            "star-package-release-inventory-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("declared.bin"), b"declared").unwrap();
        fs::write(root.join("unexpected.bin"), b"unexpected").unwrap();
        let manifest = ReleaseFileManifest {
            schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: INSTALLATION_SCHEMA_VERSION,
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            target_architecture: TargetArchitecture::X64,
            created_at: Utc::now(),
            source_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            files: vec![ReleaseFileEntry {
                path: "declared.bin".to_owned(),
                size: 8,
                sha256: Sha256Hash::digest(b"declared"),
            }],
            generated_files: vec!["star-control-install.v1.json".to_owned()],
            set_sha256: Sha256Hash::digest(b"fixture"),
            signing: PackageSigningState::UnsignedLocal,
        };
        assert!(
            verify_inventory_matches_manifest(&root, &manifest, TargetArchitecture::X64).is_err()
        );
    }
}
