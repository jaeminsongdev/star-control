use crate::DaemonAppOptions;
use serde_json::{json, Value};
use star_control_daemon::DaemonQueue;
use star_control_execution::ExecutionEngine;
use star_control_provider::{ProviderManifest, ProviderRegistry, ProviderRegistryLoader};
use star_control_state::StateStore;
use std::path::{Path, PathBuf};

const QUEUED: &str = "QUEUED";
const RUNNING: &str = "RUNNING";
const EXECUTED: &str = "EXECUTED";
const DISABLED: &str = "DISABLED";
const FAILED: &str = "FAILED";
const FAKE_DEFAULT: &str = "fake-default";
const BUILTIN_PROVIDER_REGISTRY: &str = "configs/registries/builtin-provider-registry.yaml";
const FAKE_PROVIDER_INSTANCE_EXAMPLE: &str =
    "examples/provider-contracts/provider-instance.fake.example.json";
const LOCAL_PROCESS_KIND: &str = "local_process_model";
const PROCESS_TRANSPORT: &str = "process";

pub(crate) fn run_scheduler_ticks(
    options: &DaemonAppOptions,
    queue: &DaemonQueue,
    max_ticks: u64,
) -> Result<Vec<Value>, String> {
    let mut results = Vec::new();
    for tick_index in 0..max_ticks {
        results.push(run_scheduler_tick(options, queue, tick_index + 1)?);
    }
    Ok(results)
}

fn run_scheduler_tick(
    options: &DaemonAppOptions,
    queue: &DaemonQueue,
    tick_number: u64,
) -> Result<Value, String> {
    let mut daemon_state = queue.load_state().map_err(|source| source.to_string())?;
    daemon_state["status"] = json!("running");
    let Some(entry_index) = first_queued_entry_index(&daemon_state) else {
        daemon_state["status"] = json!("stopped");
        queue
            .save_state(&daemon_state)
            .map_err(|source| source.to_string())?;
        return Ok(json!({
            "tick": tick_number,
            "status": "idle",
            "provider_scheduling_enabled": true,
            "provider_execution_performed": false,
            "live_calls_performed": false,
            "local_ai_live_connector": "disabled",
            "cloud_ai_live_connector": "disabled"
        }));
    };

    let entry = daemon_state["queue"][entry_index].clone();
    set_queue_entry_state(&mut daemon_state, entry_index, RUNNING);
    add_active_job(&mut daemon_state, &entry);
    queue
        .save_state(&daemon_state)
        .map_err(|source| source.to_string())?;

    let result = process_entry(options, &entry);

    let mut daemon_state = queue.load_state().map_err(|source| source.to_string())?;
    match &result {
        Ok(value) if value.get("status").and_then(Value::as_str) == Some(EXECUTED) => {
            remove_queue_entry(&mut daemon_state, &entry);
            daemon_state["last_error"] = Value::Null;
        }
        Ok(value) => {
            mark_matching_entry(&mut daemon_state, &entry, DISABLED, value.clone());
            daemon_state["last_error"] = Value::Null;
        }
        Err(message) => {
            mark_matching_entry(
                &mut daemon_state,
                &entry,
                FAILED,
                json!({ "error": message }),
            );
            daemon_state["last_error"] = json!({
                "job_id": entry.get("job_id").cloned().unwrap_or(Value::Null),
                "project_root": entry.get("project_root").cloned().unwrap_or(Value::Null),
                "message": message
            });
        }
    }
    remove_active_job(&mut daemon_state, &entry);
    daemon_state["status"] = if result.is_err() {
        json!("error")
    } else {
        json!("stopped")
    };
    queue
        .save_state(&daemon_state)
        .map_err(|source| source.to_string())?;

    match result {
        Ok(mut value) => {
            value["tick"] = json!(tick_number);
            Ok(value)
        }
        Err(message) => Ok(json!({
            "tick": tick_number,
            "status": "failed",
            "provider_scheduling_enabled": true,
            "provider_execution_performed": false,
            "live_calls_performed": false,
            "error": {
                "message": message
            },
            "local_ai_live_connector": "disabled",
            "cloud_ai_live_connector": "disabled"
        })),
    }
}

fn process_entry(options: &DaemonAppOptions, entry: &Value) -> Result<Value, String> {
    let job_id = required_entry_string(entry, "job_id")?;
    let project_root = PathBuf::from(required_entry_string(entry, "project_root")?);
    let stage = required_entry_string(entry, "current_stage")?;
    let store = StateStore::open(project_root, options.schema_root.clone())
        .map_err(|source| source.to_string())?;
    let workspec = store
        .load_workspec(&job_id, &stage)
        .map_err(|source| source.to_string())?;
    let provider_instance = workspec
        .get("provider_instance")
        .and_then(Value::as_str)
        .ok_or_else(|| "workspec provider_instance is required".to_string())?;

    let provider_instance_paths = provider_instance_paths(entry)?;
    if provider_instance != FAKE_DEFAULT && provider_instance_paths.is_empty() {
        return Ok(disabled_result(
            &job_id,
            &stage,
            provider_instance,
            "provider_instance_paths is required for non-default daemon scheduler execution",
            None,
        ));
    }

    let repo_root = repo_root_from_schema_root(&options.schema_root);
    let registry = scheduler_registry(&repo_root, provider_instance, &provider_instance_paths)?;
    let manifest = registry
        .manifest_for_instance(provider_instance)
        .map_err(|source| source.to_string())?;
    if !scheduler_supported_manifest(manifest) {
        return Ok(disabled_result(
            &job_id,
            &stage,
            provider_instance,
            "daemon scheduler executes fake-default and local-process providers only; Local/Cloud AI live connectors remain disabled",
            Some(manifest),
        ));
    }
    let provider_id = manifest.id().to_string();
    let provider_kind = manifest.kind().to_string();
    let provider_transport = manifest.transport().to_string();

    let outcome = ExecutionEngine::new(&store, &registry, options.schema_root.clone())
        .execute_stage(&job_id, &stage)
        .map_err(|source| source.to_string())?;

    Ok(json!({
        "status": EXECUTED,
        "job_id": job_id,
        "stage": stage,
        "provider_instance": provider_instance,
        "provider_id": provider_id,
        "provider_kind": provider_kind,
        "provider_transport": provider_transport,
        "run_state": outcome.state().get("state").cloned().unwrap_or(Value::Null),
        "provider_scheduling_enabled": true,
        "provider_execution_performed": true,
        "live_calls_performed": false,
        "local_ai_live_connector": "disabled",
        "cloud_ai_live_connector": "disabled",
        "artifacts": [
            format!(".ai-runs/{}/provider-output/{}/request.json", job_id, provider_instance),
            format!(".ai-runs/{}/provider-output/{}/response.json", job_id, provider_instance)
        ]
    }))
}

fn scheduler_registry(
    repo_root: &Path,
    provider_instance: &str,
    provider_instance_paths: &[PathBuf],
) -> Result<ProviderRegistry, String> {
    let loader = ProviderRegistryLoader::new(repo_root.to_path_buf());
    if provider_instance == FAKE_DEFAULT && provider_instance_paths.is_empty() {
        return loader
            .load_fake_default_registry()
            .map_err(|source| source.to_string());
    }

    let mut instance_paths = vec![PathBuf::from(FAKE_PROVIDER_INSTANCE_EXAMPLE)];
    instance_paths.extend(provider_instance_paths.iter().cloned());
    loader
        .load_registry(BUILTIN_PROVIDER_REGISTRY, &instance_paths)
        .map_err(|source| source.to_string())
}

fn scheduler_supported_manifest(manifest: &ProviderManifest) -> bool {
    manifest.id() == "provider.fake"
        || (manifest.kind() == LOCAL_PROCESS_KIND && manifest.transport() == PROCESS_TRANSPORT)
}

fn provider_instance_paths(entry: &Value) -> Result<Vec<PathBuf>, String> {
    let Some(paths) = entry.get("provider_instance_paths") else {
        return Ok(Vec::new());
    };
    let Some(paths) = paths.as_array() else {
        return Err("queue entry provider_instance_paths must be an array".to_string());
    };
    paths
        .iter()
        .map(|path| {
            path.as_str().map(PathBuf::from).ok_or_else(|| {
                "queue entry provider_instance_paths entries must be strings".to_string()
            })
        })
        .collect()
}

fn disabled_result(
    job_id: &str,
    stage: &str,
    provider_instance: &str,
    reason: &str,
    manifest: Option<&ProviderManifest>,
) -> Value {
    json!({
        "status": DISABLED,
        "job_id": job_id,
        "stage": stage,
        "provider_instance": provider_instance,
        "provider_id": manifest.map(ProviderManifest::id),
        "provider_kind": manifest.map(ProviderManifest::kind),
        "provider_transport": manifest.map(ProviderManifest::transport),
        "provider_scheduling_enabled": true,
        "provider_execution_performed": false,
        "live_calls_performed": false,
        "disabled_reason": reason,
        "local_ai_live_connector": "disabled",
        "cloud_ai_live_connector": "disabled"
    })
}

fn first_queued_entry_index(daemon_state: &Value) -> Option<usize> {
    daemon_state
        .get("queue")
        .and_then(Value::as_array)
        .and_then(|queue| {
            queue
                .iter()
                .position(|entry| entry.get("state").and_then(Value::as_str) == Some(QUEUED))
        })
}

fn set_queue_entry_state(daemon_state: &mut Value, index: usize, state: &str) {
    if let Some(entry) = daemon_state
        .get_mut("queue")
        .and_then(Value::as_array_mut)
        .and_then(|queue| queue.get_mut(index))
    {
        entry["state"] = json!(state);
    }
}

fn add_active_job(daemon_state: &mut Value, entry: &Value) {
    let Some(job_id) = entry.get("job_id").and_then(Value::as_str) else {
        return;
    };
    let Some(active_jobs) = daemon_state
        .get_mut("active_jobs")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if !active_jobs
        .iter()
        .any(|active| active.as_str() == Some(job_id))
    {
        active_jobs.push(json!(job_id));
    }
}

fn remove_active_job(daemon_state: &mut Value, entry: &Value) {
    let Some(job_id) = entry.get("job_id").and_then(Value::as_str) else {
        return;
    };
    if let Some(active_jobs) = daemon_state
        .get_mut("active_jobs")
        .and_then(Value::as_array_mut)
    {
        active_jobs.retain(|active| active.as_str() != Some(job_id));
    }
}

fn remove_queue_entry(daemon_state: &mut Value, entry: &Value) {
    if let Some(queue) = daemon_state.get_mut("queue").and_then(Value::as_array_mut) {
        queue.retain(|candidate| !same_queue_entry(candidate, entry));
    }
}

fn mark_matching_entry(
    daemon_state: &mut Value,
    entry: &Value,
    state: &str,
    scheduler_result: Value,
) {
    if let Some(queue) = daemon_state.get_mut("queue").and_then(Value::as_array_mut) {
        for candidate in queue {
            if same_queue_entry(candidate, entry) {
                candidate["state"] = json!(state);
                candidate["scheduler_result"] = scheduler_result;
                break;
            }
        }
    }
}

fn same_queue_entry(left: &Value, right: &Value) -> bool {
    left.get("job_id").and_then(Value::as_str) == right.get("job_id").and_then(Value::as_str)
        && left.get("project_root").and_then(Value::as_str)
            == right.get("project_root").and_then(Value::as_str)
}

fn required_entry_string(entry: &Value, field: &str) -> Result<String, String> {
    entry
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("queue entry field {} is required", field))
}

fn repo_root_from_schema_root(schema_root: &Path) -> PathBuf {
    schema_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
