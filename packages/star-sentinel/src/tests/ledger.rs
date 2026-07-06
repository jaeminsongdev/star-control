use super::*;
use star_control_state::StateStore;

#[test]
fn builds_schema_valid_gate_ledger_event() {
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let result = scope_block_result();
    let event = build_gate_ledger_event("E-P0-0001", &task, &result, "2026-01-01T00:00:00Z");

    validate_ledger_events(std::slice::from_ref(&event), schema_root()).expect("ledger schema");
    assert_eq!(event.to_value()["event_type"], "GATE_DECIDED");
    assert_eq!(event.to_value()["metadata"]["decision"], "BLOCK");
}

#[test]
fn writes_ledger_jsonl_to_tool_output() {
    let temp_project = temp_dir();
    let store = StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
    let job = store
        .create_job("write ledger", "star-sentinel", Vec::new())
        .expect("job");
    let job_id = job["job_id"].as_str().expect("job_id");
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let result = scope_block_result();
    let event = build_gate_ledger_event("E-P0-0001", &task, &result, "2026-01-01T00:00:00Z");

    let artifact_ref =
        write_ledger_artifact(&store, job_id, &[event], schema_root()).expect("ledger write");

    assert_eq!(
        artifact_ref["path"],
        "tool-output/star-sentinel/ledger.jsonl"
    );
    let ledger_path = temp_project.join(".ai-runs/J-0001/tool-output/star-sentinel/ledger.jsonl");
    assert!(ledger_path.is_file());
    let content = fs::read_to_string(ledger_path).expect("ledger content");
    assert_eq!(content.lines().count(), 1);
    fs::remove_dir_all(temp_project).ok();
}
