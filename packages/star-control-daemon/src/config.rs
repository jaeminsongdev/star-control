use crate::constants::DEFAULT_DAEMON_ID;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    daemon_id: String,
    config_root: PathBuf,
    schema_root: PathBuf,
}

impl DaemonConfig {
    pub fn new(
        daemon_id: impl Into<String>,
        config_root: impl Into<PathBuf>,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            daemon_id: daemon_id.into(),
            config_root: config_root.into(),
            schema_root: schema_root.into(),
        }
    }

    pub fn local(config_root: impl Into<PathBuf>, schema_root: impl Into<PathBuf>) -> Self {
        Self::new(DEFAULT_DAEMON_ID, config_root, schema_root)
    }

    pub fn daemon_id(&self) -> &str {
        &self.daemon_id
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }
}
