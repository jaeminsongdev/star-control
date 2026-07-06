use crate::test_support::Fixture;
use crate::ExecutionEngine;

#[test]
fn local_process_provider_executes_by_manifest_kind() {
    let mut fixture = Fixture::new();
    fixture.use_local_process_registry(vec!["--help".to_string()], Vec::new(), 10);
    fixture.assign_implement_stage_to_local_process();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute local process stage");

    assert_eq!(outcome.request().provider_instance_id(), "local-default");
    assert_eq!(outcome.provider_execution().result().status(), "success");
    assert_eq!(outcome.state()["state"], "IMPLEMENTED");
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
        "provider-output/local-default/stdout.txt"
    );
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/local-default/request.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
}
