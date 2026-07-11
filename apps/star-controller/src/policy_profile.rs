//! Fail-closed extraction of the user policy profile used by the Tool Registry.
//!
//! The full EffectiveConfig merger remains Controller application work.  This
//! module only exposes the one already-frozen setting that changes user Tool
//! Registry trust. Until the complete EffectiveConfig merger supplies this
//! value, this narrow reader accepts only the two profile scalars. Any other
//! active key, duplicate TOML key, future schema, or unknown profile keeps the
//! Controller on `safe_default` and emits a diagnostic.

use std::{fs, io, path::Path};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UserPolicyProfile {
    #[default]
    SafeDefault,
    PersonalAuto,
}

#[derive(Debug, Error)]
pub enum PolicyProfileError {
    #[error("user config I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("user config TOML is invalid")]
    InvalidToml,
    #[error("user config schema or policy profile is unsupported")]
    Unsupported,
    #[error("user config contains an unknown top-level key")]
    UnknownTopLevel,
}

const TOP_LEVEL_KEYS: &[&str] = &["schema_version", "policy_profile"];

impl UserPolicyProfile {
    pub fn load(appdata: &Path) -> Result<Self, PolicyProfileError> {
        let path = appdata.join("Star-Control").join("config.toml");
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::SafeDefault),
            Err(error) => return Err(error.into()),
        };
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
            .map_err(|_| PolicyProfileError::InvalidToml)?;
        let value: toml::Value =
            toml::from_str(text).map_err(|_| PolicyProfileError::InvalidToml)?;
        let table = value.as_table().ok_or(PolicyProfileError::InvalidToml)?;
        if table
            .keys()
            .any(|key| !TOP_LEVEL_KEYS.contains(&key.as_str()))
        {
            return Err(PolicyProfileError::UnknownTopLevel);
        }
        if table
            .get("schema_version")
            .and_then(toml::Value::as_integer)
            != Some(1)
        {
            return Err(PolicyProfileError::Unsupported);
        }
        match table
            .get("policy_profile")
            .and_then(toml::Value::as_str)
            .unwrap_or("star.policy-profile.safe-default")
        {
            "safe_default" | "star.policy-profile.safe-default" => Ok(Self::SafeDefault),
            "personal_auto" | "star.policy-profile.personal-auto" => Ok(Self::PersonalAuto),
            _ => Err(PolicyProfileError::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("star-policy-{name}-{}", star_ipc::nonce()))
    }

    fn write(root: &Path, value: &str) {
        let directory = root.join("Star-Control");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("config.toml"), value).unwrap();
    }

    #[test]
    fn missing_config_is_safe_default() {
        assert_eq!(
            UserPolicyProfile::load(&root("missing")).unwrap(),
            UserPolicyProfile::SafeDefault
        );
    }

    #[test]
    fn personal_auto_requires_an_exact_supported_user_profile() {
        let directory = root("personal");
        write(
            &directory,
            "\u{feff}schema_version = 1\npolicy_profile = \"star.policy-profile.personal-auto\"\n",
        );
        assert_eq!(
            UserPolicyProfile::load(&directory).unwrap(),
            UserPolicyProfile::PersonalAuto
        );

        let unknown = root("unknown");
        write(
            &unknown,
            "schema_version = 1\npolicy_profile = \"personal_auto\"\n[tool_registry]\nuser_trust = \"policy_profile\"\n",
        );
        assert!(matches!(
            UserPolicyProfile::load(&unknown),
            Err(PolicyProfileError::UnknownTopLevel)
        ));

        let duplicate = root("duplicate");
        write(
            &duplicate,
            "schema_version = 1\npolicy_profile = \"safe_default\"\npolicy_profile = \"personal_auto\"\n",
        );
        assert!(matches!(
            UserPolicyProfile::load(&duplicate),
            Err(PolicyProfileError::InvalidToml)
        ));
    }
}
