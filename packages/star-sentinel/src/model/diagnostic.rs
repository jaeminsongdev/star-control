use super::{Decision, Severity};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub path: String,
    pub line: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub diagnostic_id: String,
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub locations: Vec<DiagnosticLocation>,
    pub evidence: Vec<String>,
    pub recommendation: Option<String>,
}

impl Diagnostic {
    pub fn to_value(&self) -> Value {
        let mut value = json!({
            "schema_version": "1.0.0",
            "diagnostic_id": self.diagnostic_id,
            "rule_id": self.rule_id,
            "severity": self.severity.as_str(),
            "message": self.message,
            "locations": self.locations.iter().map(|location| {
                match location.line {
                    Some(line) => json!({"path": location.path, "line": line}),
                    None => json!({"path": location.path}),
                }
            }).collect::<Vec<_>>(),
            "evidence": self.evidence,
        });

        if let Some(recommendation) = &self.recommendation {
            value["recommendation"] = json!(recommendation);
        }

        value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationResult {
    pub decision: Decision,
    pub diagnostics: Vec<Diagnostic>,
}
