//! Pipe-owner PID and executable-image verification.

use std::{
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
};

use tokio::net::windows::named_pipe::NamedPipeClient;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
            Threading::{
                OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::PWSTR,
};

pub fn process_image(pid: u32) -> windows::core::Result<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? };
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    unsafe {
        let _ = CloseHandle(process);
    }
    result?;
    Ok(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

pub fn pipe_server_pid(pipe: &NamedPipeClient) -> windows::core::Result<u32> {
    let mut pid = 0;
    unsafe {
        GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle() as *mut _), &mut pid)?;
    }
    Ok(pid)
}

pub fn pipe_client_pid(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
) -> windows::core::Result<u32> {
    let mut pid = 0;
    unsafe {
        GetNamedPipeClientProcessId(HANDLE(pipe.as_raw_handle() as *mut _), &mut pid)?;
    }
    Ok(pid)
}

/// Client-side squatting defense: the pipe owner must be the expected installed
/// Controller executable after Win32 path canonicalization.
pub fn verify_pipe_server_image(
    pipe: &NamedPipeClient,
    expected: &Path,
) -> windows::core::Result<u32> {
    let pid = pipe_server_pid(pipe)?;
    let actual = process_image(pid)?;
    let expected = expected
        .canonicalize()
        .map_err(|_| windows::core::Error::from_thread())?;
    let actual = actual
        .canonicalize()
        .map_err(|_| windows::core::Error::from_thread())?;
    if !actual
        .as_os_str()
        .eq_ignore_ascii_case(expected.as_os_str())
    {
        return Err(windows::core::Error::from_thread());
    }
    Ok(pid)
}

/// Controller-side peer validation.  The PID in Hello must be the PID Win32
/// reports for this connection, and the image must be one of the two installed
/// command clients next to the Controller.  This is intentionally a name/path
/// allowlist, not PATH lookup.
pub fn verify_pipe_client_image(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
    declared_pid: u32,
    allowed_install_directories: &[&Path],
) -> windows::core::Result<PathBuf> {
    let actual_pid = pipe_client_pid(pipe)?;
    if actual_pid != declared_pid {
        return Err(windows::core::Error::from_thread());
    }
    let image = process_image(actual_pid)?;
    let image = image
        .canonicalize()
        .map_err(|_| windows::core::Error::from_thread())?;
    let allowed_install_directories = allowed_install_directories
        .iter()
        .map(|directory| {
            directory
                .canonicalize()
                .map_err(|_| windows::core::Error::from_thread())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let allowed_name = image
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("star-mcp.exe") || name.eq_ignore_ascii_case("star.exe")
        });
    if !allowed_name
        || !allowed_install_directories
            .iter()
            .any(|directory| image.parent() == Some(directory.as_path()))
    {
        return Err(windows::core::Error::from_thread());
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_the_current_process_image() {
        let image = process_image(std::process::id()).expect("current process image is queryable");
        assert!(image.is_absolute());
        assert!(image.exists());
    }
}
