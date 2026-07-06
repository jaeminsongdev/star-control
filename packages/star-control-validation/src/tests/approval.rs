use super::*;

#[test]
fn missing_approval_response_blocks_next_stage() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();

    let error = fixture
        .engine()
        .ensure_approval_response_allows_next_stage(&context())
        .unwrap_err();

    assert!(matches!(
        error,
        ValidationEngineError::ApprovalResponseMissing { .. }
    ));
}

#[test]
fn approved_response_allows_next_stage() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();
    fixture
        .store
        .write_approval_json(
            "J-0001",
            "approval-response.json",
            &json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "stage": "validate",
                "task_id": "p0-task-demo",
                "response": "approved",
                "reviewer": "human",
                "responded_at": "2026-07-01T00:00:00Z",
                "reason": "approved for test",
                "allowed_next_stage": "report",
                "constraints": []
            }),
        )
        .expect("write response");

    let response = fixture
        .engine()
        .ensure_approval_response_allows_next_stage(&context())
        .expect("approved");

    assert_eq!(response["response"], "approved");
}

#[test]
fn approval_response_task_mismatch_blocks_next_stage() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();
    fixture
        .store
        .write_approval_json(
            "J-0001",
            "approval-response.json",
            &json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "stage": "validate",
                "task_id": "different-task",
                "response": "approved",
                "reviewer": "human",
                "responded_at": "2026-07-01T00:00:00Z",
                "reason": "approved for test"
            }),
        )
        .expect("write response");

    let error = fixture
        .engine()
        .ensure_approval_response_allows_next_stage(&context())
        .unwrap_err();

    assert!(matches!(
        error,
        ValidationEngineError::ApprovalResponseMismatch { .. }
    ));
}
