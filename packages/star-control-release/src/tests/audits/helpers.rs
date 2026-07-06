use crate::test_support::schema_root;
use crate::ReleaseReadinessWriter;
use serde_json::{json, Value};

pub(super) fn release_writer() -> ReleaseReadinessWriter {
    ReleaseReadinessWriter::new(schema_root())
}

pub(super) fn assert_reserved_readiness(
    writer: &ReleaseReadinessWriter,
    readiness: &Value,
    context: &str,
    required_check_count: usize,
    first_check_name: &str,
    reserved_blocker: &str,
) {
    writer.validate_readiness(readiness).expect(context);
    assert_eq!(readiness["status"], "reserved");
    assert_eq!(
        readiness["checks"].as_array().expect("checks").len(),
        required_check_count
    );
    assert_eq!(readiness["checks"][0]["name"], first_check_name);
    assert_eq!(readiness["checks"][0]["status"], "pass");
    assert!(readiness["blockers"]
        .as_array()
        .expect("blockers")
        .contains(&json!(reserved_blocker)));
}

pub(super) fn assert_not_ready_blockers(
    writer: &ReleaseReadinessWriter,
    readiness: &Value,
    context: &str,
    expected_blockers: &[&str],
) {
    writer.validate_readiness(readiness).expect(context);
    assert_eq!(readiness["status"], "not_ready");
    let blockers = readiness["blockers"].as_array().expect("blockers");
    for blocker in expected_blockers {
        assert!(blockers.contains(&json!(blocker)));
    }
}
