use super::block::parse_yaml_block;
use super::line::collect_yaml_lines;
use crate::registry_error::ProviderRegistryError;
use serde_json::{Map, Value};
use std::path::Path;

pub(crate) fn parse_star_control_yaml_subset(
    path: &Path,
    content: &str,
) -> Result<Value, ProviderRegistryError> {
    let lines = collect_yaml_lines(path, content)?;
    if lines.is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    let mut cursor = 0;
    let value = parse_yaml_block(path, &lines, &mut cursor, lines[0].indent)?;
    if cursor != lines.len() {
        return Err(ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: lines[cursor].number,
            message: "unexpected trailing content".to_string(),
        });
    }

    Ok(value)
}
