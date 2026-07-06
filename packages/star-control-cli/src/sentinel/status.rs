use star_sentinel::Decision;

pub(super) fn status_for_sentinel_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "success",
        Decision::HumanReview => "waiting_approval",
        Decision::Block => "blocked",
    }
}

pub(super) fn validation_result_for_sentinel_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "PASS",
        Decision::HumanReview => "HUMAN_REVIEW",
        Decision::Block => "BLOCK",
    }
}
