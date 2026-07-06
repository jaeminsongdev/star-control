use crate::registry_yaml::parse_star_control_yaml_subset;
use std::path::PathBuf;

#[test]
fn parses_star_control_yaml_subset() {
    let path = PathBuf::from("fixture.yaml");
    let value = parse_star_control_yaml_subset(
        &path,
        r#"
schema_version: 0.1.0
providers:
  - id: provider.fake
    manifest: builtin-providers/test/fake-provider/provider.yaml
    capabilities: builtin-providers/test/fake-provider/capabilities.yaml
capability_profile:
  can:
    run_shell: false
    return_json: partial
  routing_tags:
    - test
"#,
    )
    .expect("parse yaml subset");

    assert_eq!(value["schema_version"], "0.1.0");
    assert_eq!(value["providers"][0]["id"], "provider.fake");
    assert_eq!(value["capability_profile"]["can"]["run_shell"], false);
    assert_eq!(value["capability_profile"]["can"]["return_json"], "partial");
    assert_eq!(value["capability_profile"]["routing_tags"][0], "test");
}
