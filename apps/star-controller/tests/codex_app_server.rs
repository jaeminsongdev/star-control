#![cfg(windows)]

use std::{path::PathBuf, time::Duration};

use chrono::{Duration as ChronoDuration, Utc};
use star_adapter_codex::app_server::{
    CodexAppServerError, CodexAppServerProcess, generate_protocol_schema_bundle,
    inspect_protocol_schema_bundle, probe_codex_version,
};
use star_contracts::{
    ArtifactId, Sha256Hash,
    evidence::{ArtifactKind, ArtifactRef, ProducerRef, RedactionStatus, RetentionClass},
    routing::ReasoningEffortV1,
};

fn fake_codex() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_star-fake-exe"))
}

fn schema_directory(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "star-codex-schema-{label}-{}",
        star_contracts::RequestId::new()
    ))
}

#[test]
fn codex_app_server_real_process_positive_capability_probe() {
    let executable = fake_codex();
    let schema_root = schema_directory("positive");
    generate_protocol_schema_bundle(&executable, &schema_root).unwrap();
    let protocol = inspect_protocol_schema_bundle(&schema_root).unwrap();
    assert!(protocol.methods.contains("model/list"));
    assert!(protocol.methods.contains("turn/start"));

    let version = probe_codex_version(&executable).unwrap();
    let captured_at = Utc::now();
    let mut process = CodexAppServerProcess::spawn(&executable).unwrap();
    let evidence = process
        .probe_capabilities(version, protocol, captured_at, Duration::from_secs(5))
        .unwrap();
    let bytes = evidence.canonical_bytes().unwrap();
    let artifact = ArtifactRef {
        artifact_id: ArtifactId::new(),
        kind: ArtifactKind::Log,
        project_id: None,
        relative_path: "codex/capability-probe.json".to_owned(),
        media_type: "application/json".to_owned(),
        size_bytes: bytes.len() as u64,
        sha256: Sha256Hash::digest(&bytes),
        created_at: captured_at,
        producer: ProducerRef {
            component: "star-controller-test".to_owned(),
            product_version: "0.1.0".to_owned(),
            build_id: "fake-codex".to_owned(),
            platform: "windows-x64".to_owned(),
        },
        redaction_status: RedactionStatus::NotNeeded,
        retention_class: RetentionClass::Evidence,
        source_artifact_ref: None,
    };
    let snapshot = evidence
        .into_snapshot(captured_at + ChronoDuration::minutes(5), artifact, false)
        .unwrap();
    snapshot.verify().unwrap();
    assert_eq!(snapshot.models[0].model_id, "gpt-5.6-terra");
}

#[test]
fn codex_app_server_real_process_negative_rejects_existing_schema_target() {
    let schema_root = schema_directory("existing");
    std::fs::create_dir_all(&schema_root).unwrap();
    assert!(matches!(
        generate_protocol_schema_bundle(&fake_codex(), &schema_root),
        Err(CodexAppServerError::Path)
    ));
}

#[test]
fn codex_app_server_real_process_failure_rejects_missing_executable() {
    let missing = schema_directory("missing").join("codex.exe");
    assert!(matches!(
        CodexAppServerProcess::spawn(&missing),
        Err(CodexAppServerError::Path)
    ));
}

#[test]
fn codex_app_server_real_process_recovery_resumes_forks_and_interrupts() {
    let mut process = CodexAppServerProcess::spawn(&fake_codex()).unwrap();
    process.initialize("test", Duration::from_secs(5)).unwrap();
    let thread = process
        .thread_start("gpt-5.6-terra", None, Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        process
            .thread_resume(&thread, Duration::from_secs(5))
            .unwrap(),
        thread
    );
    let fork = process
        .thread_fork(&thread, Duration::from_secs(5))
        .unwrap();
    let turn = process
        .turn_start(
            &fork,
            "bounded fake instruction",
            Some("gpt-5.6-terra"),
            Some(ReasoningEffortV1::High),
            Duration::from_secs(5),
        )
        .unwrap();
    process
        .turn_interrupt(&fork, &turn, Duration::from_secs(5))
        .unwrap();
    let started = process.next_notification(Duration::from_secs(5)).unwrap();
    let completed = process.next_notification(Duration::from_secs(5)).unwrap();
    assert_eq!(started["method"], "turn/started");
    assert_eq!(completed["method"], "turn/completed");
}
