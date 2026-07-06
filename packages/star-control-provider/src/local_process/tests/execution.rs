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
    fs::remove_dir_all(project).ok();
}
