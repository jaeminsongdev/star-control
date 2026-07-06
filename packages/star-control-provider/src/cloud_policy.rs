mod credentials;
mod value;

use crate::cloud_constants::CLOUD_API_KIND;
use crate::{ProviderAdapterError, ProviderInstance, ProviderManifest};
use credentials::{is_allowed_credential_ref, raw_credential_field};
pub(crate) use value::string_field;
use value::{bool_pointer, number_pointer, string_pointer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudProviderPolicyDecision {
    pub(crate) privacy_approved: bool,
    pub(crate) credential_ref_present: bool,
    pub(crate) auth_mode_login_session: bool,
    pub(crate) block: CloudProviderBlock,
}

impl CloudProviderPolicyDecision {
    pub(crate) fn evaluate(manifest: &ProviderManifest, instance: &ProviderInstance) -> Self {
        let privacy_approved = bool_pointer(
            instance.value(),
            "/transport_config/privacy_handoff_approved",
        )
        .unwrap_or(false);
        let credential_ref = string_field(instance.value(), "credential_ref");
        let credential_ref_present = credential_ref.is_some();
        let auth_mode_login_session =
            string_pointer(instance.value(), "/transport_config/auth_mode")
                == Some("login_session");

        let block = if let Some(field) = raw_credential_field(instance.value()) {
            CloudProviderBlock::new(
                "cloud_provider_raw_credential",
                "cloud provider instance contains a raw credential-like field",
                Some(field),
            )
        } else if let Some(value) = credential_ref {
            if !is_allowed_credential_ref(value) {
                CloudProviderBlock::new(
                    "cloud_provider_credential_ref_invalid",
                    "credential_ref must use an allowed reference prefix",
                    Some("credential_ref".to_string()),
                )
            } else if !privacy_approved {
                CloudProviderBlock::new(
                    "cloud_privacy_handoff_unapproved",
                    "cloud provider handoff requires explicit privacy approval",
                    Some("transport_config.privacy_handoff_approved".to_string()),
                )
            } else {
                CloudProviderBlock::transport_not_implemented()
            }
        } else if manifest.kind() == CLOUD_API_KIND {
            CloudProviderBlock::new(
                "cloud_api_credential_ref_required",
                "cloud API provider requires credential_ref and never accepts raw credential values",
                Some("credential_ref".to_string()),
            )
        } else if !auth_mode_login_session {
            CloudProviderBlock::new(
                "cloud_cli_auth_reference_required",
                "cloud CLI provider requires credential_ref or transport_config.auth_mode=login_session",
                Some("credential_ref".to_string()),
            )
        } else if !privacy_approved {
            CloudProviderBlock::new(
                "cloud_privacy_handoff_unapproved",
                "cloud provider handoff requires explicit privacy approval",
                Some("transport_config.privacy_handoff_approved".to_string()),
            )
        } else {
            CloudProviderBlock::transport_not_implemented()
        };

        Self {
            privacy_approved,
            credential_ref_present,
            auth_mode_login_session,
            block,
        }
    }

    pub(crate) fn allows_transport_execution(&self) -> bool {
        self.block.kind == "cloud_provider_transport_not_implemented"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudProviderBlock {
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) field: Option<String>,
}

impl CloudProviderBlock {
    fn new(kind: &str, message: &str, field: Option<String>) -> Self {
        Self {
            kind: kind.to_string(),
            message: message.to_string(),
            field,
        }
    }

    fn transport_not_implemented() -> Self {
        Self::new(
            "cloud_provider_transport_not_implemented",
            "cloud provider preflight passed, but transport execution is reserved for the next M6 slice",
            None,
        )
    }
}

pub(crate) fn estimated_cost(instance: &ProviderInstance) -> f64 {
    number_pointer(instance.value(), "/budget/estimated_cost").unwrap_or(0.0)
}

pub(crate) fn currency(instance: &ProviderInstance) -> String {
    string_pointer(instance.value(), "/budget/currency")
        .unwrap_or("USD")
        .to_string()
}

pub(crate) fn cloud_policy_denied(
    provider_instance_id: &str,
    reason: &str,
) -> ProviderAdapterError {
    ProviderAdapterError::CommandPolicyDenied {
        provider_instance_id: provider_instance_id.to_string(),
        reason: reason.to_string(),
    }
}
