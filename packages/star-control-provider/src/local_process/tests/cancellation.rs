use super::*;
use std::fs;
use std::thread;
use std::time::Duration;

#[test]
fn local_process_timeout_writes_timeout_result() {
    let executable = current_test_executable();
    let _env = EnvVarGuard::set("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER", "1");
    let (execution, project) = execute_with_command(
        &executable,
        vec![
            "--exact".to_string(),
            "local_process::tests::local_process_sleep_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec![executable.clone()],
        vec!["STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER".to_string()],
        1,
    )
    .expect("execute timeout helper");

    assert_eq!(execution.result().status(), "timeout");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "local_process_timeout"
    );

    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
        .is_file());
    fs::remove_dir_all(project).ok();
}

#[test]
fn local_process_cancelled_before_start_does_not_launch_command() {
    let executable = "missing-local-process-runner-for-cancel-test";
    let (execution, project) = execute_with_command_after_setup(
        executable,
        Vec::new(),
        vec![executable.to_string()],
        Vec::new(),
        10,
        |store, _project| {
            store
                .save_state("J-0001", &run_state("CANCELLED"))
                .expect("save cancelled state");
        },
    )
    .expect("execute pre-cancelled local process");

    assert_eq!(execution.result().status(), "cancelled");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "local_process_cancelled"
    );
    assert_eq!(execution.result().value()["error"]["phase"], "before_start");
    fs::remove_dir_all(project).ok();
}

#[test]
fn local_process_running_cancel_writes_cancelled_result() {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement local process feature", "codex", vec![])
        .expect("create job");
    store
        .save_state("J-0001", &run_state("IMPLEMENTING"))
        .expect("save running state");
    let executable = current_test_executable();
    let registry = registry_with_instance(
        &executable,
        vec![
            "--exact".to_string(),
            "local_process::tests::local_process_sleep_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec![executable.clone()],
        vec!["STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER".to_string()],
        10,
    )
    .expect("registry");
    let request = ExecutionRequest::from_value(request_value(), "request.json", schema_root())
        .expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    let _env = EnvVarGuard::set("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER", "1");
    let cancel_project = project.clone();
    let cancel_schemas = schema_root();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let store = StateStore::open(cancel_project, cancel_schemas).expect("open cancel store");
        store
            .save_state("J-0001", &run_state("CANCELLED"))
            .expect("save cancelled state");
    });

    let execution = LocalProcessProviderAdapter
        .execute(&request, &context)
        .expect("execute running cancel");
    cancel_thread.join().expect("cancel thread");

    assert_eq!(execution.result().status(), "cancelled");
    assert_eq!(execution.result().value()["error"]["phase"], "running");
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
    fs::remove_dir_all(project).ok();
}
