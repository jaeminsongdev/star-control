mod approval;
mod enqueue;
mod fields;
mod schema;
mod state;

use crate::config::DaemonConfig;
use crate::constants::{DAEMON_DIR, DAEMON_STATE_FILE};
use crate::error::DaemonError;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DaemonQueue {
    config: DaemonConfig,
    daemon_dir: PathBuf,
    state_path: PathBuf,
}

impl DaemonQueue {
    pub fn open(config: DaemonConfig) -> Result<Self, DaemonError> {
        let daemon_dir = config.config_root().join(DAEMON_DIR);
        fs::create_dir_all(&daemon_dir).map_err(|source| DaemonError::ConfigDirectoryFailed {
            path: daemon_dir.clone(),
            source,
        })?;
        let state_path = daemon_dir.join(DAEMON_STATE_FILE);
        let queue = Self {
            config,
            daemon_dir,
            state_path,
        };
        if !queue.state_path.is_file() {
            queue.save_state(&queue.default_state())?;
        } else {
            queue.load_state()?;
        }
        Ok(queue)
    }

    pub fn daemon_dir(&self) -> &Path {
        &self.daemon_dir
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }
}
