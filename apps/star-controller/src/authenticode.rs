//! Offline Authenticode verification for executable identity policy.

use std::{os::windows::ffi::OsStrExt, path::Path};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{HANDLE, HWND},
        Security::{
            Cryptography::{CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW},
            WinTrust::{
                WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
                WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE,
                WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
                WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvCertFromChain,
                WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust,
            },
        },
    },
    core::{GUID, PCWSTR},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticodeStatus {
    Valid,
    Unsigned,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AuthenticodeEvidence {
    pub status: AuthenticodeStatus,
    pub subject: Option<String>,
    pub network_access: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthenticodeError {
    #[error("TOOL_AUTHENTICODE_INVALID")]
    Invalid,
    #[error("TOOL_AUTHENTICODE_SUBJECT_MISMATCH")]
    SubjectMismatch,
}

/// Verifies an embedded signature without permitting URL retrieval. `record`
/// always returns evidence; enforcing policies fail closed on unsigned,
/// invalid, or subject-mismatched images.
pub fn verify_authenticode(
    path: &Path,
    policy: &str,
    expected_subject: Option<&str>,
) -> Result<AuthenticodeEvidence, AuthenticodeError> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut trust = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
        ..Default::default()
    };
    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status_code =
        unsafe { WinVerifyTrust(HWND::default(), &raw mut action, (&raw mut trust).cast()) };
    let subject = (status_code == 0)
        .then(|| signer_subject(trust.hWVTStateData))
        .flatten();
    trust.dwStateAction = WTD_STATEACTION_CLOSE;
    let _ = unsafe { WinVerifyTrust(HWND::default(), &raw mut action, (&raw mut trust).cast()) };

    // TRUST_E_NOSIGNATURE (0x800B0100) is distinct from a present but invalid
    // signature. Both remain observable under `record` and fail enforcement.
    let status = if status_code == 0 {
        AuthenticodeStatus::Valid
    } else if status_code as u32 == 0x800B_0100 {
        AuthenticodeStatus::Unsigned
    } else {
        AuthenticodeStatus::Invalid
    };
    let evidence = AuthenticodeEvidence {
        status,
        subject,
        network_access: false,
    };
    match policy {
        "record" => Ok(evidence),
        "require_valid" if evidence.status == AuthenticodeStatus::Valid => Ok(evidence),
        "require_subject" if evidence.status == AuthenticodeStatus::Valid => {
            let expected = expected_subject.ok_or(AuthenticodeError::SubjectMismatch)?;
            if evidence
                .subject
                .as_deref()
                .is_some_and(|actual| actual.trim().eq_ignore_ascii_case(expected.trim()))
            {
                Ok(evidence)
            } else {
                Err(AuthenticodeError::SubjectMismatch)
            }
        }
        "require_subject" => Err(AuthenticodeError::Invalid),
        _ => Err(AuthenticodeError::Invalid),
    }
}

fn signer_subject(state: HANDLE) -> Option<String> {
    if state.is_invalid() {
        return None;
    }
    let provider = unsafe { WTHelperProvDataFromStateData(state) };
    if provider.is_null() {
        return None;
    }
    let signer = unsafe { WTHelperGetProvSignerFromChain(provider, 0, false, 0) };
    if signer.is_null() {
        return None;
    }
    let certificate = unsafe { WTHelperGetProvCertFromChain(signer, 0) };
    if certificate.is_null() {
        return None;
    }
    let context = unsafe { (*certificate).pCert };
    if context.is_null() {
        return None;
    }
    let length =
        unsafe { CertGetNameStringW(context, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, None) };
    if length <= 1 {
        return None;
    }
    let mut value = vec![0_u16; length as usize];
    let written = unsafe {
        CertGetNameStringW(
            context,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(value.as_mut_slice()),
        )
    };
    if written <= 1 {
        return None;
    }
    value.truncate(written.saturating_sub(1) as usize);
    String::from_utf16(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // matrix: MCP-P023
    fn unsigned_invalid_offline_and_subject_policies_are_fail_closed() {
        let unsigned = std::env::current_exe().expect("test executable path");
        let recorded = verify_authenticode(&unsigned, "record", None).unwrap();
        assert!(!recorded.network_access);
        assert!(matches!(
            recorded.status,
            AuthenticodeStatus::Unsigned | AuthenticodeStatus::Invalid
        ));
        assert!(matches!(
            verify_authenticode(&unsigned, "require_valid", None),
            Err(AuthenticodeError::Invalid)
        ));

        let program_files = std::env::var_os("ProgramFiles").expect("ProgramFiles");
        let signed = Path::new(&program_files)
            .join("Git")
            .join("cmd")
            .join("git.exe");
        let signed_evidence = verify_authenticode(&signed, "record", None).unwrap();
        assert!(!signed_evidence.network_access);
        if signed_evidence.status == AuthenticodeStatus::Valid {
            let subject = signed_evidence.subject.expect("signed image subject");
            assert!(verify_authenticode(&signed, "require_subject", Some(&subject)).is_ok());
            assert!(matches!(
                verify_authenticode(&signed, "require_subject", Some("Wrong Publisher")),
                Err(AuthenticodeError::SubjectMismatch)
            ));
        } else {
            // Cache-only verification deliberately refuses to fetch a chain.
            // A host without cached signer data must fail closed.
            assert!(matches!(
                verify_authenticode(&signed, "require_valid", None),
                Err(AuthenticodeError::Invalid)
            ));
        }
    }
}
