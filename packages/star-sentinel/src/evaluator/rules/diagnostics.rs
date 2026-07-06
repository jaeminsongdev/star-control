use super::super::matchers::normalize_path;
use crate::model::{Diagnostic, DiagnosticLocation, RuleDefinition};

pub(super) fn push_file_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    rule: &RuleDefinition,
    path: &str,
    message: &str,
    evidence: Vec<String>,
    recommendation: Option<&str>,
) {
    diagnostics.push(Diagnostic {
        diagnostic_id: String::new(),
        rule_id: rule.rule_id.clone(),
        severity: rule.severity,
        message: message.to_string(),
        locations: vec![DiagnosticLocation {
            path: normalize_path(path),
            line: None,
        }],
        evidence,
        recommendation: recommendation.map(str::to_string),
    });
}

pub(super) fn push_line_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    rule: &RuleDefinition,
    path: &str,
    line: Option<i64>,
    message: &str,
    evidence: Vec<String>,
    recommendation: Option<&str>,
) {
    diagnostics.push(Diagnostic {
        diagnostic_id: String::new(),
        rule_id: rule.rule_id.clone(),
        severity: rule.severity,
        message: message.to_string(),
        locations: vec![DiagnosticLocation {
            path: normalize_path(path),
            line,
        }],
        evidence,
        recommendation: recommendation.map(str::to_string),
    });
}
