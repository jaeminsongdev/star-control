use super::replace::replace_file;
use super::time::timestamp_nanos;
use crate::{StateStore, StateStoreError};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

impl StateStore {
    pub(crate) fn write_bytes_atomic(
        &self,
        job_id: &str,
        target_path: &Path,
        bytes: &[u8],
    ) -> Result<(), StateStoreError> {
        let job_dir = self.job_dir(job_id)?;
        let tmp_dir = job_dir.join("tmp");
        fs::create_dir_all(&tmp_dir).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: tmp_dir.clone(),
            source,
        })?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let tmp_name = format!(
            "{}.tmp-{}-{}",
            target_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or("artifact.json"),
            std::process::id(),
            timestamp_nanos()
        );
        let tmp_path = tmp_dir.join(tmp_name);
        {
            let mut file =
                File::create(&tmp_path).map_err(|source| StateStoreError::AtomicWriteFailed {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(bytes)
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_all())
                .map_err(|source| StateStoreError::AtomicWriteFailed {
                    path: tmp_path.clone(),
                    source,
                })?;
        }

        replace_file(&tmp_path, target_path).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: target_path.to_path_buf(),
            source,
        })
    }
}
