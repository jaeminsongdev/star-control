use crate::constants::SCHEMA_VERSION;
use crate::support::timestamp_string;
use serde_json::{json, Value};

pub(super) fn check(
    name: impl Into<String>,
    status: impl Into<String>,
    evidence_paths: Vec<String>,
) -> Value {
    json!({
        "name": name.into(),
        "status": status.into(),
        "evidence_paths": evidence_paths
    })
}

pub(super) fn reserved_checks() -> Vec<Value> {
    vec![
        check("required-ci-passed", "reserved", Vec::new()),
        check("release-profile-passed", "reserved", Vec::new()),
        check("changelog-updated", "reserved", Vec::new()),
        check("version-consistent", "reserved", Vec::new()),
        check("artifact-signing-ready", "reserved", Vec::new()),
        check("rollback-plan-ready", "reserved", Vec::new()),
        check("package-publishing-approved", "reserved", Vec::new()),
    ]
}

pub(super) fn reserved_blockers() -> Vec<String> {
    vec!["release automation is not implemented yet".to_string()]
}

pub(super) fn readiness(
    release_id: impl Into<String>,
    target: impl Into<String>,
    version: impl Into<String>,
    status: impl Into<String>,
    checks: Vec<Value>,
    blockers: Vec<String>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "release_id": release_id.into(),
        "target": target.into(),
        "version": version.into(),
        "status": status.into(),
        "checks": checks,
        "blockers": blockers,
        "approvals": [],
        "generated_at": timestamp_string()
    })
}
