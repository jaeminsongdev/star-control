//! Authenticated Local IPC v1 primitives.
//!
//! The Windows named-pipe adapter owns I/O; this crate owns only bounded frame
//! decoding, challenge-response construction and constant-time HMAC checks.

use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use star_contracts::{
    canonical::jcs_bytes,
    ipc::{
        ControllerReadiness, IPC_MAX_FRAME_BYTES, IpcChallenge, IpcHandshakeError, IpcHello,
        IpcWelcome,
    },
};
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::Zeroizing;

#[cfg(windows)]
pub mod client;
#[cfg(windows)]
pub mod controller_start;
#[cfg(windows)]
pub mod dpapi;
#[cfg(windows)]
pub mod key_store;
#[cfg(windows)]
pub mod process_identity;
#[cfg(windows)]
pub mod windows_pipe;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum IpcCodecError {
    #[error("IPC frame exceeds the 8 MiB contract bound")]
    FrameTooLarge,
    #[error("IPC frame is truncated")]
    Truncated,
    #[error("IPC frame is not UTF-8 JSON")]
    InvalidJson,
    #[error("IPC authentication failed")]
    Authentication,
}

/// Four-byte little-endian length framing.  The caller must enforce one
/// in-flight request per connection when backpressure reaches this bound.
pub fn encode_frame(json: &[u8]) -> Result<Vec<u8>, IpcCodecError> {
    if json.is_empty() || json.len() > IPC_MAX_FRAME_BYTES {
        return Err(IpcCodecError::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&(json.len() as u32).to_le_bytes());
    frame.extend_from_slice(json);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<&[u8], IpcCodecError> {
    if frame.len() < 4 {
        return Err(IpcCodecError::Truncated);
    }
    let length = u32::from_le_bytes(frame[..4].try_into().expect("prefix length")) as usize;
    if length == 0 || length > IPC_MAX_FRAME_BYTES {
        return Err(IpcCodecError::FrameTooLarge);
    }
    if frame.len() != length + 4 {
        return Err(IpcCodecError::Truncated);
    }
    std::str::from_utf8(&frame[4..]).map_err(|_| IpcCodecError::InvalidJson)?;
    Ok(&frame[4..])
}

pub fn nonce() -> String {
    let bytes: [u8; 32] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn client_auth_tag(
    key: &[u8],
    challenge: &IpcChallenge,
    hello: &IpcHello,
) -> Result<String, IpcCodecError> {
    let mut unsigned_hello = serde_json::to_value(hello).map_err(|_| IpcCodecError::InvalidJson)?;
    unsigned_hello
        .as_object_mut()
        .expect("IpcHello serializes as an object")
        .remove("auth_tag");
    jcs_auth_tag(
        key,
        b"client-v1\n",
        &[
            serde_json::to_value(challenge).map_err(|_| IpcCodecError::InvalidJson)?,
            unsigned_hello,
        ],
    )
}
pub fn server_auth_tag(
    key: &[u8],
    client_nonce: &str,
    welcome: &IpcWelcome,
) -> Result<String, IpcCodecError> {
    server_auth_tag_value(
        key,
        client_nonce,
        serde_json::to_value(welcome).map_err(|_| IpcCodecError::InvalidJson)?,
    )
}
pub fn server_auth_tag_value(
    key: &[u8],
    client_nonce: &str,
    mut unsigned_message: serde_json::Value,
) -> Result<String, IpcCodecError> {
    let client_nonce = URL_SAFE_NO_PAD
        .decode(client_nonce)
        .map_err(|_| IpcCodecError::Authentication)?;
    unsigned_message
        .as_object_mut()
        .expect("authenticated IPC message serializes as an object")
        .remove("auth_tag");
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(b"server-v1\n");
    mac.update(&client_nonce);
    mac.update(&jcs_bytes(&unsigned_message).map_err(|_| IpcCodecError::InvalidJson)?);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}
fn jcs_auth_tag(
    key: &[u8],
    domain: &[u8],
    values: &[serde_json::Value],
) -> Result<String, IpcCodecError> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(domain);
    for value in values {
        mac.update(&jcs_bytes(value).map_err(|_| IpcCodecError::InvalidJson)?);
    }
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}
pub fn verify_auth_tag(expected: &str, received: &str) -> Result<(), IpcCodecError> {
    let expected = URL_SAFE_NO_PAD
        .decode(expected)
        .map_err(|_| IpcCodecError::Authentication)?;
    let received = URL_SAFE_NO_PAD
        .decode(received)
        .map_err(|_| IpcCodecError::Authentication)?;
    if expected.len() != received.len() {
        return Err(IpcCodecError::Authentication);
    }
    bool::from(expected.ct_eq(&received))
        .then_some(())
        .ok_or(IpcCodecError::Authentication)
}

pub const IPC_PROTOCOL_VERSION: &str = "1.0";
pub const CHALLENGE_TTL: Duration = Duration::from_secs(5);

/// One connection's server-side handshake. A challenge is consumed even when
/// validation fails, preventing replay on another connection.
pub struct ServerHandshake<'a> {
    key: &'a [u8],
    challenge: Option<IpcChallenge>,
    controller_version: String,
    readiness: ControllerReadiness,
    registry_revision: u64,
    issued_monotonic: Instant,
}
pub enum HandshakeOutcome {
    Welcome(IpcWelcome),
    ProtocolMismatch(IpcHandshakeError),
}

struct HandshakeIssue {
    controller_instance_id: String,
    server_pid: u32,
    issued_at: String,
    controller_version: String,
    readiness: ControllerReadiness,
    registry_revision: u64,
}

impl<'a> ServerHandshake<'a> {
    pub fn issue(
        key: &'a [u8],
        controller_instance_id: String,
        server_pid: u32,
        issued_at: String,
        controller_version: String,
        readiness: ControllerReadiness,
        registry_revision: u64,
    ) -> Self {
        Self::issue_at(
            key,
            HandshakeIssue {
                controller_instance_id,
                server_pid,
                issued_at,
                controller_version,
                readiness,
                registry_revision,
            },
            Instant::now(),
        )
    }
    fn issue_at(key: &'a [u8], issue: HandshakeIssue, issued_monotonic: Instant) -> Self {
        Self {
            key,
            challenge: Some(IpcChallenge::v1(
                issue.controller_instance_id,
                issue.server_pid,
                nonce(),
                issue.issued_at,
            )),
            controller_version: issue.controller_version,
            readiness: issue.readiness,
            registry_revision: issue.registry_revision,
            issued_monotonic,
        }
    }
    pub fn challenge(&self) -> Option<&IpcChallenge> {
        self.challenge.as_ref()
    }
    pub fn accept(
        &mut self,
        hello: &IpcHello,
        session_id: String,
        server_time: String,
    ) -> Result<IpcWelcome, IpcCodecError> {
        match self.accept_negotiated(hello, session_id, server_time)? {
            HandshakeOutcome::Welcome(welcome) => Ok(welcome),
            HandshakeOutcome::ProtocolMismatch(_) => Err(IpcCodecError::Authentication),
        }
    }
    pub fn accept_negotiated(
        &mut self,
        hello: &IpcHello,
        session_id: String,
        server_time: String,
    ) -> Result<HandshakeOutcome, IpcCodecError> {
        let challenge = self.challenge.take().ok_or(IpcCodecError::Authentication)?;
        if self.issued_monotonic.elapsed() > CHALLENGE_TTL {
            return Err(IpcCodecError::Authentication);
        }
        if hello.schema_id != "star.ipc.hello"
            || hello.schema_version != 1
            || hello.server_nonce != challenge.server_nonce
        {
            return Err(IpcCodecError::Authentication);
        }
        if !hello
            .protocol_versions
            .iter()
            .any(|version| version == IPC_PROTOCOL_VERSION)
        {
            let mut error = IpcHandshakeError {
                schema_id: "star.ipc.handshake-error".to_owned(),
                schema_version: 1,
                code: "IPC_PROTOCOL_MISMATCH".to_owned(),
                supported_versions: vec![IPC_PROTOCOL_VERSION.to_owned()],
                correlation_id: hello.correlation_id.clone(),
                auth_tag: String::new(),
            };
            error.auth_tag = server_auth_tag_value(
                self.key,
                &hello.client_nonce,
                serde_json::to_value(&error).map_err(|_| IpcCodecError::InvalidJson)?,
            )?;
            return Ok(HandshakeOutcome::ProtocolMismatch(error));
        }
        let expected = client_auth_tag(self.key, &challenge, hello)?;
        verify_auth_tag(&expected, &hello.auth_tag)?;
        let mut welcome = IpcWelcome {
            schema_id: "star.ipc.welcome".to_owned(),
            schema_version: 1,
            protocol_version: IPC_PROTOCOL_VERSION.to_owned(),
            controller_version: self.controller_version.clone(),
            controller_instance_id: challenge.controller_instance_id,
            session_id,
            server_nonce: challenge.server_nonce,
            auth_tag: String::new(),
            readiness: self.readiness.clone(),
            capabilities: vec![],
            registry_revision: self.registry_revision,
            server_time,
        };
        welcome.auth_tag = server_auth_tag(self.key, &hello.client_nonce, &welcome)?;
        Ok(HandshakeOutcome::Welcome(welcome))
    }
}

pub fn verify_welcome(
    key: &[u8],
    hello: &IpcHello,
    welcome: &IpcWelcome,
) -> Result<(), IpcCodecError> {
    if welcome.schema_id != "star.ipc.welcome"
        || welcome.schema_version != 1
        || welcome.protocol_version != IPC_PROTOCOL_VERSION
        || welcome.server_nonce != hello.server_nonce
    {
        return Err(IpcCodecError::Authentication);
    }
    verify_auth_tag(
        &server_auth_tag(key, &hello.client_nonce, welcome)?,
        &welcome.auth_tag,
    )
}

/// Temporary holder for a DPAPI-unsealed per-user key.  The Windows adapter is
/// responsible for DPAPI and user DACL persistence; this type ensures callers
/// do not accidentally retain a plain `Vec<u8>`.
pub struct IpcKey(Zeroizing<Vec<u8>>);
impl IpcKey {
    pub fn from_unsealed(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // matrix: MCP-I005
    fn frame_round_trip_and_bounds() {
        let frame = encode_frame(br#"{"ok":true}"#).unwrap();
        assert_eq!(decode_frame(&frame).unwrap(), br#"{"ok":true}"#);
        assert!(decode_frame(&[1, 0, 0, 0]).is_err());
        assert!(decode_frame(&[0, 0, 0, 0]).is_err());
    }
    #[test]
    // matrix: MCP-I002
    fn hmac_is_bound_to_every_handshake_field() {
        let challenge = IpcChallenge::v1(
            "controller".to_owned(),
            1,
            nonce(),
            "2026-07-11T00:00:00.000Z".to_owned(),
        );
        let hello = IpcHello {
            schema_id: "star.ipc.hello".to_owned(),
            schema_version: 1,
            protocol_versions: vec!["1.0".to_owned()],
            client_kind: star_contracts::ipc::IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "a".to_owned(),
            client_pid: 1,
            client_nonce: nonce(),
            server_nonce: challenge.server_nonce.clone(),
            auth_tag: String::new(),
            capabilities: vec![],
            correlation_id: "req_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        };
        let first = client_auth_tag(b"key", &challenge, &hello).unwrap();
        let mut changed = hello.clone();
        changed.client_pid = 2;
        let second = client_auth_tag(b"key", &challenge, &changed).unwrap();
        assert_ne!(first, second);
        assert!(verify_auth_tag(&first, &first).is_ok());
        assert!(verify_auth_tag(&first, &second).is_err());
    }
    #[test]
    // matrix: MCP-I001
    fn handshake_negotiates_once_and_rejects_replay() {
        let mut server = ServerHandshake::issue(
            b"01234567890123456789012345678901",
            "controller".to_owned(),
            77,
            "2026-07-11T00:00:00.000Z".to_owned(),
            "0.1.0".to_owned(),
            ControllerReadiness::Ready,
            4,
        );
        let challenge = server.challenge().unwrap().clone();
        let mut hello = IpcHello {
            schema_id: "star.ipc.hello".to_owned(),
            schema_version: 1,
            protocol_versions: vec![IPC_PROTOCOL_VERSION.to_owned()],
            client_kind: star_contracts::ipc::IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "client".to_owned(),
            client_pid: 88,
            client_nonce: nonce(),
            server_nonce: challenge.server_nonce.clone(),
            auth_tag: String::new(),
            capabilities: vec![],
            correlation_id: "req_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        };
        hello.auth_tag =
            client_auth_tag(b"01234567890123456789012345678901", &challenge, &hello).unwrap();
        let welcome = server
            .accept(
                &hello,
                "session".to_owned(),
                "2026-07-11T00:00:00.000Z".to_owned(),
            )
            .unwrap();
        verify_welcome(b"01234567890123456789012345678901", &hello, &welcome).unwrap();
        assert!(
            server
                .accept(
                    &hello,
                    "second".to_owned(),
                    "2026-07-11T00:00:00.000Z".to_owned()
                )
                .is_err()
        );
    }
    #[test]
    // matrix: MCP-I004
    fn handshake_returns_authenticated_protocol_mismatch() {
        let key = b"01234567890123456789012345678901";
        let mut server = ServerHandshake::issue(
            key,
            "controller".to_owned(),
            77,
            "2026-07-11T00:00:00.000Z".to_owned(),
            "0.1.0".to_owned(),
            ControllerReadiness::Ready,
            4,
        );
        let challenge = server.challenge().unwrap().clone();
        let mut hello = IpcHello {
            schema_id: "star.ipc.hello".to_owned(),
            schema_version: 1,
            protocol_versions: vec!["2.0".to_owned()],
            client_kind: star_contracts::ipc::IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "client".to_owned(),
            client_pid: 88,
            client_nonce: nonce(),
            server_nonce: challenge.server_nonce.clone(),
            auth_tag: String::new(),
            capabilities: vec![],
            correlation_id: "req_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        };
        hello.auth_tag = client_auth_tag(key, &challenge, &hello).unwrap();
        let HandshakeOutcome::ProtocolMismatch(error) = server
            .accept_negotiated(
                &hello,
                "session".to_owned(),
                "2026-07-11T00:00:00.000Z".to_owned(),
            )
            .unwrap()
        else {
            panic!("unsupported major must return handshake error");
        };
        assert_eq!(error.code, "IPC_PROTOCOL_MISMATCH");
        assert_eq!(error.supported_versions, [IPC_PROTOCOL_VERSION]);
        let expected = server_auth_tag_value(
            key,
            &hello.client_nonce,
            serde_json::to_value(&error).unwrap(),
        )
        .unwrap();
        verify_auth_tag(&expected, &error.auth_tag).unwrap();
    }
    #[test]
    fn handshake_rejects_an_expired_challenge() {
        let mut server = ServerHandshake::issue_at(
            b"01234567890123456789012345678901",
            HandshakeIssue {
                controller_instance_id: "controller".to_owned(),
                server_pid: 77,
                issued_at: "2026-07-11T00:00:00.000Z".to_owned(),
                controller_version: "0.1.0".to_owned(),
                readiness: ControllerReadiness::Ready,
                registry_revision: 4,
            },
            Instant::now() - CHALLENGE_TTL - Duration::from_millis(1),
        );
        let challenge = server.challenge().unwrap().clone();
        let mut hello = IpcHello {
            schema_id: "star.ipc.hello".to_owned(),
            schema_version: 1,
            protocol_versions: vec![IPC_PROTOCOL_VERSION.to_owned()],
            client_kind: star_contracts::ipc::IpcClientKind::InternalTest,
            client_version: "0.1.0".to_owned(),
            client_instance_id: "client".to_owned(),
            client_pid: 88,
            client_nonce: nonce(),
            server_nonce: challenge.server_nonce.clone(),
            auth_tag: String::new(),
            capabilities: vec![],
            correlation_id: "req_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        };
        hello.auth_tag =
            client_auth_tag(b"01234567890123456789012345678901", &challenge, &hello).unwrap();
        assert!(
            server
                .accept(
                    &hello,
                    "session".to_owned(),
                    "2026-07-11T00:00:00.000Z".to_owned()
                )
                .is_err()
        );
    }
}
