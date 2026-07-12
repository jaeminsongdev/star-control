//! Fixed, stateless MCP gateway.
//!
//! This binary never reads TOML, discovers executable paths, or owns Registry
//! state.  Every fixed tool call is normalized into one Controller IPC command.

use std::{future::Future, io, pin::Pin, sync::Arc};

use tokio::io::AsyncBufReadExt;

use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ListToolsResult, PaginatedRequestParams,
        ProgressNotificationParam, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use star_contracts::{
    fixed_mcp::{
        ApprovalResolveInput, CallInput, DescribeInput, FIXED_TOOLS, McpResultStatus,
        McpToolResult, OperationCancelInput, OperationGetInput, RegistryStatusInput,
        SERVER_DESCRIPTION, SERVER_INSTRUCTIONS, SERVER_NAME, SERVER_TITLE, SearchInput,
        fixed_input_schema, fixed_result_schema, ipc_command,
    },
    ids::RequestId,
    ipc::ErrorEnvelope,
    parse_no_duplicate_keys,
};
use thiserror::Error;

const SUPERVISED_STDIO_MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
const SUPERVISED_STDIO_MAX_OUTBOUND_BYTES: usize = SUPERVISED_STDIO_MAX_LINE_BYTES;
const MCP_OUTBOUND_INLINE_MAX_BYTES: usize = 4 * 1024 * 1024;
const MCP_RESULT_MAX_STRUCTURED_BYTES: usize = MCP_OUTBOUND_INLINE_MAX_BYTES - 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SupervisorState {
    #[default]
    AwaitInitialize,
    AwaitInitializedNotification,
    Ready,
}

#[derive(Debug, Default)]
struct ProgressLimiter {
    last_value: Option<f64>,
    last_emitted_at: Option<std::time::Instant>,
    completed: bool,
}

impl ProgressLimiter {
    fn should_emit(&mut self, value: f64, now: std::time::Instant) -> bool {
        if self.completed
            || self.last_value.is_some_and(|last| value < last)
            || self.last_emitted_at.is_some_and(|last| {
                now.duration_since(last) < std::time::Duration::from_millis(250)
            })
        {
            return false;
        }
        self.last_value = Some(value);
        self.last_emitted_at = Some(now);
        true
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Controller IPC unavailable: {0}")]
    Controller(String),
    #[error("Controller IPC authentication failed")]
    Authentication,
    #[error("Controller IPC server identity mismatch")]
    ServerIdentityMismatch,
    #[error("Controller IPC protocol mismatch")]
    ProtocolMismatch,
    #[error("MCP request was cancelled")]
    Cancelled,
}
impl GatewayError {
    pub fn ipc_code(&self) -> &'static str {
        match self {
            Self::Controller(_) => "IPC_CONTROLLER_UNAVAILABLE",
            Self::Authentication => "IPC_AUTH_FAILED",
            Self::ServerIdentityMismatch => "IPC_SERVER_IDENTITY_MISMATCH",
            Self::ProtocolMismatch => "IPC_PROTOCOL_MISMATCH",
            Self::Cancelled => "TOOL_CANCELLED",
        }
    }
    pub fn retryable(&self) -> bool {
        matches!(self, Self::Controller(_))
    }
}

#[derive(Clone, Debug)]
pub struct ControllerProgress {
    pub progress: f64,
    pub total: Option<f64>,
    pub message: String,
}

pub trait ControllerGateway: Send + Sync + 'static {
    fn call<'a>(
        &'a self,
        command: &str,
        mcp_tool: &str,
        arguments: serde_json::Value,
        correlation_id: RequestId,
        progress: Option<tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
    ) -> Pin<Box<dyn Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>>;

    fn cancel<'a>(
        &'a self,
        _correlation_id: RequestId,
    ) -> Pin<Box<dyn Future<Output = Result<(), GatewayError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct Gateway {
    controller: Arc<dyn ControllerGateway>,
    tool_router: ToolRouter<Self>,
}
impl Gateway {
    pub fn new(controller: Arc<dyn ControllerGateway>) -> Self {
        Self {
            controller,
            tool_router: Self::tool_router(),
        }
    }
    async fn dispatch(
        &self,
        mcp_tool: &str,
        mut arguments: serde_json::Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> CallToolResult {
        let command = ipc_command(mcp_tool).expect("only fixed gateway methods dispatch");
        let correlation_id = correlation_id_from_arguments(&arguments);
        let pending_sync = mcp_tool.starts_with("star_tool_call_")
            && arguments
                .get("wait_mode")
                .and_then(serde_json::Value::as_str)
                != Some("accepted");
        let progress_token = pending_sync
            .then(|| {
                context
                    .as_ref()
                    .and_then(|context| context.meta.get_progress_token())
            })
            .flatten();
        if command == "tool.invoke"
            && let Some(payload) = arguments.as_object_mut()
        {
            payload.insert(
                "mcp_tool_name".to_owned(),
                serde_json::Value::String(mcp_tool.to_owned()),
            );
            payload.insert(
                "mcp_risk_lane".to_owned(),
                serde_json::Value::String(
                    mcp_tool
                        .strip_prefix("star_tool_call_")
                        .unwrap_or_default()
                        .to_owned(),
                ),
            );
            payload.insert(
                "mcp_request_id".to_owned(),
                context
                    .as_ref()
                    .and_then(|context| serde_json::to_value(&context.id).ok())
                    .unwrap_or(serde_json::Value::Null),
            );
            payload.insert(
                "progress_requested".to_owned(),
                serde_json::Value::Bool(progress_token.is_some()),
            );
            payload.insert(
                "client_info".to_owned(),
                context
                    .as_ref()
                    .and_then(|context| context.peer.peer_info())
                    .as_deref()
                    .and_then(|info| serde_json::to_value(info).ok())
                    .unwrap_or_else(|| serde_json::json!({"name":"unknown","version":"unknown"})),
            );
        }
        let mut progress = ProgressLimiter::default();
        if let (Some(context), Some(token)) = (&context, progress_token.clone()) {
            if progress.should_emit(0.0, std::time::Instant::now()) {
                let _ = context
                    .peer
                    .notify_progress(
                        ProgressNotificationParam::new(token, 0.0)
                            .with_total(1.0)
                            .with_message("Controller dispatch started"),
                    )
                    .await;
            }
        }
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<ControllerProgress>();
        let mut controller_call = self.controller.call(
            command,
            mcp_tool,
            arguments,
            correlation_id.clone(),
            progress_token.as_ref().map(|_| progress_sender),
        );
        let outcome = if let Some(context) = &context {
            let completed = loop {
                tokio::select! {
                    result = &mut controller_call => break Some(result),
                    update = progress_receiver.recv(), if progress_token.is_some() => {
                        if let (Some(update), Some(token)) = (update, progress_token.clone())
                            && progress.should_emit(update.progress, std::time::Instant::now())
                        {
                            let mut notification = ProgressNotificationParam::new(token, update.progress)
                                .with_message(update.message);
                            if let Some(total) = update.total {
                                notification = notification.with_total(total);
                            }
                            let _ = context.peer.notify_progress(notification).await;
                        }
                    }
                    () = context.ct.cancelled() => break None,
                }
            };
            match completed {
                Some(result) => result,
                None => {
                    // Close the in-flight request pipe before opening the
                    // cancellation request. The single-instance Controller
                    // can then observe EOF, release the handler, and accept
                    // the idempotent cancellation command without deadlock.
                    drop(controller_call);
                    let _ = self.controller.cancel(correlation_id.clone()).await;
                    Err(GatewayError::Cancelled)
                }
            }
        } else {
            controller_call.await
        };
        if let (Some(context), Some(token)) = (&context, progress_token) {
            if progress.should_emit(1.0, std::time::Instant::now()) {
                let _ = context
                    .peer
                    .notify_progress(
                        ProgressNotificationParam::new(token, 1.0)
                            .with_total(1.0)
                            .with_message("Controller dispatch completed"),
                    )
                    .await;
            }
        }
        progress.complete();
        let result = match outcome {
            Ok(result) => result,
            Err(error) => {
                let envelope = ErrorEnvelope::new(
                    error.ipc_code(),
                    error.to_string(),
                    error.retryable(),
                    correlation_id.to_string(),
                    "star-mcp",
                );
                McpToolResult {
                    schema_id: format!("star.mcp.{mcp_tool}.result"),
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
                }
            }
        };
        mcp_call_result(result)
    }

    fn fixed_tools_in_contract_order(&self) -> Vec<rmcp::model::Tool> {
        let unordered = self.tool_router.list_all();
        star_contracts::fixed_mcp::FIXED_TOOLS
            .iter()
            .map(|fixed| {
                let mut tool = unordered
                    .iter()
                    .find(|tool| tool.name == fixed.name)
                    .expect("fixed tool registered by rmcp")
                    .clone();
                tool.title = Some(fixed.title.to_owned());
                tool.description = Some(fixed.description.into());
                tool.input_schema = Arc::new(
                    fixed_input_schema(fixed.name)
                        .and_then(|schema| schema.as_object().cloned())
                        .expect("fixed MCP input schema is an object"),
                );
                tool.output_schema = Some(Arc::new(
                    fixed_result_schema(fixed.name)
                        .and_then(|schema| schema.as_object().cloned())
                        .expect("fixed MCP result schema is an object"),
                ));
                tool.annotations = Some(rmcp::model::ToolAnnotations::from_raw(
                    None,
                    Some(fixed.read_only),
                    Some(fixed.destructive),
                    Some(fixed.idempotent),
                    Some(fixed.open_world),
                ));
                tool.execution = None;
                tool.icons = None;
                tool.meta = None;
                tool
            })
            .collect()
    }
}

fn correlation_id_from_arguments(arguments: &serde_json::Value) -> RequestId {
    arguments
        .get("client_request_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| RequestId::parse(value.to_owned()).ok())
        .unwrap_or_default()
}

fn mcp_call_result(result: McpToolResult) -> CallToolResult {
    let result = bound_mcp_result(result);
    let is_error = matches!(&result.status, McpResultStatus::Error);
    let status = serde_json::to_value(&result.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "error".to_owned());
    let mut summary = format!("[{status}] {}", result.summary);
    if let Some(operation_id) = &result.operation_id {
        summary.push_str(&format!(" operation_id={operation_id}"));
    }
    summary.push_str(&format!(" correlation_id={}", result.correlation_id));
    summary = summary.chars().take(2_000).collect();
    let structured = serde_json::to_value(result).expect("MCP result serializes");
    let mut response = if is_error {
        CallToolResult::structured_error(structured)
    } else {
        CallToolResult::structured(structured)
    };
    response.content = vec![ContentBlock::text(summary)];
    response
}

fn bound_mcp_result(result: McpToolResult) -> McpToolResult {
    if serde_json::to_vec(&result).is_ok_and(|bytes| bytes.len() <= MCP_RESULT_MAX_STRUCTURED_BYTES)
    {
        return result;
    }
    let mut envelope = ErrorEnvelope::new(
        "TOOL_OUTPUT_LIMIT",
        "Controller result exceeds the MCP outbound inline limit.",
        false,
        result.correlation_id.to_string(),
        "star-mcp",
    );
    envelope.context.insert(
        "outbound_inline_limit_bytes".to_owned(),
        serde_json::json!(MCP_OUTBOUND_INLINE_MAX_BYTES),
    );
    McpToolResult {
        schema_id: result.schema_id,
        schema_version: result.schema_version,
        status: McpResultStatus::Error,
        summary: envelope.message.clone(),
        data: None,
        operation_id: None,
        next_actions: vec![],
        artifact_refs: result.artifact_refs,
        diagnostic_refs: result.diagnostic_refs,
        error: Some(envelope),
        correlation_id: result.correlation_id,
    }
}

/// Runs rmcp behind a narrow STDIO relay.  The relay owns only protocol
/// framing: it rejects malformed JSON and pre-initialize tools/list locally,
/// then keeps reading until a valid initialize reaches rmcp.  It never reads
/// Registry files, TOML, executable state, or Controller-owned data.
pub async fn serve_supervised_stdio(server: Gateway) -> Result<(), Box<dyn std::error::Error>> {
    let (server_transport, relay_transport) = tokio::io::duplex(64 * 1024);
    let mut relay = tokio::spawn(async move { relay_stdio(relay_transport).await });
    let service = tokio::select! {
        result = server.serve(server_transport) => match result {
            Ok(service) => service,
            Err(error) => {
                relay.abort();
                return Err(Box::new(error));
            }
        },
        result = &mut relay => {
            return relay_finished(result);
        }
    };
    tokio::select! {
        result = service.waiting() => {
            relay.abort();
            result.map(|_| ()).map_err(Into::into)
        }
        result = &mut relay => relay_finished(result),
    }
}

fn relay_finished(
    result: Result<io::Result<()>, tokio::task::JoinError>,
) -> Result<(), Box<dyn std::error::Error>> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(Box::new(error)),
        Err(error) => Err(Box::new(error)),
    }
}

async fn relay_stdio(relay: tokio::io::DuplexStream) -> io::Result<()> {
    use tokio::{
        io::{AsyncWriteExt, BufReader},
        sync::Mutex,
    };

    let (relay_reader, mut relay_writer) = tokio::io::split(relay);
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
    let output = Arc::clone(&stdout);
    let forward = tokio::spawn(async move {
        let mut reader = BufReader::new(relay_reader);
        let mut line = Vec::new();
        loop {
            if !read_capped_line(&mut reader, &mut line, SUPERVISED_STDIO_MAX_OUTBOUND_BYTES)
                .await?
            {
                return Ok::<(), io::Error>(());
            }
            let mut stdout = output.lock().await;
            stdout.write_all(&line).await?;
            stdout.flush().await?;
        }
    });

    let mut state = SupervisorState::AwaitInitialize;
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut line = Vec::new();
    loop {
        if !read_capped_line(&mut stdin, &mut line, SUPERVISED_STDIO_MAX_LINE_BYTES).await? {
            break;
        }
        let decision = if line.len() > SUPERVISED_STDIO_MAX_LINE_BYTES {
            SupervisorDecision::Respond(protocol_error(
                serde_json::Value::Null,
                -32600,
                "MCP line exceeds 8 MiB.",
            ))
        } else {
            std::str::from_utf8(&line)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "MCP input is not UTF-8"))
                .map(|line| supervisor_decision(line, &mut state))?
        };
        match decision {
            SupervisorDecision::Respond(response) => {
                let mut stdout = stdout.lock().await;
                stdout.write_all(response.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
            SupervisorDecision::Ignore => continue,
            SupervisorDecision::Forward => {}
        }
        relay_writer.write_all(&line).await?;
        relay_writer.flush().await?;
    }
    drop(relay_writer);
    forward.abort();
    Ok(())
}

/// Reads one JSONL record without ever retaining more than the protocol cap.
/// A peer that keeps sending a line after the cap is disconnected rather than
/// being allowed to grow an allocation unboundedly.
async fn read_capped_line<R>(reader: &mut R, line: &mut Vec<u8>, limit: usize) -> io::Result<bool>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    line.clear();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(!line.is_empty());
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |newline| newline + 1);
        if line.len().saturating_add(take) > limit {
            reader.consume(take);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP line exceeds its configured bound",
            ));
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if line.last() == Some(&b'\n') {
            return Ok(true);
        }
    }
}

#[derive(Debug)]
enum SupervisorDecision {
    Forward,
    Respond(String),
    Ignore,
}

fn supervisor_decision(line: &str, state: &mut SupervisorState) -> SupervisorDecision {
    let value: serde_json::Value = match parse_no_duplicate_keys(line) {
        Ok(value) => value,
        Err(_) => {
            return SupervisorDecision::Respond(protocol_error(
                serde_json::Value::Null,
                -32700,
                "Invalid JSON-RPC input.",
            ));
        }
    };
    let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = value.get("method").and_then(serde_json::Value::as_str);
    let is_notification = method.is_some() && value.get("id").is_none();
    if value.get("jsonrpc").and_then(serde_json::Value::as_str) != Some("2.0") || method.is_none() {
        return reject_or_ignore(id, is_notification, -32600, "Invalid JSON-RPC request.");
    }
    match (*state, method) {
        (SupervisorState::AwaitInitialize, Some("initialize")) => {
            let valid_initialize = value.get("jsonrpc").and_then(serde_json::Value::as_str)
                == Some("2.0")
                && value.get("id").is_some_and(|id| !id.is_null())
                && value
                    .pointer("/params/protocolVersion")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|version| !version.is_empty())
                && value
                    .pointer("/params/capabilities")
                    .is_some_and(serde_json::Value::is_object)
                && value
                    .pointer("/params/clientInfo/name")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| !name.is_empty())
                && value
                    .pointer("/params/clientInfo/version")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|version| !version.is_empty());
            if !valid_initialize {
                return reject_or_ignore(
                    id,
                    is_notification,
                    -32602,
                    "initialize parameters are invalid.",
                );
            }
            *state = SupervisorState::AwaitInitializedNotification;
        }
        (SupervisorState::AwaitInitialize, _) => {
            return reject_or_ignore(
                id,
                is_notification,
                -32600,
                "initialize must complete before this request.",
            );
        }
        (SupervisorState::AwaitInitializedNotification, Some("notifications/initialized")) => {
            let valid_notification =
                is_notification && value.get("params").is_none_or(serde_json::Value::is_object);
            if !valid_notification {
                return SupervisorDecision::Respond(protocol_error(
                    id,
                    -32600,
                    "notifications/initialized must be a notification.",
                ));
            }
            *state = SupervisorState::Ready;
        }
        (SupervisorState::AwaitInitializedNotification, Some("initialize"))
        | (SupervisorState::Ready, Some("initialize")) => {
            return reject_or_ignore(
                id,
                is_notification,
                -32600,
                "initialize may be sent only once per connection.",
            );
        }
        (SupervisorState::AwaitInitializedNotification, _) => {
            return reject_or_ignore(
                id,
                is_notification,
                -32600,
                "notifications/initialized must precede this request.",
            );
        }
        (SupervisorState::Ready, Some("tools/call"))
            if value
                .pointer("/params/task")
                .is_some_and(|task| !task.is_null()) =>
        {
            return reject_or_ignore(
                id,
                is_notification,
                -32602,
                "task-augmented tool calls are not supported.",
            );
        }
        (SupervisorState::Ready, _) => {}
    }
    if method.is_some_and(|method| method.starts_with("notifications/")) && !is_notification {
        return SupervisorDecision::Respond(protocol_error(
            id,
            -32600,
            "notification methods must not include an id.",
        ));
    }
    if value.get("id").is_some()
        && method.is_some_and(|method| {
            method.starts_with("resources/")
                || method.starts_with("prompts/")
                || method.starts_with("logging/")
                || method.starts_with("completion/")
                || method.starts_with("tasks/")
        })
    {
        return SupervisorDecision::Respond(protocol_error(
            id,
            -32601,
            "MCP method is not available.",
        ));
    }
    if method == Some("tools/call")
        && value
            .get("params")
            .and_then(serde_json::Value::as_object)
            .and_then(|params| params.get("name"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| !FIXED_TOOLS.iter().any(|tool| tool.name == name))
    {
        return reject_or_ignore(id, is_notification, -32601, "MCP tool is not available.");
    }
    if method == Some("tools/call")
        && value
            .get("params")
            .and_then(serde_json::Value::as_object)
            .and_then(|params| {
                let name = params.get("name")?.as_str()?;
                let arguments = params.get("arguments")?.clone();
                fixed_arguments_valid(name, arguments).then_some(())
            })
            .is_none()
    {
        return reject_or_ignore(
            id,
            is_notification,
            -32602,
            "Fixed MCP tool arguments do not satisfy its Schema.",
        );
    }
    SupervisorDecision::Forward
}

fn reject_or_ignore(
    id: serde_json::Value,
    is_notification: bool,
    code: i32,
    message: &str,
) -> SupervisorDecision {
    if is_notification {
        SupervisorDecision::Ignore
    } else {
        SupervisorDecision::Respond(protocol_error(id, code, message))
    }
}

#[cfg(test)]
fn supervisor_response(line: &str, state: &mut SupervisorState) -> Option<String> {
    match supervisor_decision(line, state) {
        SupervisorDecision::Respond(response) => Some(response),
        SupervisorDecision::Forward | SupervisorDecision::Ignore => None,
    }
}

fn fixed_arguments_valid(name: &str, arguments: serde_json::Value) -> bool {
    star_contracts::fixed_mcp::fixed_input_valid(name, arguments)
}

fn protocol_error(id: serde_json::Value, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc":"2.0",
        "id":id,
        "error":{"code":code,"message":message}
    })
    .to_string()
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Gateway {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION"))
                    .with_title(SERVER_TITLE)
                    .with_description(SERVER_DESCRIPTION),
            )
            .with_instructions(SERVER_INSTRUCTIONS)
    }

    fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let result = if request.and_then(|params| params.cursor).is_some() {
            Err(ErrorData::invalid_params(
                "fixed tools/list does not accept a cursor",
                None,
            ))
        } else {
            Ok(ListToolsResult {
                tools: self.fixed_tools_in_contract_order(),
                ..Default::default()
            })
        };
        std::future::ready(result)
    }
}

#[tool_router(router = tool_router)]
impl Gateway {
    #[tool(
        name = "star_tool_search",
        description = "Search the current Star-Control live registry for an action. Call describe before invoking a result.",
        annotations(
            title = "Search Star-Control Tools",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search(&self, input: Parameters<SearchInput>) -> CallToolResult {
        self.dispatch(
            "star_tool_search",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
    #[tool(
        name = "star_tool_describe",
        description = "Return the current Schema, risk lane, executable readiness, and descriptor hash for one action.",
        annotations(
            title = "Describe a Star-Control Tool",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn describe(&self, input: Parameters<DescribeInput>) -> CallToolResult {
        self.dispatch(
            "star_tool_describe",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
    #[tool(
        name = "star_tool_registry_status",
        description = "Inspect live registry revisions, packages, watchers, last-known-good state, and diagnostics.",
        annotations(
            title = "Inspect the Tool Registry",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn registry_status(&self, input: Parameters<RegistryStatusInput>) -> CallToolResult {
        self.dispatch(
            "star_tool_registry_status",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
    #[tool(
        name = "star_tool_call_read_closed",
        description = "Invoke the described local read-only action. The descriptor must require this exact lane.",
        annotations(
            title = "Run a Local Read Action",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn call_read_closed(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_read_closed",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_call_read_open",
        description = "Invoke the described read-only action that may access external systems.",
        annotations(
            title = "Run an External Read Action",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn call_read_open(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_read_open",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_call_write_closed",
        description = "Invoke the described non-destructive local mutation.",
        annotations(
            title = "Run a Local Write Action",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn call_write_closed(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_write_closed",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_call_destructive_closed",
        description = "Invoke the described destructive local action after policy checks.",
        annotations(
            title = "Run a Destructive Local Action",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn call_destructive_closed(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_destructive_closed",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_call_write_open",
        description = "Invoke the described non-destructive action that changes or uses an external system.",
        annotations(
            title = "Run an External Write Action",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn call_write_open(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_write_open",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_call_destructive_open",
        description = "Invoke the described destructive external action after policy checks.",
        annotations(
            title = "Run a Destructive External Action",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn call_destructive_open(
        &self,
        input: Parameters<CallInput>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.dispatch(
            "star_tool_call_destructive_open",
            serde_json::to_value(input.0).expect("typed input serializes"),
            Some(context),
        )
        .await
    }
    #[tool(
        name = "star_tool_operation_get",
        description = "Read durable status, progress, and result for a Star-Control operation.",
        annotations(
            title = "Get an Operation",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn operation_get(&self, input: Parameters<OperationGetInput>) -> CallToolResult {
        self.dispatch(
            "star_tool_operation_get",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
    #[tool(
        name = "star_tool_operation_cancel",
        description = "Request cancellation of a durable operation and return its current state.",
        annotations(
            title = "Cancel an Operation",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn operation_cancel(&self, input: Parameters<OperationCancelInput>) -> CallToolResult {
        self.dispatch(
            "star_tool_operation_cancel",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
    #[tool(
        name = "star_approval_resolve",
        description = "Record the user's approve or deny decision for the exact approval scope.",
        annotations(
            title = "Resolve an Approval",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn approval_resolve(&self, input: Parameters<ApprovalResolveInput>) -> CallToolResult {
        self.dispatch(
            "star_approval_resolve",
            serde_json::to_value(input.0).expect("typed input serializes"),
            None,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::{
        ServiceExt,
        model::{ClientJsonRpcMessage, ProtocolVersion, ServerJsonRpcMessage, ServerResult},
        transport::{IntoTransport, Transport},
    };
    use star_contracts::fixed_mcp::{FIXED_TOOLS, SERVER_NAME};
    struct Fake;
    impl ControllerGateway for Fake {
        fn call<'a>(
            &'a self,
            command: &str,
            tool: &str,
            arguments: serde_json::Value,
            _correlation_id: RequestId,
            _progress: Option<tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
        ) -> Pin<Box<dyn Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>>
        {
            let command = command.to_owned();
            let tool = tool.to_owned();
            Box::pin(async move {
                Ok(McpToolResult {
                    schema_id: format!("star.mcp.{tool}.result"),
                    schema_version: 1,
                    status: McpResultStatus::Ok,
                    summary: command.to_owned(),
                    data: Some(arguments),
                    operation_id: None,
                    next_actions: vec![],
                    artifact_refs: vec![],
                    diagnostic_refs: vec![],
                    error: None,
                    correlation_id: RequestId::new(),
                })
            })
        }
    }
    #[test]
    // matrix: MCP-G005 MCP-G006 MCP-G018 MCP-G019 MCP-S002
    fn fixed_surface_has_no_dynamic_tools_or_extra_capabilities() {
        let server = Gateway::new(Arc::new(Fake));
        let info = server.get_info();
        assert_eq!(info.server_info.name, SERVER_NAME);
        assert_eq!(info.instructions.as_deref(), Some(SERVER_INSTRUCTIONS));
        assert!(!SERVER_INSTRUCTIONS.contains("ignore previous instructions"));
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_none());
        let tools = server.fixed_tools_in_contract_order();
        assert_eq!(tools.len(), FIXED_TOOLS.len());
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_ref())
                .collect::<Vec<_>>(),
            FIXED_TOOLS.iter().map(|tool| tool.name).collect::<Vec<_>>()
        );
        let first = serde_json::to_vec(&server.fixed_tools_in_contract_order()).unwrap();
        let second = serde_json::to_vec(&server.fixed_tools_in_contract_order()).unwrap();
        assert_eq!(first, second, "tools/list definitions must be byte-stable");
        for (tool, fixed) in tools.iter().zip(FIXED_TOOLS) {
            assert_eq!(tool.title.as_deref(), Some(fixed.title));
            assert_eq!(tool.description.as_deref(), Some(fixed.description));
            let annotations = tool.annotations.as_ref().expect("annotations are fixed");
            assert!(annotations.title.is_none());
            assert_eq!(annotations.read_only_hint, Some(fixed.read_only));
            assert_eq!(annotations.destructive_hint, Some(fixed.destructive));
            assert_eq!(annotations.idempotent_hint, Some(fixed.idempotent));
            assert_eq!(annotations.open_world_hint, Some(fixed.open_world));
            assert!(tool.execution.is_none(), "MCP Tasks must not be advertised");
            let input = serde_json::Value::Object(tool.input_schema.as_ref().clone());
            let output = serde_json::Value::Object(
                tool.output_schema
                    .as_ref()
                    .expect("every fixed tool has an output Schema")
                    .as_ref()
                    .clone(),
            );
            assert_eq!(
                input["$id"],
                format!("urn:star-control:schema:star.mcp.{}.input:v1", fixed.name)
            );
            assert_eq!(
                output["$id"],
                format!("urn:star-control:schema:star.mcp.{}.result:v1", fixed.name)
            );
            assert!(!input.to_string().contains("\"$ref\""));
            assert!(!output.to_string().contains("\"$ref\""));
        }
    }
    #[tokio::test]
    // matrix: MCP-G013
    async fn gateway_only_translates_to_controller_command() {
        let server = Gateway::new(Arc::new(Fake));
        let result = server
            .search(Parameters(SearchInput {
                query: "core".to_owned(),
                namespaces: None,
                tags: None,
                task_kinds: None,
                sources: None,
                readiness: None,
                risk_lanes: None,
                limit: None,
                cursor: None,
            }))
            .await;
        let structured: McpToolResult =
            serde_json::from_value(result.structured_content.unwrap()).unwrap();
        assert_eq!(structured.summary, "tool.search");
        assert_eq!(
            structured.data.unwrap(),
            serde_json::json!({"query":"core"})
        );
        assert_eq!(result.is_error, Some(false));
    }

    fn wire_message(raw: &str) -> ClientJsonRpcMessage {
        serde_json::from_str(raw).expect("valid test JSON-RPC message")
    }

    fn initialize(version: &str) -> ClientJsonRpcMessage {
        wire_message(&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"{version}","capabilities":{{}},"clientInfo":{{"name":"star-mcp-test","version":"0.1.0"}}}}}}"#
        ))
    }

    #[tokio::test]
    // matrix: MCP-G001 MCP-G002 MCP-G003 MCP-G009 MCP-G021 MCP-G023
    async fn gateway_uses_the_fixed_wire_protocol_across_initialize_and_ping() {
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server = Gateway::new(Arc::new(Fake));
        let task = tokio::spawn(async move { server.serve(server_transport).await });
        let mut client = IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);

        client.send(initialize("2025-11-25")).await.unwrap();
        let latest = client.receive().await.unwrap();
        let ServerJsonRpcMessage::Response(latest) = latest else {
            panic!("initialize must return a response");
        };
        let ServerResult::InitializeResult(latest) = latest.result else {
            panic!("initialize must return InitializeResult");
        };
        assert_eq!(latest.protocol_version, ProtocolVersion::LATEST);
        assert_eq!(latest.server_info.name, SERVER_NAME);
        assert_eq!(latest.instructions.as_deref(), Some(SERVER_INSTRUCTIONS));

        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .await
            .unwrap();
        client
            .send(wire_message(r#"{"jsonrpc":"2.0","id":11,"method":"ping"}"#))
            .await
            .unwrap();
        let ping = client.receive().await.unwrap();
        assert!(matches!(
            ping,
            ServerJsonRpcMessage::Response(ref response)
                if matches!(response.result, ServerResult::EmptyResult(_))
        ));
        task.abort();

        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server = Gateway::new(Arc::new(Fake));
        let legacy_task = tokio::spawn(async move { server.serve(server_transport).await });
        let mut legacy_client =
            IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);
        legacy_client.send(initialize("2025-06-18")).await.unwrap();
        let legacy = legacy_client.receive().await.unwrap();
        let ServerJsonRpcMessage::Response(legacy) = legacy else {
            panic!("legacy initialize must return a response");
        };
        let ServerResult::InitializeResult(legacy) = legacy.result else {
            panic!("legacy initialize must return InitializeResult");
        };
        assert_eq!(legacy.protocol_version, ProtocolVersion::V_2025_06_18);
        legacy_client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .await
            .unwrap();
        legacy_client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","id":12,"method":"tools/list","params":{}}"#,
            ))
            .await
            .unwrap();
        let legacy_tools = legacy_client.receive().await.unwrap();
        assert!(matches!(
            legacy_tools,
            ServerJsonRpcMessage::Response(ref response)
                if matches!(&response.result, ServerResult::ListToolsResult(result) if result.tools.len() == 12)
        ));
        legacy_client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","id":13,"method":"tools/list","params":{"cursor":"nope"}}"#,
            ))
            .await
            .unwrap();
        let cursor_error = legacy_client.receive().await.unwrap();
        assert!(matches!(
            cursor_error,
            ServerJsonRpcMessage::Error(ref error) if error.error.code.0 == -32602
        ));
        legacy_task.abort();

        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server = Gateway::new(Arc::new(Fake));
        let unsupported_task = tokio::spawn(async move { server.serve(server_transport).await });
        let mut unsupported_client =
            IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);
        unsupported_client
            .send(initialize("2099-01-01"))
            .await
            .unwrap();
        let proposed = unsupported_client.receive().await.unwrap();
        let ServerJsonRpcMessage::Response(proposed) = proposed else {
            panic!("unsupported version must receive a supported-version proposal");
        };
        let ServerResult::InitializeResult(proposed) = proposed.result else {
            panic!("unsupported version proposal must be InitializeResult");
        };
        assert_eq!(proposed.protocol_version, ProtocolVersion::LATEST);
        drop(unsupported_client);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), unsupported_task)
                .await
                .is_ok(),
            "client rejection by disconnect terminates the connection"
        );
    }

    #[test]
    // matrix: MCP-G007
    fn gateway_stdout_is_reserved_for_json_rpc_records() {
        let production = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source precedes tests");
        let main = include_str!("main.rs");
        for source in [production, main] {
            for line in source.lines().map(str::trim) {
                assert!(
                    !line.starts_with("print!(") && !line.starts_with("println!("),
                    "stdout logging is forbidden in star-mcp: {line}"
                );
            }
        }
        assert_eq!(
            production.matches("tokio::io::stdout()").count(),
            1,
            "only the supervised JSON-RPC relay may open stdout"
        );
    }

    struct DelayedFake;

    impl ControllerGateway for DelayedFake {
        fn call<'a>(
            &'a self,
            command: &str,
            tool: &str,
            arguments: serde_json::Value,
            correlation_id: RequestId,
            progress: Option<tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
        ) -> Pin<Box<dyn Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>>
        {
            let command = command.to_owned();
            let tool = tool.to_owned();
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(260)).await;
                if let Some(progress) = progress {
                    let _ = progress.send(ControllerProgress {
                        progress: 0.5,
                        total: Some(1.0),
                        message: "durable operation running".to_owned(),
                    });
                }
                tokio::time::sleep(std::time::Duration::from_millis(260)).await;
                Ok(McpToolResult {
                    schema_id: format!("star.mcp.{tool}.result"),
                    schema_version: 1,
                    status: McpResultStatus::Ok,
                    summary: command,
                    data: Some(arguments),
                    operation_id: None,
                    next_actions: vec![],
                    artifact_refs: vec![],
                    diagnostic_refs: vec![],
                    error: None,
                    correlation_id,
                })
            })
        }
    }

    #[tokio::test]
    // matrix: MCP-G016
    async fn progress_is_monotonic_rate_limited_and_stops_after_completion() {
        let start = std::time::Instant::now();
        let mut limiter = ProgressLimiter::default();
        assert!(limiter.should_emit(0.0, start));
        assert!(!limiter.should_emit(0.5, start + std::time::Duration::from_millis(249)));
        assert!(limiter.should_emit(0.5, start + std::time::Duration::from_millis(250)));
        assert!(!limiter.should_emit(0.4, start + std::time::Duration::from_millis(500)));
        limiter.complete();
        assert!(!limiter.should_emit(1.0, start + std::time::Duration::from_secs(1)));

        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server = Gateway::new(Arc::new(DelayedFake));
        let server_task = tokio::spawn(async move {
            let service = server.serve(server_transport).await.unwrap();
            service.waiting().await
        });
        let mut client = IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);
        client.send(initialize("2025-11-25")).await.unwrap();
        assert!(matches!(
            client.receive().await.unwrap(),
            ServerJsonRpcMessage::Response(_)
        ));
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .await
            .unwrap();
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"star_tool_call_read_closed","arguments":{"tool_id":"core.test.echo","descriptor_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","arguments":{},"wait_mode":"sync"},"_meta":{"progressToken":"p-1"}}}"#,
            ))
            .await
            .unwrap();

        let mut progress = Vec::new();
        let mut response_seen = false;
        for _ in 0..4 {
            let message = tokio::time::timeout(std::time::Duration::from_secs(2), client.receive())
                .await
                .expect("Gateway must answer the progress-bearing call")
                .expect("wire message");
            let value = serde_json::to_value(&message).unwrap();
            if value.get("method").and_then(serde_json::Value::as_str)
                == Some("notifications/progress")
            {
                progress.push((
                    std::time::Instant::now(),
                    value["params"]["progress"].as_f64().unwrap(),
                ));
            }
            if value.get("id").and_then(serde_json::Value::as_u64) == Some(2) {
                response_seen = true;
                break;
            }
        }
        assert!(response_seen);
        assert_eq!(
            progress.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            vec![0.0, 0.5, 1.0]
        );
        assert!(
            progress[1].0.duration_since(progress[0].0) >= std::time::Duration::from_millis(250)
        );

        client
            .send(wire_message(r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#))
            .await
            .unwrap();
        let after_completion = serde_json::to_value(client.receive().await.unwrap()).unwrap();
        assert_eq!(after_completion["id"], 3);
        drop(client);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), server_task).await;
    }

    #[derive(Default)]
    struct CancelAwareFake {
        started: tokio::sync::Notify,
        cancelled: tokio::sync::Notify,
        call_correlation: std::sync::Mutex<Option<String>>,
        cancel_correlation: std::sync::Mutex<Option<String>>,
        cancel_count: std::sync::atomic::AtomicUsize,
    }

    impl ControllerGateway for CancelAwareFake {
        fn call<'a>(
            &'a self,
            _command: &str,
            _tool: &str,
            _arguments: serde_json::Value,
            correlation_id: RequestId,
            _progress: Option<tokio::sync::mpsc::UnboundedSender<ControllerProgress>>,
        ) -> Pin<Box<dyn Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>>
        {
            Box::pin(async move {
                *self.call_correlation.lock().unwrap() = Some(correlation_id.to_string());
                self.started.notify_one();
                std::future::pending::<Result<McpToolResult, GatewayError>>().await
            })
        }

        fn cancel<'a>(
            &'a self,
            correlation_id: RequestId,
        ) -> Pin<Box<dyn Future<Output = Result<(), GatewayError>> + Send + 'a>> {
            Box::pin(async move {
                *self.cancel_correlation.lock().unwrap() = Some(correlation_id.to_string());
                self.cancel_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.cancelled.notify_one();
                Ok(())
            })
        }
    }

    #[tokio::test]
    // matrix: MCP-G017
    async fn sync_cancellation_forwards_intent_and_initialize_cancellation_is_ignored() {
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let controller = Arc::new(CancelAwareFake::default());
        let server = Gateway::new(controller.clone());
        let server_task = tokio::spawn(async move {
            let service = server.serve(server_transport).await.unwrap();
            service.waiting().await
        });
        let mut client = IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);

        client.send(initialize("2025-11-25")).await.unwrap();
        assert!(matches!(
            client.receive().await.unwrap(),
            ServerJsonRpcMessage::Response(_)
        ));
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1,"reason":"late initialize cancellation"}}"#,
            ))
            .await
            .unwrap();
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .await
            .unwrap();
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"star_tool_call_read_closed","arguments":{"tool_id":"core.test.echo","descriptor_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","arguments":{},"wait_mode":"sync"}}}"#,
            ))
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            controller.started.notified(),
        )
        .await
        .expect("Controller dispatch starts");
        client
            .send(wire_message(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"test cancellation"}}"#,
            ))
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            controller.cancelled.notified(),
        )
        .await
        .expect("Gateway forwards cancellation intent");
        assert_eq!(
            *controller.call_correlation.lock().unwrap(),
            *controller.cancel_correlation.lock().unwrap()
        );
        assert_eq!(
            controller
                .cancel_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        client
            .send(wire_message(r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#))
            .await
            .unwrap();
        let ping = tokio::time::timeout(std::time::Duration::from_secs(1), client.receive())
            .await
            .expect("Gateway survives call cancellation")
            .expect("ping response");
        let ping = serde_json::to_value(ping).unwrap();
        assert_eq!(ping["id"], 3);
        drop(client);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), server_task).await;
    }

    #[tokio::test]
    // matrix: MCP-G011 MCP-G012
    async fn gateway_rejects_unknown_tools_and_invalid_fixed_input_before_controller_dispatch() {
        let mut state = SupervisorState::AwaitInitialize;
        assert!(
            supervisor_response(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
                &mut state,
            )
            .is_none()
        );
        assert!(
            supervisor_response(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                &mut state,
            )
            .is_none()
        );
        let unknown = supervisor_response(
            r#"{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"not_a_fixed_tool","arguments":{}}}"#,
            &mut state,
        )
        .expect("unknown fixed tool rejects before the Controller");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&unknown).unwrap()["error"]["code"],
            -32601
        );
        let invalid = supervisor_response(
            r#"{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{"name":"star_tool_search","arguments":{}}}"#,
            &mut state,
        )
        .expect("invalid fixed input rejects before the Controller");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&invalid).unwrap()["error"]["code"],
            -32602
        );
        let task_augmented = supervisor_response(
            r#"{"jsonrpc":"2.0","id":23,"method":"tools/call","params":{"name":"star_tool_search","arguments":{"query":"x"},"task":{}}}"#,
            &mut state,
        )
        .expect("task augmentation rejects before the Controller");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&task_augmented).unwrap()["error"]["code"],
            -32602
        );
    }

    #[test]
    // matrix: MCP-G014 MCP-G015 MCP-G020 MCP-G022
    fn gateway_preserves_fixed_result_status_and_compact_instructions() {
        assert!(
            SERVER_INSTRUCTIONS
                .chars()
                .take(512)
                .collect::<String>()
                .contains("star_tool_search")
        );
        for status in [
            McpResultStatus::QuestionRequired,
            McpResultStatus::ApprovalRequired,
        ] {
            let response = mcp_call_result(McpToolResult {
                schema_id: "star.mcp.test.result".to_owned(),
                schema_version: 1,
                status,
                summary: "next user action required".to_owned(),
                data: None,
                operation_id: None,
                next_actions: vec![],
                artifact_refs: vec![],
                diagnostic_refs: vec![],
                error: None,
                correlation_id: RequestId::new(),
            });
            assert_eq!(response.is_error, Some(false));
        }
        let failure = mcp_call_result(McpToolResult {
            schema_id: "star.mcp.test.result".to_owned(),
            schema_version: 1,
            status: McpResultStatus::Error,
            summary: "actual error".to_owned(),
            data: None,
            operation_id: None,
            next_actions: vec![],
            artifact_refs: vec![],
            diagnostic_refs: vec![],
            error: Some(ErrorEnvelope::new(
                "INTERNAL_TEST_ERROR",
                "actual error",
                false,
                "test-correlation",
                "star-mcp-test",
            )),
            correlation_id: RequestId::new(),
        });
        assert_eq!(failure.is_error, Some(true));
        assert!(
            failure.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("actual error")
        );
    }

    #[test]
    fn oversized_controller_result_becomes_a_bounded_structured_error() {
        let response = mcp_call_result(McpToolResult {
            schema_id: "star.mcp.star_tool_search.result".to_owned(),
            schema_version: 1,
            status: McpResultStatus::Ok,
            summary: "oversized".to_owned(),
            data: Some(serde_json::json!({
                "value":"x".repeat(MCP_RESULT_MAX_STRUCTURED_BYTES)
            })),
            operation_id: None,
            next_actions: vec![],
            artifact_refs: vec![],
            diagnostic_refs: vec![],
            error: None,
            correlation_id: RequestId::new(),
        });
        assert_eq!(response.is_error, Some(true));
        let structured: McpToolResult =
            serde_json::from_value(response.structured_content.unwrap()).unwrap();
        assert!(matches!(structured.status, McpResultStatus::Error));
        assert_eq!(structured.error.unwrap().code, "TOOL_OUTPUT_LIMIT");
        assert!(
            serde_json::to_vec(&response.content).unwrap().len() < MCP_OUTBOUND_INLINE_MAX_BYTES
        );
    }

    #[test]
    fn supplied_client_request_id_is_the_ipc_and_result_correlation_id() {
        let supplied = RequestId::new();
        assert_eq!(
            correlation_id_from_arguments(&serde_json::json!({
                "client_request_id":supplied
            })),
            supplied
        );
        assert_ne!(
            correlation_id_from_arguments(&serde_json::json!({"client_request_id":null})),
            supplied
        );
    }

    #[test]
    // matrix: MCP-G004
    fn stdio_supervisor_rejects_preinitialize_tools_list_without_consuming_initialize() {
        let mut state = SupervisorState::AwaitInitialize;
        let error = supervisor_response(
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#,
            &mut state,
        )
        .expect("pre-initialize tools/list is rejected locally");
        assert_eq!(state, SupervisorState::AwaitInitialize);
        let error: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert_eq!(error["id"], 7);
        assert_eq!(error["error"]["code"], -32600);
        let malformed_initialize = supervisor_response(
            r#"{"jsonrpc":"2.0","id":8,"method":"initialize","params":{}}"#,
            &mut state,
        )
        .expect("malformed initialize is rejected without advancing lifecycle");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&malformed_initialize).unwrap()["error"]["code"],
            -32602
        );
        assert_eq!(state, SupervisorState::AwaitInitialize);
        assert!(
            supervisor_response(
                r#"{"jsonrpc":"2.0","id":9,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
                &mut state,
            )
            .is_none()
        );
        assert_eq!(state, SupervisorState::AwaitInitializedNotification);
    }

    #[test]
    // matrix: MCP-G008
    fn stdio_supervisor_rejects_malformed_and_duplicate_key_json() {
        let mut state = SupervisorState::AwaitInitialize;
        let malformed = supervisor_response("{", &mut state).expect("invalid JSON rejects");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&malformed).unwrap()["error"]["code"],
            -32700
        );
        let duplicate = supervisor_response(
            r#"{"jsonrpc":"2.0","id":1,"id":2,"method":"initialize","params":{}}"#,
            &mut state,
        )
        .expect("duplicate key rejects");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&duplicate).unwrap()["error"]["code"],
            -32700
        );
        assert_eq!(state, SupervisorState::AwaitInitialize);
    }

    #[test]
    // matrix: MCP-G024
    fn stdio_supervisor_requires_initialized_notification_and_rejects_second_initialize() {
        let mut state = SupervisorState::AwaitInitialize;
        assert!(
            supervisor_response(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
                &mut state,
            )
            .is_none()
        );
        let early_call = supervisor_response(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{}}"#,
            &mut state,
        )
        .expect("call before notification rejects");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&early_call).unwrap()["error"]["code"],
            -32600
        );
        assert!(
            supervisor_response(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
                &mut state,
            )
            .is_none()
        );
        let repeated = supervisor_response(
            r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{}}"#,
            &mut state,
        )
        .expect("second initialize rejects");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&repeated).unwrap()["error"]["code"],
            -32600
        );
        assert_eq!(state, SupervisorState::Ready);
    }

    #[test]
    fn frozen_protocol_jsonl_fixture_reaches_ready_without_local_surface_drift() {
        let fixture =
            include_str!("../../../specs/fixtures/mcp/protocol/initialize-list-2025-11-25.jsonl");
        let mut state = SupervisorState::AwaitInitialize;
        for line in fixture.lines() {
            assert!(
                supervisor_response(line, &mut state).is_none(),
                "valid conformance fixture must be forwarded to rmcp"
            );
        }
        assert_eq!(state, SupervisorState::Ready);
    }

    #[tokio::test]
    // matrix: MCP-G008
    async fn stdio_line_reader_caps_unterminated_input_before_unbounded_allocation() {
        use tokio::io::AsyncWriteExt;

        let (mut writer, reader) = tokio::io::duplex(16 * 1024);
        let writer = tokio::spawn(async move {
            writer
                .write_all(&vec![b'x'; SUPERVISED_STDIO_MAX_LINE_BYTES + 1])
                .await
                .unwrap();
        });
        let mut reader = tokio::io::BufReader::new(reader);
        let mut line = Vec::new();
        assert!(
            read_capped_line(&mut reader, &mut line, SUPERVISED_STDIO_MAX_LINE_BYTES)
                .await
                .is_err()
        );
        assert!(line.len() <= SUPERVISED_STDIO_MAX_LINE_BYTES);
        writer.await.unwrap();
    }
}
