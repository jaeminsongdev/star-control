mod haystack;
mod rules;

use super::policy::{
    approval_reasons_for, blocks, profile_for, requires_approval, risk_for, size_for,
};
use super::{ChangeType, RequestAnalysis, RouteDecision};
use crate::JobSpec;

impl RequestAnalysis {
    pub(crate) fn analyze(job: &JobSpec) -> Self {
        let haystack = haystack::normalized(job);
        let (mut change_types, reasons) = rules::matched_change_types(&haystack);

        if change_types.is_empty() {
            change_types.push(ChangeType::RuntimeCodeChange);
        }
        let routing_reasons = if reasons.is_empty() {
            vec!["defaulted to runtime code change".to_string()]
        } else {
            reasons
        };
        change_types.sort();
        change_types.dedup();

        let size = size_for(&change_types);
        let risk = risk_for(&change_types);
        let profile = profile_for(&change_types, size, risk);
        let requires_user_approval = requires_approval(&change_types);
        let blocks = blocks(&change_types);
        let decision = if blocks {
            RouteDecision::Block
        } else if requires_user_approval {
            RouteDecision::HumanReview
        } else {
            RouteDecision::AutoPass
        };
        let approval_reasons = approval_reasons_for(&change_types, profile, blocks);

        Self {
            change_types,
            routing_reasons,
            approval_reasons,
            size,
            risk,
            profile,
            decision,
            requires_user_approval,
        }
    }
}
