use std::{
    ffi::{OsStr, OsString},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    time::{Duration, Instant},
};

use star_contracts::{
    Sha256Hash,
    ids::{OperationId, RequestId},
    manifest::{ManifestSource, parse_manifest_v1},
    runtime::{
        ExternalToolContext, ExternalToolRequest, ExternalToolResponse, ExternalToolResultStatus,
    },
};
use star_controller::{
    manifest_resources::load_manifest_resources,
    process_runtime::{DirectExeSpec, execute_direct_exe, validate_star_json_stdio_output},
    registry_runtime::{RegistryRuntime, RegistrySourceRoot},
};

struct DeterministicRng(u64);

impl DeterministicRng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn index(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper.max(1)
    }
}

fn fuzz_duration() -> Duration {
    Duration::from_secs(
        std::env::var("STAR_FUZZ_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1)
            .clamp(1, 600),
    )
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("star-property-{name}-{}", star_ipc::nonce()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_star-fake-exe"))
}

fn request() -> ExternalToolRequest {
    ExternalToolRequest {
        frame: "request".to_owned(),
        protocol_version: 1,
        schema_id: "star.external-tool-request".to_owned(),
        schema_version: 1,
        request_id: RequestId::new(),
        tool_id: "user.fake.echo.run".to_owned(),
        descriptor_hash: Sha256Hash::digest(b"property-descriptor"),
        arguments: serde_json::json!({"value":"property"}),
        context: ExternalToolContext {
            operation_id: OperationId::new(),
            project_id: None,
            goal_id: None,
            run_id: None,
            stage_id: None,
            deadline_at: "2026-07-12T00:00:00.000Z".to_owned(),
            artifact_directory: "unused".to_owned(),
            temp_directory: "unused".to_owned(),
        },
    }
}

#[test]
fn local_schema_reference_resolver_arbitrary_mutations_never_escape_or_panic() {
    let root = temp_root("schema-resolver");
    let manifest_path = root.join("package.toml");
    let schema_path = root.join("input.json");
    let source = include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
        .replace(
            "[[actions.parameters]]\nname = \"value\"\ntype = \"string\"\ndescription = \"Value to echo\"\nrequired = true\n",
            "input_schema_file = \"input.json\"\n",
        )
        .replace(
            "[[actions.argv]]\nkind = \"positional\"\ninput = \"value\"\n",
            "[[actions.argv]]\nkind = \"literal\"\nvalue = \"fixed\"\n",
        );
    std::fs::write(&manifest_path, &source).unwrap();
    let manifest = parse_manifest_v1(&source, ManifestSource::User).unwrap();
    let valid = br#"{"type":"object","additionalProperties":false,"properties":{"value":{"type":"string"}}}"#;
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0xe703_7ed1_a0b4_28db);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let mut schema = valid.to_vec();
        for _ in 0..=rng.index(8) {
            if schema.is_empty() || rng.index(3) == 0 {
                schema.push(rng.next() as u8);
            } else {
                let index = rng.index(schema.len());
                schema[index] = rng.next() as u8;
            }
        }
        std::fs::write(&schema_path, schema).unwrap();
        let result = std::panic::catch_unwind(|| {
            let _ = load_manifest_resources(&manifest, &manifest_path);
        });
        assert!(result.is_ok(), "Schema resolver panicked at case {cases}");

        if cases % 32 == 0 {
            std::fs::write(&schema_path, valid).unwrap();
            assert!(load_manifest_resources(&manifest, &manifest_path).is_ok());
        }
        cases += 1;
    }
    assert!(cases > 20);
}

#[test]
fn json_stdio_frame_order_duplicate_and_sequence_mutations_never_panic() {
    let request = request();
    let response = ExternalToolResponse {
        frame: "result".to_owned(),
        protocol_version: 1,
        schema_id: "star.external-tool-response".to_owned(),
        schema_version: 1,
        request_id: request.request_id.clone(),
        status: ExternalToolResultStatus::Ok,
        summary: "property result".to_owned(),
        data: Some(serde_json::json!({"value":"ok"})),
        diagnostics: vec![],
        artifacts: vec![],
        error: None,
    };
    let valid = format!("{}\n", serde_json::to_string(&response).unwrap());
    assert!(validate_star_json_stdio_output(&valid, &request).is_ok());
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0x8ebc_6af0_9c88_c6e3);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let mut candidate = valid.clone().into_bytes();
        match rng.index(6) {
            0 if !candidate.is_empty() => candidate.truncate(rng.index(candidate.len())),
            1 if !candidate.is_empty() => {
                let index = rng.index(candidate.len());
                candidate[index] = rng.next() as u8;
            }
            2 => candidate.extend_from_slice(valid.as_bytes()),
            3 => {
                candidate.splice(
                    0..0,
                    br#"{"frame":"progress","protocol_version":1}
"#
                    .iter()
                    .copied(),
                );
            }
            4 => candidate.extend_from_slice(
                br#"{"frame":"result","frame":"result"}
"#,
            ),
            _ => candidate.reverse(),
        }
        let result = std::panic::catch_unwind(|| {
            let text = String::from_utf8_lossy(&candidate);
            let _ = validate_star_json_stdio_output(&text, &request);
        });
        assert!(result.is_ok(), "JSON-STDIO parser panicked at case {cases}");
        cases += 1;
    }
    assert!(cases > 100);
}

#[tokio::test]
async fn windows_crt_argument_encoder_round_trips_arbitrary_utf16_os_strings() {
    let executable = fake_exe();
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0x5899_65cc_7537_4cc3);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let length = rng.index(32);
        let units: Vec<u16> = (0..length)
            .map(|_| {
                let mut unit = rng.next() as u16;
                if unit == 0 {
                    unit = 1;
                }
                unit
            })
            .collect();
        let argument = OsString::from_wide(&units);
        let spec = DirectExeSpec {
            executable: executable.clone(),
            argv: vec![OsString::from("argv-utf16"), argument],
            working_directory: executable.parent().unwrap().to_path_buf(),
            environment: vec![],
            stdin: None,
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            max_memory_bytes: None,
            max_processes: 4,
            appcontainer_profile: None,
        };
        let output = execute_direct_exe(&spec).await.unwrap();
        let expected = units
            .iter()
            .map(|unit| format!("{unit:04x}"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            String::from_utf8(output.stdout.captured)
                .unwrap()
                .trim_end(),
            expected
        );
        cases += 1;
    }
    assert!(cases > 5);
}

fn live_manifest() -> String {
    let executable = fake_exe();
    let path = executable
        .as_os_str()
        .to_string_lossy()
        .replace('\\', "\\\\");
    let hash = Sha256Hash::digest(&std::fs::read(&executable).unwrap());
    include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
        .replace(r"C:\\Tools\\fake-echo.exe", &path)
        .replace(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            hash.as_str(),
        )
}

#[test]
fn registry_random_valid_invalid_delete_and_recreate_sequences_preserve_lkg_invariants() {
    let directory = temp_root("registry-state");
    let path = directory.join("package.toml");
    let root = RegistrySourceRoot {
        source: ManifestSource::User,
        directory: directory.clone(),
    };
    let mut registry = RegistryRuntime::default();
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0x1d8e_4e27_c47d_124f);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let description = format!("Property description {}.", rng.next());
        let valid = live_manifest().replace("Contract fixture process tool.", &description);
        std::fs::write(&path, &valid).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert_eq!(
            registry.active()["user.fake.echo"].manifest.description,
            description
        );
        let lkg = registry.active()["user.fake.echo"].source_hash.clone();

        std::fs::write(&path, format_version_invalid(&valid)).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert_eq!(registry.active()["user.fake.echo"].source_hash, lkg);

        std::fs::remove_file(&path).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.active().contains_key("user.fake.echo"));
        std::thread::sleep(Duration::from_millis(550));
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(!registry.active().contains_key("user.fake.echo"));
        cases += 1;
    }
    assert!(cases >= 1);
}

fn format_version_invalid(valid: &str) -> String {
    valid.replacen("format_version = 1", "format_version = [", 1)
}

#[test]
fn os_string_test_fixture_itself_preserves_non_ascii_utf16_units() {
    let units = [0x20_u16, 0x22, 0x5c, 0xd83d, 0xde00, 0xd800];
    let value = OsString::from_wide(&units);
    assert_eq!(OsStr::new(&value).encode_wide().collect::<Vec<_>>(), units);
}
