use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use star_contracts::{canonical::Sha256Hash, schema::generated_documents};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;
type GeneratedFile = (PathBuf, Vec<u8>);

fn generated_files(root: &Path) -> DynResult<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    let mut manifest = Vec::new();
    for (name, document) in generated_documents() {
        let bytes = serde_json::to_vec_pretty(&document)?;
        manifest
            .push(serde_json::json!({"file": name, "hash": Sha256Hash::digest(&bytes).as_str()}));
        files.push((root.join(name), bytes));
    }
    files.push((
        root.parent()
            .ok_or("schema output has no parent")?
            .join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"schema_version": 1, "files": manifest}))?,
    ));
    Ok(files)
}

fn write_generated(root: &Path) -> DynResult<()> {
    fs::create_dir_all(root)?;
    for (path, bytes) in generated_files(root)? {
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn check_generated(root: &Path) -> DynResult<()> {
    let generated = generated_files(root)?;
    let expected_schema_paths: BTreeSet<_> = generated
        .iter()
        .filter(|(path, _)| path.parent() == Some(root))
        .map(|(path, _)| path.clone())
        .collect();
    let mut drift = Vec::new();
    for (path, expected) in &generated {
        match fs::read(path) {
            Ok(actual) if actual == *expected => {}
            Ok(_) => drift.push(format!("changed: {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drift.push(format!("missing: {}", path.display()));
            }
            Err(error) => return Err(error.into()),
        }
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && !expected_schema_paths.contains(&path)
        {
            drift.push(format!("stale: {}", path.display()));
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!("generated schema drift:\n{}", drift.join("\n")).into())
    }
}

fn main() -> DynResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let default_root = PathBuf::from("specs/schemas/v1");
    match args.as_slice() {
        [] => write_generated(&default_root),
        [flag] if flag == "--check" => check_generated(&default_root),
        [root] => write_generated(Path::new(root)),
        _ => Err("usage: star-schema-gen [--check | output-directory]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_mode_detects_missing_changed_and_stale_schema_files_without_writing() {
        let parent = std::env::temp_dir().join(format!("star-schema-gen-{}", std::process::id()));
        let root = parent.join(format!("v1-{}", star_contracts::ids::RequestId::new()));
        write_generated(&root).unwrap();
        check_generated(&root).unwrap();

        let first = generated_files(&root).unwrap().remove(0).0;
        fs::write(&first, b"changed").unwrap();
        assert!(check_generated(&root).is_err());

        write_generated(&root).unwrap();
        fs::write(root.join("stale.schema.json"), b"{}").unwrap();
        assert!(check_generated(&root).is_err());
    }
}
