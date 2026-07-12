use serde_json::Value;

fn evidence() -> Value {
    serde_json::from_str(include_str!("evidence/mcp-inspector-v1.json"))
        .expect("checked-in MCP Inspector evidence is valid JSON")
}

fn sha256(value: &Value) -> &str {
    let value = value.as_str().expect("SHA-256 evidence is a string");
    assert!(value.starts_with("sha256:"));
    assert_eq!(value.len(), 71);
    value
}

#[test]
// matrix: MCP-G005 MCP-G006 MCP-G008 MCP-G009 MCP-G017 MCP-G018 MCP-G019 MCP-G020 MCP-C001
fn official_mcp_inspector_0_22_0_evidence_passes_fixed_surface_and_authenticated_ipc() {
    let evidence = evidence();
    assert_eq!(evidence["schema_id"], "star.mcp-inspector-evidence");
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["host"]["architecture"], "x86_64");
    assert_eq!(
        evidence["inspector"]["package"],
        "@modelcontextprotocol/inspector"
    );
    assert_eq!(evidence["inspector"]["version"], "0.22.0");
    assert_eq!(
        evidence["inspector"]["integrity"],
        "sha512-HUyvF+6C3e/sL3wZSc71Li1SkuWysixblFpVdm8csJKBOlT2kNG5kWP0AAgdXRiRWRZ27ZajNtagYgwoJ+QBpQ=="
    );
    assert_eq!(evidence["inspector"]["cli_version"], "0.22.0");
    assert_eq!(evidence["inspector"]["sdk_version"], "1.29.0");
    assert_eq!(evidence["inspector"]["mode"], "official_cli_stdio");
    assert_eq!(
        evidence["inspector"]["cwd_workaround"],
        "inspector-0.22.0-relative-package-json-resolution"
    );
    sha256(&evidence["host"]["node_sha256"]);
    sha256(&evidence["inspector"]["package_json_sha256"]);
    sha256(&evidence["inspector"]["cli_entrypoint_sha256"]);
    sha256(&evidence["inspector"]["package_lock_sha256"]);
    sha256(&evidence["binaries"]["gateway_sha256"]);
    sha256(&evidence["binaries"]["controller_sha256"]);

    let results = &evidence["results"];
    for field in [
        "tools_list",
        "fixed_titles_descriptions_annotations",
        "fully_resolved_input_output_schemas",
        "registry_status",
        "search",
        "search_contains_goal_start",
        "controller_pid_unchanged_between_calls",
    ] {
        assert_eq!(results[field], true, "{field} must pass");
    }
    assert_eq!(results["fixed_tool_count"], 12);
    assert_eq!(
        results["fixed_tool_names"],
        serde_json::json!([
            "star_tool_search",
            "star_tool_describe",
            "star_tool_registry_status",
            "star_tool_call_read_closed",
            "star_tool_call_read_open",
            "star_tool_call_write_closed",
            "star_tool_call_destructive_closed",
            "star_tool_call_write_open",
            "star_tool_call_destructive_open",
            "star_tool_operation_get",
            "star_tool_operation_cancel",
            "star_approval_resolve"
        ])
    );
    for field in [
        "tools_list_exit_code",
        "registry_status_exit_code",
        "search_exit_code",
    ] {
        assert_eq!(results[field], 0, "{field} must be zero");
    }
    assert_eq!(results["release_package_id"], "star.control.core");
    assert_eq!(results["release_trust_basis"], "release_catalog");
    assert_eq!(results["search_query"], "goal");
    assert!(
        results["search_result_count"]
            .as_u64()
            .is_some_and(|count| count >= 1)
    );

    let raw = evidence["raw_evidence"]
        .as_array()
        .expect("raw Inspector evidence is an array");
    assert_eq!(raw.len(), 3);
    for item in raw {
        assert!(
            item["duration_ms"]
                .as_f64()
                .is_some_and(|value| value > 0.0)
        );
        assert!(
            item["stdout"]["bytes"]
                .as_u64()
                .is_some_and(|bytes| bytes > 0)
        );
        assert!(
            item["stdout"]["lines"]
                .as_u64()
                .is_some_and(|lines| lines > 0)
        );
        sha256(&item["stdout"]["sha256"]);
        sha256(&item["stderr"]["sha256"]);
        assert!(
            item["stdout"]["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("target/mcp-inspector-evidence-"))
        );
    }
}
