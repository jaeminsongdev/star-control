use super::super::matchers::is_plaintext_secret_candidate;
use super::diagnostics::push_line_diagnostic;
use super::lines::added_lines;
use crate::changed_lines::ChangedLines;
use crate::model::{Diagnostic, RuleDefinition};

pub(in crate::evaluator) fn evaluate_plaintext_secrets(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        for line in added_lines(file) {
            if is_plaintext_secret_candidate(&line.content) {
                push_line_diagnostic(
                    diagnostics,
                    rule,
                    &file.path,
                    line.new_line,
                    "Added line contains a plaintext secret candidate.",
                    vec![
                        "secret-like token detected in added line; raw value intentionally omitted"
                            .to_string(),
                    ],
                    Some("Remove the secret and use an approved credential reference."),
                );
            }
        }
    }
}
