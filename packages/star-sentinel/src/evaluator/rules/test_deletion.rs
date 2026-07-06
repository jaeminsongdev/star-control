use super::super::matchers::is_test_path;
use super::diagnostics::push_file_diagnostic;
use crate::changed_lines::ChangedLines;
use crate::model::{Diagnostic, RuleDefinition};

pub(in crate::evaluator) fn evaluate_test_deletion(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if file.change_type == "deleted"
            && file.changed_paths().iter().any(|path| is_test_path(path))
        {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Test file deletion is not allowed in P0.",
                vec!["deleted changed file is classified as test coverage".to_string()],
                Some(
                    "Restore the test file or request explicit human review with replacement coverage.",
                ),
            );
        }
    }
}
