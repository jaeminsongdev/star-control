use std::sync::Arc;

use star_contracts::{
    fixed_mcp::{McpResultStatus, McpToolResult},
    ids::RequestId,
    ipc::{ErrorEnvelope, IpcResponse, IpcStatus},
};
use star_ipc::client::{ControllerClient, ControllerClientError, mcp_client_config};
use star_ipc::controller_start::{ControllerStartError, VerifiedControllerImage};
use star_mcp::{ControllerGateway, ControllerProgress, Gateway, GatewayError};

struct IpcControllerGateway {
    client: ControllerClient,
    gateway_path: std::path::PathBuf,
}
impl ControllerGateway for IpcControllerGateway {
    fn call<'a>(
        &'a self,
        command: &str,
        tool: &str,
        arguments: serde_json::Value,
        correlation_id: RequestId,
        progress: Option<tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>,
    > {
        let command = command.to_owned();
        let tool = tool.to_owned();
        Box::pin(async move {
            let poll_accepted = should_poll_accepted_operation(&command, &arguments);
            let bootstrap = VerifiedControllerImage::from_install_manifest(&self.gateway_path)
                .map_err(map_bootstrap_error)?;
            let mut response = self
                .client
                .call_with_verified_start_and_mcp_tool(
                    &bootstrap,
                    &command,
                    arguments,
                    correlation_id.clone(),
                    Some(&tool),
                )
                .await
                .map_err(map_client_error)?;
            if poll_accepted
                && response.status == IpcStatus::Accepted
                && let (Some(operation_id), Some(accepted_data)) =
                    (response.operation_id.clone(), response.data.clone())
            {
                let started = std::time::Instant::now();
                let mut after_sequence = 0_u64;
                while started.elapsed() < std::time::Duration::from_secs(30) {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    let polled = self
                        .client
                        .call_with_verified_start_and_mcp_tool(
                            &bootstrap,
                            "operation.get",
                            serde_json::json!({
                                "operation_id":operation_id.clone(),
                                "after_sequence":after_sequence,
                                "wait_ms":0
                            }),
                            RequestId::new(),
                            Some("star_tool_operation_get"),
                        )
                        .await
                        .map_err(map_client_error)?;
                    if polled.status != IpcStatus::Ok {
                        response = polled;
                        break;
                    }
                    if let Some(data) = polled.data.as_ref() {
                        after_sequence =
                            emit_operation_progress(data, after_sequence, progress.as_ref());
                        if let Some(result) = terminal_invocation_result(
                            &tool,
                            &response.correlation_id,
                            accepted_data.clone(),
                            data,
                        )? {
                            return Ok(result);
                        }
                    }
                }
            }
            ipc_response_to_mcp(response, &tool, &command)
        })
    }

    fn cancel<'a>(
        &'a self,
        correlation_id: RequestId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), GatewayError>> + Send + 'a>>
    {
        Box::pin(async move {
            let bootstrap = VerifiedControllerImage::from_install_manifest(&self.gateway_path)
                .map_err(map_bootstrap_error)?;
            self.client
                .call_with_verified_start(
                    &bootstrap,
                    "request.cancel",
                    serde_json::json!({"client_request_id":correlation_id}),
                    RequestId::new(),
                )
                .await
                .map_err(map_client_error)?;
            Ok(())
        })
    }
}

fn should_poll_accepted_operation(command: &str, arguments: &serde_json::Value) -> bool {
    command == "tool.invoke"
        && arguments
            .get("wait_mode")
            .and_then(serde_json::Value::as_str)
            != Some("accepted")
}

fn emit_operation_progress(
    data: &serde_json::Value,
    after_sequence: u64,
    sender: Option<&tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
) -> u64 {
    let mut latest = after_sequence;
    let Some(sender) = sender else {
        return latest;
    };
    for event in data
        .get("progress")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let sequence = event
            .get("sequence")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(latest);
        if sequence <= latest {
            continue;
        }
        latest = sequence;
        let phase = event
            .get("phase")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("running");
        let detail = event
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(phase);
        let (value, message) = if phase == "progress" {
            let parsed = serde_json::from_str::<serde_json::Value>(detail).ok();
            let progress = parsed
                .as_ref()
                .and_then(|value| value.get("progress"))
                .and_then(serde_json::Value::as_f64);
            let total = parsed
                .as_ref()
                .and_then(|value| value.get("total"))
                .and_then(serde_json::Value::as_f64);
            let ratio = progress
                .zip(total)
                .filter(|(_, total)| *total > 0.0)
                .map_or(0.0, |(progress, total)| (progress / total).clamp(0.0, 1.0));
            (
                0.2 + ratio * 0.75,
                parsed
                    .as_ref()
                    .and_then(|value| value.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("External tool progress")
                    .to_owned(),
            )
        } else {
            let value = match phase {
                "received" => 0.02,
                "resolving" | "approval_wait" => 0.05,
                "queued" => 0.1,
                "starting" => 0.15,
                "running" => 0.2,
                "cancelling" => 0.9,
                _ => 0.2,
            };
            (value, format!("Controller phase: {phase}"))
        };
        let _ = sender.send(ControllerProgress {
            progress: value,
            total: Some(1.0),
            message,
        });
    }
    latest
}

fn terminal_invocation_result(
    tool: &str,
    correlation_id: &str,
    mut accepted_data: serde_json::Value,
    operation_data: &serde_json::Value,
) -> Result<Option<McpToolResult>, GatewayError> {
    let operation = operation_data
        .get("operation")
        .and_then(serde_json::Value::as_object)
        .ok_or(GatewayError::ProtocolMismatch)?;
    let status = operation
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or(GatewayError::ProtocolMismatch)?;
    if !matches!(
        status,
        "succeeded" | "failed" | "cancelled" | "denied" | "outcome_unknown"
    ) {
        return Ok(None);
    }
    let correlation_id =
        RequestId::parse(correlation_id.to_owned()).map_err(|_| GatewayError::ProtocolMismatch)?;
    if status == "succeeded" {
        let object = accepted_data
            .as_object_mut()
            .ok_or(GatewayError::ProtocolMismatch)?;
        object.remove("operation");
        object.insert(
            "result".to_owned(),
            operation
                .get("result")
                .filter(|value| !value.is_null())
                .cloned()
                .ok_or(GatewayError::ProtocolMismatch)?,
        );
        return Ok(Some(McpToolResult {
            schema_id: format!("star.mcp.{tool}.result"),
            schema_version: 1,
            status: McpResultStatus::Ok,
            summary: "Controller completed tool.invoke.".to_owned(),
            data: Some(accepted_data),
            operation_id: None,
            next_actions: vec![],
            artifact_refs: vec![],
            diagnostic_refs: vec![],
            error: None,
            correlation_id,
        }));
    }
    let envelope = operation
        .get("error")
        .filter(|error| !error.is_null())
        .cloned()
        .map(serde_json::from_value::<ErrorEnvelope>)
        .transpose()
        .map_err(|_| GatewayError::ProtocolMismatch)?
        .unwrap_or_else(|| {
            ErrorEnvelope::new(
                match status {
                    "cancelled" => "TOOL_CANCELLED",
                    "denied" => "POLICY_DENIED",
                    "outcome_unknown" => "TOOL_OUTCOME_UNKNOWN",
                    _ => "TOOL_OPERATION_FAILED",
                },
                "The durable external-tool Operation did not succeed.",
                false,
                correlation_id.to_string(),
                "star-mcp",
            )
        });
    Ok(Some(McpToolResult {
        schema_id: format!("star.mcp.{tool}.result"),
        schema_version: 1,
        status: McpResultStatus::Error,
        summary: envelope.message.clone(),
        data: None,
        operation_id: None,
        next_actions: vec![],
        artifact_refs: vec![],
        diagnostic_refs: vec![],
        error: Some(envelope),
        correlation_id,
    }))
}

fn ipc_response_to_mcp(
    response: IpcResponse,
    tool: &str,
    command: &str,
) -> Result<McpToolResult, GatewayError> {
    let correlation_id = RequestId::parse(response.correlation_id.clone())
        .map_err(|_| GatewayError::ProtocolMismatch)?;
    if !ipc_response_semantics_valid(&response) {
        return Err(GatewayError::ProtocolMismatch);
    }
    let status = match response.status {
        IpcStatus::Ok => McpResultStatus::Ok,
        IpcStatus::Accepted => McpResultStatus::Accepted,
        IpcStatus::QuestionRequired => McpResultStatus::QuestionRequired,
        IpcStatus::ApprovalRequired => McpResultStatus::ApprovalRequired,
        IpcStatus::Blocked => McpResultStatus::Blocked,
        IpcStatus::Error => McpResultStatus::Error,
    };
    let next_actions = if matches!(&status, McpResultStatus::ApprovalRequired) {
        response
            .data
            .as_ref()
            .and_then(|data| data.get("approval_request"))
            .and_then(|approval| {
                Some(serde_json::json!({
                    "tool_name":"star_approval_resolve",
                    "reason":"Resolve the exact durable approval scope before dispatch.",
                    "arguments":{
                        "approval_id":approval.get("approval_id")?.clone(),
                        "scope_hash":approval.get("scope_hash")?.clone()
                    }
                }))
            })
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    let operation_id = matches!(
        &status,
        McpResultStatus::Accepted | McpResultStatus::ApprovalRequired
    )
    .then_some(response.operation_id)
    .flatten();
    let data = (!matches!(&status, McpResultStatus::Error))
        .then_some(response.data)
        .flatten()
        .map(redact_mcp_operation_data);
    Ok(McpToolResult {
        schema_id: format!("star.mcp.{tool}.result"),
        schema_version: 1,
        status,
        summary: response
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .unwrap_or_else(|| format!("Controller completed {command}.")),
        data,
        operation_id,
        next_actions,
        artifact_refs: vec![],
        diagnostic_refs: response.diagnostics,
        error: response.error,
        correlation_id,
    })
}

fn redact_mcp_operation_data(mut data: serde_json::Value) -> serde_json::Value {
    let Some(object) = data.as_object_mut() else {
        return data;
    };
    let Some(operation) = object
        .get("operation")
        .and_then(serde_json::Value::as_object)
    else {
        return data;
    };
    let mut view = serde_json::Map::new();
    for field in [
        "operation_id",
        "command",
        "correlation_id",
        "goal_id",
        "run_id",
        "stage_id",
        "status",
        "accepted_at",
        "started_at",
        "updated_at",
        "finished_at",
        "cancellable",
        "output_provenance",
        "result",
        "error",
        "latest_event_sequence",
    ] {
        if let Some(value) = operation.get(field) {
            view.insert(field.to_owned(), value.clone());
        }
    }
    if let Some(phase) = operation
        .get("events")
        .and_then(serde_json::Value::as_array)
        .and_then(|events| events.last())
        .and_then(|event| event.get("phase"))
        .and_then(serde_json::Value::as_str)
    {
        view.insert("current_phase".to_owned(), phase.into());
    }
    object.insert("operation".to_owned(), serde_json::Value::Object(view));
    if let Some(progress) = object
        .get_mut("progress")
        .and_then(serde_json::Value::as_array_mut)
    {
        for event in progress {
            let Some(source) = event.as_object() else {
                continue;
            };
            let mut redacted = serde_json::Map::new();
            for field in ["sequence", "timestamp", "phase"] {
                if let Some(value) = source.get(field) {
                    redacted.insert(field.to_owned(), value.clone());
                }
            }
            *event = serde_json::Value::Object(redacted);
        }
    }
    data
}

fn ipc_response_semantics_valid(response: &IpcResponse) -> bool {
    match response.status {
        // Some typed IPC reads return the queried OperationId for transport
        // correlation. The fixed MCP projection intentionally drops it for
        // status=ok, whose public invariant forbids operation_id.
        IpcStatus::Ok => response.error.is_none(),
        IpcStatus::Accepted => {
            response.operation_id.is_some()
                && response.error.is_none()
                && response
                    .data
                    .as_ref()
                    .and_then(|data| data.get("operation"))
                    .is_some_and(serde_json::Value::is_object)
        }
        IpcStatus::ApprovalRequired => {
            response.operation_id.is_some()
                && response.error.is_none()
                && response
                    .data
                    .as_ref()
                    .and_then(|data| data.get("operation"))
                    .is_some_and(serde_json::Value::is_object)
                && response
                    .data
                    .as_ref()
                    .and_then(|data| data.get("approval_request"))
                    .is_some_and(|approval| {
                        approval.is_object()
                            && approval
                                .get("approval_id")
                                .is_some_and(serde_json::Value::is_string)
                            && approval
                                .get("scope_hash")
                                .is_some_and(serde_json::Value::is_string)
                    })
        }
        // No typed question/core-answer bridge is registered in this bounded
        // implementation. Forwarding an untyped question would violate the
        // fixed MCP result contract, so fail closed until that owner exists.
        IpcStatus::QuestionRequired => false,
        IpcStatus::Blocked => {
            response.operation_id.is_none()
                && (response.error.is_some()
                    || !response.diagnostics.is_empty()
                    || response.data.as_ref().is_some_and(|data| {
                        data.get("policy_basis").is_some() || data.get("diagnostic").is_some()
                    }))
        }
        IpcStatus::Error => response.operation_id.is_none() && response.error.is_some(),
    }
}

fn map_client_error(error: ControllerClientError) -> GatewayError {
    match error {
        ControllerClientError::Unavailable => GatewayError::Controller(error.to_string()),
        ControllerClientError::Authentication | ControllerClientError::MalformedResponse => {
            GatewayError::Authentication
        }
        ControllerClientError::ServerIdentityMismatch => GatewayError::ServerIdentityMismatch,
        ControllerClientError::ProtocolMismatch => GatewayError::ProtocolMismatch,
    }
}

fn map_bootstrap_error(error: ControllerStartError) -> GatewayError {
    match error {
        ControllerStartError::OuterJobDenied | ControllerStartError::Start => {
            GatewayError::Controller("IPC controller unavailable".to_owned())
        }
        ControllerStartError::IdentityMismatch
        | ControllerStartError::Lease(_)
        | ControllerStartError::InstallManifest => GatewayError::ServerIdentityMismatch,
    }
}

fn install_jsonl_panic_hook() {
    std::panic::set_hook(Box::new(|_| {
        write_stderr_log(
            "error",
            "panic",
            "Gateway terminated after an internal panic.",
            serde_json::Map::new(),
        );
    }));
}

fn write_stderr_log(
    level: &str,
    event: &str,
    message: &str,
    context: serde_json::Map<String, serde_json::Value>,
) {
    let record = serde_json::json!({
        "timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "level":level,
        "component":"star-mcp",
        "event":event,
        "correlation_id":serde_json::Value::Null,
        "message":message,
        "context":context,
    });
    eprintln!("{record}");
}

#[tokio::main]
async fn main() {
    install_jsonl_panic_hook();
    if run().await.is_err() {
        write_stderr_log(
            "error",
            "gateway_terminated",
            "Gateway terminated after a startup or transport error.",
            serde_json::Map::new(),
        );
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let binary = std::env::current_exe()?;
    let controller = binary
        .parent()
        .ok_or("star-mcp executable has no installation directory")?
        .join("star-controller.exe");
    let config = mcp_client_config(controller)
        .map_err(|error| format!("cannot prepare Controller IPC client: {error}"))?;
    let server = Gateway::new(Arc::new(IpcControllerGateway {
        client: ControllerClient::new(config),
        gateway_path: binary,
    }));
    star_mcp::serve_supervised_stdio(server).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_operation_preserves_the_complete_controller_error_envelope() {
        let correlation = RequestId::new();
        let mut expected = ErrorEnvelope::new(
            "TOOL_PROTOCOL_INVALID",
            "adapter response was invalid",
            false,
            correlation.to_string(),
            "star-controller",
        );
        expected.retry_after_ms = Some(125);
        expected.user_action = Some(serde_json::json!({"kind":"inspect_diagnostic"}));
        expected.context.insert(
            "operation_id".to_owned(),
            serde_json::json!("op_01J00000000000000000000000"),
        );
        expected.caused_by = Some(star_contracts::ipc::ErrorRef {
            code: "TOOL_CHILD_EXIT".to_owned(),
            summary: "child failed".to_owned(),
        });
        expected.artifact_refs.push(serde_json::json!({
            "artifact_id":"art_01J00000000000000000000000"
        }));
        let result = terminal_invocation_result(
            "star_tool_call_write_open",
            correlation.as_str(),
            serde_json::json!({"operation":{}}),
            &serde_json::json!({
                "operation":{
                    "status":"failed",
                    "error":expected
                }
            }),
        )
        .unwrap()
        .expect("terminal error produces a result");
        let actual = result.error.expect("error envelope is present");
        assert_eq!(actual.component, "star-controller");
        assert_eq!(actual.retry_after_ms, Some(125));
        assert_eq!(actual.user_action, expected.user_action);
        assert_eq!(actual.context, expected.context);
        assert_eq!(actual.caused_by.unwrap().code, "TOOL_CHILD_EXIT");
        assert_eq!(actual.artifact_refs, expected.artifact_refs);
    }

    #[test]
    fn gateway_stderr_record_has_the_frozen_jsonl_shape_and_redacted_context() {
        let record = serde_json::json!({
            "timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "level":"error",
            "component":"star-mcp",
            "event":"gateway_terminated",
            "correlation_id":serde_json::Value::Null,
            "message":"Gateway terminated after a startup or transport error.",
            "context":serde_json::Map::<String, serde_json::Value>::new(),
        });
        let line = serde_json::to_string(&record).unwrap();
        assert!(line.len() < 64 * 1024);
        assert!(!line.contains('\n'));
        assert_eq!(record.as_object().unwrap().len(), 7);
        assert_eq!(record["component"], "star-mcp");
    }

    #[test]
    fn gateway_owns_the_sync_budget_after_controller_async_dispatch() {
        assert!(should_poll_accepted_operation(
            "tool.invoke",
            &serde_json::json!({"wait_mode":"sync"}),
        ));
        assert!(should_poll_accepted_operation(
            "tool.invoke",
            &serde_json::json!({"wait_mode":"auto"}),
        ));
        assert!(!should_poll_accepted_operation(
            "tool.invoke",
            &serde_json::json!({"wait_mode":"accepted"}),
        ));
        assert!(!should_poll_accepted_operation(
            "tool.search",
            &serde_json::json!({}),
        ));
    }

    #[test]
    fn malformed_controller_status_invariants_never_reach_mcp_structured_content() {
        let correlation_id = RequestId::new();
        let request_id = RequestId::new();
        let accepted_without_operation = IpcResponse {
            schema_id: "star.ipc.response".to_owned(),
            schema_version: 1,
            request_id: request_id.clone(),
            status: IpcStatus::Accepted,
            data: Some(serde_json::json!({"operation":{}})),
            operation_id: None,
            diagnostics: vec![],
            error: None,
            registry_revision: Some(1),
            correlation_id: correlation_id.to_string(),
        };
        assert!(
            ipc_response_to_mcp(
                accepted_without_operation,
                "star_tool_call_read_closed",
                "tool.invoke"
            )
            .is_err()
        );

        let approval_without_scope = IpcResponse {
            schema_id: "star.ipc.response".to_owned(),
            schema_version: 1,
            request_id: request_id.clone(),
            status: IpcStatus::ApprovalRequired,
            data: Some(serde_json::json!({"approval_request":{}})),
            operation_id: Some(star_contracts::OperationId::new()),
            diagnostics: vec![],
            error: None,
            registry_revision: Some(1),
            correlation_id: correlation_id.to_string(),
        };
        assert!(
            ipc_response_to_mcp(
                approval_without_scope,
                "star_tool_call_write_open",
                "tool.invoke"
            )
            .is_err()
        );

        let ok_with_error = IpcResponse {
            schema_id: "star.ipc.response".to_owned(),
            schema_version: 1,
            request_id,
            status: IpcStatus::Ok,
            data: Some(serde_json::json!({})),
            operation_id: None,
            diagnostics: vec![],
            error: Some(ErrorEnvelope::new(
                "INTERNAL_INVARIANT_BROKEN",
                "invalid mixed response",
                false,
                correlation_id.to_string(),
                "test",
            )),
            registry_revision: Some(1),
            correlation_id: correlation_id.to_string(),
        };
        assert!(ipc_response_to_mcp(ok_with_error, "star_tool_search", "tool.search").is_err());
    }

    #[test]
    fn operation_results_drop_process_and_file_identity_internals_at_the_mcp_boundary() {
        let correlation_id = RequestId::new();
        let operation_id = star_contracts::OperationId::new();
        let result = ipc_response_to_mcp(
            IpcResponse {
                schema_id: "star.ipc.response".to_owned(),
                schema_version: 1,
                request_id: RequestId::new(),
                status: IpcStatus::Ok,
                data: Some(serde_json::json!({
                    "operation":{
                        "operation_id":operation_id,
                        "command":"tool.invoke",
                        "correlation_id":correlation_id,
                        "status":"running",
                        "accepted_at":"2026-07-12T00:00:00.000Z",
                        "started_at":"2026-07-12T00:00:01.000Z",
                        "updated_at":"2026-07-12T00:00:02.000Z",
                        "finished_at":null,
                        "cancellable":true,
                        "result":null,
                        "error":null,
                        "latest_event_sequence":2,
                        "process_id":42,
                        "process_creation_time_100ns":123,
                        "job_id":"internal-job",
                        "executable_identity":{"identity":{"file_id":"internal-file"}},
                        "events":[
                            {"sequence":1,"phase":"starting","detail":"internal"},
                            {"sequence":2,"phase":"running","detail":"internal"}
                        ]
                    },
                    "progress":[{"sequence":2,"timestamp":"2026-07-12T00:00:02.000Z","phase":"running","detail":"private detail"}],
                    "next_after_sequence":2,
                    "has_more":false,
                    "wait_timed_out":false
                })),
                operation_id: Some(operation_id),
                diagnostics: vec![],
                error: None,
                registry_revision: Some(1),
                correlation_id: correlation_id.to_string(),
            },
            "star_tool_operation_get",
            "operation.get",
        )
        .unwrap();
        let data = result.data.unwrap();
        let operation = &data["operation"];
        assert_eq!(operation["current_phase"], "running");
        assert!(data["progress"][0].get("detail").is_none());
        for internal in [
            "process_id",
            "process_creation_time_100ns",
            "job_id",
            "executable_identity",
            "events",
        ] {
            assert!(
                operation.get(internal).is_none(),
                "{internal} leaked to MCP"
            );
        }
    }
}
