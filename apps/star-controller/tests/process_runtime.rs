use std::{ffi::OsString, path::PathBuf, time::Duration};

use star_contracts::{
    Sha256Hash,
    ids::{OperationId, RequestId},
    runtime::{ExternalToolContext, ExternalToolRequest},
};
#[cfg(windows)]
use star_controller::process_runtime::appcontainer_profile_folder;
use star_controller::process_runtime::{
    DirectExeSpec, JsonStdioExecutionOptions, OperationJob, RuntimeCancellation, RuntimeError,
    execute_direct_exe, execute_direct_exe_cancellable, execute_direct_exe_cancellable_with_grace,
    execute_star_json_probe, execute_star_json_stdio, execute_star_json_stdio_cancellable,
    execute_star_json_stdio_cancellable_with_cancel_mode, execute_trusted_internal_exe_cancellable,
    execute_trusted_internal_powershell_cancellable, lease_executable,
    validate_star_json_stdio_output,
};

#[tokio::test]
// matrix: MCP-P007
async fn controller_job_close_terminates_an_assigned_child_before_its_normal_exit() {
    let executable = fake_exe();
    let mut child = tokio::process::Command::new(&executable)
        .arg("sleep")
        .current_dir(executable.parent().unwrap())
        .spawn()
        .expect("fake child starts");
    let job = OperationJob::new().expect("operation job creates");
    job.assign(&child).expect("child joins operation job");
    let started = std::time::Instant::now();
    drop(job);
    let _status = tokio::time::timeout(Duration::from_secs(1), child.wait())
        .await
        .expect("job close terminates child")
        .expect("child wait succeeds");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the fake sleep normally lasts two seconds"
    );
}

#[tokio::test]
// matrix: MCP-P026
async fn operation_job_uses_nested_assignment_or_fails_without_breakaway() {
    let launcher = include_str!("../src/process_runtime/win32_launcher.rs");
    assert!(!launcher.contains("CREATE_BREAKAWAY_FROM_JOB"));
    assert!(launcher.contains("if job.assign_handle(process.hProcess).is_err()"));
    assert!(launcher.contains("TerminateProcess(process.hProcess"));

    let executable = fake_exe();
    let mut child = tokio::process::Command::new(&executable)
        .arg("sleep")
        .current_dir(executable.parent().unwrap())
        .spawn()
        .expect("fake child starts");
    let job = OperationJob::new().expect("inner operation job creates");
    match job.assign(&child) {
        Ok(()) => {
            drop(job);
            tokio::time::timeout(Duration::from_secs(1), child.wait())
                .await
                .expect("nested job close terminates the child")
                .expect("child wait succeeds");
        }
        Err(RuntimeError::Start) => {
            // The production launcher takes this exact branch before resume,
            // terminates the suspended process, and never retries unjobbed.
            child.kill().await.expect("test child cleanup");
            let _ = child.wait().await;
        }
        Err(other) => panic!("unexpected nested job result: {other}"),
    }
}

fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_star-fake-exe"))
}

fn spec(mode: &str) -> DirectExeSpec {
    let executable = fake_exe();
    DirectExeSpec {
        working_directory: executable.parent().expect("fake directory").to_path_buf(),
        executable,
        argv: vec![OsString::from(mode)],
        environment: vec![],
        stdin: None,
        timeout: Duration::from_secs(5),
        max_stdout_bytes: 1024 * 1024,
        max_stderr_bytes: 1024 * 1024,
        max_memory_bytes: None,
        max_processes: 16,
        appcontainer_profile: None,
    }
}

fn request() -> ExternalToolRequest {
    ExternalToolRequest {
        frame: "request".to_owned(),
        protocol_version: 1,
        schema_id: "star.external-tool-request".to_owned(),
        schema_version: 1,
        request_id: RequestId::new(),
        tool_id: "user.fake.echo".to_owned(),
        descriptor_hash: Sha256Hash::digest(b"fixture"),
        arguments: serde_json::json!({"value":"hello"}),
        context: ExternalToolContext {
            operation_id: OperationId::new(),
            project_id: None,
            goal_id: None,
            run_id: None,
            stage_id: None,
            deadline_at: "2026-07-11T00:00:00.000Z".to_owned(),
            artifact_directory: "artifacts".to_owned(),
            temp_directory: "temp".to_owned(),
        },
    }
}

#[tokio::test]
// matrix: MCP-P001
async fn fake_argv_exe_receives_typed_args_without_a_shell() {
    let mut spec = spec("argv");
    spec.argv.push(OsString::from("hello world"));
    let outcome = execute_direct_exe(&spec).await.expect("fake argv succeeds");
    assert_eq!(
        String::from_utf8(outcome.stdout.captured).unwrap(),
        "argv:hello world\n"
    );
}

#[test]
// matrix: MCP-P008
fn executable_lease_blocks_same_path_write_or_replacement_before_launch() {
    let executable = fake_exe();
    let lease = lease_executable(&executable).expect("fake executable leases");
    let identity = lease.identity().expect("leased identity is readable");
    assert!(identity.stable_file_id);
    assert_eq!(identity.volume_serial.len(), 16);
    assert_eq!(identity.file_id.len(), 32);
    assert_eq!(identity.size, std::fs::metadata(&executable).unwrap().len());
    chrono::DateTime::parse_from_rfc3339(&identity.last_write).unwrap();
    assert!(
        lease
            .final_path()
            .unwrap()
            .to_string_lossy()
            .starts_with(r"\\?\Volume{"),
        "identity evidence must hash a normalized volume-GUID final path"
    );
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .is_err(),
        "a verified executable lease must deny a writer until process creation completes"
    );
}

#[tokio::test]
// matrix: MCP-P009
async fn running_executable_keeps_its_image_identity_against_same_path_replacement() {
    let executable = fake_exe();
    let mut child = tokio::process::Command::new(&executable)
        .arg("sleep")
        .current_dir(executable.parent().unwrap())
        .spawn()
        .expect("fake child starts");
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .is_err(),
        "a running image must not become writable through its original path"
    );
    child.kill().await.expect("test child terminates");
    let _ = child.wait().await;
}

#[tokio::test]
// matrix: MCP-P009
async fn running_image_keeps_its_lease_and_the_next_call_observes_new_bytes() {
    use std::io::Write as _;

    let directory = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("star-running-identity-{}", star_ipc::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let live = directory.join("live.exe");
    std::fs::copy(fake_exe(), &live).unwrap();
    let original_hash = Sha256Hash::digest(&std::fs::read(&live).unwrap());
    let marker = directory.join("started.marker");
    let lease = lease_executable(&live).expect("running image leases");
    let mut running = spec("marker-sleep");
    running.executable = live.clone();
    running.working_directory = directory.clone();
    running.argv.push(OsString::from(marker.as_os_str()));
    let invocation = tokio::spawn(async move {
        let _lease = lease;
        execute_direct_exe(&running).await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("original process starts");
    assert!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(&live)
            .is_err(),
        "same-path image mutation is blocked for the running invocation"
    );
    let outcome = invocation.await.unwrap().expect("original image completes");
    assert_eq!(outcome.stdout.captured, b"original-image\n");
    assert_eq!(
        Sha256Hash::digest(&std::fs::read(&live).unwrap()),
        original_hash
    );

    std::fs::OpenOptions::new()
        .append(true)
        .open(&live)
        .unwrap()
        .write_all(b"new-identity")
        .unwrap();
    let next_hash = Sha256Hash::digest(&std::fs::read(&live).unwrap());
    assert_ne!(next_hash, original_hash);
    let mut next = spec("argv");
    next.executable = live;
    next.working_directory = directory;
    next.argv.push(OsString::from("new-call"));
    assert_eq!(
        execute_direct_exe(&next).await.unwrap().stdout.captured,
        b"argv:new-call\n"
    );
}

#[tokio::test]
// matrix: MCP-P027
async fn child_receives_minimal_environment_and_only_declared_values() {
    let mut probe = spec("env-probe");
    probe.environment = vec![(OsString::from("STAR_TEST_ALLOWED"), OsString::from("yes"))];
    let outcome = execute_direct_exe(&probe)
        .await
        .expect("environment probe runs");
    let output = String::from_utf8(outcome.stdout.captured).unwrap();
    assert!(output.contains("PATH=<missing>"));
    assert!(output.contains("STAR_TEST_ALLOWED=yes"));
    assert!(output.contains("STAR_TEST_UNDECLARED=<missing>"));

    let mut oversized = spec("env-probe");
    oversized.environment = vec![(
        OsString::from("STAR_TEST_LARGE"),
        OsString::from("x".repeat(32_768)),
    )];
    assert!(matches!(
        execute_direct_exe(&oversized).await,
        Err(RuntimeError::ProtocolInvalid)
    ));

    let mut invalid = spec("env-probe");
    invalid.environment = vec![(OsString::from("1INVALID"), OsString::from("value"))];
    assert!(matches!(
        execute_direct_exe(&invalid).await,
        Err(RuntimeError::ProtocolInvalid)
    ));

    use std::os::windows::ffi::OsStringExt;
    let mut invalid_unicode = spec("env-probe");
    invalid_unicode.environment = vec![(
        OsString::from("STAR_TEST_INVALID_UTF16"),
        OsString::from_wide(&[0xD800]),
    )];
    assert!(matches!(
        execute_direct_exe(&invalid_unicode).await,
        Err(RuntimeError::ProtocolInvalid)
    ));
}

#[tokio::test]
async fn trusted_internal_child_inherits_path_without_opening_package_environment() {
    let probe = spec("env-probe");
    let outcome = execute_trusted_internal_exe_cancellable(&probe, None)
        .await
        .expect("trusted internal environment probe runs");
    let output = String::from_utf8(outcome.stdout.captured).unwrap();
    assert!(output.contains("PATH="));
    assert!(!output.contains("PATH=<missing>"));

    let mut invalid = spec("env-probe");
    invalid.environment = vec![(OsString::from("STAR_TEST_ALLOWED"), OsString::from("yes"))];
    assert!(matches!(
        execute_trusted_internal_exe_cancellable(&invalid, None).await,
        Err(RuntimeError::ProtocolInvalid)
    ));

    assert!(matches!(
        execute_trusted_internal_powershell_cancellable(&probe, None).await,
        Err(RuntimeError::ExecutableInvalid)
    ));
}

#[tokio::test]
// matrix: MCP-P021
async fn child_inherits_only_the_three_declared_stdio_handles() {
    use windows::{
        Win32::{
            Foundation::CloseHandle, Security::SECURITY_ATTRIBUTES, System::Threading::CreateEventW,
        },
        core::PCWSTR,
    };

    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    let sentinel =
        unsafe { CreateEventW(Some(&raw const attributes), true, false, PCWSTR::null()) }
            .expect("inheritable sentinel handle creates");
    let mut probe = spec("handle-probe");
    probe
        .argv
        .push(OsString::from((sentinel.0 as usize).to_string()));
    let outcome = execute_direct_exe(&probe)
        .await
        .expect("handle probe child runs");
    unsafe {
        let _ = CloseHandle(sentinel);
    }
    assert_eq!(outcome.stdout.captured, b"not-inherited\n");
}

#[tokio::test]
// matrix: MCP-S013 MCP-S014
async fn appcontainer_adapter_cannot_escape_to_user_files_or_loopback_network() {
    let directory = std::env::temp_dir().join(format!("star-appcontainer-{}", star_ipc::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let outside = directory.join("outside-secret.txt");
    std::fs::write(&outside, b"outside AppContainer broker scope").unwrap();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let identity = Sha256Hash::digest(format!("test-{}", star_ipc::nonce()).as_bytes()).to_string();
    let profile = format!(
        "StarControl.Tool.{}",
        &identity.trim_start_matches("sha256:")[..32]
    );
    // The adapter binary lives in its own AppContainer package directory;
    // the ungranted `outside` file deliberately remains outside that broker
    // scope.  This avoids accidentally granting the test build directory.
    let adapter_directory =
        appcontainer_profile_folder(&profile).expect("AppContainer package directory is available");
    let adapter = adapter_directory.join("star-fake-exe.exe");
    std::fs::copy(fake_exe(), &adapter).expect("adapter image copied into package directory");
    let mut probe = spec("appcontainer-probe");
    probe.executable = adapter;
    probe.working_directory = adapter_directory;
    probe.argv.push(OsString::from(outside.as_os_str()));
    probe.argv.push(OsString::from(port.to_string()));
    probe.appcontainer_profile = Some(profile);
    let outcome = execute_direct_exe(&probe)
        .await
        .expect("AppContainer adapter launches with no capabilities");
    let evidence: serde_json::Value = serde_json::from_slice(&outcome.stdout.captured).unwrap();
    assert_eq!(evidence["path_denied"], true);
    assert_eq!(evidence["network_denied"], true);
    listener.set_nonblocking(true).unwrap();
    assert!(
        listener.accept().is_err(),
        "loopback listener saw no connection"
    );
}

#[tokio::test]
// matrix: MCP-P002
async fn oversized_windows_command_line_is_rejected_before_process_creation() {
    let mut oversized = spec("argv");
    oversized.argv.push(OsString::from("x".repeat(32_767)));
    assert!(matches!(
        execute_direct_exe(&oversized).await,
        Err(RuntimeError::ProtocolInvalid)
    ));
}

#[tokio::test]
// matrix: MCP-P010
async fn fake_json_stdio_exe_returns_one_bound_result() {
    let request = request();
    let response = execute_star_json_stdio(&spec("json"), &request)
        .await
        .expect("fake JSON-STDIO succeeds");
    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.data, Some(serde_json::json!({"accepted":true})));
}

#[tokio::test]
// matrix: MCP-P011
async fn fake_json_stdio_probe_returns_strict_versions_and_capabilities() {
    let response = execute_star_json_probe(&spec("json-probe"), RequestId::new())
        .await
        .unwrap();
    assert_eq!(response.product_version, "1.2.3");
    assert_eq!(response.interface_version.as_deref(), Some("1.0.0"));
    assert_eq!(
        response.capabilities,
        ["progress", "stdin_cancel", "artifact_output"]
    );
}

#[tokio::test]
// matrix: MCP-P014
async fn json_stdio_accepts_monotonic_progress_before_one_final_result() {
    let request = request();
    let response = execute_star_json_stdio(&spec("json-progress"), &request)
        .await
        .expect("monotonic progress and final result succeed");
    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.summary, "fake progress result");
}

#[test]
// matrix: MCP-P015
fn json_stdio_rejects_empty_jsonl_records() {
    let request = request();
    let result = serde_json::json!({
        "frame":"result",
        "protocol_version":1,
        "schema_id":"star.external-tool-response",
        "schema_version":1,
        "request_id":request.request_id,
        "status":"ok",
        "summary":"result",
        "data":{"accepted":true},
        "diagnostics":[],
        "artifacts":[],
        "error":null
    });
    assert!(matches!(
        validate_star_json_stdio_output(&format!("\n{result}"), &request),
        Err(RuntimeError::ProtocolInvalid)
    ));
    assert!(matches!(
        validate_star_json_stdio_output(&format!("{result}\n\n"), &request),
        Err(RuntimeError::ProtocolInvalid)
    ));
}

#[tokio::test]
// matrix: MCP-P014 MCP-G016
async fn json_stdio_progress_observer_receives_a_frame_before_process_completion() {
    let request = request();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let observer = std::sync::Arc::new(
        move |progress: star_contracts::runtime::ExternalToolProgress| {
            sender.send(progress.sequence).is_ok()
        },
    );
    let task = tokio::spawn(async move {
        execute_star_json_stdio_cancellable_with_cancel_mode(
            &spec("json-progress"),
            &request,
            JsonStdioExecutionOptions {
                progress_observer: Some(observer),
                ..Default::default()
            },
        )
        .await
    });
    let first = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("progress arrives while the adapter is running")
        .expect("progress channel stays live");
    assert_eq!(first, 1);
    assert!(
        !task.is_finished(),
        "result must still be pending after progress"
    );
    let outcome = task.await.unwrap().unwrap();
    assert_eq!(outcome.progress.len(), 2);
}

#[tokio::test]
// matrix: MCP-P006
async fn timeout_terminates_a_job_bound_fake_process() {
    let mut slow = spec("sleep");
    slow.timeout = Duration::from_millis(100);
    assert!(matches!(
        execute_direct_exe(&slow).await,
        Err(RuntimeError::Timeout)
    ));
}

#[tokio::test]
// matrix: MCP-P003
async fn unused_stdin_is_closed_so_the_child_receives_eof() {
    let mut eof = spec("wait-eof");
    eof.timeout = Duration::from_secs(2);
    let outcome = execute_direct_exe(&eof).await.expect("child receives EOF");
    assert_eq!(outcome.stdout.captured, b"eof\n");
}

#[tokio::test]
// matrix: MCP-P017
async fn cancellation_terminates_the_job_bound_argv_process() {
    let cancellation = RuntimeCancellation::default();
    let signal = cancellation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(75)).await;
        signal.cancel();
    });
    let evidence = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observed = std::sync::Arc::clone(&evidence);
    let observer = std::sync::Arc::new(
        move |value: star_controller::process_runtime::ProcessEndEvidence| {
            *observed.lock().unwrap() = Some(value);
            true
        },
    );
    assert!(matches!(
        execute_direct_exe_cancellable_with_grace(
            &spec("sleep"),
            Some(cancellation),
            Duration::from_millis(25),
            None,
            Some(observer),
        )
        .await,
        Err(RuntimeError::Cancelled)
    ));
    let evidence = evidence.lock().unwrap().clone().unwrap();
    assert_eq!(evidence.termination, "cancelled");
    assert!(evidence.exit_code.is_some());
}

#[tokio::test]
// matrix: MCP-P016
async fn cancellation_terminates_the_json_stdio_adapter_process() {
    let cancellation = RuntimeCancellation::default();
    let signal = cancellation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(75)).await;
        signal.cancel();
    });
    assert!(matches!(
        execute_star_json_stdio_cancellable(&spec("json-sleep"), &request(), Some(cancellation))
            .await,
        Err(RuntimeError::Cancelled)
    ));
}

#[tokio::test]
// matrix: MCP-P005
async fn cancellation_terminates_the_entire_job_child_tree() {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS, WaitForSingleObject},
    };

    let directory = std::env::temp_dir().join(format!("star-runtime-tree-{}", star_ipc::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let pid_file = directory.join("child.pid");
    let mut tree = spec("tree");
    tree.working_directory = directory;
    tree.argv.push(OsString::from(pid_file.as_os_str()));
    let cancellation = RuntimeCancellation::default();
    let signal = cancellation.clone();
    let observed_pid_file = pid_file.clone();
    let pid_watcher = tokio::spawn(async move {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(text) = std::fs::read_to_string(&observed_pid_file)
                && let Ok(child_pid) = text.parse::<u32>()
            {
                signal.cancel();
                return Some(child_pid);
            }
            if std::time::Instant::now() >= deadline {
                signal.cancel();
                return None;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    assert!(matches!(
        execute_direct_exe_cancellable(&tree, Some(cancellation)).await,
        Err(RuntimeError::Cancelled)
    ));
    let child_pid = pid_watcher
        .await
        .expect("PID watcher joins")
        .expect("child PID is written before bounded cancellation");
    // A just-terminated child can already have no process object, in which
    // case OpenProcess itself is the proof that the Job did not leak it.
    if let Ok(child) = unsafe { OpenProcess(PROCESS_ACCESS_RIGHTS(0x0010_0000), false, child_pid) }
    {
        assert_eq!(unsafe { WaitForSingleObject(child, 1_000) }, WAIT_OBJECT_0);
        unsafe {
            let _ = CloseHandle(child);
        }
    }
}

#[tokio::test]
// matrix: MCP-P015
async fn json_stdio_garbage_is_not_accepted_as_a_tool_result() {
    assert!(matches!(
        execute_star_json_stdio(&spec("json-garbage"), &request()).await,
        Err(RuntimeError::ProtocolInvalid)
    ));
}

#[tokio::test]
// matrix: MCP-P030 MCP-P031
async fn json_stdio_rejects_duplicate_unknown_and_nonzero_result_frames() {
    for mode in [
        "json-duplicate",
        "json-unknown",
        "json-nonzero",
        "json-artifact-escape",
        "json-artifact-bad-hash",
    ] {
        assert!(matches!(
            execute_star_json_stdio(&spec(mode), &request()).await,
            Err(RuntimeError::ProtocolInvalid)
        ));
    }
}

#[tokio::test]
// matrix: MCP-P004 MCP-P018
async fn fake_output_flood_is_drained_then_rejected() {
    let mut flood = spec("flood-both");
    flood.max_stdout_bytes = 1024;
    flood.max_stderr_bytes = 1024;
    assert!(matches!(
        execute_direct_exe(&flood).await,
        Err(RuntimeError::OutputLimit)
    ));
}
