//! Durable DPAPI key file lifecycle. Rotation decisions stay with Controller.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW,
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            DACL_SECURITY_INFORMATION, GetFileSecurityW, PROTECTED_DACL_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR, SetFileSecurityW,
        },
    },
    core::{HSTRING, PWSTR, w},
};

use thiserror::Error;

use crate::{
    IpcKey,
    dpapi::{protect_current_user, unprotect_current_user},
};

pub const IPC_KEY_BYTES: usize = 32;

#[derive(Debug, Error)]
pub enum KeyStoreError {
    #[error("LOCALAPPDATA is not available")]
    LocalAppDataUnavailable,
    #[error("IPC key file I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("IPC key DPAPI operation failed: {0}")]
    Dpapi(#[from] windows::core::Error),
    #[error("IPC key DACL operation failed: {0}")]
    Dacl(windows::core::Error),
    #[error("IPC key blob is corrupt or has an invalid size")]
    Corrupt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyRecoveryAudit {
    Loaded,
    RewroteLiveKey,
    RotatedMissingWithoutOwner,
    RotatedCorruptWithoutOwner { preserved_as: PathBuf },
}

pub struct KeyRecovery {
    pub key: IpcKey,
    pub audit: KeyRecoveryAudit,
}

pub fn default_key_path() -> Result<PathBuf, KeyStoreError> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or(KeyStoreError::LocalAppDataUnavailable)?,
    )
    .join("Star-Control")
    .join("secrets")
    .join("ipc-key.v1"))
}

pub fn load(path: &Path) -> Result<IpcKey, KeyStoreError> {
    let encrypted = fs::read(path)?;
    let plaintext = unprotect_current_user(&encrypted).map_err(|_| KeyStoreError::Corrupt)?;
    if plaintext.len() != IPC_KEY_BYTES {
        return Err(KeyStoreError::Corrupt);
    }
    Ok(IpcKey::from_unsealed(plaintext))
}

/// Writes a new DPAPI blob through a sibling temporary file then atomically
/// replaces the target. A corrupt existing blob is deliberately not overwritten.
pub fn store_atomic(path: &Path, key: &IpcKey) -> Result<(), KeyStoreError> {
    if key.as_bytes().len() != IPC_KEY_BYTES {
        return Err(KeyStoreError::Corrupt);
    }
    let encrypted = protect_current_user(key.as_bytes())?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("key path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".ipc-key-{}.tmp", crate::nonce()));
    fs::write(&temporary, encrypted)?;
    let file = fs::OpenOptions::new().write(true).open(&temporary)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    apply_owner_system_dacl(path)?;
    Ok(())
}

/// The file is independently protected because a current-user pipe DACL does
/// not stop another local process from reading a loose DPAPI blob. `OW` binds
/// to the actual file owner and `SY` preserves installer recovery.
/// Applies the same protected owner+LocalSystem ACL used for the IPC DPAPI
/// key to another Controller-private state file.
pub fn apply_owner_system_dacl(path: &Path) -> Result<(), KeyStoreError> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let file_name = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;GA;;;OW)(A;;GA;;;SY)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(KeyStoreError::Dacl)?;
        let result = SetFileSecurityW(
            &file_name,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
        .ok();
        let _ = LocalFree(Some(HLOCAL(descriptor.0 as *mut _)));
        result.map_err(KeyStoreError::Dacl)?;
    }
    Ok(())
}

pub fn file_dacl_sddl(path: &Path) -> Result<String, KeyStoreError> {
    let file_name = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    let mut required = 0u32;
    unsafe {
        let _ = GetFileSecurityW(
            &file_name,
            DACL_SECURITY_INFORMATION.0,
            None,
            0,
            &mut required,
        );
    }
    if required == 0 {
        return Err(KeyStoreError::Corrupt);
    }
    let mut descriptor = vec![0u8; required as usize];
    let security_descriptor = PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast());
    unsafe {
        GetFileSecurityW(
            &file_name,
            DACL_SECURITY_INFORMATION.0,
            Some(security_descriptor),
            required,
            &mut required,
        )
        .ok()
        .map_err(KeyStoreError::Dacl)?;
    }
    let mut sddl = PWSTR::null();
    unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            security_descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut sddl,
            None,
        )
        .map_err(KeyStoreError::Dacl)?;
    }
    let value = unsafe { sddl.to_string() }.map_err(|_| KeyStoreError::Corrupt)?;
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sddl.0.cast())));
    }
    Ok(value)
}

pub fn load_or_create(path: &Path) -> Result<IpcKey, KeyStoreError> {
    match load(path) {
        Ok(key) => Ok(key),
        Err(KeyStoreError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            let key = IpcKey::from_unsealed(rand::random::<[u8; IPC_KEY_BYTES]>().to_vec());
            store_atomic(path, &key)?;
            Ok(key)
        }
        Err(error) => Err(error),
    }
}

/// Reconciles disk state with the Controller's in-memory key.  A live owner is
/// always authoritative, so deletion, corruption, or a different valid blob
/// is rewritten with the same bytes.  Rotation is allowed only when no owner
/// exists; corrupt evidence is preserved beside the key before rotation.
pub fn reconcile(
    path: &Path,
    live_owner_key: Option<&IpcKey>,
) -> Result<KeyRecovery, KeyStoreError> {
    match load(path) {
        Ok(disk_key) => {
            if let Some(live_key) = live_owner_key
                && disk_key.as_bytes() != live_key.as_bytes()
            {
                store_atomic(path, live_key)?;
                return Ok(KeyRecovery {
                    key: IpcKey::from_unsealed(live_key.as_bytes().to_vec()),
                    audit: KeyRecoveryAudit::RewroteLiveKey,
                });
            }
            Ok(KeyRecovery {
                key: disk_key,
                audit: KeyRecoveryAudit::Loaded,
            })
        }
        Err(KeyStoreError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            let (key, audit) = if let Some(live_key) = live_owner_key {
                (
                    IpcKey::from_unsealed(live_key.as_bytes().to_vec()),
                    KeyRecoveryAudit::RewroteLiveKey,
                )
            } else {
                (
                    IpcKey::from_unsealed(rand::random::<[u8; IPC_KEY_BYTES]>().to_vec()),
                    KeyRecoveryAudit::RotatedMissingWithoutOwner,
                )
            };
            store_atomic(path, &key)?;
            Ok(KeyRecovery { key, audit })
        }
        Err(KeyStoreError::Corrupt) => {
            if let Some(live_key) = live_owner_key {
                store_atomic(path, live_key)?;
                return Ok(KeyRecovery {
                    key: IpcKey::from_unsealed(live_key.as_bytes().to_vec()),
                    audit: KeyRecoveryAudit::RewroteLiveKey,
                });
            }
            let preserved_as = path.with_extension(format!("corrupt-{}", crate::nonce()));
            fs::rename(path, &preserved_as)?;
            let key = IpcKey::from_unsealed(rand::random::<[u8; IPC_KEY_BYTES]>().to_vec());
            store_atomic(path, &key)?;
            Ok(KeyRecovery {
                key,
                audit: KeyRecoveryAudit::RotatedCorruptWithoutOwner { preserved_as },
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // matrix: MCP-I012 MCP-I016 MCP-S015
    fn creates_then_reloads_the_same_dpapi_key() {
        let path = std::env::temp_dir().join(format!(
            "star-control-ipc-key-test-{}-{}.v1",
            std::process::id(),
            crate::nonce()
        ));
        let first = load_or_create(&path).unwrap();
        let second = load(&path).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        let dacl = file_dacl_sddl(&path).unwrap();
        assert!(dacl.starts_with("D:P"));
        assert_eq!(dacl.matches("(A;").count(), 2);
        assert!(!dacl.contains(";;;WD)"));
        assert!(!dacl.contains(";;;BU)"));
        assert!(!dacl.contains(";;;AU)"));

        fs::remove_file(&path).unwrap();
        let same_live = reconcile(&path, Some(&first)).unwrap();
        assert_eq!(same_live.audit, KeyRecoveryAudit::RewroteLiveKey);
        assert_eq!(same_live.key.as_bytes(), first.as_bytes());

        let other = IpcKey::from_unsealed(vec![7; IPC_KEY_BYTES]);
        store_atomic(&path, &other).unwrap();
        let mismatch = reconcile(&path, Some(&first)).unwrap();
        assert_eq!(mismatch.audit, KeyRecoveryAudit::RewroteLiveKey);
        assert_eq!(load(&path).unwrap().as_bytes(), first.as_bytes());

        fs::write(&path, b"corrupt live DPAPI evidence").unwrap();
        let repaired_corrupt = reconcile(&path, Some(&first)).unwrap();
        assert_eq!(repaired_corrupt.audit, KeyRecoveryAudit::RewroteLiveKey);
        assert_eq!(load(&path).unwrap().as_bytes(), first.as_bytes());

        fs::write(&path, b"corrupt DPAPI evidence").unwrap();
        let rotated = reconcile(&path, None).unwrap();
        let KeyRecoveryAudit::RotatedCorruptWithoutOwner { preserved_as } = rotated.audit else {
            panic!("corruption without an owner must rotate with preserved evidence");
        };
        assert!(preserved_as.is_file());
        assert_eq!(load(&path).unwrap().as_bytes(), rotated.key.as_bytes());
    }
}
