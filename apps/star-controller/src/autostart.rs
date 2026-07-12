//! Current-user logon lifecycle only; no service or Task Scheduler support.

use std::path::Path;

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE, RegCloseKey,
            RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        },
    },
    core::w,
};

const RUN_SUBKEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("Star-Control");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutostartState {
    Missing,
    Owned,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutostartMutation {
    Noop,
    SetOwned,
    DeleteOwned,
    Conflict,
}

fn decide_mutation(current: Option<&str>, expected: &str, enable: bool) -> AutostartMutation {
    match (current, enable) {
        (None, true) => AutostartMutation::SetOwned,
        (None, false) => AutostartMutation::Noop,
        (Some(current), true) if current == expected => AutostartMutation::Noop,
        (Some(current), false) if current == expected => AutostartMutation::DeleteOwned,
        (Some(_), _) => AutostartMutation::Conflict,
    }
}

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("Windows registry operation failed")]
    Registry,
    #[error("Run value is owned by another command and will not be changed")]
    Conflict,
}

pub fn expected_command(controller: &Path) -> Result<String, AutostartError> {
    let controller = controller
        .canonicalize()
        .map_err(|_| AutostartError::Registry)?;
    Ok(format!("\"{}\" --background", controller.display()))
}

pub fn status(expected: &str) -> Result<AutostartState, AutostartError> {
    match read_value()? {
        None => Ok(AutostartState::Missing),
        Some(value) if value == expected => Ok(AutostartState::Owned),
        Some(_) => Ok(AutostartState::Conflict),
    }
}

pub fn enable(expected: &str) -> Result<(), AutostartError> {
    match decide_mutation(read_value()?.as_deref(), expected, true) {
        AutostartMutation::Noop => return Ok(()),
        AutostartMutation::Conflict => return Err(AutostartError::Conflict),
        AutostartMutation::SetOwned => {}
        AutostartMutation::DeleteOwned => unreachable!("enable never deletes"),
    }
    let key = open_run_key()?;
    let mut utf16: Vec<u16> = expected.encode_utf16().collect();
    utf16.push(0);
    let bytes = unsafe {
        std::slice::from_raw_parts(
            utf16.as_ptr().cast::<u8>(),
            utf16.len() * std::mem::size_of::<u16>(),
        )
    };
    let result = unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(bytes)) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    (result == ERROR_SUCCESS)
        .then_some(())
        .ok_or(AutostartError::Registry)
}

/// Removes the value only if it exactly matches the Controller installation
/// command. A foreign value is evidence of another owner and remains intact.
pub fn disable(expected: &str) -> Result<(), AutostartError> {
    match decide_mutation(read_value()?.as_deref(), expected, false) {
        AutostartMutation::Noop => return Ok(()),
        AutostartMutation::Conflict => return Err(AutostartError::Conflict),
        AutostartMutation::DeleteOwned => {}
        AutostartMutation::SetOwned => unreachable!("disable never sets"),
    }
    let key = open_run_key()?;
    let result = unsafe { RegDeleteValueW(key, VALUE_NAME) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    (result == ERROR_SUCCESS)
        .then_some(())
        .ok_or(AutostartError::Registry)
}

fn open_run_key() -> Result<HKEY, AutostartError> {
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            None,
            KEY_READ | KEY_SET_VALUE,
            &mut key,
        )
    };
    (result == ERROR_SUCCESS)
        .then_some(key)
        .ok_or(AutostartError::Registry)
}

fn read_value() -> Result<Option<String>, AutostartError> {
    let key = open_run_key()?;
    let mut kind = REG_VALUE_TYPE(0);
    let mut bytes = 0u32;
    let first = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut kind),
            None,
            Some(&mut bytes),
        )
    };
    if first != ERROR_SUCCESS {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return if first == ERROR_FILE_NOT_FOUND {
            Ok(None)
        } else {
            Err(AutostartError::Registry)
        };
    }
    if kind != REG_SZ || bytes % 2 != 0 {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return Err(AutostartError::Registry);
    }
    let mut raw = vec![0u8; bytes as usize];
    let second = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut kind),
            Some(raw.as_mut_ptr()),
            Some(&mut bytes),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if second != ERROR_SUCCESS {
        return Err(AutostartError::Registry);
    }
    let words: Vec<u16> = raw
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .take_while(|word| *word != 0)
        .collect();
    Ok(Some(
        String::from_utf16(&words).map_err(|_| AutostartError::Registry)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // matrix: MCP-I015
    fn expected_command_is_quoted_and_background_only() {
        let command = expected_command(&std::env::current_exe().unwrap()).unwrap();
        assert!(command.starts_with('"'));
        assert!(command.ends_with("\" --background"));
        assert_eq!(
            decide_mutation(None, &command, true),
            AutostartMutation::SetOwned
        );
        assert_eq!(
            decide_mutation(Some(&command), &command, true),
            AutostartMutation::Noop
        );
        assert_eq!(
            decide_mutation(Some(&command), &command, false),
            AutostartMutation::DeleteOwned
        );
        assert_eq!(
            decide_mutation(Some("foreign command"), &command, true),
            AutostartMutation::Conflict
        );
        assert_eq!(
            decide_mutation(Some("foreign command"), &command, false),
            AutostartMutation::Conflict
        );
    }
}
