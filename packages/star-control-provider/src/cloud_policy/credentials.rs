use serde_json::Value;

pub(super) fn raw_credential_field(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_raw_credential_key(key) && child.as_str().is_some_and(|text| !text.is_empty())
                {
                    return Some(key.to_string());
                }
                if let Some(field) = raw_credential_field(child) {
                    return Some(format!("{}.{}", key, field));
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(raw_credential_field),
        _ => None,
    }
}

fn is_raw_credential_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| *character != '-' && *character != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "apikey"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "secret"
            | "password"
            | "credential"
            | "credentials"
            | "bearertoken"
    )
}

pub(super) fn is_allowed_credential_ref(value: &str) -> bool {
    const ALLOWED_PREFIXES: &[&str] = &["env:", "keychain:", "secret-manager:", "login-session:"];
    ALLOWED_PREFIXES.iter().any(|prefix| {
        value
            .strip_prefix(prefix)
            .is_some_and(|suffix| !suffix.is_empty())
    })
}
