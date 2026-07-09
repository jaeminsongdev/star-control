use crate::cloud_api_artifacts::prepared_request_value;
use crate::cloud_cli::timeout_seconds;
use crate::cloud_constants::{
    COST_METRIC_FILE, HTTP_REQUEST_FILE, RAW_RESPONSE_FILE, REQUEST_FILE, RESPONSE_FILE,
    STDERR_FILE, STDOUT_FILE,
};
use crate::cloud_sidecars::cost_metric_value_with_response_usage;
use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::provider_redaction::{
    redact_provider_json_artifact, redact_provider_text_file_artifact,
};
use crate::{
    ExecutionRequest, OpenAiCompatiblePreparedRequest, OpenAiCompatibleRequestBuilder,
    OpenAiCompatibleResponseParser, ProviderAdapter, ProviderAdapterError, ProviderExecution,
    ProviderRunContext, ProviderRunResult,
};
use serde_json::{json, Value};
use star_control_state::ArtifactKind;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const LOCAL_OPENAI_COMPATIBLE_KIND: &str = "local_openai_compatible_server";
const HTTP_TRANSPORT: &str = "http";
const OPENAI_COMPATIBLE_ADAPTER: &str = "openai_compatible";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalOpenAiCompatibleServerAdapter;

pub fn is_local_openai_compatible_manifest(manifest: &crate::ProviderManifest) -> bool {
    manifest.kind() == LOCAL_OPENAI_COMPATIBLE_KIND
        && manifest.transport() == HTTP_TRANSPORT
        && manifest.adapter() == OPENAI_COMPATIBLE_ADAPTER
}

impl ProviderAdapter for LocalOpenAiCompatibleServerAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_local_openai_compatible_manifest(manifest) {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }
        let instance = context
            .registry()
            .instance(request.provider_instance_id())
            .ok_or_else(|| crate::ProviderRegistryError::InstanceNotFound {
                instance_id: request.provider_instance_id().to_string(),
            })?;
        let prepared_request = OpenAiCompatibleRequestBuilder
            .build(request, instance)
            .map_err(|source| transport_failed(request, source.to_string()))?;
        let url = LoopbackHttpUrl::parse(prepared_request.url())
            .map_err(|message| transport_failed(request, message))?;
        let output_files = planned_output_files(request.provider_instance_id());
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let request_redaction =
            redact_provider_json_artifact(context, request, REQUEST_FILE, request.value())?;
        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            REQUEST_FILE,
            request_redaction.value(),
        )?;
        context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            HTTP_REQUEST_FILE,
            &prepared_request_value(&prepared_request),
        )?;
        context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDERR_FILE,
            "",
        )?;

        let started_at = Instant::now();
        let raw_response = execute_loopback_http(
            &url,
            &prepared_request,
            timeout_seconds(instance.value(), instance.id())?,
            request.provider_instance_id(),
        )?;
        let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let raw_response_value: Value = serde_json::from_str(&raw_response).map_err(|source| {
            ProviderAdapterError::InvalidJson {
                path: context
                    .state_store()
                    .resolve_job_path(
                        request.job_id(),
                        &provider_output_path(request.provider_instance_id(), RAW_RESPONSE_FILE),
                    )
                    .unwrap_or_else(|_| RAW_RESPONSE_FILE.into()),
                source,
            }
        })?;
        let parsed_response = OpenAiCompatibleResponseParser
            .parse(&raw_response_value)
            .map_err(|source| transport_failed(request, source.to_string()))?;

        context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            RAW_RESPONSE_FILE,
            &raw_response_value,
        )?;
        context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDOUT_FILE,
            &local_stdout_value(manifest, &prepared_request, &parsed_response),
        )?;
        let stdout_path = context.state_store().resolve_job_path(
            request.job_id(),
            &provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        )?;
        let stderr_path = context.state_store().resolve_job_path(
            request.job_id(),
            &provider_output_path(request.provider_instance_id(), STDERR_FILE),
        )?;
        let stdout_redaction =
            redact_provider_text_file_artifact(context, request, STDOUT_FILE, &stdout_path)?;
        let stderr_redaction =
            redact_provider_text_file_artifact(context, request, STDERR_FILE, &stderr_path)?;
        let cost_metric = cost_metric_value_with_response_usage(
            request,
            instance,
            &parsed_response,
            wall_time_ms,
        );
        crate::cloud_io::validate_contract(
            &cost_metric,
            std::path::Path::new(COST_METRIC_FILE),
            context.schema_root(),
            crate::cloud_constants::COST_METRIC_SCHEMA,
        )?;
        let cost_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            COST_METRIC_FILE,
            &cost_metric,
        )?;
        debug_assert_eq!(cost_ref["kind"], "provider_output");

        let redaction_artifacts = [
            request_redaction.report_path().map(ToString::to_string),
            stdout_redaction,
            stderr_redaction,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let response_value = local_response_value(
            request,
            manifest,
            &prepared_request,
            &parsed_response,
            wall_time_ms,
            &redaction_artifacts,
        );
        let response_redaction =
            redact_provider_json_artifact(context, request, RESPONSE_FILE, &response_value)?;
        let result = ProviderRunResult::from_value(
            response_redaction.value().clone(),
            provider_output_path(request.provider_instance_id(), RESPONSE_FILE),
            context.schema_root(),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            RESPONSE_FILE,
            response_redaction.value(),
        )?;
        let stdout_ref = artifact_ref(context, request, STDOUT_FILE)?;
        let stderr_ref = artifact_ref(context, request, STDERR_FILE)?;

        Ok(ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        ))
    }
}

fn planned_output_files(provider_instance_id: &str) -> Vec<String> {
    [
        REQUEST_FILE,
        HTTP_REQUEST_FILE,
        RAW_RESPONSE_FILE,
        STDOUT_FILE,
        STDERR_FILE,
        RESPONSE_FILE,
        COST_METRIC_FILE,
    ]
    .iter()
    .map(|file| provider_output_path(provider_instance_id, file))
    .collect()
}

fn artifact_ref(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
) -> Result<Value, ProviderAdapterError> {
    let path = provider_output_path(request.provider_instance_id(), file_name);
    Ok(context.state_store().artifact_ref(
        request.job_id(),
        &path,
        ArtifactKind::ProviderOutput,
        request.provider_instance_id(),
        None,
        Some("local OpenAI-compatible provider output"),
    )?)
}

fn local_response_value(
    request: &ExecutionRequest,
    manifest: &crate::ProviderManifest,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    parsed_response: &crate::OpenAiCompatibleParsedResponse,
    wall_time_ms: u64,
    redaction_artifacts: &[String],
) -> Value {
    let response_path = provider_output_path(request.provider_instance_id(), RESPONSE_FILE);
    let request_path = provider_output_path(request.provider_instance_id(), REQUEST_FILE);
    let http_request_path = provider_output_path(request.provider_instance_id(), HTTP_REQUEST_FILE);
    let raw_response_path = provider_output_path(request.provider_instance_id(), RAW_RESPONSE_FILE);
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);
    let mut artifacts = vec![
        response_path,
        request_path,
        http_request_path,
        raw_response_path,
        stdout_path.clone(),
        stderr_path.clone(),
        cost_path,
    ];
    artifacts.extend(redaction_artifacts.iter().cloned());

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": "success",
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "summary": parsed_response.text(),
        "changed_files": [],
        "artifacts": artifacts,
        "metrics": {
            "estimated_cost": 0,
            "currency": "USD",
            "input_tokens": parsed_response.input_tokens(),
            "output_tokens": parsed_response.output_tokens(),
            "total_tokens": parsed_response.total_tokens(),
            "wall_time_ms": wall_time_ms,
            "transport": HTTP_TRANSPORT,
            "transport_execution": "loopback_http",
            "request_api": request_api_name(prepared_request.api()),
            "response_kind": response_kind_name(parsed_response.kind()),
            "response_id": parsed_response.response_id(),
            "model": parsed_response.model(),
            "provider_id": manifest.id(),
            "live_api_call": true,
            "loopback_only": true
        },
        "error": Value::Null
    })
}

fn local_stdout_value(
    manifest: &crate::ProviderManifest,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    parsed_response: &crate::OpenAiCompatibleParsedResponse,
) -> String {
    format!(
        "local OpenAI-compatible loopback HTTP\nprovider_id={}\nkind={}\ntransport={}\nrequest_method={}\nrequest_url={}\nresponse_model={}\ntransport_execution=loopback_http\n",
        manifest.id(),
        manifest.kind(),
        manifest.transport(),
        prepared_request.method(),
        prepared_request.url(),
        parsed_response.model().unwrap_or("unknown"),
    )
}

fn request_api_name(api: crate::OpenAiCompatibleRequestApi) -> &'static str {
    match api {
        crate::OpenAiCompatibleRequestApi::Responses => "responses",
        crate::OpenAiCompatibleRequestApi::ChatCompletions => "chat_completions",
    }
}

fn response_kind_name(kind: crate::OpenAiCompatibleResponseKind) -> &'static str {
    match kind {
        crate::OpenAiCompatibleResponseKind::Responses => "responses",
        crate::OpenAiCompatibleResponseKind::ChatCompletions => "chat_completions",
    }
}

fn execute_loopback_http(
    url: &LoopbackHttpUrl,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    timeout_seconds: u64,
    provider_instance_id: &str,
) -> Result<String, ProviderAdapterError> {
    let body = serde_json::to_string(prepared_request.body()).map_err(|source| {
        ProviderAdapterError::TransportFailed {
            provider_instance_id: provider_instance_id.to_string(),
            message: source.to_string(),
        }
    })?;
    let timeout = Duration::from_secs(timeout_seconds.max(1));
    let mut stream = TcpStream::connect((url.host.as_str(), url.port)).map_err(|source| {
        ProviderAdapterError::TransportFailed {
            provider_instance_id: provider_instance_id.to_string(),
            message: source.to_string(),
        }
    })?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();
    let request = format!(
        "{} {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        prepared_request.method(),
        url.path,
        url.host,
        url.port,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).map_err(|source| {
        ProviderAdapterError::TransportFailed {
            provider_instance_id: provider_instance_id.to_string(),
            message: source.to_string(),
        }
    })?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|source| ProviderAdapterError::TransportFailed {
            provider_instance_id: provider_instance_id.to_string(),
            message: source.to_string(),
        })?;
    parse_http_response(&response, provider_instance_id)
}

fn parse_http_response(
    response: &[u8],
    provider_instance_id: &str,
) -> Result<String, ProviderAdapterError> {
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            transport_failed_id(
                provider_instance_id,
                "HTTP response missing header separator",
            )
        })?;
    let headers = String::from_utf8_lossy(&response[..separator]);
    let mut lines = headers.lines();
    let status_line = lines.next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| {
            transport_failed_id(provider_instance_id, "HTTP response missing status code")
        })?;
    let body = &response[separator + 4..];
    let is_chunked = headers.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    });
    let body = if is_chunked {
        decode_chunked_body(body, provider_instance_id)?
    } else {
        body.to_vec()
    };
    let body = String::from_utf8(body)
        .map_err(|source| transport_failed_id(provider_instance_id, source.to_string()))?;
    if !(200..300).contains(&status_code) {
        return Err(transport_failed_id(
            provider_instance_id,
            format!("HTTP status {}: {}", status_code, body),
        ));
    }
    Ok(body)
}

fn decode_chunked_body(
    body: &[u8],
    provider_instance_id: &str,
) -> Result<Vec<u8>, ProviderAdapterError> {
    let mut index = 0;
    let mut decoded = Vec::new();
    loop {
        let Some(remaining) = body.get(index..) else {
            return Err(transport_failed_id(
                provider_instance_id,
                "invalid chunked response",
            ));
        };
        let Some(line_end) = find_crlf(remaining) else {
            return Err(transport_failed_id(
                provider_instance_id,
                "invalid chunked response",
            ));
        };
        let size_text = String::from_utf8_lossy(&body[index..index + line_end]);
        let size_token = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_token, 16)
            .map_err(|source| transport_failed_id(provider_instance_id, source.to_string()))?;
        index += line_end + 2;
        if size == 0 {
            break;
        }
        let chunk_end = index.checked_add(size).ok_or_else(|| {
            transport_failed_id(provider_instance_id, "chunked response size overflow")
        })?;
        if chunk_end > body.len() {
            return Err(transport_failed_id(
                provider_instance_id,
                "truncated chunked response",
            ));
        }
        decoded.extend_from_slice(&body[index..chunk_end]);
        index = chunk_end;
        if body.get(index..index + 2) != Some(b"\r\n") {
            return Err(transport_failed_id(
                provider_instance_id,
                "invalid chunked response terminator",
            ));
        }
        index += 2;
    }
    Ok(decoded)
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopbackHttpUrl {
    host: String,
    port: u16,
    path: String,
}

impl LoopbackHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "local OpenAI-compatible endpoint must use http://".to_string())?;
        let (host_port, path) = rest
            .split_once('/')
            .map(|(host, path)| (host, format!("/{}", path)))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = host_port
            .rsplit_once(':')
            .map(|(host, port)| {
                port.parse::<u16>()
                    .map(|port| (host.to_string(), port))
                    .map_err(|_| "endpoint port must be a number".to_string())
            })
            .unwrap_or_else(|| Ok((host_port.to_string(), 80)))?;
        if host != "127.0.0.1" && host != "localhost" {
            return Err("local OpenAI-compatible endpoint must be loopback-only".to_string());
        }
        Ok(Self { host, port, path })
    }
}

fn transport_failed(request: &ExecutionRequest, message: String) -> ProviderAdapterError {
    transport_failed_id(request.provider_instance_id(), message)
}

fn transport_failed_id(
    provider_instance_id: &str,
    message: impl Into<String>,
) -> ProviderAdapterError {
    ProviderAdapterError::TransportFailed {
        provider_instance_id: provider_instance_id.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use star_control_state::StateStore;
    use std::fs;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;

    #[test]
    fn loopback_url_policy_rejects_non_local_hosts() {
        assert!(LoopbackHttpUrl::parse("http://127.0.0.1:11434/v1/chat/completions").is_ok());
        assert!(LoopbackHttpUrl::parse("http://localhost:11434/v1/chat/completions").is_ok());
        assert!(LoopbackHttpUrl::parse("https://127.0.0.1/v1/chat/completions").is_err());
        assert!(LoopbackHttpUrl::parse("http://api.example.com/v1/chat/completions").is_err());
    }

    #[test]
    fn adapter_executes_loopback_openai_compatible_chat_completion() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock local server");
        let port = listener.local_addr().expect("local addr").port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).expect("read request");
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.contains("POST /v1/chat/completions HTTP/1.1"));
            assert!(request.contains("\"model\":\"mock-local\""));
            let body = r#"{"id":"chatcmpl-local","model":"mock-local","choices":[{"message":{"content":"local answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let project = temp_project();
        let schemas = schema_root();
        let store = StateStore::open(&project, &schemas).expect("open store");
        store
            .create_job("use local server", "codex", vec![])
            .expect("create job");
        let registry = registry_with_local_instance(port);
        let request = ExecutionRequest::from_value(request_value(), "request.json", &schemas)
            .expect("request");
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        let execution = LocalOpenAiCompatibleServerAdapter
            .execute(&request, &context)
            .expect("execute local provider");

        assert_eq!(execution.result().status(), "success");
        assert_eq!(execution.result().value()["summary"], "local answer");
        assert_eq!(
            execution.result().value()["metrics"]["transport_execution"],
            "loopback_http"
        );
        assert_eq!(execution.result().value()["metrics"]["input_tokens"], 3);
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/raw-response.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/cost-metric.json")
            .is_file());
        server.join().expect("server thread");
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn parses_chunked_http_response() {
        let response =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nlocal\r\n0\r\n\r\n";

        let body = parse_http_response(response, "local-default").expect("parse chunked response");

        assert_eq!(body, "local");
    }

    #[test]
    fn rejects_truncated_chunked_response_without_panic() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nlocal";

        let error = parse_http_response(response, "local-default").expect_err("reject response");

        assert!(error.to_string().contains("terminator"));
    }

    fn registry_with_local_instance(port: u16) -> crate::ProviderRegistry {
        let mut registry = crate::ProviderRegistry::new();
        registry
            .register_manifest(crate::ProviderManifest {
                id: "provider.local".to_string(),
                kind: LOCAL_OPENAI_COMPATIBLE_KIND.to_string(),
                transport: HTTP_TRANSPORT.to_string(),
                adapter: OPENAI_COMPATIBLE_ADAPTER.to_string(),
                path: PathBuf::from("provider.local.json"),
                value: json!({
                    "id": "provider.local",
                    "kind": LOCAL_OPENAI_COMPATIBLE_KIND,
                    "transport": HTTP_TRANSPORT,
                    "adapter": OPENAI_COMPATIBLE_ADAPTER
                }),
            })
            .expect("register manifest");
        registry
            .register_instance(crate::ProviderInstance {
                id: "local-default".to_string(),
                provider_id: "provider.local".to_string(),
                enabled: true,
                routing_tags: vec!["local".to_string()],
                path: PathBuf::from("local-default.json"),
                value: json!({
                    "id": "local-default",
                    "provider": "provider.local",
                    "enabled": true,
                    "limits": {
                        "timeout_seconds": 5,
                        "max_parallel_jobs": 1
                    },
                    "routing_tags": ["local", "http"],
                    "endpoint": {
                        "base_url": format!("http://127.0.0.1:{}/v1", port),
                        "model": "mock-local",
                        "api": "chat_completions"
                    }
                }),
            })
            .expect("register instance");
        registry
    }

    fn request_value() -> Value {
        json!({
            "schema_version": "1.0.0",
            "request_id": "request-0001",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "local-default",
            "attempt_id": "attempt-0001",
            "workspec_path": "workspecs/implement.json",
            "created_at": "2026-06-28T00:00:00Z",
            "goal": "run local provider",
            "allowed_scope": ["src/**", "tests/**"],
            "forbidden_actions": ["dependency_install", "file_delete"],
            "required_outputs": ["provider-output/local-default/response.json"],
            "validation_requirements": ["policy:p0"],
            "context_pack": { "files": [] }
        })
    }

    fn schema_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs/schemas")
    }

    fn temp_project() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "star-control-local-provider-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }
}
