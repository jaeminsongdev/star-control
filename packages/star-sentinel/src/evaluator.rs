mod fixture_outcome;
mod matchers;
mod rules;

use crate::changed_lines::ChangedLines;
use crate::constants::{
    RULE_DEPENDENCY_REQUIRES_APPROVAL, RULE_SCOPE_ALLOWED_PATHS, RULE_SECRET_NO_PLAINTEXT_SECRET,
    RULE_TEST_NO_DELETION, RULE_VALIDATOR_NO_SELF_BYPASS,
};
use crate::model::{Decision, EvaluationResult, P0RuleRegistry};
use crate::{SentinelError, SentinelTask};

pub use fixture_outcome::FixtureOutcome;
pub(crate) use matchers::normalize_path;

#[derive(Debug, Clone)]
pub struct P0Evaluator {
    registry: P0RuleRegistry,
}

impl P0Evaluator {
    pub fn new(registry: P0RuleRegistry) -> Self {
        Self { registry }
    }

    pub fn evaluate(
        &self,
        task: &SentinelTask,
        changed_lines: &ChangedLines,
    ) -> Result<EvaluationResult, SentinelError> {
        if task.task_id != changed_lines.task_id {
            return Err(SentinelError::InvalidField {
                artifact: "ChangedLines".to_string(),
                field: "task_id".to_string(),
                message: format!(
                    "must match SentinelTask task_id {}, got {}",
                    task.task_id, changed_lines.task_id
                ),
            });
        }

        let mut diagnostics = Vec::new();
        for rule in &self.registry.rules {
            match rule.rule_id.as_str() {
                RULE_SCOPE_ALLOWED_PATHS => {
                    rules::evaluate_allowed_paths(rule, task, changed_lines, &mut diagnostics)
                }
                RULE_TEST_NO_DELETION => {
                    rules::evaluate_test_deletion(rule, changed_lines, &mut diagnostics)
                }
                RULE_DEPENDENCY_REQUIRES_APPROVAL => {
                    rules::evaluate_dependency_changes(rule, changed_lines, &mut diagnostics)
                }
                RULE_SECRET_NO_PLAINTEXT_SECRET => {
                    rules::evaluate_plaintext_secrets(rule, changed_lines, &mut diagnostics)
                }
                RULE_VALIDATOR_NO_SELF_BYPASS => {
                    rules::evaluate_validator_self_bypass(rule, changed_lines, &mut diagnostics)
                }
                _ => {}
            }
        }

        for (index, diagnostic) in diagnostics.iter_mut().enumerate() {
            diagnostic.diagnostic_id = format!("P0-D{:04}", index + 1);
        }

        let decision = diagnostics
            .iter()
            .fold(Decision::AutoPass, |decision, diagnostic| {
                let effect = self
                    .registry
                    .rule(&diagnostic.rule_id)
                    .map(|rule| rule.decision_effect)
                    .unwrap_or_else(|| Decision::default_for_severity(diagnostic.severity));
                decision.max(effect)
            });

        Ok(EvaluationResult {
            decision,
            diagnostics,
        })
    }
}
