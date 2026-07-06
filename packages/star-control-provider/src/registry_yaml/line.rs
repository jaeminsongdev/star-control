use crate::registry_error::ProviderRegistryError;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct YamlLine {
    pub(super) number: usize,
    pub(super) indent: usize,
    pub(super) text: String,
}

pub(super) fn collect_yaml_lines(
    path: &Path,
    content: &str,
) -> Result<Vec<YamlLine>, ProviderRegistryError> {
    let mut lines = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let without_comment = strip_yaml_comment(raw_line);
        if without_comment.trim().is_empty() {
            continue;
        }
        if without_comment.starts_with('\t') {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: index + 1,
                message: "tabs are not supported".to_string(),
            });
        }

        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        lines.push(YamlLine {
            number: index + 1,
            indent,
            text: without_comment.trim().to_string(),
        });
    }

    Ok(lines)
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, character) in line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}
