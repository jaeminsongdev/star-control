mod assertions;

use super::fixture::Fixture;
use super::helpers::EnvVarGuard;
use crate::ExecutionEngine;
use assertions::assert_local_process_output_contract;
use serde_json::json;
use star_control_state::StateStore;
use std::time::Duration;

pub(crate) struct LocalProcessConformanceCase {
    pub(crate) id: &'static str,
    pub(crate) args: Vec<String>,
    pub(crate) env_name: Option<&'static str>,
    pub(crate) timeout_seconds: u64,
    pub(crate) cancel_after: Option<Duration>,
    pub(crate) expected_status: &'static str,
    pub(crate) expected_state: &'static str,
    pub(crate) expected_error_kind: Option<&'static str>,
    pub(crate) expected_error_action: Option<&'static str>,
}

pub(crate) fn run_local_process_conformance_case(case: LocalProcessConformanceCase) {
    let mut fixture = Fixture::new();
    let _env = case.env_name.map(|name| EnvVarGuard::set(name, "1"));
    fixture.use_local_process_registry(
        case.args.clone(),
        case.env_name
            .map(|name| vec![name.to_string()])
            .unwrap_or_default(),
        case.timeout_seconds,
    );
    fixture.assign_implement_stage_to_local_process();

    let cancel_thread = case.cancel_after.map(|delay| {
        let cancel_project = fixture.project.clone();
        let cancel_schemas = fixture.schemas.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let store =
                StateStore::open(cancel_project, cancel_schemas).expect("open cancel store");
            let mut state = store.load_state("J-0001").expect("load state");
            state["state"] = json!("CANCELLED");
            state["next_action"] = json!("stop");
            store
                .save_state("J-0001", &state)
                .expect("save cancelled state");
        })
    });

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .unwrap_or_else(|error| panic!("{} execute local process: {}", case.id, error));
    if let Some(cancel_thread) = cancel_thread {
        cancel_thread.join().expect("cancel thread");
    }

    assert_eq!(
        outcome.request().provider_instance_id(),
        "local-default",
        "{} provider instance",
        case.id
    );
    assert_eq!(
        outcome.provider_execution().result().status(),
        case.expected_status,
        "{} provider status",
        case.id
    );
    assert_eq!(
        outcome.attempt()["status"],
        case.expected_status,
        "{} execution attempt status",
        case.id
    );
    assert_eq!(
        outcome.state()["state"],
        case.expected_state,
        "{} run state",
        case.id
    );
    assert_local_process_output_contract(&fixture, &outcome, &case);
}

pub(crate) fn local_process_sleep_args() -> Vec<String> {
    vec![
        "--exact".to_string(),
        "tests::execution_sleep_helper".to_string(),
        "--nocapture".to_string(),
    ]
}

pub(crate) fn local_process_forbidden_evidence_args() -> Vec<String> {
    vec![
        "--exact".to_string(),
        "tests::execution_forbidden_evidence_helper".to_string(),
        "--nocapture".to_string(),
    ]
}
