//! Strict JSON decoding for untrusted wire frames.
//!
//! `serde_json::Value` intentionally accepts duplicate object keys and keeps
//! the last value.  MCP, IPC and JSON-STDIO frames are protocol boundaries,
//! where that ambiguity is forbidden by the implementation contract.

use std::collections::BTreeMap;

use serde::{Deserialize, de};

/// Parses JSON while rejecting a duplicate key at every object depth.
pub fn parse_no_duplicate_keys(input: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str::<StrictValue>(input).map(StrictValue::into_value)
}

struct StrictValue(serde_json::Value);

impl StrictValue {
    fn into_value(self) -> serde_json::Value {
        self.0
    }
}

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct StrictVisitor;

        impl<'de> de::Visitor<'de> for StrictVisitor {
            type Value = StrictValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::Bool(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::Number(value.into())))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::Number(value.into())))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                serde_json::Number::from_f64(value)
                    .map(serde_json::Value::Number)
                    .map(StrictValue)
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::String(value.to_owned())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::String(value)))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::Null))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(serde_json::Value::Null))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<StrictValue>()? {
                    values.push(value.into_value());
                }
                Ok(StrictValue(serde_json::Value::Array(values)))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    let value = map.next_value::<StrictValue>()?.into_value();
                    if values.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("duplicate JSON key: {key}")));
                    }
                }
                Ok(StrictValue(serde_json::Value::Object(
                    values.into_iter().collect(),
                )))
            }
        }

        deserializer.deserialize_any(StrictVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_no_duplicate_keys;

    #[test]
    fn rejects_duplicate_keys_at_any_depth() {
        assert!(parse_no_duplicate_keys(r#"{"a":1,"a":2}"#).is_err());
        assert!(parse_no_duplicate_keys(r#"{"a":{"b":1,"b":2}}"#).is_err());
        assert!(parse_no_duplicate_keys(r#"[{"a":1,"a":2}]"#).is_err());
    }

    #[test]
    fn retains_valid_json_values() {
        assert_eq!(
            parse_no_duplicate_keys(r#"{"a":[true,null,"x"]}"#).unwrap(),
            serde_json::json!({"a":[true,null,"x"]})
        );
    }
}
