use serde_json::Value;

fn evidence() -> Value {
    serde_json::from_str(include_str!("evidence/mcp-arm64-native-smoke-v1.json"))
        .expect("checked-in native ARM64 evidence is valid JSON")
}

fn sha256(value: &Value) -> &str {
    let value = value.as_str().expect("SHA-256 evidence is a string");
    assert!(value.starts_with("sha256:"));
    assert_eq!(value.len(), 71);
    value
}

#[test]
// matrix: MCP-G001 MCP-G005 MCP-I001 MCP-I009 MCP-P001 MCP-P010 MCP-P025 MCP-S012 MCP-S013 MCP-S014
fn native_windows_arm64_evidence_covers_gateway_ipc_registry_process_and_sandbox_tests() {
    let evidence = evidence();
    assert_eq!(
        evidence["schema_id"],
        "star.mcp-arm64-native-smoke-evidence"
    );
    assert_eq!(evidence["schema_version"], 1);

    let full = &evidence["workflow"]["full_gate_run"];
    assert_eq!(full["overall_conclusion"], "failure");
    for field in [
        "native_workspace_tests",
        "native_clippy",
        "schema_matrix_format",
        "native_release_build",
        "initial_smoke_fail_closed",
    ] {
        assert_eq!(full[field], true, "full gate step {field} must pass");
    }
    assert_eq!(full["initial_smoke"], false);
    assert_eq!(
        full["initial_smoke_failure"],
        "github_actions_outer_job_denied_verified_controller_breakaway"
    );
    let smoke = &evidence["workflow"]["smoke_run"];
    assert_eq!(smoke["overall_conclusion"], "success");
    assert_eq!(smoke["native_release_build"], true);
    assert_eq!(smoke["native_gateway_ipc_registry_process_smoke"], true);

    let host = &evidence["host"];
    assert_eq!(host["os_architecture"], "Arm64");
    assert_eq!(host["process_architecture"], "Arm64");
    assert_eq!(host["processor_architecture"], "ARM64");
    assert_eq!(host["runner_image"], "win11-arm64");
    assert_eq!(host["minimum_supported_build"], 26100);
    assert!(
        host["current_build"]
            .as_u64()
            .is_some_and(|build| build >= 26100)
    );
    assert_eq!(host["minimum_build_met"], true);
    assert_eq!(host["display_version"], "25H2");
    assert_eq!(host["exact_24h2_baseline_executed"], false);

    for field in [
        "cli_sha256",
        "gateway_sha256",
        "controller_sha256",
        "fake_sha256",
    ] {
        sha256(&evidence["binaries"][field]);
    }
    assert_eq!(evidence["binaries"]["pe_machine"], "0xaa64");
    for field in [
        "named_pipe_dacl_dpapi_hmac",
        "job_object_child_tree",
        "appcontainer_adapter_acl_loopback",
        "argv_json_stdio_output_handle",
        "watcher_registry_lkg",
    ] {
        assert_eq!(
            evidence["native_test_assertions"][field], true,
            "native test assertion {field} must pass"
        );
    }

    let results = &evidence["results"];
    for field in [
        "controller_start",
        "verified_existing_controller_identity",
        "only_tools_capability",
        "authenticated_ipc",
        "release_core_ready",
        "user_manifest_trusted",
        "search_ready",
        "external_process_untrusted_output",
    ] {
        assert_eq!(results[field], true, "native result {field} must pass");
    }
    assert_eq!(
        results["controller_launch_mode"],
        "runner_prestarted_outer_job_bound"
    );
    assert_eq!(results["protocol_version"], "2025-11-25");
    assert_eq!(results["fixed_tool_count"], 12);
    assert_eq!(results["descriptor_architecture"], "aarch64");
    assert_eq!(results["external_process_outcome"], "success");
    assert_eq!(results["external_process_exit_code"], 0);
    assert_eq!(results["external_process_stdout"], "argv:native-arm64\n");

    let raw = evidence["raw_evidence"]
        .as_array()
        .expect("native raw evidence is an array");
    assert_eq!(raw.len(), 5);
    for item in raw {
        assert!(item["bytes"].as_u64().is_some_and(|bytes| bytes > 0));
        assert!(item["lines"].as_u64().is_some_and(|lines| lines > 0));
        sha256(&item["sha256"]);
        assert!(
            item["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("target/arm64-ci-artifact-run"))
        );
    }
}
