//! Authenticated one-request Controller client.
//!
//! The client deliberately has no Registry or process-start ownership.  Its
//! caller supplies the already verified Controller image and endpoint, then it
//! performs the per-connection identity and HMAC checks required by IPC v1.

use std::{path::PathBuf, time::Duration};

use star_contracts::{
    ids::RequestId,
    ipc::{IPC_PROTOCOL_MAJOR, IpcChallenge, IpcClientKind, IpcHello, IpcRequest, IpcResponse},
};
use thiserror::Error;

use crate::{
    IPC_PROTOCOL_VERSION, IpcCodecError, client_auth_tag,
    controller_start::ControllerStartError,
    key_store::{default_key_path, load},
    nonce,
    process_identity::verify_pipe_server_image,
    server_auth_tag, verify_auth_tag,
    windows_pipe::{open_client, read_json, read_json_with_timeout, write_json},
};

const RESPONSE_IO_GRACE: Duration = Duration::from_secs(5);
const DEMAND_SCAN_RESPONSE_BUDGET: Duration = Duration::from_secs(10);
const DISCOVERY_PROBE_RESPONSE_BUDGET: Duration = Duration::from_secs(40);

#[derive(Debug, Error)]
pub enum ControllerClientError {
    #[error("IPC controller unavailable")]
    Unavailable,
    #[error("IPC server identity does not match the installed Controller")]
    ServerIdentityMismatch,
    #[error("IPC authentication failed")]
    Authentication,
    #[error("IPC protocol version does not match")]
    ProtocolMismatch,
    #[error("IPC response was malformed")]
    MalformedResponse,
}

/// Caller-owned bootstrap inputs.  The endpoint and absolute Controller image
/// are deliberately explicit, so this transport never searches PATH or reads
/// application TOML.
#[derive(Clone, Debug)]
pub struct ControllerClientConfig {
    pub pipe_name: String,
    pub expected_server_image: PathBuf,
    pub key_path: PathBuf,
    pub client_kind: IpcClientKind,
    pub client_version: String,
    pub client_instance_id: String,
    pub capabilities: Vec<String>,
    pub connect_timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct ControllerClient {
    config: ControllerClientConfig,
}

impl ControllerClient {
    pub fn new(config: ControllerClientConfig) -> Self {
        Self { config }
    }

    pub async fn call(
        &self,
        command: &str,
        payload: serde_json::Value,
        correlation_id: RequestId,
    ) -> Result<IpcResponse, ControllerClientError> {
        self.call_with_mcp_tool(command, payload, correlation_id, None)
            .await
    }

    #[cfg(windows)]
    pub async fn call_with_verified_start(
        &self,
        bootstrap: &crate::controller_start::VerifiedControllerImage,
        command: &str,
        payload: serde_json::Value,
        correlation_id: RequestId,
    ) -> Result<IpcResponse, ControllerClientError> {
        self.call_with_verified_start_and_mcp_tool(
            bootstrap,
            command,
            payload,
            correlation_id,
            None,
        )
        .await
    }

    #[cfg(windows)]
    pub async fn call_with_verified_start_and_mcp_tool(
        &self,
        bootstrap: &crate::controller_start::VerifiedControllerImage,
        command: &str,
        payload: serde_json::Value,
        correlation_id: RequestId,
        mcp_tool: Option<&str>,
    ) -> Result<IpcResponse, ControllerClientError> {
        let mut initial = self.clone();
        initial.config.connect_timeout = Duration::from_millis(250);
        match initial
            .call_with_mcp_tool(command, payload.clone(), correlation_id.clone(), mcp_tool)
            .await
        {
            Ok(response) => Ok(response),
            Err(ControllerClientError::Unavailable) => {
                bootstrap.start_background().map_err(map_start_error)?;
                let deadline = tokio::time::Instant::now() + self.config.connect_timeout;
                loop {
                    match self
                        .call_with_mcp_tool(
                            command,
                            payload.clone(),
                            correlation_id.clone(),
                            mcp_tool,
                        )
                        .await
                    {
                        Ok(response) => return Ok(response),
                        Err(ControllerClientError::Unavailable)
                            if tokio::time::Instant::now() < deadline =>
                        {
                            tokio::time::sleep(Duration::from_millis(25)).await;
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            Err(error) => Err(error),
        }
    }

    pub async fn call_with_mcp_tool(
        &self,
        command: &str,
        payload: serde_json::Value,
        correlation_id: RequestId,
        mcp_tool: Option<&str>,
    ) -> Result<IpcResponse, ControllerClientError> {
        let mut pipe = tokio::time::timeout(self.config.connect_timeout, async {
            open_client(&self.config.pipe_name)
        })
        .await
        .map_err(|_| ControllerClientError::Unavailable)?
        .map_err(|_| ControllerClientError::Unavailable)?;
        let server_pid = verify_pipe_server_image(&pipe, &self.config.expected_server_image)
            .map_err(|_| ControllerClientError::ServerIdentityMismatch)?;

        let challenge: IpcChallenge = read_typed(&mut pipe).await?;
        if challenge.schema_id != "star.ipc.challenge"
            || challenge.schema_version != 1
            || challenge.protocol_major != IPC_PROTOCOL_MAJOR
        {
            return Err(ControllerClientError::ProtocolMismatch);
        }
        verify_challenge_server_pid(&challenge, server_pid)?;
        if crate::decode_nonce(&challenge.server_nonce).is_err() {
            return Err(ControllerClientError::Authentication);
        }
        let key = load(&self.config.key_path).map_err(|_| ControllerClientError::Authentication)?;
        let mut hello = IpcHello {
            schema_id: "star.ipc.hello".to_owned(),
            schema_version: 1,
            protocol_versions: vec![IPC_PROTOCOL_VERSION.to_owned()],
            client_kind: self.config.client_kind.clone(),
            client_version: self.config.client_version.clone(),
            client_instance_id: self.config.client_instance_id.clone(),
            client_pid: std::process::id(),
            client_nonce: nonce(),
            server_nonce: challenge.server_nonce.clone(),
            auth_tag: String::new(),
            capabilities: self.config.capabilities.clone(),
            correlation_id: correlation_id.to_string(),
        };
        hello.auth_tag = client_auth_tag(key.as_bytes(), &challenge, &hello)
            .map_err(|_| ControllerClientError::Authentication)?;
        write_typed(&mut pipe, &hello).await?;

        let welcome_value = read_json(&mut pipe).await.map_err(map_codec_error)?;
        if welcome_value
            .get("schema_id")
            .and_then(|value| value.as_str())
            == Some("star.ipc.handshake-error")
        {
            let error: star_contracts::ipc::IpcHandshakeError =
                serde_json::from_value(welcome_value)
                    .map_err(|_| ControllerClientError::MalformedResponse)?;
            let expected_tag = crate::server_auth_tag_value(
                key.as_bytes(),
                &hello.client_nonce,
                serde_json::to_value(&error)
                    .map_err(|_| ControllerClientError::MalformedResponse)?,
            )
            .map_err(|_| ControllerClientError::Authentication)?;
            verify_auth_tag(&expected_tag, &error.auth_tag)
                .map_err(|_| ControllerClientError::Authentication)?;
            if error.schema_id != "star.ipc.handshake-error"
                || error.schema_version != 1
                || error.code != "IPC_PROTOCOL_MISMATCH"
                || error.supported_versions != [IPC_PROTOCOL_VERSION]
                || error.correlation_id != correlation_id.to_string()
            {
                return Err(ControllerClientError::MalformedResponse);
            }
            return Err(ControllerClientError::ProtocolMismatch);
        }
        let welcome: star_contracts::ipc::IpcWelcome = serde_json::from_value(welcome_value)
            .map_err(|_| ControllerClientError::MalformedResponse)?;
        let expected_tag = server_auth_tag(key.as_bytes(), &hello.client_nonce, &welcome)
            .map_err(|_| ControllerClientError::Authentication)?;
        verify_auth_tag(&expected_tag, &welcome.auth_tag)
            .map_err(|_| ControllerClientError::Authentication)?;
        if welcome.protocol_version != IPC_PROTOCOL_VERSION {
            return Err(ControllerClientError::ProtocolMismatch);
        }
        if !crate::welcome_shape_valid(&challenge, &hello, &welcome) {
            return Err(ControllerClientError::MalformedResponse);
        }

        let request_id = RequestId::new();
        let response_timeout = response_read_timeout(command, &payload);
        let idempotency_key = payload
            .get("idempotency_key")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let project_root = std::env::current_dir()
            .ok()
            .and_then(|path| path.to_str().map(str::to_owned))
            .ok_or(ControllerClientError::Unavailable)?;
        let request = IpcRequest {
            schema_id: "star.ipc.request".to_owned(),
            schema_version: 1,
            request_id: request_id.clone(),
            command: command.to_owned(),
            payload,
            client_request_id: correlation_id.to_string(),
            idempotency_key,
            deadline: None,
            actor: serde_json::json!({
                "kind": self.config.client_kind,
                "mcp_tool": mcp_tool,
                "project_root": project_root
            }),
            trace_context: None,
        };
        write_typed(&mut pipe, &request).await?;
        let response: IpcResponse = read_typed_with_timeout(&mut pipe, response_timeout).await?;
        if response.schema_id != "star.ipc.response"
            || response.schema_version != 1
            || response.request_id != request_id
            || response.correlation_id != correlation_id.to_string()
        {
            return Err(ControllerClientError::MalformedResponse);
        }
        Ok(response)
    }
}

fn verify_challenge_server_pid(
    challenge: &IpcChallenge,
    actual_server_pid: u32,
) -> Result<(), ControllerClientError> {
    (challenge.server_pid == actual_server_pid)
        .then_some(())
        .ok_or(ControllerClientError::ServerIdentityMismatch)
}

fn map_start_error(error: ControllerStartError) -> ControllerClientError {
    match error {
        ControllerStartError::OuterJobDenied | ControllerStartError::Start => {
            ControllerClientError::Unavailable
        }
        ControllerStartError::IdentityMismatch
        | ControllerStartError::Lease(_)
        | ControllerStartError::InstallManifest => ControllerClientError::ServerIdentityMismatch,
    }
}

async fn write_typed<T: serde::Serialize>(
    pipe: &mut (impl tokio::io::AsyncWrite + Unpin),
    value: &T,
) -> Result<(), ControllerClientError> {
    let value =
        serde_json::to_value(value).map_err(|_| ControllerClientError::MalformedResponse)?;
    write_json(pipe, &value).await.map_err(map_codec_error)
}

async fn read_typed<T: serde::de::DeserializeOwned>(
    pipe: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<T, ControllerClientError> {
    let value = read_json(pipe).await.map_err(map_codec_error)?;
    serde_json::from_value(value).map_err(|_| ControllerClientError::MalformedResponse)
}

async fn read_typed_with_timeout<T: serde::de::DeserializeOwned>(
    pipe: &mut (impl tokio::io::AsyncRead + Unpin),
    timeout: Duration,
) -> Result<T, ControllerClientError> {
    let value = read_json_with_timeout(pipe, timeout)
        .await
        .map_err(map_codec_error)?;
    serde_json::from_value(value).map_err(|_| ControllerClientError::MalformedResponse)
}

fn response_read_timeout(command: &str, payload: &serde_json::Value) -> Duration {
    if command == "operation.get" {
        let wait_ms = payload
            .get("wait_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(30_000);
        return RESPONSE_IO_GRACE + Duration::from_millis(wait_ms);
    }
    if matches!(
        command,
        "tool.search" | "tool.describe" | "tool.registry.status" | "tool.probe"
    ) {
        // A discovery request may perform one trusted compatible-version
        // probe (30 s maximum) after a stabilizing demand scan (5 s maximum).
        return DISCOVERY_PROBE_RESPONSE_BUDGET;
    }
    if matches!(
        command,
        "scan.run" | "patch.apply" | "management.rebuild.apply"
    ) {
        return Duration::from_secs(10 * 60);
    }
    // Every application command performs the bounded demand scan before
    // dispatch, so the transport must not expire at the exact 5 s stability
    // boundary and turn a valid response into an authentication-looking error.
    DEMAND_SCAN_RESPONSE_BUDGET
}

fn map_codec_error(error: IpcCodecError) -> ControllerClientError {
    match error {
        IpcCodecError::Authentication => ControllerClientError::Authentication,
        IpcCodecError::InvalidJson | IpcCodecError::FrameTooLarge | IpcCodecError::Truncated => {
            ControllerClientError::MalformedResponse
        }
    }
}

/// Uses the contract location without exposing a username or project path.
pub fn current_user_pipe_name() -> Result<String, ControllerClientError> {
    let sid_hash = current_user_sid_hash()?;
    Ok(format!(r"\\.\pipe\star-control-{}-v1", sid_hash))
}

pub fn current_user_sid_hash() -> Result<String, ControllerClientError> {
    let sid = current_user_sid_string().map_err(|_| ControllerClientError::Unavailable)?;
    let digest = <sha2::Sha256 as sha2::Digest>::digest(sid.as_bytes());
    Ok(hex::encode(digest)[..16].to_owned())
}

/// Returns the current user SID as the Windows canonical SDDL string.
pub fn current_user_sid_string() -> windows::core::Result<String> {
    use std::ffi::c_void;
    use windows::{
        Win32::{
            Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
            Security::Authorization::ConvertSidToStringSidW,
            Security::{
                GetTokenInformation, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenUser,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
        core::PWSTR,
    };

    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
    }
    let mut required = 0;
    unsafe {
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);
    }
    let mut buffer = vec![0u8; required as usize];
    unsafe {
        GetTokenInformation(
            token,
            TOKEN_INFORMATION_CLASS(TokenUser.0),
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            required,
            &mut required,
        )?;
    }
    let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    let mut sid = PWSTR::null();
    unsafe {
        ConvertSidToStringSidW(user.User.Sid, &mut sid)?;
    }
    let value = unsafe { sid.to_string() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid.0.cast())));
        let _ = CloseHandle(token);
    }
    Ok(value?)
}

/// Convenience default for an MCP process after its bootstrap has selected the
/// installed Controller image.
pub fn mcp_client_config(
    expected_server_image: PathBuf,
) -> Result<ControllerClientConfig, ControllerClientError> {
    Ok(ControllerClientConfig {
        pipe_name: current_user_pipe_name()?,
        expected_server_image,
        key_path: default_key_path().map_err(|_| ControllerClientError::Unavailable)?,
        client_kind: IpcClientKind::Mcp,
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
        client_instance_id: format!("mcp_{}", nonce()),
        capabilities: vec![],
        connect_timeout: Duration::from_millis(5_000),
    })
}

/// Convenience default for the management CLI. Like the MCP configuration it
/// only receives an already selected installed Controller image; it does not
/// discover packages or mutate Controller state locally.
pub fn cli_client_config(
    expected_server_image: PathBuf,
) -> Result<ControllerClientConfig, ControllerClientError> {
    Ok(ControllerClientConfig {
        pipe_name: current_user_pipe_name()?,
        expected_server_image,
        key_path: default_key_path().map_err(|_| ControllerClientError::Unavailable)?,
        client_kind: IpcClientKind::Cli,
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
        client_instance_id: format!("cli_{}", nonce()),
        capabilities: vec![],
        connect_timeout: Duration::from_millis(5_000),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::ipc::{ControllerReadiness, IpcStatus};

    use crate::{ServerHandshake, key_store::load_or_create, windows_pipe::create_server};

    #[tokio::test]
    // matrix: MCP-I001 MCP-I006
    async fn client_verifies_image_hmac_and_response_binding() {
        let pipe_name = format!(r"\\.\pipe\star-control-client-test-{}", nonce());
        let key_path = std::env::temp_dir().join(format!("star-control-client-key-{}.v1", nonce()));
        let key = load_or_create(&key_path).expect("test DPAPI key");
        let server_key = key.as_bytes().to_vec();
        let server_pipe = create_server(&pipe_name).expect("server pipe");
        let server = tokio::spawn(async move {
            let mut pipe = server_pipe;
            pipe.connect().await.expect("client connects");
            let mut handshake = ServerHandshake::issue(
                &server_key,
                "ctl_test".to_owned(),
                std::process::id(),
                "2026-07-11T00:00:00.000Z".to_owned(),
                "0.1.0".to_owned(),
                ControllerReadiness::Ready,
                7,
            );
            let challenge = handshake.challenge().expect("fresh challenge").clone();
            write_typed(&mut pipe, &challenge)
                .await
                .expect("challenge writes");
            let hello: IpcHello = read_typed(&mut pipe).await.expect("hello reads");
            let welcome = handshake
                .accept(
                    &hello,
                    "ses_test".to_owned(),
                    "2026-07-11T00:00:00Z".to_owned(),
                )
                .expect("hello authenticates");
            write_typed(&mut pipe, &welcome)
                .await
                .expect("welcome writes");
            let request: IpcRequest = read_typed(&mut pipe).await.expect("request reads");
            let project_root = request.actor["project_root"]
                .as_str()
                .expect("authenticated clients bind their current project root");
            assert!(PathBuf::from(project_root).is_absolute());
            assert!(PathBuf::from(project_root).is_dir());
            assert_eq!(
                request.idempotency_key.as_deref(),
                Some("client-idempotency")
            );
            let response = IpcResponse {
                schema_id: "star.ipc.response".to_owned(),
                schema_version: 1,
                request_id: request.request_id,
                status: IpcStatus::Ok,
                data: Some(serde_json::json!({"echo":request.payload})),
                operation_id: None,
                diagnostics: vec![],
                error: None,
                registry_revision: Some(7),
                correlation_id: request.client_request_id,
            };
            write_typed(&mut pipe, &response)
                .await
                .expect("response writes");
        });
        let client = ControllerClient::new(ControllerClientConfig {
            pipe_name,
            expected_server_image: std::env::current_exe().expect("test image"),
            key_path,
            client_kind: IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "test-client".to_owned(),
            capabilities: vec![],
            connect_timeout: Duration::from_secs(2),
        });
        let correlation = RequestId::new();
        let response = client
            .call(
                "tool.search",
                serde_json::json!({"query":"test","idempotency_key":"client-idempotency"}),
                correlation.clone(),
            )
            .await
            .expect("authenticated request succeeds");
        assert_eq!(response.status, IpcStatus::Ok);
        assert_eq!(response.correlation_id, correlation.to_string());
        assert_eq!(
            response.data,
            Some(
                serde_json::json!({"echo":{"query":"test","idempotency_key":"client-idempotency"}})
            )
        );
        server.await.expect("server task finishes");
    }

    #[tokio::test]
    async fn operation_long_poll_response_survives_the_default_five_second_io_window() {
        let pipe_name = format!(r"\\.\pipe\star-control-long-poll-test-{}", nonce());
        let key_path =
            std::env::temp_dir().join(format!("star-control-long-poll-key-{}.v1", nonce()));
        let key = load_or_create(&key_path).expect("test DPAPI key");
        let server_key = key.as_bytes().to_vec();
        let server_pipe = create_server(&pipe_name).expect("server pipe");
        let server = tokio::spawn(async move {
            let mut pipe = server_pipe;
            pipe.connect().await.expect("client connects");
            let mut handshake = ServerHandshake::issue(
                &server_key,
                "ctl_long_poll".to_owned(),
                std::process::id(),
                "2026-07-12T00:00:00.000Z".to_owned(),
                "0.1.0".to_owned(),
                ControllerReadiness::Ready,
                11,
            );
            let challenge = handshake.challenge().expect("fresh challenge").clone();
            write_typed(&mut pipe, &challenge).await.unwrap();
            let hello: IpcHello = read_typed(&mut pipe).await.unwrap();
            let welcome = handshake
                .accept(
                    &hello,
                    "ses_long_poll".to_owned(),
                    "2026-07-12T00:00:00Z".to_owned(),
                )
                .unwrap();
            write_typed(&mut pipe, &welcome).await.unwrap();
            let request: IpcRequest = read_typed(&mut pipe).await.unwrap();
            assert_eq!(request.command, "operation.get");
            assert_eq!(request.payload["wait_ms"], 30_000);
            tokio::time::sleep(Duration::from_millis(5_100)).await;
            let response = IpcResponse {
                schema_id: "star.ipc.response".to_owned(),
                schema_version: 1,
                request_id: request.request_id,
                status: IpcStatus::Ok,
                data: Some(serde_json::json!({"wait_timed_out":true})),
                operation_id: None,
                diagnostics: vec![],
                error: None,
                registry_revision: Some(11),
                correlation_id: request.client_request_id,
            };
            write_typed(&mut pipe, &response).await.unwrap();
        });
        let client = ControllerClient::new(ControllerClientConfig {
            pipe_name,
            expected_server_image: std::env::current_exe().unwrap(),
            key_path,
            client_kind: IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "long-poll-client".to_owned(),
            capabilities: vec![],
            connect_timeout: Duration::from_secs(2),
        });
        let response = client
            .call(
                "operation.get",
                serde_json::json!({
                    "operation_id":"opn_01KXA6W7G6GPVKDRE7B68H238J",
                    "after_sequence":0,
                    "wait_ms":30_000
                }),
                RequestId::new(),
            )
            .await
            .expect("the response budget includes the requested long-poll window");
        assert_eq!(response.status, IpcStatus::Ok);
        assert_eq!(response.data.unwrap()["wait_timed_out"], true);
        server.await.unwrap();
    }

    #[test]
    fn sid_hash_endpoint_has_no_user_name_or_path() {
        let endpoint = current_user_pipe_name().expect("current SID is available");
        assert!(endpoint.starts_with(r"\\.\pipe\star-control-"));
        assert!(endpoint.ends_with("-v1"));
        assert_eq!(endpoint.len(), r"\\.\pipe\star-control-".len() + 16 + 3);
    }

    #[test]
    fn response_read_budget_covers_long_poll_demand_scan_and_probe_contracts() {
        assert_eq!(
            response_read_timeout("operation.get", &serde_json::json!({"wait_ms":30_000})),
            Duration::from_secs(35)
        );
        assert_eq!(
            response_read_timeout("operation.get", &serde_json::json!({"wait_ms":99_999})),
            Duration::from_secs(35)
        );
        assert_eq!(
            response_read_timeout("tool.search", &serde_json::json!({"wait_ms":30_000})),
            Duration::from_secs(40)
        );
        assert_eq!(
            response_read_timeout("tool.probe", &serde_json::json!({})),
            Duration::from_secs(40)
        );
        assert_eq!(
            response_read_timeout("tool.invoke", &serde_json::json!({"wait_mode":"accepted"})),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn challenge_pid_must_match_the_verified_pipe_server_pid() {
        let challenge = IpcChallenge::v1(
            "ctl_test".to_owned(),
            41,
            nonce(),
            "2026-07-12T00:00:00.000Z".to_owned(),
        );
        assert!(verify_challenge_server_pid(&challenge, 41).is_ok());
        assert!(matches!(
            verify_challenge_server_pid(&challenge, 42),
            Err(ControllerClientError::ServerIdentityMismatch)
        ));
    }

    #[test]
    fn outer_job_start_denial_is_unavailable_not_an_image_identity_mismatch() {
        assert!(matches!(
            map_start_error(ControllerStartError::OuterJobDenied),
            ControllerClientError::Unavailable
        ));
        assert!(matches!(
            map_start_error(ControllerStartError::IdentityMismatch),
            ControllerClientError::ServerIdentityMismatch
        ));
    }
}
