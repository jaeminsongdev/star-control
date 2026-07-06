use super::helpers::{assert_success, cleanup_project, config, json_output, path_arg};
use crate::run_cli;
use crate::test_support::{temp_project, write_local_process_instance};
use serde_json::Value;
use std::fs;

pub(super) fn run_with_local_process_provider_instance_executes_process() {
    let project = temp_project();
    let provider_instance = write_local_process_instance(&project, vec!["--help".to_string()]);
    let run = run_cli(
        [
            "run",
            "--project",
            path_arg(&project),
            "--request",
            "runtime code 구현",
            "--provider",
            "local-default",
            "--provider-instance",
            provider_instance.to_str().expect("provider instance path"),
            "--json",
        ],
        &config(),
    );

    assert_success(&run);
    let run_json = json_output(&run, "run json");
    assert_eq!(run_json["command"], "run");
    assert_eq!(run_json["status"], "success");
    assert_eq!(run_json["data"]["state"], "IMPLEMENTED");
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
        .is_file());
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .exists());

    let route: Value = serde_json::from_str(
        &fs::read_to_string(project.join(".ai-runs/J-0001/route.json")).expect("route"),
    )
    .expect("route json");
    assert_eq!(
        route["assignments"]["implement"]["provider"],
        "local-default"
    );
    let workspec: Value = serde_json::from_str(
        &fs::read_to_string(project.join(".ai-runs/J-0001/workspecs/implement.json"))
            .expect("workspec"),
    )
    .expect("workspec json");
    assert_eq!(workspec["provider_instance"], "local-default");

    cleanup_project(project);
}
