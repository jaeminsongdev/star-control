use crate::test_support::{EnvVarGuard, Fixture};
use crate::ExecutionEngine;
use serde_json::json;
use star_control_state::StateStore;
use std::time::Duration;

#[test]
fn local_process_cancelled_updates_run_state_to_cancelled() {
    let mut fixture = Fixture::new();
    let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_SLEEP_HELPER", "1");
    fixture.use_local_process_registry(
        vec![
            "--exact".to_string(),
            "tests::execution_sleep_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec!["STAR_CONTROL_EXECUTION_SLEEP_HELPER".to_string()],
        10,
    );
    fixture.assign_implement_stage_to_local_process();

    let cancel_project = fixture.project.clone();
    let cancel_schemas = fixture.schemas.clone();
    let cancel_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let store = StateStore::open(cancel_project, cancel_schemas).expect("open cancel store");
        let mut state = store.load_state("J-0001").expect("load state");
        state["state"] = json!("CANCELLED");
        state["next_action"] = json!("stop");
        store
            .save_state("J-0001", &state)
            .expect("save cancelled state");
    });

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute cancelled stage");
    cancel_thread.join().expect("cancel thread");

    assert_eq!(outcome.provider_execution().result().status(), "cancelled");
    assert_eq!(outcome.state()["state"], "CANCELLED");
    assert_eq!(outcome.state()["next_action"], "stop");
}
