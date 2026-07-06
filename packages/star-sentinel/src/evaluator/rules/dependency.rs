use super::super::matchers::is_dependency_path;
use super::diagnostics::push_file_diagnostic;
use crate::changed_lines::ChangedLines;
use crate::model::{Diagnostic, RuleDefinition};

pub(in crate::evaluator) fn evaluate_dependency_changes(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if file
            .changed_paths()
            .iter()
            .any(|path| is_dependency_path(path))
        {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Dependency manifest or lockfile changed and requires human review.",
                vec!["dependency-related artifact changed".to_string()],
                Some("Record approval before automatic progress continues."),
            );
        }
    }
}
