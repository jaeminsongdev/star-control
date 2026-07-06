use crate::error::DaemonError;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write_bytes_atomic(
    tmp_dir: &Path,
    target_path: &Path,
    bytes: &[u8],
) -> Result<(), DaemonError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|source| DaemonError::StateWriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(tmp_dir).map_err(|source| DaemonError::StateWriteFailed {
        path: tmp_dir.to_path_buf(),
        source,
    })?;

    let tmp_name = format!(
        "{}.tmp-{}-{}",
        target_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("state.json"),
        std::process::id(),
        timestamp_nanos()
    );
    let tmp_path = tmp_dir.join(tmp_name);
    {
        let mut file = File::create(&tmp_path).map_err(|source| DaemonError::StateWriteFailed {
            path: tmp_path.clone(),
            source,
        })?;
        file.write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| DaemonError::StateWriteFailed {
                path: tmp_path.clone(),
                source,
            })?;
    }
    replace_file(&tmp_path, target_path).map_err(|source| DaemonError::StateWriteFailed {
        path: target_path.to_path_buf(),
        source,
    })
}

pub(crate) fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let source = wide(source);
    let target = wide(target);
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}
