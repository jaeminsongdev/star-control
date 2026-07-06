use crate::test_support::{EnvVarGuard, Fixture};
use crate::ExecutionEngine;

#[test]
fn local_process_forbidden_action_evidence_updates_run_state_to_blocked() {
    let mut fixture = Fixture::new();
    let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER", "1");
    fixture.use_local_process_registry(
        vec![
            "--exact".to_string(),
            "tests::execution_forbidden_evidence_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec!["STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER".to_string()],
        10,
    );
    fixture.assign_implement_stage_to_local_process();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute forbidden evidence stage");

    assert_eq!(outcome.provider_execution().result().status(), "blocked");
    assert_eq!(outcome.state()["state"], "BLOCKED");
    assert_eq!(
        outcome.provider_execution().result().value()["error"]["kind"],
        "local_process_forbidden_action"
    );
    assert_eq!(
        outcome.provider_execution().result().value()["error"]["action"],
        "dependency_install"
    );
}
