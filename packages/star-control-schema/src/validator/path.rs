pub(super) fn child_location(location: &str, key: &str) -> String {
    if key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        format!("{}.{}", location, key)
    } else {
        format!("{}[{}]", location, serde_json::to_string(key).unwrap())
    }
}

pub(super) fn child_schema_path(schema_path: &str, key: &str) -> String {
    format!("{}.{}", schema_path, key)
}
