use crate::error::ReleaseReadinessError;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(super) fn write_new_text(path: &Path, content: &str) -> Result<(), ReleaseReadinessError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ReleaseReadinessError::WriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(content.as_bytes())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })
}
