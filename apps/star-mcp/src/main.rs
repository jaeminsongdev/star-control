use std::sync::Arc;

use star_contracts::{
    fixed_mcp::{McpResultStatus, McpToolResult},
    ids::RequestId,
    ipc::IpcStatus,
};
use star_ipc::client::{ControllerClient, ControllerClientError, mcp_client_config};
use star_mcp::{ControllerGateway, Gateway, GatewayError};

struct IpcControllerGateway(ControllerClient);
impl ControllerGateway for IpcControllerGateway {
    fn call<'a>(
        &'a self,
        command: &str,
        tool: &str,
        arguments: serde_json::Value,
        correlation_id: RequestId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<McpToolResult, GatewayError>> + Send + 'a>,
    > {
        let command = command.to_owned();
        let tool = tool.to_owned();
        Box::pin(async move {
            let response = self
                .0
                .call_with_mcp_tool(&command, arguments, correlation_id.clone(), Some(&tool))
                .await
                .map_err(map_client_error)?;
            let correlation_id = RequestId::parse(response.correlation_id.clone())
                .map_err(|_| GatewayError::ProtocolMismatch)?;
            let mut data = response.data;
            if let Some(object) = data.as_mut().and_then(serde_json::Value::as_object_mut) {
                object.insert(
                    "gateway".to_owned(),
                    serde_json::json!({
                        "pid": std::process::id(),
                        "server_version": env!("CARGO_PKG_VERSION")
                    }),
                );
            }
            Ok(McpToolResult {
                schema_id: format!("star.mcp.{tool}.result"),
                schema_version: 1,
                status: match response.status {
                    IpcStatus::Ok => McpResultStatus::Ok,
                    IpcStatus::Accepted => McpResultStatus::Accepted,
                    IpcStatus::QuestionRequired => McpResultStatus::QuestionRequired,
                    IpcStatus::ApprovalRequired => McpResultStatus::ApprovalRequired,
                    IpcStatus::Blocked => McpResultStatus::Blocked,
                    IpcStatus::Error => McpResultStatus::Error,
                },
                summary: response
                    .error
                    .as_ref()
                    .map(|error| error.message.clone())
                    .unwrap_or_else(|| format!("Controller completed {command}.")),
                data,
                operation_id: response.operation_id,
                next_actions: vec![],
                artifact_refs: vec![],
                diagnostic_refs: response.diagnostics,
                error: response
                    .error
                    .and_then(|error| serde_json::to_value(error).ok()),
                correlation_id,
            })
        })
    }

    fn cancel<'a>(
        &'a self,
        correlation_id: RequestId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), GatewayError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.0
                .call(
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let binary = std::env::current_exe()?;
    let controller = binary
        .parent()
        .ok_or("star-mcp executable has no installation directory")?
        .join("star-controller.exe");
    let config = mcp_client_config(controller)
        .map_err(|error| format!("cannot prepare Controller IPC client: {error}"))?;
    let server = Gateway::new(Arc::new(IpcControllerGateway(ControllerClient::new(
        config,
    ))));
    star_mcp::serve_supervised_stdio(server).await?;
    Ok(())
}
