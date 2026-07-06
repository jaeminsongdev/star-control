use crate::ProviderRegistryLoader;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn provider_registry_loader() -> ProviderRegistryLoader {
    ProviderRegistryLoader::new(repo_root())
}

pub(super) fn write_temp_json(name: &str, value: &Value) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "star-control-provider-{}-{}-{}",
        std::process::id(),
        nanos,
        name
    ));
    fs::write(
        &path,
        serde_json::to_string_pretty(value).expect("serialize fixture"),
    )
    .expect("write fixture");
    path
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}
