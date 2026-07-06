use serde_json::Value;

pub(in crate::control) fn body_string(body: &Value, field: &str) -> Result<String, String> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{} string field is required", field))
}

pub(in crate::control) fn body_string_array(
    body: &Value,
    field: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = body.get(field) else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(format!("{} must be an array of strings", field));
    };
    let mut output = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(format!("{} must be an array of strings", field));
        };
        output.push(text.to_string());
    }
    Ok(output)
}

pub(in crate::control) fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}
