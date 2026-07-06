use super::line::YamlLine;
use super::pair::{split_inline_yaml_pair, split_yaml_key_value};
use super::scalar::parse_yaml_scalar;
use crate::registry_error::ProviderRegistryError;
use serde_json::{Map, Value};
use std::path::Path;

pub(super) fn parse_yaml_block(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let line = lines
        .get(*cursor)
        .ok_or_else(|| ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: 0,
            message: "unexpected end of document".to_string(),
        })?;

    if line.indent != indent {
        return Err(ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: line.number,
            message: format!("expected indent {}, got {}", indent, line.indent),
        });
    }

    if line.text.starts_with("- ") {
        parse_yaml_sequence(path, lines, cursor, indent)
    } else {
        parse_yaml_mapping(path, lines, cursor, indent)
    }
}

fn parse_yaml_mapping(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let mut object = Map::new();

    while let Some(line) = lines.get(*cursor) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: line.number,
                message: format!("unexpected nested indent {}", line.indent),
            });
        }
        if line.text.starts_with("- ") {
            break;
        }

        let (key, raw_value) = split_yaml_key_value(path, line)?;
        *cursor += 1;
        let value = if raw_value.is_empty() {
            if let Some(next_line) = lines.get(*cursor) {
                if next_line.indent <= indent {
                    Value::Null
                } else {
                    parse_yaml_block(path, lines, cursor, next_line.indent)?
                }
            } else {
                Value::Null
            }
        } else {
            parse_yaml_scalar(raw_value)
        };
        object.insert(key.to_string(), value);
    }

    Ok(Value::Object(object))
}

fn parse_yaml_sequence(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let mut values = Vec::new();

    while let Some(line) = lines.get(*cursor) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: line.number,
                message: format!("unexpected nested indent {}", line.indent),
            });
        }
        if !line.text.starts_with("- ") {
            break;
        }

        let rest = line.text[2..].trim();
        *cursor += 1;
        if rest.is_empty() {
            if let Some(next_line) = lines.get(*cursor) {
                values.push(parse_yaml_block(path, lines, cursor, next_line.indent)?);
            } else {
                values.push(Value::Null);
            }
            continue;
        }

        if let Some((key, raw_value)) = split_inline_yaml_pair(rest) {
            let mut item = Map::new();
            let value = if raw_value.is_empty() {
                if let Some(next_line) = lines.get(*cursor) {
                    if next_line.indent <= indent {
                        Value::Null
                    } else {
                        parse_yaml_block(path, lines, cursor, next_line.indent)?
                    }
                } else {
                    Value::Null
                }
            } else {
                parse_yaml_scalar(raw_value)
            };
            item.insert(key.to_string(), value);

            while let Some(next_line) = lines.get(*cursor) {
                if next_line.indent <= indent {
                    break;
                }
                if next_line.text.starts_with("- ") {
                    break;
                }
                let nested_indent = next_line.indent;
                let nested = parse_yaml_mapping(path, lines, cursor, nested_indent)?;
                if let Value::Object(nested_map) = nested {
                    for (nested_key, nested_value) in nested_map {
                        item.insert(nested_key, nested_value);
                    }
                }
            }

            values.push(Value::Object(item));
        } else {
            values.push(parse_yaml_scalar(rest));
        }
    }

    Ok(Value::Array(values))
}
