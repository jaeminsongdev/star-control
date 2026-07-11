use serde_json::Value;

fn evidence() -> Value {
    serde_json::from_str(include_str!("evidence/codex-same-session-v1.json"))
        .expect("checked-in Codex evidence is valid JSON")
}

fn sha256(value: &Value) -> &str {
    let value = value.as_str().expect("SHA-256 evidence is a string");
    assert!(value.starts_with("sha256:"));
    assert_eq!(value.len(), 71);
    value
}

#[test]
// matrix: MCP-C001 MCP-C002 MCP-C003 MCP-C004 MCP-C005 MCP-C006 MCP-C007 MCP-C008
fn actual_codex_same_session_evidence_is_complete_and_internally_consistent() {
    let evidence = evidence();
    assert_eq!(evidence["schema_id"], "star.codex-same-session-evidence");
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["host"]["kind"], "actual_codex_cli_mcp_host");
    assert_eq!(evidence["host"]["fixed_tool_count"], 12);
    assert_eq!(evidence["host"]["mcp_server_version"], "0.1.0");
    sha256(&evidence["host"]["codex_executable_sha256"]);
    sha256(&evidence["host"]["mcp_server_sha256"]);
    sha256(&evidence["host"]["controller_sha256"]);

    for id in 1..=8 {
        assert_eq!(
            evidence[format!("c{id:03}")]["passed"],
            true,
            "C{id:03} must have actual-host evidence"
        );
    }
    assert_eq!(evidence["c001"]["trust_basis"], "release_catalog");
    assert_eq!(
        evidence["c002"]["trust_basis"],
        "personal_auto_user_manifest"
    );
    assert_eq!(evidence["c002"]["pids_unchanged"], true);
    assert_eq!(evidence["c003"]["pids_unchanged"], true);
    assert_ne!(
        sha256(&evidence["c002"]["descriptor_hash"]),
        sha256(&evidence["c003"]["descriptor_hash"])
    );
    assert_ne!(
        sha256(&evidence["c004"]["old_executable_sha256"]),
        sha256(&evidence["c004"]["new_executable_sha256"])
    );
    assert_eq!(evidence["c004"]["stale_error"], "TOOL_DESCRIPTOR_STALE");
    assert_eq!(evidence["c004"]["stale_process_started"], false);
    assert_eq!(evidence["c005"]["diagnostic"], "TOOL_MANIFEST_INVALID");
    assert_eq!(
        evidence["c005"]["descriptor_hash"],
        evidence["c004"]["new_descriptor_hash"]
    );
    assert_eq!(evidence["c006"]["status"], "approval_required");
    assert_eq!(evidence["c006"]["side_effect_marker_exists"], false);
    assert_eq!(evidence["c007"]["terminal_status"], "cancelled");
    assert_eq!(evidence["c007"]["cancel_effective"], true);
    assert_eq!(evidence["c007"]["child_running_after_cancel"], false);
    assert_eq!(
        evidence["c007"]["progress"],
        serde_json::json!([
            "received",
            "resolving",
            "queued",
            "starting",
            "running",
            "cancel_requested",
            "cancelled"
        ])
    );

    assert_eq!(evidence["c008"]["transport_closed_before_reconnect"], true);
    assert_eq!(evidence["c008"]["gateway_restart_count"], 1);
    assert_ne!(
        evidence["c008"]["old_gateway_pid"],
        evidence["c008"]["new_gateway_pid"]
    );
    assert_eq!(
        evidence["c008"]["controller_pid_before"],
        evidence["c008"]["controller_pid_after"]
    );
    assert_eq!(
        evidence["c008"]["controller_instance_before"],
        evidence["c008"]["controller_instance_after"]
    );
    assert_eq!(
        evidence["c008"]["registry_revision_before"],
        evidence["c008"]["registry_revision_after"]
    );
    assert_eq!(
        evidence["c008"]["operation_id_before"],
        evidence["c008"]["operation_id_after"]
    );
    assert_eq!(evidence["c008"]["operation_status_after"], "cancelled");

    let raw = evidence["raw_evidence"]
        .as_array()
        .expect("raw evidence attestations are an array");
    assert_eq!(raw.len(), 3);
    for item in raw {
        assert!(item["bytes"].as_u64().is_some_and(|bytes| bytes > 0));
        assert!(item["lines"].as_u64().is_some_and(|lines| lines > 0));
        sha256(&item["sha256"]);
    }
}
