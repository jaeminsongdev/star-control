use std::path::{Path, PathBuf};

mod assembly;
mod contract_io;
mod documents;
mod fields;
mod paths;

#[derive(Debug, Clone)]
pub struct ProviderRegistryLoader {
    repo_root: PathBuf,
    schema_root: PathBuf,
}

impl ProviderRegistryLoader {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        let schema_root = repo_root.join("specs").join("schemas");
        Self {
            repo_root,
            schema_root,
        }
    }

    pub fn with_schema_root(
        repo_root: impl Into<PathBuf>,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            schema_root: schema_root.into(),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }
}
