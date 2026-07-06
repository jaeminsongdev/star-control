use super::super::{ChangeType, Risk, Size};

pub(super) fn size_for_change_type(change_type: ChangeType) -> Size {
    match change_type {
        ChangeType::RuntimeCodeChange | ChangeType::SchemaChange => Size::Medium,
        ChangeType::MultiPackageChange
        | ChangeType::ProviderContractChange
        | ChangeType::PublicApiChange => Size::Large,
        ChangeType::ReleaseChange
        | ChangeType::DeployChange
        | ChangeType::CredentialChange
        | ChangeType::SensitiveDataExposure
        | ChangeType::ValidatorSelfBypass
        | ChangeType::ExternalAccountChange => Size::Critical,
        ChangeType::DependencyAddition
        | ChangeType::DependencyVersionChange
        | ChangeType::WorkflowChange
        | ChangeType::SchemaBreakingChange
        | ChangeType::FileDeletion
        | ChangeType::BulkMove
        | ChangeType::RiskPathChange
        | ChangeType::BudgetExceeded
        | ChangeType::UnknownHighRisk
        | ChangeType::ValidatorSensitiveChange => Size::Medium,
        ChangeType::DocsOnly | ChangeType::ExampleChange => Size::Small,
    }
}

pub(super) fn risk_for_change_type(change_type: ChangeType) -> Risk {
    match change_type {
        ChangeType::RuntimeCodeChange => Risk::Medium,
        ChangeType::SchemaChange
        | ChangeType::SchemaBreakingChange
        | ChangeType::MultiPackageChange
        | ChangeType::ProviderContractChange
        | ChangeType::DependencyAddition
        | ChangeType::DependencyVersionChange
        | ChangeType::WorkflowChange
        | ChangeType::PublicApiChange
        | ChangeType::ValidatorSensitiveChange
        | ChangeType::FileDeletion
        | ChangeType::BulkMove
        | ChangeType::RiskPathChange
        | ChangeType::BudgetExceeded
        | ChangeType::UnknownHighRisk => Risk::High,
        ChangeType::CredentialChange
        | ChangeType::SensitiveDataExposure
        | ChangeType::ReleaseChange
        | ChangeType::DeployChange
        | ChangeType::ValidatorSelfBypass
        | ChangeType::ExternalAccountChange => Risk::Critical,
        ChangeType::DocsOnly | ChangeType::ExampleChange => Risk::Low,
    }
}

pub(super) fn requires_validator_profile(change_type: ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::SchemaChange
            | ChangeType::SchemaBreakingChange
            | ChangeType::ValidatorSensitiveChange
            | ChangeType::ValidatorSelfBypass
    )
}

pub(super) fn requires_release_profile(change_type: ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::ReleaseChange | ChangeType::DeployChange
    )
}

pub(super) fn requires_security_profile(change_type: ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::DependencyAddition
            | ChangeType::DependencyVersionChange
            | ChangeType::WorkflowChange
            | ChangeType::CredentialChange
            | ChangeType::SensitiveDataExposure
            | ChangeType::ExternalAccountChange
    )
}

pub(super) fn approval_required_for(change_type: ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::DependencyAddition
            | ChangeType::DependencyVersionChange
            | ChangeType::WorkflowChange
            | ChangeType::ReleaseChange
            | ChangeType::DeployChange
            | ChangeType::PublicApiChange
            | ChangeType::SchemaBreakingChange
            | ChangeType::SchemaChange
            | ChangeType::FileDeletion
            | ChangeType::BulkMove
            | ChangeType::RiskPathChange
            | ChangeType::CredentialChange
            | ChangeType::SensitiveDataExposure
            | ChangeType::ValidatorSensitiveChange
            | ChangeType::ValidatorSelfBypass
            | ChangeType::ExternalAccountChange
            | ChangeType::BudgetExceeded
            | ChangeType::UnknownHighRisk
    )
}

pub(super) fn blocks_for(change_type: ChangeType) -> bool {
    matches!(
        change_type,
        ChangeType::SensitiveDataExposure | ChangeType::ValidatorSelfBypass
    )
}

pub(super) fn approval_reason_for(change_type: ChangeType) -> Option<&'static str> {
    match change_type {
        ChangeType::SchemaChange => Some("schema_change_requires_approval"),
        ChangeType::SchemaBreakingChange => Some("schema_breaking_change_requires_approval"),
        ChangeType::DependencyAddition => Some("dependency_addition_requires_approval"),
        ChangeType::DependencyVersionChange => Some("dependency_version_change_requires_approval"),
        ChangeType::WorkflowChange => Some("workflow_change_requires_approval"),
        ChangeType::PublicApiChange => Some("public_api_change_requires_approval"),
        ChangeType::CredentialChange => Some("credential_change_requires_approval"),
        ChangeType::SensitiveDataExposure => Some("sensitive_data_exposure_blocked"),
        ChangeType::ReleaseChange => Some("release_change_requires_approval"),
        ChangeType::DeployChange => Some("deploy_change_requires_approval"),
        ChangeType::ValidatorSensitiveChange => {
            Some("validator_sensitive_change_requires_approval")
        }
        ChangeType::ValidatorSelfBypass => Some("validator_self_bypass_blocked"),
        ChangeType::FileDeletion => Some("file_deletion_requires_approval"),
        ChangeType::BulkMove => Some("bulk_move_requires_approval"),
        ChangeType::RiskPathChange => Some("risk_path_change_requires_approval"),
        ChangeType::ExternalAccountChange => Some("external_account_change_requires_approval"),
        ChangeType::BudgetExceeded => Some("budget_exceeded_requires_approval"),
        ChangeType::UnknownHighRisk => Some("unknown_high_risk_requires_approval"),
        _ => None,
    }
}
