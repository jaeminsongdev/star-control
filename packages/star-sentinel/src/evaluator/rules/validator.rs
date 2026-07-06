use super::super::matchers::{is_self_bypass_line, is_validator_path};
use super::diagnostics::{push_file_diagnostic, push_line_diagnostic};
use super::lines::changed_content_lines;
use crate::changed_lines::ChangedLines;
use crate::model::{Diagnostic, RuleDefinition};

pub(in crate::evaluator) fn evaluate_validator_self_bypass(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if !file
            .changed_paths()
            .iter()
            .any(|path| is_validator_path(path))
        {
            continue;
        }

        if file.change_type == "deleted" {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Validator, policy, schema, or CI artifact deletion can bypass validation.",
                vec!["validation-related artifact was deleted".to_string()],
                Some("Restore the validator artifact or route through explicit review."),
            );
            continue;
        }

        for line in changed_content_lines(file) {
            if is_self_bypass_line(&line.content) {
                push_line_diagnostic(
                    diagnostics,
                    rule,
                    &file.path,
                    line.new_line.or(line.old_line),
                    "Validator-related change appears to bypass enforcement.",
                    vec![
                        "bypass-like validation change detected; raw line intentionally omitted"
                            .to_string(),
                    ],
                    Some("Keep validation strict or request explicit policy review."),
                );
            }
        }
    }
}
