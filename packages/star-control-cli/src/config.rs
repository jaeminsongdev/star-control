use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CliConfig {
    repo_root: PathBuf,
}

impl CliConfig {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn schema_root(&self) -> PathBuf {
        self.repo_root.join("specs").join("schemas")
    }
}
