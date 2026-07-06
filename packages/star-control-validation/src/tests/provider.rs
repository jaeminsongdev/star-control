use super::*;

#[test]
fn missing_provider_response_is_an_error() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();

    let error = fixture
        .engine()
        .ensure_provider_response("J-0001", "fake-default")
        .unwrap_err();

    assert!(matches!(
        error,
        ValidationEngineError::ProviderOutputMissing { .. }
    ));
}
