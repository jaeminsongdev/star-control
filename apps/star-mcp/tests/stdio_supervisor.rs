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

async fn initialize(
    stdin: &mut tokio::process::ChildStdin,
    stdout: &mut BufReader<tokio::process::ChildStdout>,
) {
    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":100,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"0.1.0"}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let initialized: serde_json::Value = serde_json::from_str(&read_line(stdout).await).unwrap();
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    stdin
        .write_all(br#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
}

#[tokio::test]
async fn preinitialize_notifications_are_silent_and_do_not_block_initialize() {
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
        .write_all(br#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":9,"reason":"preinitialize"}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"0.1.0"}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();

    let response: serde_json::Value = serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(
        response["id"], 1,
        "notifications must never receive a response"
    );
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");

    child.kill().await.expect("star-mcp terminates after smoke");
}

#[tokio::test]
async fn request_shaped_initialized_message_cannot_transition_the_lifecycle() {
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
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"0.1.0"}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let initialized: serde_json::Value =
        serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(initialized["id"], 1);

    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":2,"method":"notifications/initialized","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();

    let malformed: serde_json::Value = serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(malformed["id"], 2);
    assert_eq!(malformed["error"]["code"], -32600);
    let still_waiting: serde_json::Value =
        serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(still_waiting["id"], 3);
    assert_eq!(still_waiting["error"]["code"], -32600);

    stdin
        .write_all(br#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let ready: serde_json::Value = serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(ready["id"], 4);
    assert_eq!(ready["result"]["tools"].as_array().map(Vec::len), Some(12));

    child.kill().await.expect("star-mcp terminates after smoke");
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
        .write_all(br#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let rejected: serde_json::Value = serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(rejected["error"]["code"], -32602);

    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"0.1.0"}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let initialized: serde_json::Value =
        serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(initialized["id"], 3);
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

#[tokio::test]
// matrix: MCP-G019 MCP-G022
async fn unadvertised_surfaces_are_method_not_found_and_task_calls_are_rejected_locally() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_star-mcp"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("star-mcp starts");
    let mut stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let mut stdout = BufReader::new(stdout);
    initialize(&mut stdin, &mut stdout).await;

    for (id, method) in [
        (1, "resources/list"),
        (2, "prompts/list"),
        (3, "logging/setLevel"),
        (4, "completion/complete"),
        (5, "tasks/list"),
    ] {
        let request = serde_json::json!({
            "jsonrpc":"2.0",
            "id":id,
            "method":method,
            "params":{}
        });
        stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();
        stdin.flush().await.unwrap();
        let response: serde_json::Value =
            serde_json::from_str(&read_line(&mut stdout).await).unwrap();
        assert_eq!(response["id"], id);
        assert_eq!(response["error"]["code"], -32601, "method={method}");
    }

    stdin
        .write_all(br#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"star_tool_search","arguments":{"query":"x"},"task":{}}}"#)
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let task_rejected: serde_json::Value =
        serde_json::from_str(&read_line(&mut stdout).await).unwrap();
    assert_eq!(task_rejected["error"]["code"], -32602);

    child.kill().await.expect("star-mcp terminates after smoke");
    let mut stderr = BufReader::new(child.stderr.take().expect("child stderr"));
    let mut line = String::new();
    while stderr.read_line(&mut line).await.unwrap() != 0 {
        serde_json::from_str::<serde_json::Value>(line.trim_end())
            .expect("every stderr record is JSONL");
        line.clear();
    }
}
