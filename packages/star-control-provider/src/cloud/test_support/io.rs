use serde_json::Value;
use std::fs;
use std::path::Path;

pub(crate) fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read json")).expect("parse json")
}
