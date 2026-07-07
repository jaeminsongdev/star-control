use super::summary::provider_summary_value;
use crate::config::CliConfig;
use serde_json::{json, Value};
use star_control_provider::{CapabilityProfile, ProviderManifest};

pub(super) fn provider_readiness_value(
    manifest: &ProviderManifest,
    profile: Option<&CapabilityProfile>,
    config: &CliConfig,
) -> Value {
    let classification = classify_provider(manifest);
    json!({
        "provider": provider_summary_value(manifest, profile, config),
        "status": classification.status,
        "connector_scope": classification.connector_scope,
        "live_execution": classification.live_execution,
        "live_calls_performed": false,
        "checks": {
            "manifest_present": true,
            "capability_profile_present": profile.is_some(),
            "credential_raw_value_accessed": false,
            "network_or_process_probe_performed": false
        },
        "disabled_reason": classification.disabled_reason
    })
}

pub(super) fn readiness_summary_value(providers: &[Value]) -> Value {
    let mut ready = 0usize;
    let mut disabled = 0usize;
    let mut manual = 0usize;
    let mut missing_capability_profile = 0usize;

    for provider in providers {
        match provider.get("status").and_then(Value::as_str).unwrap_or("") {
            "ready" => ready += 1,
            "disabled" => disabled += 1,
            "manual" => manual += 1,
            _ => {}
        }
        if provider
            .pointer("/checks/capability_profile_present")
            .and_then(Value::as_bool)
            != Some(true)
        {
            missing_capability_profile += 1;
        }
    }

    json!({
        "ready": ready,
        "disabled": disabled,
        "manual": manual,
        "missing_capability_profile": missing_capability_profile,
        "local_ai_live_connector": "disabled",
        "cloud_ai_live_connector": "disabled"
    })
}

struct ProviderReadiness {
    status: &'static str,
    connector_scope: &'static str,
    live_execution: &'static str,
    disabled_reason: Value,
}

fn classify_provider(manifest: &ProviderManifest) -> ProviderReadiness {
    match manifest.kind() {
        "fake_provider" => ProviderReadiness {
            status: "ready",
            connector_scope: "offline_fixture",
            live_execution: "not_required",
            disabled_reason: Value::Null,
        },
        "human_handoff" => ProviderReadiness {
            status: "manual",
            connector_scope: "human_handoff",
            live_execution: "manual",
            disabled_reason: Value::Null,
        },
        "local_process_model" | "local_openai_compatible_server" => ProviderReadiness {
            status: "disabled",
            connector_scope: "local_ai",
            live_execution: "reserved",
            disabled_reason: json!(
                "Local AI connector live execution remains intentionally unimplemented"
            ),
        },
        "cloud_api_model" | "cloud_cli_agent" => ProviderReadiness {
            status: "disabled",
            connector_scope: "cloud_ai",
            live_execution: "reserved",
            disabled_reason: json!(
                "Cloud AI connector live execution remains intentionally unimplemented"
            ),
        },
        _ => ProviderReadiness {
            status: "disabled",
            connector_scope: "unknown",
            live_execution: "unsupported",
            disabled_reason: json!("provider kind is not mapped to a product readiness class"),
        },
    }
}
