use super::issue::RecoveryIssue;
use crate::{StateStore, StateStoreError};
use std::fs;
use std::path::Path;

impl StateStore {
    pub(crate) fn tmp_file_issues(
        &self,
        job_id: &str,
    ) -> Result<Vec<RecoveryIssue>, StateStoreError> {
        let tmp_dir = self.resolve_job_path(job_id, "tmp")?;
        if !tmp_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut issues = Vec::new();
        collect_tmp_file_issues(&tmp_dir, "tmp", &mut issues)?;
        issues.sort_by(|left, right| left.artifact_path.cmp(&right.artifact_path));
        Ok(issues)
    }
}

fn collect_tmp_file_issues(
    directory: &Path,
    relative_dir: &str,
    issues: &mut Vec<RecoveryIssue>,
) -> Result<(), StateStoreError> {
    for entry in fs::read_dir(directory).map_err(|source| StateStoreError::AiRunsNotWritable {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
            path: directory.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        let relative_path = format!("{}/{}", relative_dir, name);
        let file_type = entry
            .file_type()
            .map_err(|source| StateStoreError::AiRunsNotWritable {
                path: entry.path(),
                source,
            })?;
        if file_type.is_dir() {
            collect_tmp_file_issues(&entry.path(), &relative_path, issues)?;
        } else if file_type.is_file() {
            issues.push(RecoveryIssue::new(
                relative_path,
                "partial_tmp_file",
                "warn",
                "tmp file is not a canonical artifact",
                "leave the tmp file untouched until an explicit discard-tmp recovery command is approved",
            ));
        }
    }
    Ok(())
}
