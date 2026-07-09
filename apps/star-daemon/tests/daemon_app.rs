use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn status_opens_daemon_state_without_ai_live_connectors() {
    let config_root = temp_config_root("status");
    let output = run_daemon([
        "status",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("status json");
    assert_eq!(value["app"], "star-daemon");
    assert_eq!(value["command"], "status");
    assert_eq!(value["status"], "success");
    assert_eq!(value["data"]["daemon_state"]["status"], "reserved");
    assert_eq!(value["data"]["process"]["http_server_enabled"], false);
    assert_eq!(
        value["data"]["process"]["provider_scheduling_enabled"],
        false
    );
    assert_eq!(
        value["data"]["process"]["local_ai_live_connector"],
        "disabled"
    );
    assert_eq!(
        value["data"]["process"]["cloud_ai_live_connector"],
        "disabled"
    );
    assert_eq!(value["data"]["process"]["live_calls_performed"], false);
    assert!(config_root.join("daemon").join("state.json").is_file());

    fs::remove_dir_all(config_root).ok();
}

#[test]
fn serve_once_reports_process_tick_without_running_provider() {
    let config_root = temp_config_root("serve");
    let output = run_daemon([
        "serve",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--max-ticks",
        "1",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("serve json");
    assert_eq!(value["command"], "serve");
    assert_eq!(value["data"]["process"]["mode"], "serve");
    assert_eq!(value["data"]["process"]["tick_count"], 1);
    assert_eq!(
        value["data"]["process"]["provider_scheduling_enabled"],
        true
    );
    assert_eq!(value["data"]["scheduler_ticks"][0]["status"], "idle");
    assert_eq!(value["data"]["process"]["live_calls_performed"], false);

    fs::remove_dir_all(config_root).ok();
}

#[test]
fn serve_tick_executes_queued_fake_default_job_without_live_connectors() {
    let config_root = temp_config_root("serve-scheduler");
    let project_root = temp_config_root("serve-scheduler-project");
    fs::create_dir_all(&project_root).expect("create project");
    create_fake_schedulable_job(&project_root);
    let store = star_control_state::StateStore::open(&project_root, schema_root())
        .expect("open project store");
    let queue = star_control_daemon::DaemonQueue::open(star_control_daemon::DaemonConfig::local(
        config_root.clone(),
        schema_root(),
    ))
    .expect("open daemon queue");
    queue
        .enqueue_project_job(&store, "J-0001")
        .expect("enqueue fake job");

    let output = run_daemon([
        "serve",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--max-ticks",
        "1",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("serve json");
    assert_eq!(value["data"]["scheduler_ticks"][0]["status"], "EXECUTED");
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_instance"],
        "fake-default"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_execution_performed"],
        true
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["live_calls_performed"],
        false
    );
    assert_eq!(value["data"]["daemon_state"]["queue"], json!([]));
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
        .is_file());
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .is_file());
    let state = store.load_state("J-0001").expect("load executed state");
    assert_eq!(state["state"], "IMPLEMENTED");

    fs::remove_dir_all(config_root).ok();
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn serve_tick_disables_non_fake_provider_without_live_calls() {
    let config_root = temp_config_root("serve-disabled");
    let project_root = temp_config_root("serve-disabled-project");
    fs::create_dir_all(&project_root).expect("create project");
    create_schedulable_job(&project_root, "local-vllm");
    let store = star_control_state::StateStore::open(&project_root, schema_root())
        .expect("open project store");
    let queue = star_control_daemon::DaemonQueue::open(star_control_daemon::DaemonConfig::local(
        config_root.clone(),
        schema_root(),
    ))
    .expect("open daemon queue");
    queue
        .enqueue_project_job(&store, "J-0001")
        .expect("enqueue disabled provider job");

    let output = run_daemon([
        "serve",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--max-ticks",
        "1",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("serve json");
    assert_eq!(value["data"]["scheduler_ticks"][0]["status"], "DISABLED");
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_instance"],
        "local-vllm"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_execution_performed"],
        false
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["live_calls_performed"],
        false
    );
    assert_eq!(
        value["data"]["daemon_state"]["queue"][0]["state"],
        "DISABLED"
    );
    assert!(!project_root
        .join(".ai-runs/J-0001/provider-output/local-vllm")
        .exists());
    let state = store.load_state("J-0001").expect("load disabled state");
    assert_eq!(state["state"], "ROUTED");

    fs::remove_dir_all(config_root).ok();
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn serve_tick_executes_queued_local_process_provider_without_live_connectors() {
    let config_root = temp_config_root("serve-local-process");
    let project_root = temp_config_root("serve-local-process-project");
    fs::create_dir_all(&project_root).expect("create project");
    create_schedulable_job(&project_root, "local-default");
    let provider_instance = write_local_process_instance(&project_root, vec!["--help".to_string()]);
    let store = star_control_state::StateStore::open(&project_root, schema_root())
        .expect("open project store");
    let queue = star_control_daemon::DaemonQueue::open(star_control_daemon::DaemonConfig::local(
        config_root.clone(),
        schema_root(),
    ))
    .expect("open daemon queue");
    queue
        .enqueue_project_job_with_provider_instances(&store, "J-0001", vec![provider_instance])
        .expect("enqueue local process provider job");

    let output = run_daemon([
        "serve",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--max-ticks",
        "1",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("serve json");
    assert_eq!(value["data"]["scheduler_ticks"][0]["status"], "EXECUTED");
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_instance"],
        "local-default"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_kind"],
        "local_process_model"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_transport"],
        "process"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["provider_execution_performed"],
        true
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["live_calls_performed"],
        false
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["local_ai_live_connector"],
        "disabled"
    );
    assert_eq!(
        value["data"]["scheduler_ticks"][0]["cloud_ai_live_connector"],
        "disabled"
    );
    assert_eq!(value["data"]["daemon_state"]["queue"], json!([]));
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/local-default/request.json")
        .is_file());
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
        .is_file());
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
        .is_file());
    assert!(project_root
        .join(".ai-runs/J-0001/provider-output/local-default/response.json")
        .is_file());
    let state = store
        .load_state("J-0001")
        .expect("load local process state");
    assert_eq!(state["state"], "IMPLEMENTED");

    fs::remove_dir_all(config_root).ok();
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn api_plan_binds_local_http_without_ai_live_connectors() {
    let config_root = temp_config_root("api-plan");
    let output = run_daemon([
        "api",
        "--config-root",
        config_root.to_str().expect("config root"),
        "--schema-root",
        schema_root().to_str().expect("schema root"),
        "--bind",
        "127.0.0.1:0",
        "--max-requests",
        "0",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = serde_json::from_str(&stdout).expect("api json");
    assert_eq!(value["command"], "api");
    assert_eq!(value["data"]["process"]["http_server_enabled"], true);
    assert_eq!(value["data"]["process"]["remote_exposure_enabled"], false);
    assert_eq!(
        value["data"]["process"]["provider_scheduling_enabled"],
        false
    );
    assert_eq!(
        value["data"]["process"]["local_ai_live_connector"],
        "disabled"
    );
    assert_eq!(
        value["data"]["process"]["cloud_ai_live_connector"],
        "disabled"
    );
    assert_eq!(value["data"]["process"]["live_calls_performed"], false);
    assert_eq!(value["data"]["handled_requests"], 0);

    fs::remove_dir_all(config_root).ok();
}

#[test]
fn http_server_serves_api_control_service_get_requests() {
    let config_root = temp_config_root("http-api");
    let project_root = temp_config_root("http-project");
    fs::create_dir_all(&project_root).expect("create project");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind api test listener");
    let address = listener.local_addr().expect("local addr");
    let service = star_daemon::api_service(
        config_root.clone(),
        schema_root(),
        Some(("local".to_string(), project_root.clone())),
    )
    .expect("api service");
    let handle =
        thread::spawn(move || star_daemon::serve_api_listener(listener, service, 5).unwrap());

    let daemon_response = http_get(address, "/daemon/state");
    assert!(daemon_response.starts_with("HTTP/1.1 200 OK"));
    let daemon_json = response_json(&daemon_response);
    assert_eq!(daemon_json["status"], "success");
    assert_eq!(daemon_json["data"]["daemon_state"]["status"], "reserved");

    let preflight_response = http_options(address, "/projects", "http://127.0.0.1:18788");
    assert!(preflight_response.starts_with("HTTP/1.1 204 No Content"));
    assert!(preflight_response.contains("Access-Control-Allow-Origin: http://127.0.0.1:18788"));
    assert!(preflight_response
        .contains("Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS"));
    assert!(preflight_response.contains("Access-Control-Allow-Headers: Content-Type"));

    let cors_get_response = http_get_with_origin(address, "/projects", "http://127.0.0.1:18788");
    assert!(cors_get_response.starts_with("HTTP/1.1 200 OK"));
    assert!(cors_get_response.contains("Access-Control-Allow-Origin: http://127.0.0.1:18788"));
    let cors_get_json = response_json(&cors_get_response);
    assert_eq!(cors_get_json["status"], "success");

    let projects_response = http_get(address, "/projects");
    assert!(projects_response.starts_with("HTTP/1.1 200 OK"));
    let projects_json = response_json(&projects_response);
    assert_eq!(projects_json["status"], "success");
    assert_eq!(projects_json["data"]["projects"][0]["project_id"], "local");

    let post_response = http_post(address, "/projects/local/jobs/J-404/cancel", "{}");
    assert!(post_response.starts_with("HTTP/1.1 200 OK"));
    let post_json = response_json(&post_response);
    assert_eq!(post_json["status"], "failed");
    assert_eq!(post_json["error"]["code"], "state_read_failed");

    assert_eq!(handle.join().expect("server join"), 5);
    fs::remove_dir_all(config_root).ok();
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn http_control_actions_append_audit_events() {
    let config_root = temp_config_root("http-audit");
    let project_root = temp_config_root("http-audit-project");
    fs::create_dir_all(&project_root).expect("create project");
    create_cancelable_job(&project_root);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind audit api listener");
    let address = listener.local_addr().expect("local addr");
    let service = star_daemon::api_service(
        config_root.clone(),
        schema_root(),
        Some(("local".to_string(), project_root.clone())),
    )
    .expect("api service");
    let handle =
        thread::spawn(move || star_daemon::serve_api_listener(listener, service, 1).unwrap());

    let response = http_post(address, "/projects/local/jobs/J-0001/cancel", "{}");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let body = response_json(&response);
    assert_eq!(body["status"], "success");
    assert_eq!(body["data"]["observability"]["audit_event_recorded"], true);
    assert_eq!(
        body["data"]["observability"]["audit_event_ref"]["path"],
        "audit/audit-events.jsonl"
    );
    assert_eq!(handle.join().expect("server join"), 1);

    let audit_path = project_root.join(".ai-runs/J-0001/audit/audit-events.jsonl");
    let audit_text = fs::read_to_string(audit_path).expect("read audit log");
    assert!(audit_text.contains("\"type\":\"api_control_action\""));
    assert!(audit_text.contains("HTTP API cancel action"));
    assert!(!audit_text.contains("Bearer"));

    fs::remove_dir_all(config_root).ok();
    fs::remove_dir_all(project_root).ok();
}

fn run_daemon<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_star-daemon"))
        .args(args)
        .output()
        .expect("run star-daemon")
}

fn http_get(address: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("connect api server");
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, address
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    response
}

fn http_get_with_origin(address: SocketAddr, path: &str, origin: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("connect api server");
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nConnection: close\r\n\r\n",
        path, address, origin
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    response
}

fn http_options(address: SocketAddr, path: &str, origin: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("connect api server");
    let request = format!(
        "OPTIONS {} HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nAccess-Control-Request-Method: POST\r\nAccess-Control-Request-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        path, address, origin
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    response
}

fn http_post(address: SocketAddr, path: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("connect api server");
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        address,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    response
}

fn response_json(response: &str) -> Value {
    let body = response
        .split_once("\r\n\r\n")
        .expect("http body separator")
        .1;
    serde_json::from_str(body).expect("response json")
}

fn schema_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("apps dir")
        .parent()
        .expect("repo root")
        .join("specs")
        .join("schemas")
}

fn temp_config_root(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "star-daemon-{}-{}-{}",
        label,
        std::process::id(),
        now
    ))
}

fn create_cancelable_job(project_root: &PathBuf) {
    let store = star_control_state::StateStore::open(project_root, schema_root())
        .expect("open test project store");
    let job = store
        .create_job(
            "cancel from HTTP API",
            "star-daemon-test",
            vec!["no live AI connector".to_string()],
        )
        .expect("create test job");
    let job_id = job["job_id"].as_str().expect("job id");
    store
        .save_state(
            job_id,
            &json!({
                "schema_version": "1.0.0",
                "job_id": job_id,
                "state": "IMPLEMENTING",
                "current_stage": "implement",
                "updated_at": "2026-07-06T00:00:00Z",
                "threads": {},
                "workers": {},
                "artifacts": {},
                "latest_event_id": "J-0001-0001",
                "active_provider": null,
                "next_action": "continue",
                "budget": {},
                "history": []
            }),
        )
        .expect("save test state");
}

fn create_fake_schedulable_job(project_root: &PathBuf) {
    create_schedulable_job(project_root, "fake-default");
}

fn create_schedulable_job(project_root: &PathBuf, provider_instance: &str) {
    let store = star_control_state::StateStore::open(project_root, schema_root())
        .expect("open test project store");
    let job = store
        .create_job(
            "execute queued fake provider",
            "star-daemon-test",
            vec!["no live AI connector".to_string()],
        )
        .expect("create test job");
    let job_id = job["job_id"].as_str().expect("job id");
    store
        .save_workspec(
            job_id,
            "implement",
            &json!({
                "schema_version": "1.0.0",
                "job_id": job_id,
                "stage": "implement",
                "role": "implementer",
                "provider": provider_instance,
                "provider_instance": provider_instance,
                "project_root": project_root.display().to_string(),
                "goal": "execute queued fake provider",
                "allowed_scope": ["."],
                "forbidden_actions": [],
                "required_outputs": ["provider response"]
            }),
        )
        .expect("save workspec");
    store
        .save_state(
            job_id,
            &json!({
                "schema_version": "1.0.0",
                "job_id": job_id,
                "state": "ROUTED",
                "current_stage": "implement",
                "updated_at": "2026-07-06T00:00:00Z",
                "threads": {},
                "workers": {},
                "artifacts": {},
                "latest_event_id": "J-0001-0001",
                "active_provider": null,
                "next_action": "execute",
                "budget": {},
                "history": []
            }),
        )
        .expect("save routable state");
}

fn write_local_process_instance(project_root: &Path, args: Vec<String>) -> PathBuf {
    let path = project_root.join("local-process-instance.json");
    let executable = std::env::current_exe()
        .expect("current test executable")
        .display()
        .to_string();
    fs::write(
        &path,
        serde_json::to_string_pretty(&json!({
            "id": "local-default",
            "provider": "provider.local-process",
            "enabled": true,
            "limits": {
                "timeout_seconds": 10,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["local", "process"],
            "command_policy": {
                "shell": false,
                "allowed_executables": [executable],
                "env_allowlist": [],
                "cwd_policy": "project_root",
                "network": "deny",
                "workspace_write": "deny"
            },
            "command": {
                "executable": executable,
                "args": args
            }
        }))
        .expect("serialize local process instance"),
    )
    .expect("write local process instance");
    path
}
