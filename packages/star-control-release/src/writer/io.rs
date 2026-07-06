use crate::error::ReleaseReadinessError;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(super) fn read_json(path: &Path) -> Result<Value, ReleaseReadinessError> {
    let content = fs::read_to_string(path).map_err(|source| ReleaseReadinessError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ReleaseReadinessError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn write_new_json(path: &Path, value: &Value) -> Result<(), ReleaseReadinessError> {
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
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| ReleaseReadinessError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ReleaseReadinessError::WriteFailed {
            path: path.to_path_buf(),
            source,
        })
}
