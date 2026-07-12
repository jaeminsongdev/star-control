//! Offline Authenticode verification for executable identity policy.

use std::{
    collections::HashMap,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use star_contracts::Sha256Hash;
use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM},
        Globalization::{
            LCMAP_LINGUISTIC_CASING, LCMAP_LOWERCASE, LCMapStringEx, LOCALE_NAME_INVARIANT,
            NormalizationKC, NormalizeString,
        },
        Security::{
            Cryptography::{CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW},
            WinTrust::{
                WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
                WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE,
                WTD_REVOKE_WHOLECHAIN, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
                WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain,
                WTHelperProvDataFromStateData, WinVerifyTrust,
            },
        },
    },
    core::{GUID, PCWSTR},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticodeStatus {
    NotChecked,
    Valid,
    Unsigned,
    OfflineIndeterminate,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AuthenticodeEvidence {
    pub status: AuthenticodeStatus,
    pub subject: Option<String>,
    pub normalized_subject: Option<String>,
    pub network_access: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AuthenticodeError {
    #[error("TOOL_AUTHENTICODE_INVALID")]
    Invalid,
    #[error("TOOL_AUTHENTICODE_SUBJECT_MISMATCH")]
    SubjectMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SignatureCacheKey {
    executable_hash: String,
    policy: String,
    normalized_subject: Option<String>,
}

type SignatureCacheValue = (Instant, Result<AuthenticodeEvidence, AuthenticodeError>);
static SIGNATURE_CACHE: OnceLock<Mutex<HashMap<SignatureCacheKey, SignatureCacheValue>>> =
    OnceLock::new();
const SIGNATURE_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

pub fn clear_authenticode_cache() {
    if let Some(cache) = SIGNATURE_CACHE.get()
        && let Ok(mut cache) = cache.lock()
    {
        cache.clear();
    }
}

/// Verifies an embedded signature without permitting URL retrieval. `record`
/// always returns evidence; enforcing policies fail closed on unsigned,
/// invalid, or subject-mismatched images.
pub fn verify_authenticode(
    path: &Path,
    executable_hash: &Sha256Hash,
    policy: &str,
    expected_subject: Option<&str>,
) -> Result<AuthenticodeEvidence, AuthenticodeError> {
    if policy == "ignore" {
        return Ok(AuthenticodeEvidence {
            status: AuthenticodeStatus::NotChecked,
            subject: None,
            normalized_subject: None,
            network_access: false,
        });
    }
    let normalized_expected_subject = expected_subject.and_then(normalize_subject);
    let cache_key = SignatureCacheKey {
        executable_hash: executable_hash.to_string(),
        policy: policy.to_owned(),
        normalized_subject: normalized_expected_subject.clone(),
    };
    let cache = SIGNATURE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.retain(|_, (inserted, _)| inserted.elapsed() <= SIGNATURE_CACHE_TTL);
        if let Some((_, result)) = cache.get(&cache_key) {
            return result.clone();
        }
    }
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
        fdwRevocationChecks: WTD_REVOKE_WHOLECHAIN,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
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
    } else if matches!(status_code as u32, 0x8009_2013 | 0x800B_010E) {
        AuthenticodeStatus::OfflineIndeterminate
    } else {
        AuthenticodeStatus::Invalid
    };
    let normalized_subject = subject.as_deref().and_then(normalize_subject);
    let evidence = AuthenticodeEvidence {
        status,
        subject,
        normalized_subject,
        network_access: false,
    };
    let result = match policy {
        "record" => Ok(evidence),
        "require_valid" if evidence.status == AuthenticodeStatus::Valid => Ok(evidence),
        "require_subject" if evidence.status == AuthenticodeStatus::Valid => {
            if normalized_expected_subject.is_some_and(|expected| {
                evidence.normalized_subject.as_deref() == Some(expected.as_str())
            }) {
                Ok(evidence)
            } else {
                Err(AuthenticodeError::SubjectMismatch)
            }
        }
        "require_subject" => Err(AuthenticodeError::Invalid),
        _ => Err(AuthenticodeError::Invalid),
    };
    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key, (Instant::now(), result.clone()));
    }
    result
}

fn normalize_subject(value: &str) -> Option<String> {
    let source: Vec<u16> = value.trim().encode_utf16().collect();
    if source.is_empty() {
        return Some(String::new());
    }
    let normalized_length = unsafe { NormalizeString(NormalizationKC, &source, None) };
    if normalized_length <= 0 {
        return None;
    }
    let mut normalized = vec![0_u16; normalized_length as usize];
    let normalized_written =
        unsafe { NormalizeString(NormalizationKC, &source, Some(&mut normalized)) };
    if normalized_written <= 0 {
        return None;
    }
    normalized.truncate(normalized_written as usize);
    let folded_length = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_LOWERCASE | LCMAP_LINGUISTIC_CASING,
            &normalized,
            None,
            None,
            None,
            LPARAM(0),
        )
    };
    if folded_length <= 0 {
        return None;
    }
    let mut folded = vec![0_u16; folded_length as usize];
    let folded_written = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_LOWERCASE | LCMAP_LINGUISTIC_CASING,
            &normalized,
            Some(&mut folded),
            None,
            None,
            LPARAM(0),
        )
    };
    if folded_written <= 0 {
        return None;
    }
    folded.truncate(folded_written as usize);
    String::from_utf16(&folded).ok()
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
        let ignored = verify_authenticode(
            Path::new("not-opened.exe"),
            &Sha256Hash::digest(b"not-opened"),
            "ignore",
            None,
        )
        .unwrap();
        assert_eq!(ignored.status, AuthenticodeStatus::NotChecked);
        assert!(ignored.subject.is_none());
        assert!(ignored.normalized_subject.is_none());
        assert_eq!(
            normalize_subject("  Ａcme Publisher  ").as_deref(),
            Some("acme publisher")
        );

        let unsigned = std::env::current_exe().expect("test executable path");
        let unsigned_hash = Sha256Hash::digest(&std::fs::read(&unsigned).unwrap());
        let recorded = verify_authenticode(&unsigned, &unsigned_hash, "record", None).unwrap();
        assert!(!recorded.network_access);
        assert!(matches!(
            recorded.status,
            AuthenticodeStatus::Unsigned
                | AuthenticodeStatus::OfflineIndeterminate
                | AuthenticodeStatus::Invalid
        ));
        assert!(matches!(
            verify_authenticode(&unsigned, &unsigned_hash, "require_valid", None),
            Err(AuthenticodeError::Invalid)
        ));

        let system_root = std::env::var_os("SystemRoot").expect("SystemRoot");
        let signed = Path::new(&system_root).join("System32").join("cmd.exe");
        let signed_hash = Sha256Hash::digest(&std::fs::read(&signed).unwrap());
        let signed_evidence = verify_authenticode(&signed, &signed_hash, "record", None).unwrap();
        assert!(!signed_evidence.network_access);
        if signed_evidence.status == AuthenticodeStatus::Valid {
            let subject = signed_evidence.subject.expect("signed image subject");
            assert!(
                verify_authenticode(&signed, &signed_hash, "require_subject", Some(&subject))
                    .is_ok()
            );
            assert!(matches!(
                verify_authenticode(
                    &signed,
                    &signed_hash,
                    "require_subject",
                    Some("Wrong Publisher")
                ),
                Err(AuthenticodeError::SubjectMismatch)
            ));
        } else {
            // Cache-only verification deliberately refuses to fetch a chain.
            // A host without cached signer data must fail closed.
            assert!(matches!(
                verify_authenticode(&signed, &signed_hash, "require_valid", None),
                Err(AuthenticodeError::Invalid)
            ));
        }
    }
}
