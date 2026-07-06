use super::line::YamlLine;
use crate::registry_error::ProviderRegistryError;
use std::path::Path;

pub(super) fn split_yaml_key_value<'a>(
    path: &Path,
    line: &'a YamlLine,
) -> Result<(&'a str, &'a str), ProviderRegistryError> {
    split_inline_yaml_pair(&line.text).ok_or_else(|| ProviderRegistryError::InvalidYamlSubset {
        path: path.to_path_buf(),
        line: line.number,
        message: "expected key: value mapping".to_string(),
    })
}

pub(super) fn split_inline_yaml_pair(text: &str) -> Option<(&str, &str)> {
    let index = text.find(':')?;
    let key = text[..index].trim();
    if key.is_empty() {
        return None;
    }
    Some((key, text[index + 1..].trim()))
}
