use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use star_contracts::profile::{
    DevelopmentProfileCatalogEntryV1, DevelopmentProfileCatalogSnapshotV1,
    DevelopmentProfileContractError, DevelopmentProfileDescriptorV1,
    DevelopmentProfileResolutionV1, build_development_profile_catalog,
    resolve_development_profiles,
};
use thiserror::Error;

const MAX_PROFILE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PROFILE_FILES: usize = 64;

#[derive(Debug, Error)]
pub enum ProfileCatalogLoadError {
    #[error("profile catalog path is unsafe or unavailable")]
    UnsafePath,
    #[error("profile catalog contains an unsupported entry")]
    UnsupportedEntry,
    #[error("profile catalog file is too large")]
    TooLarge,
    #[error("profile catalog file is not UTF-8")]
    Encoding,
    #[error("profile catalog contract failed: {0}")]
    Contract(#[from] DevelopmentProfileContractError),
    #[error("profile catalog I/O failed")]
    Io(#[from] std::io::Error),
}

pub fn load_development_profile_catalog(
    root: &Path,
) -> Result<DevelopmentProfileCatalogSnapshotV1, ProfileCatalogLoadError> {
    if !root.is_absolute() {
        return Err(ProfileCatalogLoadError::UnsafePath);
    }
    let root_metadata = std::fs::symlink_metadata(root)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(ProfileCatalogLoadError::UnsafePath);
    }
    let canonical_root = root.canonicalize()?;
    let mut paths = Vec::<PathBuf>::new();
    for entry in std::fs::read_dir(&canonical_root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("toml")
        {
            return Err(ProfileCatalogLoadError::UnsupportedEntry);
        }
        if metadata.len() > MAX_PROFILE_FILE_BYTES {
            return Err(ProfileCatalogLoadError::TooLarge);
        }
        paths.push(path);
        if paths.len() > MAX_PROFILE_FILES {
            return Err(ProfileCatalogLoadError::UnsupportedEntry);
        }
    }
    paths.sort();
    let mut descriptors = Vec::with_capacity(paths.len());
    for path in paths {
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(MAX_PROFILE_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_PROFILE_FILE_BYTES {
            return Err(ProfileCatalogLoadError::TooLarge);
        }
        let source = std::str::from_utf8(&bytes).map_err(|_| ProfileCatalogLoadError::Encoding)?;
        let descriptor = DevelopmentProfileDescriptorV1::parse_toml(source)?;
        if path.file_stem().and_then(|value| value.to_str()) != Some(descriptor.profile_id.as_str())
        {
            return Err(ProfileCatalogLoadError::UnsupportedEntry);
        }
        descriptors.push(descriptor);
    }
    Ok(build_development_profile_catalog(descriptors)?)
}

pub fn show_development_profile<'a>(
    catalog: &'a DevelopmentProfileCatalogSnapshotV1,
    profile_id: &str,
) -> Result<&'a DevelopmentProfileCatalogEntryV1, DevelopmentProfileContractError> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.profile_ref.profile_id == profile_id)
        .ok_or(DevelopmentProfileContractError::NotFound)
}

pub fn resolve_loaded_development_profiles(
    catalog: &DevelopmentProfileCatalogSnapshotV1,
    profile_ids: &[String],
) -> Result<DevelopmentProfileResolutionV1, DevelopmentProfileContractError> {
    resolve_development_profiles(catalog, profile_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_profiles() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("application crate is under workspace/crates/control")
            .join("catalog/profiles")
    }

    #[test]
    fn product_catalog_loads_all_sixteen_and_resolves_deterministically() {
        let catalog = load_development_profile_catalog(&workspace_profiles()).unwrap();
        assert_eq!(catalog.entries.len(), 16);
        let first = resolve_loaded_development_profiles(
            &catalog,
            &[
                "rust_style_auto_fix".to_owned(),
                "security_supply_chain".to_owned(),
            ],
        )
        .unwrap();
        let second = resolve_loaded_development_profiles(
            &catalog,
            &[
                "security_supply_chain".to_owned(),
                "rust_style_auto_fix".to_owned(),
            ],
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .parent_closure
                .iter()
                .any(|item| item.profile_id == "ai_development_validation")
        );
        assert!(
            first
                .required_check_families
                .iter()
                .any(|family| family == "security")
        );
    }
}
