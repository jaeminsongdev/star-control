use std::time::{SystemTime, UNIX_EPOCH};

pub(in crate::control) fn timestamp_string() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("unix:{}", nanos)
}
