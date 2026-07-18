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

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "star-package-release stage --architecture x64|arm64 --binary-dir <dir> --output <dir> --source-revision <value>\n\
star-package-release verify --architecture x64|arm64 --stage <dir>";

#[derive(Debug)]
enum Action {
    Stage {
        architecture: TargetArchitecture,
        binary_dir: PathBuf,
        output: PathBuf,
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

    for name in ["star.exe", "star-controller.exe", "star-mcp.exe"] {
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

fn stage_runtime_generation(
    workspace: &Path,
    binary_dir: &Path,
    output: &Path,
    architecture: TargetArchitecture,
    source_revision: &str,
    signing: PackageSigningState,
) -> DynResult<()> {
    let digest = Sha256Hash::digest(
        format!(
            "{}:{}:{}",
            env!("CARGO_PKG_VERSION"),
            architecture,
            source_revision
        )
        .as_bytes(),
    );
    let generation_id = format!("rt_{}", &digest.as_str()[7..23]);
    let generation = output
        .join("runtime")
        .join("generations")
        .join(&generation_id);
    fs::create_dir_all(&generation)?;
    copy_file(
        &binary_dir.join("star-controller.exe"),
        &generation.join("star-controller.exe"),
    )?;
    copy_file(
        &binary_dir.join("star.exe"),
        &generation.join("star-cli-runtime.exe"),
    )?;
    copy_tree(&workspace.join("catalog"), &generation.join("catalog"))?;
    copy_tree(
        &workspace.join("specs/schemas/v1"),
        &generation.join("schemas/v1"),
    )?;
    let runtime_files = collect_release_entries(&generation)?;
    let runtime_set_sha256 = canonical_sha256(&serde_json::to_value(&runtime_files)?)?;
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
        || manifest.signing != PackageSigningState::UnsignedLocal
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
    for name in ["star.exe", "star-controller.exe", "star-mcp.exe"] {
        if !expected_paths.contains(name) {
            return Err(format!("required runtime is missing: {name}").into());
        }
        verify_pe_architecture(&stage.join(name), expected_architecture)?;
    }
    verify_runtime_generation(&stage, expected_architecture)?;
    Ok(manifest)
}

fn verify_runtime_generation(
    stage: &Path,
    expected_architecture: TargetArchitecture,
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
        || runtime_release.generated_files != ["runtime-generation.v1.json"]
    {
        return Err("Runtime Generation release contract mismatch".into());
    }
    let expected = runtime_release
        .files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
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
}
