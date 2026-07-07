use super::super::{create_job, open_store, temp_project};
use crate::StateStoreError;
use serde_json::json;
use std::fs;

#[test]
fn writes_output_artifacts_and_artifact_refs_inside_job() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    let provider_ref = store
        .write_provider_json(
            "J-0001",
            "fake-default",
            "request.json",
            &json!({ "goal": "test" }),
        )
        .expect("write provider json");
    let stdout_ref = store
        .write_provider_text("J-0001", "fake-default", "stdout.txt", "ok\n")
        .expect("write provider stdout");
    let tool_ref = store
        .write_tool_json("J-0001", "star-sentinel", "diagnostics.json", &json!([]))
        .expect("write tool json");
    let tool_markdown_ref = store
        .write_tool_text("J-0001", "star-sentinel", "review_pack.md", "# Review\n")
        .expect("write tool markdown");
    let approval_ref = store
        .write_approval_json("J-0001", "approval-request.json", &json!({ "ok": true }))
        .expect("write approval");
    let review_json_ref = store
        .write_review_pack_json("J-0001", "review_pack.json", &json!({ "items": [] }))
        .expect("write review json");
    let review_md_ref = store
        .write_review_pack_markdown("J-0001", "review_pack.md", "# Review\n")
        .expect("write review markdown");
    let validation_ref = store
        .write_validation_json(
            "J-0001",
            "validation-decision.json",
            &json!({ "decision": "AUTO_PASS" }),
        )
        .expect("write validation json");
    let redaction_ref = store
        .write_redaction_report_json(
            "J-0001",
            "redaction-report.json",
            &json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "artifact_path": "provider-output/fake-default/stdout.txt",
                "redacted": true,
                "placeholder": "[REDACTED]",
                "findings": [{
                    "kind": "credential_candidate",
                    "path": "$.stdout",
                    "action": "redacted"
                }]
            }),
        )
        .expect("write redaction report");
    let tmp_path = store
        .write_tmp_json("J-0001", "run-state.json", &json!({ "tmp": true }))
        .expect("write tmp json");

    assert_eq!(
        provider_ref["path"],
        "provider-output/fake-default/request.json"
    );
    assert_eq!(provider_ref["kind"], "provider_output");
    assert_eq!(stdout_ref["kind"], "log");
    assert_eq!(
        tool_ref["path"],
        "tool-output/star-sentinel/diagnostics.json"
    );
    assert_eq!(
        tool_markdown_ref["path"],
        "tool-output/star-sentinel/review_pack.md"
    );
    assert_eq!(approval_ref["kind"], "approval");
    assert_eq!(review_json_ref["kind"], "review_pack");
    assert_eq!(review_md_ref["path"], "review-packs/review_pack.md");
    assert_eq!(
        validation_ref["path"],
        "validation/validation-decision.json"
    );
    assert_eq!(validation_ref["kind"], "other");
    assert_eq!(redaction_ref["path"], "audit/redaction-report.json");
    assert_eq!(redaction_ref["kind"], "other");
    assert_eq!(
        redaction_ref["schema_path"],
        "specs/schemas/redaction-report.schema.json"
    );
    assert!(tmp_path.starts_with("tmp/run-state.json.tmp-"));
    assert!(project
        .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/review_pack.md")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/approvals/approval-request.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/review-packs/review_pack.md")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/validation/validation-decision.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/audit/redaction-report.json")
        .is_file());

    assert!(matches!(
        store.write_provider_json(
            "J-0001",
            "fake-default",
            "request.json",
            &json!({ "goal": "overwrite" }),
        ),
        Err(StateStoreError::ArtifactAlreadyExists { .. })
    ));

    fs::remove_dir_all(project).ok();
}
