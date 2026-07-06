use crate::paths::AI_RUNS_DIR;
use crate::{StateStore, StateStoreError};
use std::fs;
use std::path::Path;

impl StateStore {
    pub fn open(
        project_root: impl AsRef<Path>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, StateStoreError> {
        let project_root = project_root.as_ref();
        if !project_root.exists() {
            return Err(StateStoreError::ProjectRootNotFound {
                path: project_root.to_path_buf(),
            });
        }
        if !project_root.is_dir() {
            return Err(StateStoreError::ProjectRootNotDirectory {
                path: project_root.to_path_buf(),
            });
        }

        let project_root = fs::canonicalize(project_root).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: project_root.to_path_buf(),
                source,
            }
        })?;
        let ai_runs_dir = project_root.join(AI_RUNS_DIR);
        fs::create_dir_all(&ai_runs_dir).map_err(|source| StateStoreError::AiRunsNotWritable {
            path: ai_runs_dir.clone(),
            source,
        })?;

        Ok(Self {
            project_root,
            ai_runs_dir,
            schema_root: schema_root.as_ref().to_path_buf(),
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn ai_runs_dir(&self) -> &Path {
        &self.ai_runs_dir
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }
}
