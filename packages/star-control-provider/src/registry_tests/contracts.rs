use super::helpers::provider_registry_loader;
use crate::CapabilityValue;
use std::path::PathBuf;

#[test]
fn loads_fake_default_registry_from_json_contracts() {
    let loader = provider_registry_loader();
    let registry = loader
        .load_fake_default_registry()
        .expect("load fake default registry");

    let instance = registry
        .instance("fake-default")
        .expect("fake-default instance");
    assert_eq!(instance.provider_id(), "provider.fake");
    assert!(instance.enabled());

    let manifest = registry
        .manifest_for_instance("fake-default")
        .expect("manifest for fake-default");
    assert_eq!(manifest.id(), "provider.fake");
    assert_eq!(manifest.kind(), "fake_provider");
    assert_eq!(manifest.transport(), "manual");

    let profile = registry
        .capability_for_instance("fake-default")
        .expect("capability for fake-default");
    assert_eq!(
        profile.capability("read_repo"),
        Some(CapabilityValue::Bool(true))
    );
    assert_eq!(
        profile.capability("run_shell"),
        Some(CapabilityValue::Bool(false))
    );
}

#[test]
fn loads_builtin_yaml_registry_and_fake_provider_contracts() {
    let loader = provider_registry_loader();
    let registry = loader
        .load_registry(
            "configs/registries/builtin-provider-registry.yaml",
            &[PathBuf::from(
                "configs/provider-instances/fake-provider.example.yaml",
            )],
        )
        .expect("load builtin registry");

    let fake = registry.manifest("provider.fake").expect("fake provider");
    assert_eq!(fake.adapter(), "code_agent");
    assert!(registry
        .providers()
        .iter()
        .any(|provider| provider.id() == "provider.fake"));
    assert_eq!(registry.providers_by_kind("fake_provider").len(), 1);
    assert!(registry.providers_by_transport("manual").len() >= 2);

    let instance = registry.instance("fake-default").expect("fake instance");
    assert_eq!(instance.provider_id(), "provider.fake");

    let profile = registry
        .capability_for_instance("fake-default")
        .expect("fake capability");
    assert!(profile.routing_tags().contains(&"test".to_string()));
    assert_eq!(
        profile.capability("return_json"),
        Some(CapabilityValue::Bool(true))
    );
}
