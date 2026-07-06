use super::*;
use crate::constants::CONTROL_TRANSPORT;

#[test]
fn browser_shell_action_panel_exposes_control_actions_without_network_runtime() {
    let project = temp_project("browser-actions");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    let browser = browser_with_store(store);

    let panel = browser
        .action_panel("local", "J-0001")
        .expect("action panel");
    assert_eq!(panel["view"], "browser_control_shell");
    assert_eq!(panel["render_target"], "browser");
    assert_eq!(panel["runtime"], "library_model");
    assert_eq!(panel["transport"], CONTROL_TRANSPORT);
    assert_eq!(panel["mutations_enabled"], true);
    assert_eq!(panel["network_server_enabled"], false);
    assert_eq!(panel["package_manager_required"], false);
    browser
        .validate_job_view(&panel["job"])
        .expect("schema-valid job view");

    let actions = panel["actions"].as_array().expect("actions");
    let approve = actions
        .iter()
        .find(|action| action["id"] == "approve")
        .expect("approve action");
    let cancel = actions
        .iter()
        .find(|action| action["id"] == "cancel")
        .expect("cancel action");
    let resume = actions
        .iter()
        .find(|action| action["id"] == "resume")
        .expect("resume action");
    assert_eq!(approve["enabled"], true);
    assert_eq!(approve["endpoint"], "/projects/local/jobs/J-0001/approve");
    assert_eq!(cancel["enabled"], true);
    assert_eq!(resume["enabled"], false);
    assert_eq!(
        resume["disabled_reason"],
        "resume requires an approved approval response"
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn browser_shell_approve_then_resume_uses_api_control_service() {
    let project = temp_project("browser-approve-resume");
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate", "approve");
    write_approval_request(&store, "validate");
    let browser = browser_with_store(store.clone());

    let approve = browser
        .approve(
            "local",
            "J-0001",
            "approved",
            "reviewed in browser shell",
            vec!["keep schema stable".to_string()],
        )
        .expect("approve result");
    assert_eq!(approve["view"], "browser_control_result");
    assert_eq!(approve["command"], "approve");
    assert_eq!(approve["succeeded"], true);
    assert_eq!(approve["api_response"]["data"]["state"], "WAITING_APPROVAL");
    assert_eq!(
        store.load_state("J-0001").expect("state after approve")["next_action"],
        "resume"
    );
    assert!(project
        .join(".ai-runs/J-0001/approvals/approval-response.json")
        .is_file());

    let panel = browser
        .action_panel("local", "J-0001")
        .expect("resume action panel");
    let resume = panel["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|action| action["id"] == "resume")
        .expect("resume action")
        .clone();
    assert_eq!(resume["enabled"], true);

    let resume_result = browser.resume("local", "J-0001").expect("resume result");
    assert_eq!(resume_result["command"], "resume");
    assert_eq!(resume_result["succeeded"], true);
    assert_eq!(resume_result["api_response"]["data"]["state"], "VALIDATED");
    assert_eq!(
        store.load_state("J-0001").expect("state after resume")["state"],
        "VALIDATED"
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn browser_shell_surfaces_terminal_cancel_failure_as_result_view() {
    let project = temp_project("browser-cancel-terminal");
    let store = open_store(&project);
    create_job(&store, "DONE", "report", "none");
    let browser = browser_with_store(store);

    let panel = browser
        .action_panel("local", "J-0001")
        .expect("action panel");
    let cancel = panel["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|action| action["id"] == "cancel")
        .expect("cancel action")
        .clone();
    assert_eq!(cancel["enabled"], false);
    assert_eq!(
        cancel["disabled_reason"],
        "terminal job cannot be cancelled"
    );

    let result = browser.cancel("local", "J-0001").expect("cancel result");
    assert_eq!(result["view"], "browser_control_result");
    assert_eq!(result["command"], "cancel");
    assert_eq!(result["succeeded"], false);
    assert_eq!(result["status"], "failed");
    assert_eq!(
        result["api_response"]["error"]["code"],
        "invalid_control_state"
    );

    fs::remove_dir_all(project).ok();
}
