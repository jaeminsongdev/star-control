use serde_json::Value;

fn evidence() -> Value {
    serde_json::from_str(include_str!("evidence/management-cli-smoke-v1.json"))
        .expect("checked-in management CLI evidence is valid JSON")
}

fn sha256(value: &Value) -> &str {
    let value = value.as_str().expect("SHA-256 evidence is a string");
    assert!(value.starts_with("sha256:"));
    assert_eq!(value.len(), 71);
    value
}

#[test]
fn actual_windows_management_cli_smoke_evidence_is_complete() {
    let evidence = evidence();
    assert_eq!(evidence["schema_id"], "star.management-cli-smoke-evidence");
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["host"]["architecture"], "x86_64");
    sha256(&evidence["binaries"]["cli_sha256"]);
    sha256(&evidence["binaries"]["controller_sha256"]);
    assert_eq!(
        sha256(&evidence["binaries"]["fake_sha256"]),
        sha256(&evidence["results"]["scaffold_sha256"])
    );
    for result in [
        "validate",
        "trust",
        "revoke",
        "scaffold",
        "scaffold_validate",
        "controller_start",
    ] {
        assert_eq!(evidence["results"][result], true, "{result} must pass");
    }
    assert_eq!(evidence["results"]["list_count"], 3);
    assert_eq!(
        evidence["results"]["describe_trust_basis"],
        "explicit_trust_store"
    );
    assert_eq!(evidence["results"]["probe_product_version"], "1.2.3");
    assert_eq!(evidence["results"]["probe_interface_version"], "1.0.0");
    assert_eq!(evidence["results"]["probe_network_access"], false);
    assert_eq!(evidence["results"]["probe_recorded"], true);
    assert!(
        evidence["results"]["last_probe_at"]
            .as_str()
            .is_some_and(|value| value.ends_with('Z'))
    );
    assert_eq!(evidence["results"]["revoked_list_count"], 0);
    assert_eq!(
        evidence["results"]["revoked_active_state"],
        "last_known_good"
    );
    assert_eq!(evidence["results"]["revoked_candidate_state"], "revoked");
    assert_eq!(evidence["results"]["autostart_initial_state"], "disabled");
    assert_eq!(evidence["results"]["autostart_enabled_state"], "enabled");
    assert_eq!(evidence["results"]["autostart_disabled_state"], "disabled");
    assert_eq!(evidence["results"]["autostart_value_kind"], "String");
    for result in [
        "autostart_exact_command",
        "autostart_enable_idempotent",
        "autostart_disable_idempotent",
        "autostart_mutation_executed",
        "autostart_original_state_restored",
    ] {
        assert_eq!(evidence["results"][result], true, "{result} must pass");
    }
}
