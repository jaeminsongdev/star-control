//! Test-only external EXE fixture.  It accepts no shell syntax and speaks the
//! minimal two protocols needed by the Controller integration tests.

use std::io::{Read, Write};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "argv" => {
            let args: Vec<_> = std::env::args().skip(2).collect();
            println!("argv:{}", args.join("|"));
        }
        "env-probe" => {
            for name in ["PATH", "STAR_TEST_ALLOWED", "STAR_TEST_UNDECLARED"] {
                let value = std::env::var(name).unwrap_or_else(|_| "<missing>".to_owned());
                println!("{name}={value}");
            }
        }
        "probe-version" => println!("1.2.3 interface=1.0.0"),
        "handle-probe" => {
            let raw = std::env::args()
                .nth(2)
                .expect("raw handle argument")
                .parse::<usize>()
                .expect("numeric raw handle");
            let mut flags = 0_u32;
            let inherited = unsafe {
                windows::Win32::Foundation::GetHandleInformation(
                    windows::Win32::Foundation::HANDLE(raw as *mut core::ffi::c_void),
                    &mut flags,
                )
            }
            .is_ok();
            println!(
                "{}",
                if inherited {
                    "unexpected-inherited-handle"
                } else {
                    "not-inherited"
                }
            );
        }
        "appcontainer-probe" => {
            let outside_path = std::env::args().nth(2).expect("outside path");
            let port = std::env::args()
                .nth(3)
                .expect("loopback port")
                .parse::<u16>()
                .expect("numeric port");
            let path_denied = std::fs::read(outside_path).is_err();
            let network_denied = std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                std::time::Duration::from_millis(300),
            )
            .is_err();
            println!(
                "{}",
                serde_json::json!({
                    "path_denied":path_denied,
                    "network_denied":network_denied
                })
            );
        }
        "marker-sleep" => {
            let marker = std::env::args().nth(2).expect("marker path");
            std::fs::write(marker, b"started").expect("marker writes");
            std::thread::sleep(std::time::Duration::from_millis(500));
            println!("original-image");
        }
        "record-pid-sleep" => {
            let pid_file = std::env::args().nth(2).expect("pid file path");
            std::fs::write(pid_file, std::process::id().to_string()).expect("pid writes");
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        "runtime-parent" => {
            use star_controller::process_runtime::{DirectExeSpec, execute_direct_exe};

            let pid_file = std::env::args().nth(2).expect("pid file path");
            let executable = std::env::current_exe().expect("current executable");
            let spec = DirectExeSpec {
                working_directory: executable
                    .parent()
                    .expect("executable parent")
                    .to_path_buf(),
                executable,
                argv: vec!["record-pid-sleep".into(), pid_file.into()],
                environment: vec![],
                stdin: None,
                timeout: std::time::Duration::from_secs(60),
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
                appcontainer_profile: None,
            };
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("test runtime creates");
            let _ = runtime.block_on(execute_direct_exe(&spec));
        }
        "flood" => {
            let mut stdout = std::io::stdout().lock();
            for _ in 0..32 {
                stdout.write_all(&[b'x'; 16 * 1024]).expect("stdout writes");
            }
        }
        "flood-both" => {
            let stderr = std::thread::spawn(|| {
                let mut stderr = std::io::stderr().lock();
                for _ in 0..32 {
                    stderr.write_all(&[b'e'; 16 * 1024]).expect("stderr writes");
                }
            });
            let mut stdout = std::io::stdout().lock();
            for _ in 0..32 {
                stdout.write_all(&[b'o'; 16 * 1024]).expect("stdout writes");
            }
            stderr.join().expect("stderr writer joins");
        }
        "json" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("stdin reads");
            let request: serde_json::Value =
                serde_json::from_str(input.trim()).expect("JSON request");
            let request_id = request["request_id"].clone();
            println!(
                "{}",
                serde_json::json!({
                    "frame":"result",
                    "protocol_version":1,
                    "schema_id":"star.external-tool-response",
                    "schema_version":1,
                    "request_id":request_id,
                    "status":"ok",
                    "summary":"fake result",
                    "data":{"accepted":true},
                    "diagnostics":[],
                    "artifacts":[],
                    "error":null
                })
            );
        }
        "json-progress" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("stdin reads");
            let request: serde_json::Value =
                serde_json::from_str(input.trim()).expect("JSON request");
            let request_id = request["request_id"].clone();
            for (sequence, progress) in [(1, 1), (2, 2)] {
                println!(
                    "{}",
                    serde_json::json!({
                        "frame":"progress",
                        "protocol_version":1,
                        "request_id":request_id,
                        "sequence":sequence,
                        "progress":progress,
                        "total":2,
                        "message":format!("step {progress}")
                    })
                );
            }
            println!(
                "{}",
                serde_json::json!({
                    "frame":"result",
                    "protocol_version":1,
                    "schema_id":"star.external-tool-response",
                    "schema_version":1,
                    "request_id":request_id,
                    "status":"ok",
                    "summary":"fake progress result",
                    "data":{"accepted":true},
                    "diagnostics":[],
                    "artifacts":[],
                    "error":null
                })
            );
        }
        "json-garbage" => println!("not-json"),
        "json-artifact-escape" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("stdin reads");
            let request: serde_json::Value =
                serde_json::from_str(input.trim()).expect("JSON request");
            println!(
                "{}",
                serde_json::json!({
                    "frame":"result", "protocol_version":1,
                    "schema_id":"star.external-tool-response", "schema_version":1,
                    "request_id":request["request_id"], "status":"ok", "summary":"escape",
                    "data":{}, "diagnostics":[],
                    "artifacts":[{"path":"../escape","media_type":"text/plain","role":"result","sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}],
                    "error":null
                })
            );
        }
        "json-artifact-bad-hash" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("stdin reads");
            let request: serde_json::Value =
                serde_json::from_str(input.trim()).expect("JSON request");
            let artifact_root = request["context"]["artifact_directory"]
                .as_str()
                .expect("artifact root");
            std::fs::create_dir_all(artifact_root).expect("artifact directory creates");
            std::fs::write(
                std::path::Path::new(artifact_root).join("result.txt"),
                b"actual bytes",
            )
            .expect("artifact writes");
            println!(
                "{}",
                serde_json::json!({
                    "frame":"result", "protocol_version":1,
                    "schema_id":"star.external-tool-response", "schema_version":1,
                    "request_id":request["request_id"], "status":"ok", "summary":"bad hash",
                    "data":{}, "diagnostics":[],
                    "artifacts":[{"path":"result.txt","media_type":"text/plain","role":"result","sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}],
                    "error":null
                })
            );
        }
        "json-duplicate" => {
            println!(
                r#"{{"frame":"result","frame":"result","protocol_version":1,"schema_id":"star.external-tool-response","schema_version":1,"request_id":"x","status":"ok","summary":"duplicate","data":{{}},"error":null}}"#
            );
        }
        "json-unknown" => {
            println!(
                r#"{{"frame":"result","protocol_version":1,"schema_id":"star.external-tool-response","schema_version":1,"request_id":"x","status":"ok","summary":"unknown","data":{{}},"error":null,"unknown":true}}"#
            );
        }
        "json-nonzero" => std::process::exit(7),
        "json-sleep" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("stdin reads");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        // The parent deliberately stays alive until its Operation Job is
        // terminated by the integration test; waiting here would invalidate
        // the child-tree termination assertion.
        #[allow(clippy::zombie_processes)]
        "tree" => {
            let pid_file = std::env::args().nth(2).expect("pid file argument");
            let child = std::process::Command::new(std::env::current_exe().expect("current exe"))
                .arg("sleep")
                .spawn()
                .expect("child starts");
            std::fs::write(pid_file, child.id().to_string()).expect("pid writes");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        "sleep" => std::thread::sleep(std::time::Duration::from_secs(2)),
        "wait-eof" => {
            let mut input = Vec::new();
            std::io::stdin()
                .read_to_end(&mut input)
                .expect("stdin reads");
            println!("eof");
        }
        _ => std::process::exit(64),
    }
}
