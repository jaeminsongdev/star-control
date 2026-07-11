//! DPAPI current-user protection for the per-user IPC key.

use windows::Win32::{
    Foundation::{HLOCAL, LocalFree},
    Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    },
};

fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

fn owned_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
    }
    bytes
}

/// Protects data with the current Windows user profile. UI is forbidden so an
/// unattended Controller never creates an interactive credential prompt.
pub fn protect_current_user(plaintext: &[u8]) -> windows::core::Result<Vec<u8>> {
    let input = blob(plaintext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;
    }
    Ok(owned_blob(output))
}

pub fn unprotect_current_user(ciphertext: &[u8]) -> windows::core::Result<Vec<u8>> {
    let input = blob(ciphertext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;
    }
    Ok(owned_blob(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_user_dpapi_round_trip() {
        let cipher = protect_current_user(b"star-control-ipc-key-fixture")
            .expect("DPAPI protects current-user bytes");
        assert_ne!(cipher, b"star-control-ipc-key-fixture");
        assert_eq!(
            unprotect_current_user(&cipher).unwrap(),
            b"star-control-ipc-key-fixture"
        );
    }
}
