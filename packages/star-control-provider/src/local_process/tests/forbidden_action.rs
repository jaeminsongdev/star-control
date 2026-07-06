use super::*;
use std::fs;

#[test]
fn local_process_forbidden_action_evidence_blocks_result() {
    let executable = current_test_executable();
    let _env = EnvVarGuard::set("STAR_CONTROL_LOCAL_PROCESS_FORBIDDEN_EVIDENCE_HELPER", "1");
    let (execution, project) = execute_with_command(
        &executable,
        vec![
            "--exact".to_string(),
            "local_process::tests::local_process_forbidden_evidence_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec![executable.clone()],
        vec!["STAR_CONTROL_LOCAL_PROCESS_FORBIDDEN_EVIDENCE_HELPER".to_string()],
        10,
    )
    .expect("execute forbidden evidence local process");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "local_process_forbidden_action"
    );
    assert_eq!(
        execution.result().value()["error"]["action"],
        "dependency_install"
    );
    assert_eq!(execution.result().value()["error"]["source"], STDOUT_FILE);
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
    fs::remove_dir_all(project).ok();
}
