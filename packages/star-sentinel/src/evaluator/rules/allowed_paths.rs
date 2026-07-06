use super::super::matchers::path_is_allowed;
use super::diagnostics::push_file_diagnostic;
use crate::changed_lines::ChangedLines;
use crate::model::{Diagnostic, RuleDefinition};
use crate::SentinelTask;

pub(in crate::evaluator) fn evaluate_allowed_paths(
    rule: &RuleDefinition,
    task: &SentinelTask,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        for path in file.changed_paths() {
            if !path_is_allowed(path, &task.allowed_paths) {
                push_file_diagnostic(
                    diagnostics,
                    rule,
                    path,
                    "Changed file is outside task allowed_paths.",
                    vec![format!(
                        "path {} is not covered by task.allowed_paths",
                        path
                    )],
                    Some("Keep changes inside the task allowed_paths or request scope approval."),
                );
            }
        }
    }
}
