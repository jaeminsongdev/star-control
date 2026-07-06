use crate::cloud_policy::CloudProviderPolicyDecision;
use crate::ProviderManifest;

pub(crate) fn stdout_value(
    manifest: &ProviderManifest,
    decision: &CloudProviderPolicyDecision,
) -> String {
    format!(
        "cloud provider preflight\nprovider_id={}\nkind={}\ntransport={}\ncredential_ref_present={}\nauth_mode_login_session={}\nprivacy_handoff_approved={}\ntransport_execution=false\n",
        manifest.id(),
        manifest.kind(),
        manifest.transport(),
        decision.credential_ref_present,
        decision.auth_mode_login_session,
        decision.privacy_approved,
    )
}

pub(crate) fn stderr_value(decision: &CloudProviderPolicyDecision) -> String {
    format!(
        "blocked kind={} field={} message={}\n",
        decision.block.kind,
        decision.block.field.as_deref().unwrap_or(""),
        decision.block.message
    )
}
