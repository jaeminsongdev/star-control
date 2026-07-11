use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::{Duration, timeout},
};

async fn read_line(reader: &mut BufReader<tokio::process::ChildStdout>) -> String {
    let mut line = String::new();
    timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .expect("star-mcp response timeout")
        .expect("star-mcp stdout is readable");
    line
}

#[tokio::test]
// matrix: MCP-G004
async fn preinitialize_tools_list_is_an_error_but_the_stdio_server_stays_alive() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_star-mcp"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("star-mcp starts");
    let mut stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let mut stdout = BufReader::new(stdout);

    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let rejected: serde_json::Value = serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(rejected["error"]["code"], -32600);

    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"0.1.0"}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let initialized: serde_json::Value =
        serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(initialized["id"], 2);
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");

    child.kill().await.expect("star-mcp terminates after smoke");
}

#[tokio::test]
// matrix: MCP-G010
async fn stdin_eof_terminates_the_gateway_promptly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_star-mcp"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("star-mcp starts");
    drop(child.stdin.take());
    let _status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("gateway exits after stdin EOF")
        .expect("gateway wait succeeds");
}
