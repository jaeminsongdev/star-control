use serde_json::Value;

pub(super) fn render_release_review_pack_markdown(readiness: &Value) -> String {
    let mut markdown = String::new();
    markdown.push_str("# Release Review Pack\n\n");
    markdown.push_str("## Summary\n\n");
    markdown.push_str(&format!(
        "- release_id: `{}`\n",
        markdown_inline(release_field(readiness, "release_id"))
    ));
    markdown.push_str(&format!(
        "- target: `{}`\n",
        markdown_inline(release_field(readiness, "target"))
    ));
    markdown.push_str(&format!(
        "- version: `{}`\n",
        markdown_inline(release_field(readiness, "version"))
    ));
    markdown.push_str(&format!(
        "- status: `{}`\n",
        markdown_inline(release_field(readiness, "status"))
    ));
    markdown.push_str(&format!(
        "- generated_at: `{}`\n\n",
        markdown_inline(release_field(readiness, "generated_at"))
    ));

    markdown.push_str("## Checks\n\n");
    let checks = readiness
        .get("checks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if checks.is_empty() {
        markdown.push_str("- none recorded\n\n");
    } else {
        for check in checks {
            let name = markdown_inline(release_field(check, "name"));
            let status = markdown_inline(release_field(check, "status"));
            let evidence_paths = release_string_array(check, "evidence_paths");
            markdown.push_str(&format!("- `{}`: `{}`", name, status));
            if !evidence_paths.is_empty() {
                markdown.push_str(&format!(
                    " (evidence: {})",
                    markdown_code_list(&evidence_paths)
                ));
            }
            markdown.push('\n');
        }
        markdown.push('\n');
    }

    markdown.push_str("## Blockers\n\n");
    push_markdown_bullets(
        &mut markdown,
        &release_string_array(readiness, "blockers"),
        "none recorded",
    );

    markdown.push_str("## Approvals\n\n");
    push_markdown_bullets(
        &mut markdown,
        &release_string_array(readiness, "approvals"),
        "none recorded",
    );

    markdown.push_str("## Guardrails\n\n");
    markdown.push_str("- This artifact is for human review only.\n");
    markdown.push_str(
        "- Release, deploy, publish, signing, repository settings, and external account actions remain reserved.\n",
    );
    markdown.push_str(
        "- A review pack is not an approval record and must not trigger release automation.\n",
    );
    markdown
}

fn release_field<'a>(value: &'a Value, field: &str) -> &'a str {
    value.get(field).and_then(Value::as_str).unwrap_or("")
}

fn release_string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(markdown_inline)
                .collect()
        })
        .unwrap_or_default()
}

fn push_markdown_bullets(markdown: &mut String, values: &[String], empty_label: &str) {
    if values.is_empty() {
        markdown.push_str(&format!("- {}\n\n", empty_label));
        return;
    }
    for value in values {
        markdown.push_str(&format!("- {}\n", value));
    }
    markdown.push('\n');
}

fn markdown_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{}`", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn markdown_inline(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "<empty>".to_string()
    } else {
        collapsed.replace('`', "'")
    }
}
