use super::LocalProcessConformanceCase;
use crate::test_support::Fixture;
use crate::ExecutionOutcome;
use serde_json::{json, Value};

pub(super) fn assert_local_process_output_contract(
    fixture: &Fixture,
    outcome: &ExecutionOutcome,
    case: &LocalProcessConformanceCase,
) {
    let expected_paths = [
        "provider-output/local-default/response.json",
        "provider-output/local-default/stdout.txt",
        "provider-output/local-default/stderr.txt",
        "provider-output/local-default/cost-metric.json",
    ];
    assert!(
        fixture
            .project
            .join(".ai-runs/J-0001/provider-output/local-default/request.json")
            .is_file(),
        "{} missing request artifact",
        case.id
    );
    for relative_path in expected_paths {
        assert!(
            fixture
                .project
                .join(".ai-runs/J-0001")
                .join(relative_path)
                .is_file(),
            "{} missing artifact {}",
            case.id,
            relative_path
        );
    }

    let result = outcome.provider_execution().result().value();
    let artifacts = result["artifacts"]
        .as_array()
        .expect("provider result artifacts");
    assert!(
        artifacts.iter().all(|path| path
            .as_str()
            .map(|path| path.starts_with("provider-output/local-default/"))
            .unwrap_or(false)),
        "{} artifacts stay inside provider output directory",
        case.id
    );
    assert_eq!(
        result["artifacts"],
        json!(expected_paths),
        "{} provider result artifacts",
        case.id
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_request"]["path"],
        "provider-output/local-default/request.json",
        "{} request artifact ref",
        case.id
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_response"]["path"],
        "provider-output/local-default/response.json",
        "{} response artifact ref",
        case.id
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
        "provider-output/local-default/stdout.txt",
        "{} stdout artifact ref",
        case.id
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_stderr"]["path"],
        "provider-output/local-default/stderr.txt",
        "{} stderr artifact ref",
        case.id
    );
    assert_eq!(
        result["metrics"]["estimated_cost"],
        json!(0),
        "{} cost metric estimate",
        case.id
    );
    assert_eq!(
        result["metrics"]["currency"], "USD",
        "{} cost metric currency",
        case.id
    );
    assert!(
        result["metrics"]["wall_time_ms"].as_u64().is_some(),
        "{} cost metric wall time",
        case.id
    );

    match case.expected_error_kind {
        Some(kind) => assert_eq!(
            result["error"]["kind"], kind,
            "{} provider error kind",
            case.id
        ),
        None => assert_eq!(result["error"], Value::Null, "{} provider error", case.id),
    }
    if let Some(action) = case.expected_error_action {
        assert_eq!(
            result["error"]["action"], action,
            "{} provider error action",
            case.id
        );
    }

    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(
        events.iter().any(|event| {
            event["type"] == "PROVIDER_FINISHED"
                && event["details"]["status"] == case.expected_status
        }),
        "{} provider finished event",
        case.id
    );
}
