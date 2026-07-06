use super::ProviderRegistryLoader;
use crate::registry_error::ProviderRegistryError;
use std::path::{Component, Path, PathBuf};

impl ProviderRegistryLoader {
    pub(super) fn resolve_input_path(&self, path: &Path) -> Result<PathBuf, ProviderRegistryError> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            self.resolve_registry_entry_path(path.to_string_lossy().as_ref())
        }
    }

    pub(crate) fn resolve_registry_entry_path(
        &self,
        path: &str,
    ) -> Result<PathBuf, ProviderRegistryError> {
        let relative = Path::new(path);
        if relative.is_absolute() {
            return Err(ProviderRegistryError::AbsoluteRegistryPathBlocked {
                path: path.to_string(),
            });
        }
        for component in relative.components() {
            if matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            ) {
                return Err(ProviderRegistryError::PathTraversalBlocked {
                    path: path.to_string(),
                });
            }
        }

        Ok(self.repo_root.join(relative))
    }
}
