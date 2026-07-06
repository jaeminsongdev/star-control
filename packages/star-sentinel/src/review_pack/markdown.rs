use crate::model::{Decision, ReviewValidation};

pub(super) fn render_review_pack_markdown(
    decision: Decision,
    summary: &str,
    changed_files: &[String],
    risks: &[String],
    validations: &[ReviewValidation],
    questions: &[String],
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# Review Pack\n\n");
    markdown.push_str("## Decision\n");
    markdown.push_str(decision.as_str());
    markdown.push_str("\n\n## Summary\n");
    markdown.push_str(summary);
    markdown.push_str("\n\n## Changed Files\n");
    push_markdown_list(&mut markdown, changed_files);
    markdown.push_str("\n## Risks\n");
    push_markdown_list(&mut markdown, risks);
    markdown.push_str("\n## Validations\n");
    if validations.is_empty() {
        markdown.push_str("- none\n");
    } else {
        for validation in validations {
            markdown.push_str(&format!(
                "- {}: {}\n",
                validation.command, validation.result
            ));
        }
    }
    markdown.push_str("\n## Questions For Human\n");
    push_markdown_list(&mut markdown, questions);
    markdown
}

fn push_markdown_list(markdown: &mut String, items: &[String]) {
    if items.is_empty() {
        markdown.push_str("- none\n");
        return;
    }
    for item in items {
        markdown.push_str("- ");
        markdown.push_str(item);
        markdown.push('\n');
    }
}
