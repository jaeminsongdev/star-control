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
    key_store::{default_key_path, load},
    nonce,
    process_identity::verify_pipe_server_image,
    server_auth_tag, verify_auth_tag,
    windows_pipe::{open_client, read_json, write_json},
};

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
        match self
            .call(command, payload.clone(), correlation_id.clone())
            .await
        {
            Ok(response) => Ok(response),
            Err(ControllerClientError::Unavailable) => {
                bootstrap
                    .start_background()
                    .map_err(|_| ControllerClientError::ServerIdentityMismatch)?;
                let deadline = tokio::time::Instant::now() + self.config.connect_timeout;
                loop {
                    match self
                        .call(command, payload.clone(), correlation_id.clone())
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
        verify_pipe_server_image(&pipe, &self.config.expected_server_image)
            .map_err(|_| ControllerClientError::ServerIdentityMismatch)?;

        let challenge: IpcChallenge = read_typed(&mut pipe).await?;
        if challenge.schema_id != "star.ipc.challenge"
            || challenge.schema_version != 1
            || challenge.protocol_major != IPC_PROTOCOL_MAJOR
        {
            return Err(ControllerClientError::ProtocolMismatch);
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
            if error.schema_version != 1
                || error.code != "IPC_PROTOCOL_MISMATCH"
                || error.correlation_id != correlation_id.to_string()
            {
                return Err(ControllerClientError::MalformedResponse);
            }
            let expected_tag = crate::server_auth_tag_value(
                key.as_bytes(),
                &hello.client_nonce,
                serde_json::to_value(&error)
                    .map_err(|_| ControllerClientError::MalformedResponse)?,
            )
            .map_err(|_| ControllerClientError::Authentication)?;
            verify_auth_tag(&expected_tag, &error.auth_tag)
                .map_err(|_| ControllerClientError::Authentication)?;
            return Err(ControllerClientError::ProtocolMismatch);
        }
        let welcome: star_contracts::ipc::IpcWelcome = serde_json::from_value(welcome_value)
            .map_err(|_| ControllerClientError::MalformedResponse)?;
        if welcome.schema_id != "star.ipc.welcome"
            || welcome.schema_version != 1
            || welcome.protocol_version != IPC_PROTOCOL_VERSION
            || welcome.server_nonce != challenge.server_nonce
        {
            return Err(ControllerClientError::ProtocolMismatch);
        }
        let expected_tag = server_auth_tag(key.as_bytes(), &hello.client_nonce, &welcome)
            .map_err(|_| ControllerClientError::Authentication)?;
        verify_auth_tag(&expected_tag, &welcome.auth_tag)
            .map_err(|_| ControllerClientError::Authentication)?;

        let request_id = RequestId::new();
        let request = IpcRequest {
            schema_id: "star.ipc.request".to_owned(),
            schema_version: 1,
            request_id: request_id.clone(),
            command: command.to_owned(),
            payload,
            client_request_id: correlation_id.to_string(),
            idempotency_key: None,
            deadline: None,
            actor: serde_json::json!({"kind": self.config.client_kind, "mcp_tool": mcp_tool}),
            trace_context: None,
        };
        write_typed(&mut pipe, &request).await?;
        let response: IpcResponse = read_typed(&mut pipe).await?;
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
                serde_json::json!({"query":"test"}),
                correlation.clone(),
            )
            .await
            .expect("authenticated request succeeds");
        assert_eq!(response.status, IpcStatus::Ok);
        assert_eq!(response.correlation_id, correlation.to_string());
        assert_eq!(
            response.data,
            Some(serde_json::json!({"echo":{"query":"test"}}))
        );
        server.await.expect("server task finishes");
    }

    #[test]
    fn sid_hash_endpoint_has_no_user_name_or_path() {
        let endpoint = current_user_pipe_name().expect("current SID is available");
        assert!(endpoint.starts_with(r"\\.\pipe\star-control-"));
        assert!(endpoint.ends_with("-v1"));
        assert_eq!(endpoint.len(), r"\\.\pipe\star-control-".len() + 16 + 3);
    }
}
