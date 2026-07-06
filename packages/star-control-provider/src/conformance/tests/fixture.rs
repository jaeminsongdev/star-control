use super::super::{
    provider_path, LOG_KIND, PROVIDER_OUTPUT_KIND, REQUEST_FILE, RESPONSE_FILE, STDOUT_FILE,
};
use crate::{ProviderExecution, ProviderRegistry, ProviderRunContext, ProviderRunResult};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct Fixture {
    project: PathBuf,
    schema_root: PathBuf,
    store: StateStore,
    registry: ProviderRegistry,
}

impl Fixture {
    pub(super) fn new(label: &str) -> Self {
        let project = temp_project(label);
        let schema_root = schema_root();
        fs::create_dir_all(
            project
                .join(".ai-runs")
                .join("J-0001")
                .join("provider-output")
                .join("cloud-default"),
        )
        .expect("create provider output dir");
        let store = StateStore::open(&project, &schema_root).expect("open state store");
        Self {
            project,
            schema_root,
            store,
            registry: ProviderRegistry::new(),
        }
    }

    pub(super) fn context(&self) -> ProviderRunContext<'_> {
        ProviderRunContext::new(&self.registry, &self.store, &self.schema_root)
    }

    pub(super) fn write_provider_json(&self, file_name: &str, value: &Value) {
        fs::write(
            self.provider_output_dir().join(file_name),
            serde_json::to_string_pretty(value).expect("serialize provider JSON"),
        )
        .expect("write provider JSON");
    }

    pub(super) fn write_provider_text(&self, file_name: &str, value: &str) {
        fs::write(self.provider_output_dir().join(file_name), value).expect("write provider text");
    }

    fn provider_output_dir(&self) -> PathBuf {
        self.project
            .join(".ai-runs")
            .join("J-0001")
            .join("provider-output")
            .join("cloud-default")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.project).ok();
    }
}

pub(super) fn execution_from_value(value: Value) -> ProviderExecution {
    let provider_instance_id = value["provider_instance_id"]
        .as_str()
        .expect("provider instance id")
        .to_string();
    let result = ProviderRunResult::from_value(
        value,
        provider_path(&provider_instance_id, RESPONSE_FILE),
        schema_root(),
    )
    .expect("provider result");
    ProviderExecution::new(
        result,
        artifact_ref(&provider_instance_id, REQUEST_FILE, PROVIDER_OUTPUT_KIND),
        artifact_ref(&provider_instance_id, RESPONSE_FILE, PROVIDER_OUTPUT_KIND),
        artifact_ref(&provider_instance_id, STDOUT_FILE, LOG_KIND),
        None,
    )
}

pub(super) fn result_value(
    provider_instance_id: &str,
    status: &str,
    artifacts: Vec<&str>,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": provider_instance_id,
        "job_id": "J-0001",
        "stage": "implement",
        "status": status,
        "started_at": "unix:0",
        "finished_at": "unix:1",
        "stdout_path": provider_path(provider_instance_id, STDOUT_FILE),
        "stderr_path": Value::Null,
        "summary": "provider completed",
        "changed_files": [],
        "artifacts": artifacts,
        "metrics": {
            "estimated_cost": 0,
            "input_tokens": 0,
            "output_tokens": 0
        },
        "error": Value::Null
    })
}

fn artifact_ref(provider_instance_id: &str, file_name: &str, kind: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "path": provider_path(provider_instance_id, file_name),
        "kind": kind,
        "producer": provider_instance_id,
        "schema_path": Value::Null,
        "description": "provider conformance test artifact"
    })
}

fn temp_project(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "star-control-provider-conformance-{}-{}-{}",
        std::process::id(),
        nanos,
        label
    ))
}

fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .join("specs")
        .join("schemas")
}
