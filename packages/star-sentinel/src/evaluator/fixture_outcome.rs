use super::matchers::diagnostic_matches_expected;
use crate::json_fields::{required_array, required_string};
use crate::model::{Decision, EvaluationResult};
use crate::SentinelError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureOutcome {
    pub fixture_id: String,
    pub profile: String,
    pub expected_decision: Decision,
    pub expected_diagnostics: Vec<Value>,
}

impl FixtureOutcome {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        Ok(Self {
            fixture_id: required_string(value, "fixture_id", "FixtureOutcome")?,
            profile: required_string(value, "profile", "FixtureOutcome")?,
            expected_decision: Decision::parse(
                &required_string(value, "expected_decision", "FixtureOutcome")?,
                "FixtureOutcome",
                "expected_decision",
            )?,
            expected_diagnostics: required_array(value, "expected_diagnostics", "FixtureOutcome")?
                .to_vec(),
        })
    }

    pub fn matches_result(&self, result: &EvaluationResult) -> bool {
        if self.expected_decision != result.decision {
            return false;
        }

        self.expected_diagnostics.iter().all(|expected| {
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic_matches_expected(diagnostic, expected))
        })
    }
}
