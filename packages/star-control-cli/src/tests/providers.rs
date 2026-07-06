use crate::{run_cli, CliConfig};
use serde_json::Value;

use crate::test_support::repo_root;

#[test]
fn providers_list_and_show_are_schema_valid_and_read_only() {
    let config = CliConfig::new(repo_root());

    let list = run_cli(["providers", "list", "--json"], &config);
    assert_eq!(list.exit_code, 0, "{}", list.stderr);
    let list_json: Value = serde_json::from_str(&list.stdout).expect("providers list json");
    assert_eq!(list_json["command"], "providers");
    assert_eq!(list_json["data"]["subcommand"], "list");
    assert_eq!(list_json["data"]["actions_enabled"], false);
    assert_eq!(list_json["data"]["healthcheck_enabled"], false);
    assert_eq!(
        list_json["artifacts"].as_array().expect("artifacts").len(),
        0
    );
    let providers = list_json["data"]["providers"]
        .as_array()
        .expect("providers array");
    assert!(providers.len() >= 20);
    let fake = providers
        .iter()
        .find(|provider| provider["id"] == "provider.fake")
        .expect("provider.fake listed");
    assert_eq!(fake["kind"], "fake_provider");
    assert_eq!(
        fake["manifest_path"],
        "builtin-providers/test/fake-provider/provider.yaml"
    );

    let show = run_cli(["providers", "show", "provider.fake", "--json"], &config);
    assert_eq!(show.exit_code, 0, "{}", show.stderr);
    let show_json: Value = serde_json::from_str(&show.stdout).expect("providers show json");
    assert_eq!(show_json["command"], "providers");
    assert_eq!(show_json["data"]["subcommand"], "show");
    assert_eq!(show_json["data"]["provider"]["id"], "provider.fake");
    assert_eq!(
        show_json["data"]["capability_profile"]["provider"],
        "provider.fake"
    );
    assert_eq!(show_json["data"]["actions_enabled"], false);
    assert_eq!(show_json["data"]["healthcheck_enabled"], false);

    let show_with_option = run_cli(
        ["providers", "show", "--provider", "provider.fake", "--json"],
        &config,
    );
    assert_eq!(show_with_option.exit_code, 0, "{}", show_with_option.stderr);
    let show_with_option_json: Value =
        serde_json::from_str(&show_with_option.stdout).expect("providers show option json");
    assert_eq!(
        show_with_option_json["data"]["provider"]["id"],
        "provider.fake"
    );
}

#[test]
fn providers_rejects_mutating_or_reserved_options() {
    let config = CliConfig::new(repo_root());

    let missing = run_cli(["providers", "show", "--json"], &config);
    assert_eq!(missing.exit_code, 2);
    let missing_json: Value =
        serde_json::from_str(&missing.stdout).expect("missing provider error");
    assert_eq!(missing_json["error"]["code"], "InvalidInput");

    let reserved = run_cli(["providers", "healthcheck", "--json"], &config);
    assert_eq!(reserved.exit_code, 2);
    let reserved_json: Value =
        serde_json::from_str(&reserved.stdout).expect("reserved provider error");
    assert_eq!(reserved_json["error"]["code"], "InvalidInput");
    assert!(reserved_json["error"]["message"]
        .as_str()
        .expect("message")
        .contains("reserved"));

    let invalid_option = run_cli(
        [
            "providers",
            "list",
            "--project",
            "target/not-used",
            "--json",
        ],
        &config,
    );
    assert_eq!(invalid_option.exit_code, 2);
}
