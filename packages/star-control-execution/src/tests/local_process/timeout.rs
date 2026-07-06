use crate::test_support::{EnvVarGuard, Fixture};
use crate::ExecutionEngine;

#[test]
fn local_process_timeout_updates_run_state_to_failed() {
    let mut fixture = Fixture::new();
    let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_SLEEP_HELPER", "1");
    fixture.use_local_process_registry(
        vec![
            "--exact".to_string(),
            "tests::execution_sleep_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec!["STAR_CONTROL_EXECUTION_SLEEP_HELPER".to_string()],
        1,
    );
    fixture.assign_implement_stage_to_local_process();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute timeout stage");

    assert_eq!(outcome.provider_execution().result().status(), "timeout");
    assert_eq!(outcome.state()["state"], "FAILED");
}
