mod approval;
mod outcome;

use super::ValidationEngine;
use crate::artifacts::diagnostic_for_error;
use crate::constants::{
    SENTINEL_APPROVAL_PATH, SENTINEL_APPROVAL_SCHEMA, SENTINEL_REVIEW_PACK_JSON_PATH,
    SENTINEL_REVIEW_PACK_SCHEMA,
};
use crate::error::ValidationEngineError;
use crate::types::{ValidationContext, ValidationOutcome};
use approval::SentinelApproval;
use serde_json::{json, Value};

impl<'a> ValidationEngine<'a> {
    pub fn evaluate_star_sentinel_gate(
        &self,
        context: &ValidationContext,
        approval: &Value,
        review_pack: Option<&Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        if let Err(error) = self.validate_sentinel_schema(
            approval,
            SENTINEL_APPROVAL_SCHEMA,
            SENTINEL_APPROVAL_PATH,
        ) {
            return self.failed_outcome(
                context,
                "star_sentinel_output_invalid",
                vec![diagnostic_for_error("star-sentinel.output.invalid", &error)],
            );
        }

        let approval = SentinelApproval::from_value(approval)?;

        if let Some(diagnostic) = approval.task_mismatch_diagnostic(context.task_id()) {
            return self.failed_outcome(context, "star_sentinel_task_mismatch", vec![diagnostic]);
        }

        if let Some(diagnostics) = approval.inconsistent_auto_pass_diagnostics() {
            return self.failed_outcome(context, "star_sentinel_output_inconsistent", diagnostics);
        }

        match approval.decision.as_str() {
            "AUTO_PASS" => self.normal_outcome(
                context,
                &approval.decision,
                "VALIDATED",
                approval.reasons,
                approval.diagnostics,
                None,
            ),
            "HUMAN_REVIEW" => {
                let review_pack = match review_pack {
                    Some(review_pack) => review_pack,
                    None => {
                        return self.failed_outcome(
                            context,
                            "review_pack_missing",
                            vec![json!({
                                "rule_id": "validation.review_pack.missing",
                                "severity": "block",
                                "message": "HUMAN_REVIEW requires a review pack handoff"
                            })],
                        )
                    }
                };
                if let Err(error) = self.validate_sentinel_schema(
                    review_pack,
                    SENTINEL_REVIEW_PACK_SCHEMA,
                    SENTINEL_REVIEW_PACK_JSON_PATH,
                ) {
                    return self.failed_outcome(
                        context,
                        "review_pack_invalid",
                        vec![diagnostic_for_error(
                            "star-sentinel.review_pack.invalid",
                            &error,
                        )],
                    );
                }
                self.normal_outcome(
                    context,
                    &approval.decision,
                    "WAITING_APPROVAL",
                    approval.reasons,
                    approval.diagnostics,
                    Some(review_pack),
                )
            }
            "BLOCK" => {
                let review_pack = match review_pack {
                    Some(review_pack) => review_pack,
                    None => {
                        return self.failed_outcome(
                            context,
                            "review_pack_missing",
                            vec![json!({
                                "rule_id": "validation.review_pack.missing",
                                "severity": "block",
                                "message": "BLOCK requires a review pack handoff"
                            })],
                        )
                    }
                };
                if let Err(error) = self.validate_sentinel_schema(
                    review_pack,
                    SENTINEL_REVIEW_PACK_SCHEMA,
                    SENTINEL_REVIEW_PACK_JSON_PATH,
                ) {
                    return self.failed_outcome(
                        context,
                        "review_pack_invalid",
                        vec![diagnostic_for_error(
                            "star-sentinel.review_pack.invalid",
                            &error,
                        )],
                    );
                }
                self.normal_outcome(
                    context,
                    &approval.decision,
                    "BLOCKED",
                    approval.reasons,
                    approval.diagnostics,
                    Some(review_pack),
                )
            }
            _ => self.failed_outcome(
                context,
                "approval_decision_invalid",
                vec![json!({
                    "rule_id": "star-sentinel.output.decision_invalid",
                    "severity": "block",
                    "message": format!("unsupported approval decision {}", approval.decision)
                })],
            ),
        }
    }
}
