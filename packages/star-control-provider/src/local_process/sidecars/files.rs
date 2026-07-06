use crate::ProviderAdapterError;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub(crate) fn create_new_output_file(path: &Path) -> Result<File, ProviderAdapterError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ProviderAdapterError::Io {
            path: path.to_path_buf(),
            source,
        })
}
