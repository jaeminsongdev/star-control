#![cfg(windows)]

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

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
#[ignore = "requires an approved signed live Codex executable and provider turn"]
fn codex_app_server_signed_live_provider_and_thread_lifecycle() {
    let executable = std::env::var_os("STAR_LIVE_CODEX_EXE")
        .map(PathBuf::from)
        .expect("STAR_LIVE_CODEX_EXE points to the approved signed Codex executable");
    assert!(executable.is_absolute());
    assert!(executable.is_file());

    let schema_root = schema_directory("signed-live");
    generate_protocol_schema_bundle(&executable, &schema_root).unwrap();
    let protocol = inspect_protocol_schema_bundle(&schema_root).unwrap();
    for method in [
        "initialize",
        "model/list",
        "thread/start",
        "thread/resume",
        "thread/delete",
    ] {
        assert!(
            protocol.methods.contains(method),
            "missing method: {method}"
        );
    }
    assert!(protocol.fields.contains("ephemeral"));

    let version = probe_codex_version(&executable).unwrap();
    let mut process = CodexAppServerProcess::spawn(&executable).unwrap();
    let evidence = process
        .probe_capabilities(version, protocol, Utc::now(), Duration::from_secs(20))
        .unwrap();
    let model = evidence
        .models
        .iter()
        .find(|model| model.is_default)
        .or_else(|| evidence.models.first())
        .expect("the live App Server advertises at least one model");
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("controller crate belongs to the workspace")
        .to_path_buf();
    let ephemeral_thread = process
        .thread_start_ephemeral(&model.model_id, Some(&workspace), Duration::from_secs(20))
        .unwrap();
    assert!(!ephemeral_thread.is_empty());

    let live_thread = process
        .thread_start_with_policy(
            &model.model_id,
            Some(&workspace),
            Some("never"),
            Some("read-only"),
            std::slice::from_ref(&workspace),
            Duration::from_secs(20),
        )
        .unwrap();
    let lifecycle = (|| -> Result<_, CodexAppServerError> {
        let turn_id = process.turn_start_with_policy(
            &live_thread,
            "Reply with exactly STAR_LIVE_OK. Do not call tools, read files, or modify anything.",
            Some(&model.model_id),
            Some(model.default_reasoning_effort),
            Some("never"),
            Some("read-only"),
            std::slice::from_ref(&workspace),
            &[],
            Duration::from_secs(20),
        )?;
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut saw_agent_message = false;
        let completion = loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(CodexAppServerError::Timeout)?;
            let notification = process.next_notification(remaining)?;
            if matches!(
                notification["method"].as_str(),
                Some("item/started" | "item/completed")
            ) {
                match notification["params"]["item"]["type"].as_str() {
                    Some("agentMessage") => saw_agent_message = true,
                    Some("userMessage" | "reasoning") => {}
                    _ => return Err(CodexAppServerError::Protocol),
                }
            }
            if notification["method"] == "turn/completed"
                && notification["params"]["threadId"] == live_thread
                && notification["params"]["turn"]["id"] == turn_id
            {
                break notification;
            }
        };
        if !saw_agent_message {
            return Err(CodexAppServerError::Protocol);
        }
        let resumed = process.thread_resume(&live_thread, Duration::from_secs(20))?;
        Ok((turn_id, completion, resumed))
    })();
    let cleanup = process.thread_delete(&live_thread, Duration::from_secs(20));
    let (turn_id, completion, resumed) = lifecycle.unwrap();
    cleanup.unwrap();
    assert_eq!(resumed, live_thread);
    assert_eq!(completion["params"]["turn"]["id"], turn_id);
    assert_eq!(completion["params"]["turn"]["status"], "completed");
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
    process
        .thread_delete(&thread, Duration::from_secs(5))
        .unwrap();
}
