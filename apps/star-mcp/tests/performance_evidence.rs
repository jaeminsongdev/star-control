use serde_json::Value;

fn evidence() -> Value {
    serde_json::from_str(include_str!("evidence/mcp-performance-v1.json"))
        .expect("checked-in performance evidence is valid JSON")
}

fn sha256(value: &Value) {
    let value = value.as_str().expect("binary hash is a string");
    assert!(value.starts_with("sha256:"));
    assert_eq!(value.len(), 71);
}

#[test]
fn windows_release_performance_evidence_meets_every_frozen_budget() {
    let evidence = evidence();
    assert_eq!(evidence["schema_id"], "star.mcp-performance-evidence");
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["host"]["architecture"], "x86_64");
    assert!(evidence["host"]["os_build"].as_u64().is_some());
    sha256(&evidence["binaries"]["gateway_sha256"]);
    sha256(&evidence["binaries"]["controller_sha256"]);

    assert_eq!(evidence["registry_capacity"]["total_actions"], 512);
    assert_eq!(evidence["registry_capacity"]["release_actions"], 13);
    assert_eq!(evidence["registry_capacity"]["user_actions"], 499);

    let latency = evidence["latency_ms"]
        .as_object()
        .expect("latency evidence is an object");
    assert_eq!(latency.len(), 7);
    for (name, measurement) in latency {
        assert!(
            measurement["samples"]
                .as_u64()
                .is_some_and(|samples| samples >= 30),
            "{name} must have at least 30 samples"
        );
        let p95 = measurement["p95"].as_f64().expect("p95 is numeric");
        let limit = measurement["limit"].as_f64().expect("limit is numeric");
        assert!(p95 <= limit, "{name}: p95={p95} exceeds limit={limit}");
    }

    let memory = &evidence["memory_mib"];
    assert!(memory["gateway_idle_max"].as_f64().unwrap() <= 64.0);
    assert!(memory["gateway_512_action_working_set"].as_f64().unwrap() <= 64.0);
    assert!(memory["controller_512_action_increment"].as_f64().unwrap() <= 128.0);
}
