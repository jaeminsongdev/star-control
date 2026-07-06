use super::{ChangeType, RouteDecision};

pub(super) fn stages_for(
    decision: RouteDecision,
    change_types: &[ChangeType],
) -> Vec<&'static str> {
    if decision == RouteDecision::Block {
        return vec!["route", "report"];
    }
    if change_types.iter().any(release_or_deploy_change) {
        return vec!["design", "validate", "review", "report"];
    }
    if change_types.iter().any(validator_or_schema_change) {
        return vec!["design", "implement", "validate", "review", "report"];
    }
    if change_types.contains(&ChangeType::RuntimeCodeChange) {
        return vec![
            "design",
            "implement",
            "validate",
            "review",
            "polish",
            "report",
        ];
    }
    vec!["implement", "validate", "review", "report"]
}

fn release_or_deploy_change(change_type: &ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::ReleaseChange | ChangeType::DeployChange
    )
}

fn validator_or_schema_change(change_type: &ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::SchemaChange
            | ChangeType::SchemaBreakingChange
            | ChangeType::ValidatorSensitiveChange
            | ChangeType::ValidatorSelfBypass
    )
}
