use super::*;
use std::fs;

#[test]
fn local_process_executes_allowlisted_command_and_captures_output() {
    let executable = current_test_executable();
    let (execution, project) = execute_with_command(
        &executable,
        vec!["--help".to_string()],
        vec![executable.clone()],
        Vec::new(),
        10,
    )
    .expect("execute local process");

    assert_eq!(execution.result().status(), "success");
    assert_eq!(
        execution.result().value()["stdout_path"],
        "provider-output/local-default/stdout.txt"
    );
    assert_eq!(
        execution.result().value()["stderr_path"],
        "provider-output/local-default/stderr.txt"
    );
    assert!(execution.stderr_ref().is_some());

    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/request.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
    let cost_metric: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            project.join(".ai-runs/J-0001/provider-output/local-default/cost-metric.json"),
        )
        .expect("read local process cost metric"),
    )
    .expect("parse local process cost metric");
    assert_eq!(cost_metric["estimated_cost"], 0);
    assert_eq!(cost_metric["currency"], "USD");
    assert!(cost_metric["wall_time_ms"].as_u64().is_some());
    fs::remove_dir_all(project).ok();
}

#[test]
fn local_process_redacts_stdout_and_writes_redaction_report() {
    let executable = current_test_executable();
    let _guard = EnvVarGuard::set("STAR_CONTROL_LOCAL_PROCESS_SECRET_HELPER", "1");
    let (execution, project) = execute_with_command(
        &executable,
        vec![
            "--exact".to_string(),
            "local_process::tests::local_process_secret_output_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec![executable.clone()],
        vec!["STAR_CONTROL_LOCAL_PROCESS_SECRET_HELPER".to_string()],
        10,
    )
    .expect("execute local process");

    assert_eq!(execution.result().status(), "success");
    let stdout_text = fs::read_to_string(
        project.join(".ai-runs/J-0001/provider-output/local-default/stdout.txt"),
    )
    .expect("read stdout");
    assert_eq!(stdout_text, "[REDACTED]");
    let report_path =
        project.join(".ai-runs/J-0001/audit/provider-redaction-local-default-stdout-txt.json");
    assert!(report_path.is_file());
    let report_text = fs::read_to_string(report_path).expect("read redaction report");
    assert!(!report_text.contains("sk-test-secret"));
    assert!(execution.result().value()["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .any(|artifact| artifact == "audit/provider-redaction-local-default-stdout-txt.json"));

    fs::remove_dir_all(project).ok();
}
