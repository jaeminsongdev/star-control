//! Windows local named-pipe transport with an owner+LocalSystem DACL.

use std::{
    ffi::c_void,
    mem::size_of,
    ops::{Deref, DerefMut},
    os::windows::io::AsRawHandle,
    pin::Pin,
    task::{Context, Poll},
};

use star_contracts::{ipc::IPC_MAX_FRAME_BYTES, parse_no_duplicate_keys};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, PipeMode, ServerOptions,
    },
};
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree},
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW,
                ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
                SDDL_REVISION_1, SE_KERNEL_OBJECT,
            },
            DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
    },
    core::{PWSTR, w},
};

use crate::{IpcCodecError, encode_frame};

pub const PIPE_BUFFER_BYTES: u32 = 64 * 1024;
pub const PIPE_MAX_INSTANCES: usize = 16;
const PIPE_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

type AcceptedPipe = std::io::Result<NamedPipeServer>;

pub struct PipeAcceptPool {
    name: String,
    sender: tokio::sync::mpsc::Sender<AcceptedPipe>,
    receiver: tokio::sync::mpsc::Receiver<AcceptedPipe>,
}

pub struct PooledConnection {
    pipe: Option<NamedPipeServer>,
    name: String,
    sender: tokio::sync::mpsc::Sender<AcceptedPipe>,
}

impl PipeAcceptPool {
    pub fn start(name: String) -> std::io::Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::channel(PIPE_MAX_INSTANCES);
        for _ in 0..PIPE_MAX_INSTANCES {
            spawn_accept(create_server(&name)?, sender.clone());
        }
        Ok(Self {
            name,
            sender,
            receiver,
        })
    }

    pub async fn accept(&mut self) -> std::io::Result<PooledConnection> {
        let pipe = self
            .receiver
            .recv()
            .await
            .ok_or_else(|| std::io::Error::other("named-pipe accept pool stopped"))??;
        Ok(PooledConnection {
            pipe: Some(pipe),
            name: self.name.clone(),
            sender: self.sender.clone(),
        })
    }
}

impl Deref for PooledConnection {
    type Target = NamedPipeServer;
    fn deref(&self) -> &Self::Target {
        self.pipe.as_ref().expect("pooled pipe is live")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.pipe.as_mut().expect("pooled pipe is live")
    }
}

impl AsyncRead for PooledConnection {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(self.get_mut().pipe.as_mut().expect("pooled pipe is live"))
            .poll_read(context, buffer)
    }
}

impl AsyncWrite for PooledConnection {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(self.get_mut().pipe.as_mut().expect("pooled pipe is live"))
            .poll_write(context, buffer)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(self.get_mut().pipe.as_mut().expect("pooled pipe is live")).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(self.get_mut().pipe.as_mut().expect("pooled pipe is live")).poll_shutdown(context)
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        drop(self.pipe.take());
        let name = self.name.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            // The closed instance can remain observable for a scheduler tick.
            // Retry is bounded by receiver lifetime and never creates more
            // than the frozen 16 instances.
            loop {
                if sender.is_closed() {
                    break;
                }
                match create_server(&name) {
                    Ok(server) => {
                        spawn_accept(server, sender);
                        break;
                    }
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(5)).await,
                }
            }
        });
    }
}

fn spawn_accept(server: NamedPipeServer, sender: tokio::sync::mpsc::Sender<AcceptedPipe>) {
    tokio::spawn(async move {
        let accepted = server.connect().await.map(|()| server);
        let _ = sender.send(accepted).await;
    });
}

/// The pipe owner is the Controller's current user. `OW` grants that owner
/// full control, and `SY` keeps LocalSystem available for installer recovery.
const OWNER_AND_SYSTEM_DACL: windows::core::PCWSTR = w!("D:P(A;;GA;;;OW)(A;;GA;;;SY)");

struct SecurityDescriptor(PSECURITY_DESCRIPTOR);
impl SecurityDescriptor {
    fn owner_and_system() -> windows::core::Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                OWNER_AND_SYSTEM_DACL,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
        }
        Ok(Self(descriptor))
    }
    fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: windows::core::BOOL(0),
        }
    }
}
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0.0 as *mut _)));
        }
    }
}

pub fn create_server(name: &str) -> std::io::Result<NamedPipeServer> {
    let mut options = ServerOptions::new();
    options
        .pipe_mode(PipeMode::Byte)
        .access_inbound(true)
        .access_outbound(true)
        .reject_remote_clients(true)
        .max_instances(PIPE_MAX_INSTANCES)
        .in_buffer_size(PIPE_BUFFER_BYTES)
        .out_buffer_size(PIPE_BUFFER_BYTES);
    let mut descriptor = SecurityDescriptor::owner_and_system().map_err(std::io::Error::other)?;
    let mut attributes = descriptor.attributes();
    unsafe {
        options.create_with_security_attributes_raw(
            name,
            (&mut attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
        )
    }
}

pub fn pipe_dacl_sddl(pipe: &NamedPipeServer) -> std::io::Result<String> {
    let handle = HANDLE(pipe.as_raw_handle().cast());
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            Some(&mut descriptor),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status.0 as i32));
    }
    let mut sddl = PWSTR::null();
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut sddl,
            None,
        )
    };
    if let Err(error) = converted {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        }
        return Err(std::io::Error::other(error));
    }
    let value = unsafe { sddl.to_string() }.map_err(std::io::Error::other)?;
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sddl.0.cast())));
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
    }
    Ok(value)
}

pub fn open_client(name: &str) -> std::io::Result<NamedPipeClient> {
    ClientOptions::new().open(name)
}

pub async fn write_json(
    pipe: &mut (impl AsyncWriteExt + Unpin),
    value: &serde_json::Value,
) -> Result<(), IpcCodecError> {
    let json = serde_json::to_vec(value).map_err(|_| IpcCodecError::InvalidJson)?;
    let frame = encode_frame(&json)?;
    tokio::time::timeout(PIPE_IO_TIMEOUT, pipe.write_all(&frame))
        .await
        .map_err(|_| IpcCodecError::Truncated)?
        .map_err(|_| IpcCodecError::Truncated)?;
    tokio::time::timeout(PIPE_IO_TIMEOUT, pipe.flush())
        .await
        .map_err(|_| IpcCodecError::Truncated)?
        .map_err(|_| IpcCodecError::Truncated)
}

pub async fn read_json(
    pipe: &mut (impl AsyncReadExt + Unpin),
) -> Result<serde_json::Value, IpcCodecError> {
    read_json_with_timeout(pipe, PIPE_IO_TIMEOUT).await
}

pub async fn read_json_with_timeout(
    pipe: &mut (impl AsyncReadExt + Unpin),
    timeout: std::time::Duration,
) -> Result<serde_json::Value, IpcCodecError> {
    let mut prefix = [0; 4];
    tokio::time::timeout(timeout, pipe.read_exact(&mut prefix))
        .await
        .map_err(|_| IpcCodecError::Truncated)?
        .map_err(|_| IpcCodecError::Truncated)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > IPC_MAX_FRAME_BYTES {
        return Err(IpcCodecError::FrameTooLarge);
    }
    let mut payload = vec![0; length];
    tokio::time::timeout(timeout, pipe.read_exact(&mut payload))
        .await
        .map_err(|_| IpcCodecError::Truncated)?
        .map_err(|_| IpcCodecError::Truncated)?;
    let text = std::str::from_utf8(&payload).map_err(|_| IpcCodecError::InvalidJson)?;
    parse_no_duplicate_keys(text).map_err(|_| IpcCodecError::InvalidJson)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    // matrix: MCP-I003 MCP-I012
    async fn owner_dacl_pipe_round_trips_a_bounded_json_frame() {
        let name = format!(r"\\.\pipe\star-control-ipc-test-{}", std::process::id());
        let mut server = create_server(&name).expect("DACL named pipe creates");
        let dacl = pipe_dacl_sddl(&server).expect("pipe DACL can be inspected");
        assert!(dacl.starts_with("D:P"));
        assert_eq!(dacl.matches("(A;").count(), 2);
        assert!(!dacl.contains(";;;WD)"));
        let client_name = name.clone();
        let client = tokio::spawn(async move {
            let mut client = open_client(&client_name).expect("same-user client opens DACL pipe");
            write_json(&mut client, &serde_json::json!({"request":"hello"}))
                .await
                .unwrap();
            read_json(&mut client).await.unwrap()
        });
        server.connect().await.expect("client connects");
        assert_eq!(
            read_json(&mut server).await.unwrap(),
            serde_json::json!({"request":"hello"})
        );
        write_json(&mut server, &serde_json::json!({"response":"welcome"}))
            .await
            .unwrap();
        assert_eq!(
            client.await.unwrap(),
            serde_json::json!({"response":"welcome"})
        );
    }

    #[tokio::test]
    // matrix: MCP-I007 MCP-I008
    async fn sixteen_clients_are_bounded_and_the_seventeenth_gets_backpressure() {
        let name = format!(r"\\.\pipe\star-control-pool-test-{}", crate::nonce());
        let mut pool = PipeAcceptPool::start(name.clone()).unwrap();
        let mut clients = Vec::new();
        for request_id in 0..PIPE_MAX_INSTANCES {
            let mut client = open_client(&name).expect("one of sixteen clients connects");
            write_json(&mut client, &serde_json::json!({"request_id":request_id}))
                .await
                .unwrap();
            clients.push((request_id, client));
        }
        let seventeenth = open_client(&name);
        assert!(
            seventeenth.is_err(),
            "the seventeenth client must not be unbounded"
        );

        for expected in 0..PIPE_MAX_INSTANCES {
            let mut server = pool.accept().await.unwrap();
            let request = read_json(&mut server).await.unwrap();
            let request_id = request["request_id"].as_u64().unwrap() as usize;
            write_json(
                &mut server,
                &serde_json::json!({"request_id":request_id,"ok":true}),
            )
            .await
            .unwrap();
            let (client_id, client) = clients
                .iter_mut()
                .find(|(client_id, _)| *client_id == request_id)
                .unwrap();
            assert_eq!(*client_id, request_id);
            assert_eq!(
                read_json(client).await.unwrap(),
                serde_json::json!({"request_id":request_id,"ok":true})
            );
            assert!(request_id < PIPE_MAX_INSTANCES, "response IDs never mix");
            assert!(expected < PIPE_MAX_INSTANCES);
        }
    }

    #[tokio::test]
    // matrix: MCP-I005
    async fn pipe_reader_rejects_bounds_truncation_and_duplicate_keys_before_dispatch() {
        for invalid_length in [0_u32, (IPC_MAX_FRAME_BYTES as u32) + 1, u32::MAX] {
            let (mut writer, mut reader) = tokio::io::duplex(16);
            writer
                .write_all(&invalid_length.to_le_bytes())
                .await
                .unwrap();
            assert!(matches!(
                read_json(&mut reader).await,
                Err(IpcCodecError::FrameTooLarge)
            ));
        }

        let duplicate = br#"{"request_id":1,"request_id":2}"#;
        let (mut writer, mut reader) = tokio::io::duplex(128);
        writer
            .write_all(&(duplicate.len() as u32).to_le_bytes())
            .await
            .unwrap();
        writer.write_all(duplicate).await.unwrap();
        assert!(matches!(
            read_json(&mut reader).await,
            Err(IpcCodecError::InvalidJson)
        ));

        let (mut writer, mut reader) = tokio::io::duplex(16);
        writer.write_all(&8_u32.to_le_bytes()).await.unwrap();
        writer.write_all(b"{}").await.unwrap();
        drop(writer);
        assert!(matches!(
            read_json(&mut reader).await,
            Err(IpcCodecError::Truncated)
        ));
    }
}
