mod mapping;

use super::{ChangeType, PolicyProfile, Risk, Size};
use mapping::{
    approval_reason_for, approval_required_for, blocks_for, requires_release_profile,
    requires_security_profile, requires_validator_profile, risk_for_change_type,
    size_for_change_type,
};

pub(super) fn size_for(change_types: &[ChangeType]) -> Size {
    change_types
        .iter()
        .copied()
        .map(size_for_change_type)
        .max()
        .unwrap_or(Size::Small)
}

pub(super) fn risk_for(change_types: &[ChangeType]) -> Risk {
    change_types
        .iter()
        .copied()
        .map(risk_for_change_type)
        .max()
        .unwrap_or(Risk::Low)
}

pub(super) fn profile_for(change_types: &[ChangeType], size: Size, risk: Risk) -> PolicyProfile {
    if change_types.iter().copied().any(requires_validator_profile) {
        return PolicyProfile::Validator;
    }
    if change_types.iter().copied().any(requires_release_profile) {
        return PolicyProfile::Release;
    }
    if change_types.iter().copied().any(requires_security_profile) {
        return PolicyProfile::Security;
    }
    if size >= Size::Large || risk >= Risk::High {
        return PolicyProfile::Full;
    }
    if size == Size::Medium || risk == Risk::Medium {
        return PolicyProfile::Near;
    }
    PolicyProfile::Quick
}

pub(super) fn requires_approval(change_types: &[ChangeType]) -> bool {
    change_types.iter().copied().any(approval_required_for)
}

pub(super) fn blocks(change_types: &[ChangeType]) -> bool {
    change_types.iter().copied().any(blocks_for)
}

pub(super) fn approval_reasons_for(
    change_types: &[ChangeType],
    profile: PolicyProfile,
    blocks: bool,
) -> Vec<String> {
    let mut reasons = Vec::new();
    for change_type in change_types.iter().copied() {
        if let Some(reason) = approval_reason_for(change_type) {
            reasons.push(reason);
        }
    }
    if profile == PolicyProfile::Validator {
        reasons.push("validator_profile_requires_review");
    }
    if blocks {
        reasons.push("blocked_route_requires_report_only");
    }
    reasons.sort();
    reasons.dedup();
    reasons.into_iter().map(str::to_string).collect()
}
