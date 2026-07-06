pub(super) fn yaml_list_section(content: &str, section: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == format!("{}:", section) {
            in_section = true;
            continue;
        }
        if in_section && !line.starts_with(' ') && trimmed.ends_with(':') {
            break;
        }
        if in_section {
            if let Some(value) = trimmed.strip_prefix("- ") {
                values.push(value.trim_matches('"').to_string());
            }
        }
    }
    values
}
