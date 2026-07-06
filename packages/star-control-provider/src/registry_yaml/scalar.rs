use serde_json::{Number, Value};

pub(super) fn parse_yaml_scalar(raw_value: &str) -> Value {
    let value = raw_value.trim();
    if value.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if value.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if value.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Ok(number) = value.parse::<i64>() {
        return Value::Number(Number::from(number));
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return Value::String(value[1..value.len() - 1].to_string());
    }
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Value::String(value[1..value.len() - 1].to_string());
    }
    Value::String(value.to_string())
}
