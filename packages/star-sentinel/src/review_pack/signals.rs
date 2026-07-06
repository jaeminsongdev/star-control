use crate::changed_lines::ChangedLines;
use crate::constants::{
    RULE_DEPENDENCY_REQUIRES_APPROVAL, RULE_SCOPE_ALLOWED_PATHS, RULE_SECRET_NO_PLAINTEXT_SECRET,
    RULE_TEST_NO_DELETION, RULE_VALIDATOR_NO_SELF_BYPASS,
};
use crate::evaluator::normalize_path;
use crate::model::{Decision, EvaluationResult, ReviewValidation};
use std::collections::BTreeSet;

pub(super) fn changed_file_paths(changed_lines: &ChangedLines) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for file in &changed_lines.files {
        for path in file.changed_paths() {
            paths.insert(normalize_path(path));
        }
    }
    paths.into_iter().collect()
}

pub(super) fn review_summary(decision: Decision) -> String {
    match decision {
        Decision::AutoPass => "P0 review passed with no diagnostics.".to_string(),
        Decision::HumanReview => {
            "P0 diagnostics require human review before proceeding.".to_string()
        }
        Decision::Block => "P0 diagnostics block automatic progress.".to_string(),
    }
}

pub(super) fn review_risks(result: &EvaluationResult) -> Vec<String> {
    let mut risks = BTreeSet::new();
    for diagnostic in &result.diagnostics {
        risks.insert(risk_for_rule(&diagnostic.rule_id).to_string());
    }
    risks.into_iter().collect()
}

fn risk_for_rule(rule_id: &str) -> &'static str {
    match rule_id {
        RULE_SCOPE_ALLOWED_PATHS => "scope_violation",
        RULE_TEST_NO_DELETION => "test_deletion",
        RULE_DEPENDENCY_REQUIRES_APPROVAL => "dependency_addition",
        RULE_SECRET_NO_PLAINTEXT_SECRET => "secret_exposure",
        RULE_VALIDATOR_NO_SELF_BYPASS => "validator_bypass",
        _ => "unknown_p0_risk",
    }
}

pub(super) fn review_validations(
    result: &EvaluationResult,
    validations: &[ReviewValidation],
) -> Vec<ReviewValidation> {
    if !validations.is_empty() {
        return validations.to_vec();
    }

    let status = match result.decision {
        Decision::AutoPass => "passed",
        Decision::HumanReview => "requires_human_review",
        Decision::Block => "blocked",
    };
    vec![ReviewValidation::new("policy:p0", status)]
}

pub(super) fn review_questions(result: &EvaluationResult) -> Vec<String> {
    if result.decision == Decision::AutoPass {
        return Vec::new();
    }

    let mut questions = BTreeSet::new();
    for diagnostic in &result.diagnostics {
        let question = match diagnostic.rule_id.as_str() {
            RULE_SCOPE_ALLOWED_PATHS => {
                "Should the task scope be expanded, or should the out-of-scope change be removed?"
            }
            RULE_TEST_NO_DELETION => {
                "What replacement validation covers the deleted test behavior?"
            }
            RULE_DEPENDENCY_REQUIRES_APPROVAL => "Was this dependency change explicitly approved?",
            RULE_SECRET_NO_PLAINTEXT_SECRET => {
                "Has the plaintext secret candidate been removed and rotated if needed?"
            }
            RULE_VALIDATOR_NO_SELF_BYPASS => {
                "Does this validator-related change preserve enforcement?"
            }
            _ => "Does a human approve continuing with this diagnostic?",
        };
        questions.insert(question.to_string());
    }
    questions.into_iter().collect()
}
