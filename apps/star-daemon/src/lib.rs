use serde_json::{json, Value};
use star_control_api::{ApiControlService, ApiError, ApiMethod, ApiRequest};
use star_control_daemon::{DaemonConfig, DaemonQueue};
use star_control_observability::AuditEventWriter;
use star_control_state::StateStore;
use std::collections::BTreeMap;
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod scheduler;

pub fn run_args(args: impl IntoIterator<Item = String>) -> Result<Value, String> {
    let options = DaemonAppOptions::parse(args)?;
    let queue = DaemonQueue::open(DaemonConfig::local(
        options.config_root.clone(),
        options.schema_root.clone(),
    ))
    .map_err(|source| source.to_string())?;

    match options.command.as_str() {
        "status" => status_output(&options, &queue),
        "serve" => serve_output(&options, &queue),
        "api" => api_output(options, queue),
        other => Err(format!("unsupported daemon command {}", other)),
    }
}

pub fn error_output(message: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "app": "star-daemon",
        "status": "failed",
        "exit_code": 2,
        "error": {
            "code": "InvalidInput",
            "message": message
        },
        "warnings": []
    })
}

pub fn serve_api_listener(
    listener: TcpListener,
    mut service: DaemonApiService,
    max_requests: usize,
) -> Result<usize, String> {
    let mut handled = 0usize;
    while handled < max_requests {
        let (mut stream, _) = listener.accept().map_err(|source| source.to_string())?;
        let response = handle_http_stream(&mut stream, &mut service);
        stream
            .write_all(response.as_bytes())
            .map_err(|source| source.to_string())?;
        handled += 1;
    }
    Ok(handled)
}

pub fn api_service(
    config_root: PathBuf,
    schema_root: PathBuf,
    project: Option<(String, PathBuf)>,
) -> Result<DaemonApiService, String> {
    let queue = DaemonQueue::open(DaemonConfig::local(config_root, schema_root.clone()))
        .map_err(|source| source.to_string())?;
    let mut service = DaemonApiService::new(schema_root.clone());
    service.register_daemon_queue(queue);
    if let Some((project_id, project_root)) = project {
        let store =
            StateStore::open(project_root, schema_root).map_err(|source| source.to_string())?;
        service
            .register_project_store(project_id, store)
            .map_err(|source| source.to_string())?;
    }
    Ok(service)
}

fn status_output(options: &DaemonAppOptions, queue: &DaemonQueue) -> Result<Value, String> {
    let daemon_state = queue.load_state().map_err(|source| source.to_string())?;
    Ok(success_output(
        "status",
        json!({
            "config_root": options.config_root.display().to_string(),
            "schema_root": options.schema_root.display().to_string(),
            "daemon_dir": queue.daemon_dir().display().to_string(),
            "state_path": queue.state_path().display().to_string(),
            "daemon_state": daemon_state,
            "process": process_capabilities("status", 0)
        }),
    ))
}

fn serve_output(options: &DaemonAppOptions, queue: &DaemonQueue) -> Result<Value, String> {
    let max_ticks = options.max_ticks.unwrap_or(1);
    let scheduler_ticks = scheduler::run_scheduler_ticks(options, queue, max_ticks)?;
    let daemon_state = queue.load_state().map_err(|source| source.to_string())?;
    Ok(success_output(
        "serve",
        json!({
            "config_root": options.config_root.display().to_string(),
            "schema_root": options.schema_root.display().to_string(),
            "daemon_dir": queue.daemon_dir().display().to_string(),
            "state_path": queue.state_path().display().to_string(),
            "daemon_state": daemon_state,
            "scheduler_ticks": scheduler_ticks,
            "process": serve_process_capabilities(max_ticks)
        }),
    ))
}

fn api_output(options: DaemonAppOptions, queue: DaemonQueue) -> Result<Value, String> {
    let bind = options
        .bind
        .clone()
        .unwrap_or_else(|| "127.0.0.1:0".to_string());
    ensure_local_bind(&bind)?;
    let listener = TcpListener::bind(&bind).map_err(|source| source.to_string())?;
    let bound_addr = listener.local_addr().map_err(|source| source.to_string())?;
    let max_requests = options.max_requests.unwrap_or(0);
    let project = options.project_id.clone().zip(options.project_root.clone());
    let mut service = DaemonApiService::new(options.schema_root.clone());
    service.register_daemon_queue(queue);
    if let Some((project_id, project_root)) = project {
        let store = StateStore::open(project_root, options.schema_root.clone())
            .map_err(|source| source.to_string())?;
        service
            .register_project_store(project_id, store)
            .map_err(|source| source.to_string())?;
    }

    let handled_requests = if max_requests == 0 {
        0
    } else {
        serve_api_listener(listener, service, max_requests)?
    };

    Ok(success_output(
        "api",
        json!({
            "config_root": options.config_root.display().to_string(),
            "schema_root": options.schema_root.display().to_string(),
            "bind": bind,
            "local_addr": bound_addr.to_string(),
            "max_requests": max_requests,
            "handled_requests": handled_requests,
            "project_registered": options.project_id.is_some(),
            "process": {
                "mode": "api",
                "http_server_enabled": true,
                "remote_exposure_enabled": false,
                "provider_scheduling_enabled": false,
                "local_ai_live_connector": "disabled",
                "cloud_ai_live_connector": "disabled",
                "live_calls_performed": false
            }
        }),
    ))
}

#[derive(Debug, Clone)]
pub struct DaemonApiService {
    control: ApiControlService,
    audit_writer: AuditEventWriter,
    projects: BTreeMap<String, StateStore>,
}

impl DaemonApiService {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        let schema_root = schema_root.into();
        Self {
            control: ApiControlService::new(schema_root.clone()),
            audit_writer: AuditEventWriter::new(schema_root),
            projects: BTreeMap::new(),
        }
    }

    pub fn register_daemon_queue(&mut self, daemon_queue: DaemonQueue) {
        self.control.register_daemon_queue(daemon_queue);
    }

    pub fn register_project_store(
        &mut self,
        project_id: impl Into<String>,
        store: StateStore,
    ) -> Result<(), ApiError> {
        let project_id = project_id.into();
        self.control
            .register_project_store(project_id.clone(), store.clone())?;
        self.projects.insert(project_id, store);
        Ok(())
    }

    pub fn handle(&mut self, request: ApiRequest) -> Result<Value, ApiError> {
        let audit_action = ApiAuditAction::from_request(&request);
        let mut response = self.control.handle(request)?;
        if let Some(action) = audit_action {
            self.record_control_audit(&mut response, &action);
        }
        Ok(response)
    }

    fn record_control_audit(&self, response: &mut Value, action: &ApiAuditAction) {
        let Some(store) = self.projects.get(&action.project_id) else {
            push_warning(
                response,
                format!(
                    "audit event not recorded: project {} is not registered",
                    action.project_id
                ),
            );
            return;
        };

        let event_id = format!(
            "{}-http-api-{}-{}",
            action.job_id.to_ascii_lowercase(),
            action.command,
            event_suffix()
        );
        let mut event = self.audit_writer.event(
            &action.job_id,
            event_id,
            "api_control_action",
            "star-daemon-http-api",
            action.summary(response),
        );
        if let Some(object) = event.as_object_mut() {
            object.insert(
                "artifact_paths".to_string(),
                Value::Array(
                    action
                        .artifact_paths()
                        .iter()
                        .map(|path| Value::String((*path).to_string()))
                        .collect(),
                ),
            );
            object.insert(
                "risk_level".to_string(),
                Value::String(action.risk_level().to_string()),
            );
        }

        match self.audit_writer.append(store, &event) {
            Ok(artifact_ref) => attach_audit_ref(response, artifact_ref),
            Err(source) => push_warning(response, format!("audit event not recorded: {}", source)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiAuditAction {
    project_id: String,
    job_id: String,
    command: String,
}

impl ApiAuditAction {
    fn from_request(request: &ApiRequest) -> Option<Self> {
        if request.method() != ApiMethod::Post {
            return None;
        }
        let path = request
            .path()
            .split_once('?')
            .map_or(request.path(), |(path, _)| path);
        let segments = path
            .trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<&str>>();
        let ["projects", project_id, "jobs", job_id, command] = segments.as_slice() else {
            return None;
        };
        if !matches!(*command, "approve" | "cancel" | "resume") {
            return None;
        }
        Some(Self {
            project_id: (*project_id).to_string(),
            job_id: (*job_id).to_string(),
            command: (*command).to_string(),
        })
    }

    fn summary(&self, response: &Value) -> String {
        let status = response
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let code = response
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str);
        match code {
            Some(code) => format!(
                "HTTP API {} action for {} returned {} ({}).",
                self.command, self.job_id, status, code
            ),
            None => format!(
                "HTTP API {} action for {} returned {}.",
                self.command, self.job_id, status
            ),
        }
    }

    fn artifact_paths(&self) -> &'static [&'static str] {
        match self.command.as_str() {
            "approve" => &[
                "run-state.json",
                "events.jsonl",
                "approvals/approval-response.json",
            ],
            "cancel" | "resume" => &["run-state.json", "events.jsonl"],
            _ => &[],
        }
    }

    fn risk_level(&self) -> &'static str {
        match self.command.as_str() {
            "approve" | "cancel" => "MEDIUM",
            "resume" => "LOW",
            _ => "LOW",
        }
    }
}

fn attach_audit_ref(response: &mut Value, artifact_ref: Value) {
    if let Some(data) = response.get_mut("data").and_then(Value::as_object_mut) {
        data.insert(
            "observability".to_string(),
            json!({
                "audit_event_recorded": true,
                "audit_event_ref": artifact_ref
            }),
        );
    }
}

fn push_warning(response: &mut Value, message: String) {
    if let Some(warnings) = response.get_mut("warnings").and_then(Value::as_array_mut) {
        warnings.push(Value::String(message));
    } else if let Some(object) = response.as_object_mut() {
        object.insert("warnings".to_string(), json!([message]));
    }
}

fn event_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn success_output(command: &str, data: Value) -> Value {
    json!({
        "schema_version": "1.0.0",
        "app": "star-daemon",
        "command": command,
        "status": "success",
        "exit_code": 0,
        "data": data,
        "warnings": []
    })
}

fn process_capabilities(mode: &str, tick_count: u64) -> Value {
    json!({
        "mode": mode,
        "tick_count": tick_count,
        "http_server_enabled": false,
        "provider_scheduling_enabled": false,
        "local_ai_live_connector": "disabled",
        "cloud_ai_live_connector": "disabled",
        "live_calls_performed": false
    })
}

fn serve_process_capabilities(tick_count: u64) -> Value {
    json!({
        "mode": "serve",
        "tick_count": tick_count,
        "http_server_enabled": false,
        "provider_scheduling_enabled": true,
        "local_ai_live_connector": "disabled",
        "cloud_ai_live_connector": "disabled",
        "live_calls_performed": false
    })
}

fn ensure_local_bind(bind: &str) -> Result<(), String> {
    if bind.starts_with("127.0.0.1:")
        || bind.starts_with("localhost:")
        || bind.starts_with("[::1]:")
    {
        Ok(())
    } else {
        Err("HTTP API server bind must be loopback-only unless remote exposure is explicitly approved".to_string())
    }
}

fn handle_http_stream(stream: &mut TcpStream, service: &mut DaemonApiService) -> String {
    let request = match read_http_request(stream) {
        Ok(value) => value,
        Err(message) => return http_response(400, error_body("bad_request", &message), None),
    };
    let cors_origin = allowed_cors_origin(request.origin.as_deref());
    if request.method == "OPTIONS" {
        return http_response(204, Value::Null, cors_origin.as_deref());
    }
    let api_request = match request.to_api_request() {
        Ok(value) => value,
        Err(message) => {
            return http_response(
                400,
                error_body("bad_request", &message),
                cors_origin.as_deref(),
            )
        }
    };
    match service.handle(api_request) {
        Ok(value) => http_response(200, value, cors_origin.as_deref()),
        Err(source) => http_response(
            500,
            error_body("api_error", &source.to_string()),
            cors_origin.as_deref(),
        ),
    }
}

fn http_response(status_code: u16, body: Value, cors_origin: Option<&str>) -> String {
    let reason = match status_code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let body = if status_code == 204 {
        String::new()
    } else {
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
    };
    let cors_headers = cors_headers(cors_origin);
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason,
        cors_headers,
        body.len(),
        body
    )
}

fn cors_headers(origin: Option<&str>) -> String {
    let Some(origin) = origin else {
        return String::new();
    };
    format!(
        "Access-Control-Allow-Origin: {}\r\nVary: Origin\r\nAccess-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Max-Age: 600\r\n",
        origin
    )
}

fn allowed_cors_origin(origin: Option<&str>) -> Option<String> {
    let origin = origin?;
    let lower = origin.to_ascii_lowercase();
    if lower.starts_with("http://127.0.0.1:")
        || lower.starts_with("http://localhost:")
        || lower.starts_with("http://[::1]:")
    {
        Some(origin.to_string())
    } else {
        None
    }
}

fn error_body(code: &str, message: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "status": "failed",
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = [0u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|source| source.to_string())?;
    if bytes_read == 0 {
        return Err("empty HTTP request".to_string());
    }
    let raw = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
    HttpRequest::parse(&raw)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Value,
    origin: Option<String>,
}

impl HttpRequest {
    fn parse(raw: &str) -> Result<Self, String> {
        let Some((head, body_text)) = raw.split_once("\r\n\r\n") else {
            return Err("HTTP request missing header terminator".to_string());
        };
        let mut lines = head.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| "HTTP request line missing".to_string())?;
        let mut parts = request_line.split_whitespace();
        let method = parts
            .next()
            .ok_or_else(|| "HTTP method missing".to_string())?
            .to_string();
        let path = parts
            .next()
            .ok_or_else(|| "HTTP path missing".to_string())?
            .to_string();
        let mut content_length = 0usize;
        let mut origin = None;
        for (name, value) in lines.filter_map(|line| line.split_once(':')) {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.trim().to_string());
            }
        }
        let body = if content_length == 0 {
            Value::Null
        } else {
            let body_slice = body_text
                .as_bytes()
                .get(..content_length)
                .ok_or_else(|| "HTTP body shorter than content-length".to_string())?;
            serde_json::from_slice(body_slice).map_err(|source| source.to_string())?
        };
        Ok(Self {
            method,
            path,
            body,
            origin,
        })
    }

    fn to_api_request(&self) -> Result<ApiRequest, String> {
        let method = match self.method.as_str() {
            "GET" => ApiMethod::Get,
            "POST" => ApiMethod::Post,
            "PUT" => ApiMethod::Put,
            "PATCH" => ApiMethod::Patch,
            "DELETE" => ApiMethod::Delete,
            other => return Err(format!("unsupported HTTP method {}", other)),
        };
        Ok(ApiRequest::with_body(
            method,
            self.path.clone(),
            self.body.clone(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaemonAppOptions {
    command: String,
    config_root: PathBuf,
    schema_root: PathBuf,
    max_ticks: Option<u64>,
    bind: Option<String>,
    max_requests: Option<usize>,
    project_id: Option<String>,
    project_root: Option<PathBuf>,
}

impl DaemonAppOptions {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut command = "status".to_string();
        let mut config_root = env::var_os("STAR_CONTROL_CONFIG_ROOT").map(PathBuf::from);
        let mut schema_root = env::var_os("STAR_CONTROL_SCHEMA_ROOT").map(PathBuf::from);
        let mut max_ticks = None;
        let mut bind = None;
        let mut max_requests = None;
        let mut project_id = None;
        let mut project_root = None;

        let args = args.into_iter().collect::<Vec<String>>();
        let mut index = 0usize;
        while index < args.len() {
            match args[index].as_str() {
                "--config-root" => {
                    index += 1;
                    config_root =
                        Some(PathBuf::from(require_value(&args, index, "--config-root")?));
                }
                "--schema-root" => {
                    index += 1;
                    schema_root =
                        Some(PathBuf::from(require_value(&args, index, "--schema-root")?));
                }
                "--max-ticks" => {
                    index += 1;
                    let value = require_value(&args, index, "--max-ticks")?;
                    max_ticks =
                        Some(value.parse::<u64>().map_err(|_| {
                            "--max-ticks must be a non-negative integer".to_string()
                        })?);
                }
                "--bind" => {
                    index += 1;
                    bind = Some(require_value(&args, index, "--bind")?);
                }
                "--max-requests" => {
                    index += 1;
                    let value = require_value(&args, index, "--max-requests")?;
                    max_requests = Some(value.parse::<usize>().map_err(|_| {
                        "--max-requests must be a non-negative integer".to_string()
                    })?);
                }
                "--project-id" => {
                    index += 1;
                    project_id = Some(require_value(&args, index, "--project-id")?);
                }
                "--project-root" => {
                    index += 1;
                    project_root = Some(PathBuf::from(require_value(
                        &args,
                        index,
                        "--project-root",
                    )?));
                }
                "--json" => {}
                value if !value.starts_with("--") && command == "status" && index == 0 => {
                    command = value.to_string();
                }
                value => return Err(format!("unsupported argument {}", value)),
            }
            index += 1;
        }

        let config_root = config_root.ok_or_else(|| {
            "--config-root or STAR_CONTROL_CONFIG_ROOT is required for star-daemon".to_string()
        })?;
        let schema_root = schema_root.ok_or_else(|| {
            "--schema-root or STAR_CONTROL_SCHEMA_ROOT is required for star-daemon".to_string()
        })?;
        if project_id.is_some() != project_root.is_some() {
            return Err("--project-id and --project-root must be provided together".to_string());
        }

        Ok(Self {
            command,
            config_root,
            schema_root,
            max_ticks,
            bind,
            max_requests,
            project_id,
            project_root,
        })
    }
}

fn require_value(args: &[String], index: usize, option: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("missing value for {}", option))
}
