//! Private Rust syntax and pinned rust-analyzer semantic adapters.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use star_contracts::{
    Sha256Hash,
    index::IndexLimitation,
    management::{SourceRange, SymbolResolution},
};
use star_project::{
    FileObservation, ProjectObservation,
    index::{
        AdapterFailure, SemanticAdapter, SemanticAnalysis, SyntaxAdapter, SyntaxAnalysis,
        SyntaxDefinition, SyntaxReference,
    },
};
use thiserror::Error;
use tree_sitter::{Node, Parser};

const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TREE_DEPTH: usize = 512;
const MAX_NAMED_NODES: usize = 1_000_000;
const PINNED_RUST_ANALYZER_VERSION: &str = "rust-analyzer 1.96.0 (ac68faa2 2026-05-25)";
const PINNED_RUST_ANALYZER_X64_SHA256: &str =
    "sha256:9564c8fe6f9d0c71233a211780c87ebba728f1d8e157c3a67748e4ff5d6840ff";
const RUST_ANALYZER_TOOLCHAIN: &str = "1.96.0";
const RUST_SRC_RELATIVE_PATH: &str = "lib/rustlib/src/rust/library";
const LSP_MESSAGE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default)]
pub struct RustSyntaxAdapter;

impl SyntaxAdapter for RustSyntaxAdapter {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn fingerprint(&self) -> Sha256Hash {
        Sha256Hash::digest(
            b"star.rust-syntax-adapter.v1;tree-sitter=0.26.11;tree-sitter-rust=0.24.2",
        )
    }

    fn analyze(&self, source: &FileObservation) -> Result<SyntaxAnalysis, AdapterFailure> {
        let text = source.text.as_deref().ok_or(AdapterFailure::ParseFailed)?;
        if text.len() > MAX_SOURCE_BYTES {
            return Err(AdapterFailure::ResourceLimit);
        }
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|_| AdapterFailure::Unavailable)?;
        let tree = parser
            .parse(text, None)
            .ok_or(AdapterFailure::ResourceLimit)?;
        if tree.root_node().has_error() {
            return Err(AdapterFailure::ParseFailed);
        }

        let mut analysis = SyntaxAnalysis::default();
        let mut definition_ranges = BTreeSet::new();
        let mut visited = 0_usize;
        collect_definitions(
            tree.root_node(),
            text.as_bytes(),
            &mut Vec::new(),
            &mut definition_ranges,
            &mut analysis.definitions,
            &mut visited,
            0,
        )?;
        collect_references(
            tree.root_node(),
            text.as_bytes(),
            &definition_ranges,
            &mut analysis.references,
            &mut visited,
            0,
        )?;
        Ok(analysis)
    }
}

#[derive(Debug, Error)]
pub enum RustAnalyzerDiscoveryError {
    #[error("pinned rust-analyzer could not be resolved")]
    Unavailable,
    #[error("resolved rust-analyzer path or version did not match the pin")]
    PinMismatch,
    #[error("resolved rust-analyzer binary could not be fingerprinted")]
    Fingerprint,
}

#[derive(Clone, Debug)]
struct PreparedSemanticEntry {
    content_sha256: Sha256Hash,
    analysis: SemanticAnalysis,
}

#[derive(Debug)]
pub struct RustAnalyzerSemanticAdapter {
    executable: PathBuf,
    binary_sha256: Sha256Hash,
    max_workspace_files: usize,
    max_workspace_bytes: u64,
    max_reference_queries: usize,
    prepared: Mutex<BTreeMap<String, PreparedSemanticEntry>>,
}

impl RustAnalyzerSemanticAdapter {
    pub fn discover_pinned() -> Result<Self, RustAnalyzerDiscoveryError> {
        let output = run_bounded_command(
            Path::new("rustup"),
            &[
                "which",
                "--toolchain",
                RUST_ANALYZER_TOOLCHAIN,
                "rust-analyzer",
            ],
            Duration::from_secs(5),
        )?;
        if !output.success {
            return Err(RustAnalyzerDiscoveryError::Unavailable);
        }
        let executable = PathBuf::from(
            String::from_utf8(output.stdout)
                .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?
                .trim(),
        )
        .canonicalize()
        .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?;
        if !executable.is_absolute() || !executable.is_file() {
            return Err(RustAnalyzerDiscoveryError::Unavailable);
        }
        require_pinned_rust_src(&executable)?;
        let version = run_bounded_command(&executable, &["--version"], Duration::from_secs(5))?;
        if !version.success
            || String::from_utf8_lossy(&version.stdout).trim() != PINNED_RUST_ANALYZER_VERSION
        {
            return Err(RustAnalyzerDiscoveryError::PinMismatch);
        }
        let binary = fs::read(&executable).map_err(|_| RustAnalyzerDiscoveryError::Fingerprint)?;
        let binary_sha256 = Sha256Hash::digest(&binary);
        if std::env::consts::ARCH != "x86_64"
            || binary_sha256.as_str() != PINNED_RUST_ANALYZER_X64_SHA256
        {
            return Err(RustAnalyzerDiscoveryError::PinMismatch);
        }
        Ok(Self {
            executable,
            binary_sha256,
            max_workspace_files: 256,
            max_workspace_bytes: 256 * 1024 * 1024,
            max_reference_queries: 512,
            prepared: Mutex::new(BTreeMap::new()),
        })
    }

    #[cfg(test)]
    fn with_limits(mut self, max_workspace_files: usize, max_reference_queries: usize) -> Self {
        self.max_workspace_files = max_workspace_files;
        self.max_reference_queries = max_reference_queries;
        self
    }
}

fn require_pinned_rust_src(
    rust_analyzer_executable: &Path,
) -> Result<(), RustAnalyzerDiscoveryError> {
    let toolchain_root = rust_analyzer_executable
        .parent()
        .and_then(Path::parent)
        .ok_or(RustAnalyzerDiscoveryError::Unavailable)?;
    let library = toolchain_root.join(RUST_SRC_RELATIVE_PATH);
    if !library.is_dir()
        || !library.join("core/src/lib.rs").is_file()
        || !library.join("std/src/lib.rs").is_file()
    {
        return Err(RustAnalyzerDiscoveryError::Unavailable);
    }
    Ok(())
}

struct BoundedCommandOutput {
    success: bool,
    stdout: Vec<u8>,
}

fn run_bounded_command(
    executable: &Path,
    arguments: &[&str],
    timeout: Duration,
) -> Result<BoundedCommandOutput, RustAnalyzerDiscoveryError> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let mut child = command
        .spawn()
        .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(RustAnalyzerDiscoveryError::Unavailable)?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RustAnalyzerDiscoveryError::Unavailable);
        }
        thread::sleep(Duration::from_millis(25));
    };
    let stdout = reader
        .join()
        .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?
        .map_err(|_| RustAnalyzerDiscoveryError::Unavailable)?;
    if stdout.len() > 64 * 1024 {
        return Err(RustAnalyzerDiscoveryError::Unavailable);
    }
    Ok(BoundedCommandOutput {
        success: status.success(),
        stdout,
    })
}

impl SemanticAdapter for RustAnalyzerSemanticAdapter {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn fingerprint(&self) -> Sha256Hash {
        Sha256Hash::digest(
            format!(
                "star.rust-analyzer-semantic-adapter.v2;toolchain={RUST_ANALYZER_TOOLCHAIN};rust-src=required;version={PINNED_RUST_ANALYZER_VERSION};binary={};workspace_files={};workspace_bytes={};reference_queries={};request_timeout_seconds=45",
                self.binary_sha256
                ,self.max_workspace_files
                ,self.max_workspace_bytes
                ,self.max_reference_queries
            )
            .as_bytes(),
        )
    }

    fn prepare(
        &self,
        project_root: &Path,
        observation: &ProjectObservation,
    ) -> Result<(), AdapterFailure> {
        let root = project_root
            .canonicalize()
            .map_err(|_| AdapterFailure::Unavailable)?;
        let files = observation
            .files
            .iter()
            .filter(|file| file.language_id.as_deref() == Some("rust") && file.text.is_some())
            .collect::<Vec<_>>();
        if files.len() > self.max_workspace_files {
            return Err(AdapterFailure::ResourceLimit);
        }
        let total_bytes = files.iter().try_fold(0_u64, |total, file| {
            total
                .checked_add(file.size_bytes)
                .ok_or(AdapterFailure::ResourceLimit)
        })?;
        if total_bytes > self.max_workspace_bytes {
            return Err(AdapterFailure::ResourceLimit);
        }
        let prepared =
            run_rust_analyzer(&self.executable, &root, &files, self.max_reference_queries)?;
        *self
            .prepared
            .lock()
            .map_err(|_| AdapterFailure::Unavailable)? = prepared;
        Ok(())
    }

    fn analyze(&self, source: &FileObservation) -> Result<SemanticAnalysis, AdapterFailure> {
        let prepared = self
            .prepared
            .lock()
            .map_err(|_| AdapterFailure::Unavailable)?;
        let entry = prepared
            .get(source.path.as_str())
            .ok_or(AdapterFailure::Unavailable)?;
        if entry.content_sha256 != source.content_sha256 {
            return Err(AdapterFailure::Unavailable);
        }
        Ok(entry.analysis.clone())
    }
}

#[derive(Clone, Debug)]
struct SemanticDefinitionLocation {
    path: String,
    qualified_name: String,
    symbol_kind: String,
    range: SourceRange,
    position_line: u64,
    position_character: u64,
}

fn run_rust_analyzer(
    executable: &Path,
    root: &Path,
    files: &[&FileObservation],
    max_reference_queries: usize,
) -> Result<BTreeMap<String, PreparedSemanticEntry>, AdapterFailure> {
    let mut prepared = BTreeMap::new();
    let mut uri_to_path = BTreeMap::new();
    let text_by_path = files
        .iter()
        .filter_map(|file| {
            file.text
                .as_deref()
                .map(|text| (file.path.as_str().to_owned(), text))
        })
        .collect::<BTreeMap<_, _>>();
    for file in files {
        let absolute = root.join(file.path.as_str());
        if !absolute.starts_with(root) {
            return Err(AdapterFailure::Unavailable);
        }
        let observed = fs::read(&absolute).map_err(|_| AdapterFailure::Unavailable)?;
        if Sha256Hash::digest(&observed) != file.content_sha256 {
            return Err(AdapterFailure::Unavailable);
        }
        let uri = file_uri(&absolute)?;
        uri_to_path.insert(normalized_uri_key(&uri), file.path.as_str().to_owned());
        prepared.insert(
            file.path.as_str().to_owned(),
            PreparedSemanticEntry {
                content_sha256: file.content_sha256.clone(),
                analysis: SemanticAnalysis::default(),
            },
        );
    }

    let mut client = LspClient::start(executable, root)?;
    let root_uri = file_uri(root)?;
    client.request(
        "initialize",
        serde_json::json!({
            "processId":null,
            "clientInfo":{"name":"star-control","version":env!("CARGO_PKG_VERSION")},
            "rootUri":root_uri,
            "workspaceFolders":[{"uri":root_uri,"name":"star-project"}],
            "capabilities":{
                "textDocument":{
                    "documentSymbol":{"hierarchicalDocumentSymbolSupport":true},
                    "references":{}
                },
                "workspace":{"configuration":true,"workspaceFolders":true}
                ,"experimental":{"serverStatusNotification":true}
            },
            "initializationOptions":{"checkOnSave":false}
        }),
        Duration::from_secs(45),
    )?;
    client.notify("initialized", serde_json::json!({}))?;
    for file in files {
        let absolute = root.join(file.path.as_str());
        let uri = file_uri(&absolute)?;
        client.notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument":{
                    "uri":uri,
                    "languageId":"rust",
                    "version":1,
                    "text":file.text.as_deref().unwrap_or_default(),
                }
            }),
        )?;
    }
    client.wait_for_quiescent(Duration::from_secs(45))?;

    let mut definitions = Vec::new();
    for file in files {
        let uri = file_uri(&root.join(file.path.as_str()))?;
        let result = client.request(
            "textDocument/documentSymbol",
            serde_json::json!({"textDocument":{"uri":uri}}),
            Duration::from_secs(45),
        )?;
        flatten_document_symbols(
            result.as_array().map(Vec::as_slice).unwrap_or_default(),
            file.path.as_str(),
            file.text.as_deref().unwrap_or_default(),
            None,
            &mut definitions,
        );
    }
    definitions.sort_by(|left, right| {
        (
            &left.path,
            left.range.start_line,
            left.range.start_column,
            &left.qualified_name,
        )
            .cmp(&(
                &right.path,
                right.range.start_line,
                right.range.start_column,
                &right.qualified_name,
            ))
    });
    definitions.dedup_by(|left, right| {
        left.path == right.path
            && left.qualified_name == right.qualified_name
            && left.range == right.range
    });
    for definition in &definitions {
        if let Some(entry) = prepared.get_mut(&definition.path) {
            entry.analysis.definitions.push(SyntaxDefinition {
                qualified_name: definition.qualified_name.clone(),
                symbol_kind: definition.symbol_kind.clone(),
                range: definition.range.clone(),
                visibility: None,
            });
        }
    }

    let reference_budget_exhausted = definitions.len() > max_reference_queries;
    for definition in definitions.iter().take(max_reference_queries) {
        let definition_uri = uri_to_path
            .iter()
            .find_map(|(uri, path)| (path == &definition.path).then_some(uri))
            .ok_or(AdapterFailure::Unavailable)?;
        let result = client.request(
            "textDocument/references",
            serde_json::json!({
                "textDocument":{"uri":definition_uri},
                "position":{
                    "line":definition.position_line,
                    "character":definition.position_character,
                },
                "context":{"includeDeclaration":true}
            }),
            Duration::from_secs(45),
        )?;
        for location in result.as_array().map(Vec::as_slice).unwrap_or_default() {
            let Some(uri) = location.get("uri").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(reference_path) = uri_to_path.get(&normalized_uri_key(uri)) else {
                continue;
            };
            let Some(reference_text) = text_by_path.get(reference_path) else {
                continue;
            };
            let Some(range) = location
                .get("range")
                .and_then(|value| lsp_range(value, reference_text))
            else {
                continue;
            };
            if reference_path == &definition.path && range == definition.range {
                continue;
            }
            let resolution = if reference_path == &definition.path {
                SymbolResolution::Resolved
            } else {
                SymbolResolution::Unresolved
            };
            if let Some(entry) = prepared.get_mut(reference_path) {
                entry.analysis.references.push(SyntaxReference {
                    target_name: definition.qualified_name.clone(),
                    range,
                    reference_kind: "rust_analyzer_reference".to_owned(),
                    resolution,
                });
                if resolution == SymbolResolution::Unresolved {
                    push_limitation(
                        &mut entry.analysis,
                        "INDEX_RUST_ANALYZER_CROSS_FILE_TARGET_DEFERRED",
                        reference_path,
                    );
                }
            }
        }
    }
    if reference_budget_exhausted {
        for (path, entry) in &mut prepared {
            push_limitation(
                &mut entry.analysis,
                "INDEX_RUST_ANALYZER_REFERENCE_BUDGET",
                path,
            );
        }
    }
    for entry in prepared.values_mut() {
        entry.analysis.definitions.sort_by(|left, right| {
            (
                &left.qualified_name,
                left.range.start_line,
                left.range.start_column,
            )
                .cmp(&(
                    &right.qualified_name,
                    right.range.start_line,
                    right.range.start_column,
                ))
        });
        entry.analysis.references.sort_by(|left, right| {
            (
                &left.target_name,
                left.range.start_line,
                left.range.start_column,
            )
                .cmp(&(
                    &right.target_name,
                    right.range.start_line,
                    right.range.start_column,
                ))
        });
        entry.analysis.references.dedup_by(|left, right| {
            left.target_name == right.target_name && left.range == right.range
        });
        entry.analysis.limitations.sort_by(|left, right| {
            (&left.code, &left.scope, &left.parameters).cmp(&(
                &right.code,
                &right.scope,
                &right.parameters,
            ))
        });
        entry.analysis.limitations.dedup();
    }
    client.shutdown();
    Ok(prepared)
}

fn push_limitation(analysis: &mut SemanticAnalysis, code: &str, path: &str) {
    analysis.limitations.push(IndexLimitation {
        code: code.to_owned(),
        scope: Some(path.to_owned()),
        parameters: BTreeMap::new(),
    });
}

fn flatten_document_symbols(
    values: &[serde_json::Value],
    path: &str,
    text: &str,
    parent: Option<&str>,
    output: &mut Vec<SemanticDefinitionLocation>,
) {
    for value in values {
        let Some(name) = value.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if name.is_empty() || name.len() > 256 {
            continue;
        }
        let Some(selection) = value.get("selectionRange").or_else(|| value.get("range")) else {
            continue;
        };
        let Some(range) = lsp_range(selection, text) else {
            continue;
        };
        let Some(start) = selection.get("start") else {
            continue;
        };
        let Some(position_line) = start.get("line").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(position_character) = start.get("character").and_then(serde_json::Value::as_u64)
        else {
            continue;
        };
        let qualified_name =
            parent.map_or_else(|| name.to_owned(), |parent| format!("{parent}::{name}"));
        output.push(SemanticDefinitionLocation {
            path: path.to_owned(),
            qualified_name: qualified_name.clone(),
            symbol_kind: lsp_symbol_kind(
                value
                    .get("kind")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(13),
            )
            .to_owned(),
            range,
            position_line,
            position_character,
        });
        if let Some(children) = value.get("children").and_then(serde_json::Value::as_array) {
            flatten_document_symbols(children, path, text, Some(&qualified_name), output);
        }
    }
}

fn lsp_symbol_kind(kind: u64) -> &'static str {
    match kind {
        2 => "module",
        5 => "class",
        6 => "method",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "function",
        14 => "constant",
        22 => "enum_variant",
        23 => "struct",
        26 => "type_parameter",
        _ => "variable",
    }
}

fn lsp_range(value: &serde_json::Value, text: &str) -> Option<SourceRange> {
    let start = value.get("start")?;
    let end = value.get("end")?;
    let start_line = usize::try_from(start.get("line")?.as_u64()?).ok()?;
    let end_line = usize::try_from(end.get("line")?.as_u64()?).ok()?;
    Some(SourceRange {
        start_line: u32::try_from(start_line + 1).ok()?,
        start_column: scalar_column_from_utf16(
            text,
            start_line,
            usize::try_from(start.get("character")?.as_u64()?).ok()?,
        )?,
        end_line: u32::try_from(end_line + 1).ok()?,
        end_column: scalar_column_from_utf16(
            text,
            end_line,
            usize::try_from(end.get("character")?.as_u64()?).ok()?,
        )?,
    })
}

fn scalar_column_from_utf16(text: &str, line_index: usize, utf16_offset: usize) -> Option<u32> {
    let line = text.split('\n').nth(line_index)?;
    let mut units = 0_usize;
    let mut scalars = 0_usize;
    for character in line.chars() {
        if units == utf16_offset {
            break;
        }
        let width = character.len_utf16();
        if units.saturating_add(width) > utf16_offset {
            return None;
        }
        units += width;
        scalars += 1;
    }
    (units == utf16_offset).then(|| u32::try_from(scalars + 1).unwrap_or(u32::MAX))
}

fn file_uri(path: &Path) -> Result<String, AdapterFailure> {
    let absolute = path
        .canonicalize()
        .map_err(|_| AdapterFailure::Unavailable)?;
    let normalized = absolute
        .to_str()
        .ok_or(AdapterFailure::Unavailable)?
        .replace('\\', "/");
    let normalized = normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_owned();
    let prefix = if normalized.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    let mut encoded = String::with_capacity(normalized.len());
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(format!("{prefix}{encoded}"))
}

#[cfg(windows)]
fn normalized_uri_key(uri: &str) -> String {
    uri.to_ascii_lowercase()
}

#[cfg(not(windows))]
fn normalized_uri_key(uri: &str) -> String {
    uri.to_owned()
}

struct LspClient {
    child: Child,
    stdin: Option<ChildStdin>,
    messages: mpsc::Receiver<Result<serde_json::Value, ()>>,
    next_id: u64,
}

impl LspClient {
    fn start(executable: &Path, root: &Path) -> Result<Self, AdapterFailure> {
        Self::start_with_args(executable, root, &[])
    }

    fn start_with_args(
        executable: &Path,
        root: &Path,
        arguments: &[&str],
    ) -> Result<Self, AdapterFailure> {
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        let mut child = command.spawn().map_err(|_| AdapterFailure::Unavailable)?;
        let stdin = child.stdin.take().ok_or(AdapterFailure::Unavailable)?;
        let stdout = child.stdout.take().ok_or(AdapterFailure::Unavailable)?;
        let (sender, messages) = mpsc::sync_channel(64);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let message = read_lsp_message(&mut reader);
                let finished = message.is_err();
                if sender.send(message).is_err() || finished {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            messages,
            next_id: 1,
        })
    }

    fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, AdapterFailure> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.send(&serde_json::json!({
            "jsonrpc":"2.0",
            "id":id,
            "method":method,
            "params":params,
        }))?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(AdapterFailure::ResourceLimit)?;
            let message = self
                .messages
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => AdapterFailure::ResourceLimit,
                    mpsc::RecvTimeoutError::Disconnected => AdapterFailure::Unavailable,
                })?
                .map_err(|_| AdapterFailure::Unavailable)?;
            if message.get("method").is_some() && message.get("id").is_some() {
                self.respond_to_server_request(&message)?;
                continue;
            }
            if message.get("id").and_then(serde_json::Value::as_u64) != Some(id) {
                continue;
            }
            if message.get("error").is_some() {
                report_lsp_parse_failure(method, &message);
                return Err(AdapterFailure::ParseFailed);
            }
            return Ok(message
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null));
        }
    }

    fn wait_for_quiescent(&mut self, timeout: Duration) -> Result<(), AdapterFailure> {
        let deadline = Instant::now() + timeout;
        let mut saw_busy = false;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(AdapterFailure::ResourceLimit)?;
            let message = self
                .messages
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => AdapterFailure::ResourceLimit,
                    mpsc::RecvTimeoutError::Disconnected => AdapterFailure::Unavailable,
                })?
                .map_err(|_| AdapterFailure::Unavailable)?;
            if message.get("method").is_some() && message.get("id").is_some() {
                self.respond_to_server_request(&message)?;
                continue;
            }
            if message.get("method").and_then(serde_json::Value::as_str)
                != Some("experimental/serverStatus")
            {
                continue;
            }
            let health = message
                .pointer("/params/health")
                .and_then(serde_json::Value::as_str);
            if health == Some("error") {
                report_lsp_parse_failure("experimental/serverStatus health", &message);
                return Err(AdapterFailure::ParseFailed);
            }
            let Some(quiescent) = message
                .pointer("/params/quiescent")
                .and_then(serde_json::Value::as_bool)
            else {
                report_lsp_parse_failure("experimental/serverStatus quiescent", &message);
                return Err(AdapterFailure::ParseFailed);
            };
            if !quiescent {
                saw_busy = true;
            } else if saw_busy {
                return Ok(());
            }
        }
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), AdapterFailure> {
        self.send(&serde_json::json!({
            "jsonrpc":"2.0",
            "method":method,
            "params":params,
        }))
    }

    fn respond_to_server_request(
        &mut self,
        request: &serde_json::Value,
    ) -> Result<(), AdapterFailure> {
        let id = request
            .get("id")
            .cloned()
            .ok_or(AdapterFailure::ParseFailed)?;
        let result = if request.get("method").and_then(serde_json::Value::as_str)
            == Some("workspace/configuration")
        {
            let count = request
                .pointer("/params/items")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            serde_json::Value::Array(vec![serde_json::Value::Null; count])
        } else {
            serde_json::Value::Null
        };
        self.send(&serde_json::json!({"jsonrpc":"2.0","id":id,"result":result}))
    }

    fn send(&mut self, value: &serde_json::Value) -> Result<(), AdapterFailure> {
        let body = serde_json::to_vec(value).map_err(|_| AdapterFailure::ParseFailed)?;
        if body.len() > LSP_MESSAGE_LIMIT {
            return Err(AdapterFailure::ResourceLimit);
        }
        let stdin = self.stdin.as_mut().ok_or(AdapterFailure::Unavailable)?;
        write!(stdin, "Content-Length: {}\r\n\r\n", body.len())
            .and_then(|_| stdin.write_all(&body))
            .and_then(|_| stdin.flush())
            .map_err(|_| AdapterFailure::Unavailable)
    }

    fn shutdown(&mut self) {
        let _ = self.request("shutdown", serde_json::Value::Null, Duration::from_secs(5));
        let _ = self.notify("exit", serde_json::Value::Null);
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
fn report_lsp_parse_failure(stage: &str, message: &serde_json::Value) {
    eprintln!("rust-analyzer LSP ParseFailed at {stage}: {message}");
}

#[cfg(not(test))]
fn report_lsp_parse_failure(_stage: &str, _message: &serde_json::Value) {}

impl Drop for LspClient {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_lsp_message(reader: &mut BufReader<impl Read>) -> Result<serde_json::Value, ()> {
    let mut content_length = None;
    let mut header_bytes = 0_usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).map_err(|_| ())? == 0 {
            return Err(());
        }
        header_bytes = header_bytes.saturating_add(header.len());
        if header_bytes > 8 * 1024 {
            return Err(());
        }
        let header = header.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            content_length = Some(value.trim().parse::<usize>().map_err(|_| ())?);
        }
    }
    let content_length = content_length.ok_or(())?;
    if content_length > LSP_MESSAGE_LIMIT {
        return Err(());
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body).map_err(|_| ())?;
    serde_json::from_slice(&body).map_err(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn collect_definitions(
    node: Node<'_>,
    source: &[u8],
    scopes: &mut Vec<String>,
    definition_ranges: &mut BTreeSet<(usize, usize)>,
    definitions: &mut Vec<SyntaxDefinition>,
    visited: &mut usize,
    depth: usize,
) -> Result<(), AdapterFailure> {
    enforce_limits(node, visited, depth)?;
    let definition = definition_node(node, source);
    if let Some((name_node, name, symbol_kind)) = &definition {
        let qualified_name = if scopes.is_empty() {
            name.clone()
        } else {
            format!("{}::{name}", scopes.join("::"))
        };
        definition_ranges.insert((name_node.start_byte(), name_node.end_byte()));
        definitions.push(SyntaxDefinition {
            qualified_name,
            symbol_kind: (*symbol_kind).to_owned(),
            range: source_range(*name_node, source),
            visibility: visibility(node, source),
        });
    }

    let scope = scope_name(
        node,
        source,
        definition.as_ref().map(|(_, name, _)| name.as_str()),
    );
    if let Some(scope) = scope.as_ref() {
        scopes.push(scope.clone());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_definitions(
            child,
            source,
            scopes,
            definition_ranges,
            definitions,
            visited,
            depth + 1,
        )?;
    }
    if scope.is_some() {
        scopes.pop();
    }
    Ok(())
}

fn collect_references(
    node: Node<'_>,
    source: &[u8],
    definition_ranges: &BTreeSet<(usize, usize)>,
    references: &mut Vec<SyntaxReference>,
    visited: &mut usize,
    depth: usize,
) -> Result<(), AdapterFailure> {
    enforce_limits(node, visited, depth)?;
    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier"
    ) && !definition_ranges.contains(&(node.start_byte(), node.end_byte()))
        && let Some(name) = node_text(node, source).filter(|value| valid_identifier(value))
    {
        references.push(SyntaxReference {
            target_name: name.to_owned(),
            range: source_range(node, source),
            reference_kind: match node.kind() {
                "type_identifier" => "type_use",
                "field_identifier" | "shorthand_field_identifier" => "field_use",
                _ => "identifier_use",
            }
            .to_owned(),
            resolution: SymbolResolution::Unresolved,
        });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_references(
            child,
            source,
            definition_ranges,
            references,
            visited,
            depth + 1,
        )?;
    }
    Ok(())
}

fn enforce_limits(node: Node<'_>, visited: &mut usize, depth: usize) -> Result<(), AdapterFailure> {
    if depth > MAX_TREE_DEPTH {
        return Err(AdapterFailure::ResourceLimit);
    }
    if node.is_named() {
        *visited = visited.saturating_add(1);
        if *visited > MAX_NAMED_NODES {
            return Err(AdapterFailure::ResourceLimit);
        }
    }
    Ok(())
}

fn definition_node<'tree>(
    node: Node<'tree>,
    source: &[u8],
) -> Option<(Node<'tree>, String, &'static str)> {
    let (field, kind) = match node.kind() {
        "function_item" => ("name", "function"),
        "struct_item" => ("name", "struct"),
        "enum_item" => ("name", "enum"),
        "union_item" => ("name", "union"),
        "trait_item" => ("name", "trait"),
        "type_item" => ("name", "type_alias"),
        "const_item" => ("name", "constant"),
        "static_item" => ("name", "static"),
        "mod_item" => ("name", "module"),
        "macro_definition" => ("name", "macro"),
        "enum_variant" => ("name", "enum_variant"),
        "field_declaration" => ("name", "field"),
        _ => return None,
    };
    let name_node = node.child_by_field_name(field)?;
    let name = node_text(name_node, source)?.to_owned();
    valid_identifier(&name).then_some((name_node, name, kind))
}

fn scope_name(node: Node<'_>, source: &[u8], definition_name: Option<&str>) -> Option<String> {
    match node.kind() {
        "mod_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" => {
            definition_name.map(ToOwned::to_owned)
        }
        "impl_item" => node
            .child_by_field_name("type")
            .and_then(|node| node_text(node, source))
            .map(normalize_scope),
        _ => None,
    }
}

fn normalize_scope(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .take(256)
        .collect()
}

fn visibility(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .and_then(|child| node_text(child, source))
        .map(ToOwned::to_owned)
}

fn node_text<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    std::str::from_utf8(source.get(node.byte_range())?).ok()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .chars()
            .all(|character| character == '_' || character.is_alphanumeric())
}

fn source_range(node: Node<'_>, source: &[u8]) -> SourceRange {
    let start_line_byte = node
        .start_byte()
        .saturating_sub(node.start_position().column);
    let end_line_byte = node.end_byte().saturating_sub(node.end_position().column);
    SourceRange {
        start_line: u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX),
        start_column: scalar_column(source, start_line_byte, node.start_byte()),
        end_line: u32::try_from(node.end_position().row + 1).unwrap_or(u32::MAX),
        end_column: scalar_column(source, end_line_byte, node.end_byte()),
    }
}

fn scalar_column(source: &[u8], line_start: usize, offset: usize) -> u32 {
    source
        .get(line_start..offset)
        .and_then(|value| std::str::from_utf8(value).ok())
        .map(|value| u32::try_from(value.chars().count() + 1).unwrap_or(u32::MAX))
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use star_contracts::{Sha256Hash, management::ProjectPathRef};
    use star_project::{
        FileObservation,
        index::{SemanticAdapter, SyntaxAdapter},
    };

    use super::{
        AdapterFailure, LspClient, RustAnalyzerSemanticAdapter, RustSyntaxAdapter,
        require_pinned_rust_src, run_rust_analyzer, scalar_column_from_utf16,
    };

    fn source_at(path: &str, text: &str) -> FileObservation {
        FileObservation {
            path: ProjectPathRef::parse(path).unwrap(),
            content_sha256: Sha256Hash::digest(text.as_bytes()),
            size_bytes: text.len() as u64,
            text: Some(text.to_owned()),
            language_id: Some("rust".to_owned()),
            line_count: text.lines().count() as u32,
        }
    }

    fn source(text: &str) -> FileObservation {
        source_at("src/lib.rs", text)
    }

    #[test]
    fn rust_syntax_definitions_are_confirmed_but_references_are_not() {
        let analysis = RustSyntaxAdapter
            .analyze(&source(
                r#"
                pub mod api {
                    pub struct Item { pub value: usize }
                    impl Item {
                        pub fn value(&self) -> usize { self.value }
                    }
                }
                fn use_item(item: api::Item) -> usize { item.value() }
                "#,
            ))
            .unwrap();
        assert!(
            analysis
                .definitions
                .iter()
                .any(|item| item.qualified_name == "api::Item::value")
        );
        assert!(analysis.references.iter().all(|item| matches!(
            item.resolution,
            star_contracts::management::SymbolResolution::Unresolved
        )));
    }

    #[test]
    fn rust_syntax_handles_cfg_and_macro_definitions_without_token_false_positives() {
        let analysis = RustSyntaxAdapter
            .analyze(&source(
                r#"
                #[cfg(feature = "extra")]
                pub fn optional() {}
                macro_rules! make_item { () => { struct Generated; } }
                "#,
            ))
            .unwrap();
        assert!(
            analysis
                .definitions
                .iter()
                .any(|item| item.qualified_name == "optional")
        );
        assert!(
            analysis
                .definitions
                .iter()
                .any(|item| item.symbol_kind == "macro")
        );
        assert!(
            analysis
                .references
                .iter()
                .all(|item| item.target_name != "extra")
        );
    }

    #[test]
    fn invalid_rust_is_not_promoted_to_confirmed_syntax() {
        assert!(RustSyntaxAdapter.analyze(&source("fn broken( {")).is_err());
    }

    #[test]
    fn oversized_rust_source_is_a_resource_limit_not_a_partial_parse() {
        let text = " ".repeat(super::MAX_SOURCE_BYTES + 1);
        assert!(matches!(
            RustSyntaxAdapter.analyze(&source(&text)),
            Err(AdapterFailure::ResourceLimit)
        ));
    }

    #[test]
    fn pinned_rust_src_is_required_at_the_toolchain_boundary() {
        let root = std::env::temp_dir().join(format!(
            "star-rust-src-boundary-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let executable = root.join("bin/rust-analyzer.exe");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"fixture").unwrap();
        assert!(matches!(
            require_pinned_rust_src(&executable),
            Err(super::RustAnalyzerDiscoveryError::Unavailable)
        ));
        for path in ["core/src/lib.rs", "std/src/lib.rs"] {
            let path = root.join(super::RUST_SRC_RELATIVE_PATH).join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"// fixture").unwrap();
        }
        require_pinned_rust_src(&executable).unwrap();
    }

    #[test]
    fn syntax_and_lsp_ranges_project_to_one_based_unicode_scalar_columns() {
        let analysis = RustSyntaxAdapter
            .analyze(&source(
                "const S: &str = \"😀\"; pub fn alpha() -> usize { 1 }\n",
            ))
            .unwrap();
        let alpha = analysis
            .definitions
            .iter()
            .find(|definition| definition.qualified_name == "alpha")
            .unwrap();
        assert_eq!(alpha.range.start_column, 29);
        assert_eq!(scalar_column_from_utf16("😀alpha", 0, 2), Some(2));
        assert_eq!(scalar_column_from_utf16("😀alpha", 0, 3), Some(3));
        assert_eq!(scalar_column_from_utf16("😀alpha", 0, 1), None);
    }

    #[cfg(windows)]
    #[test]
    fn lsp_timeout_and_process_crash_remain_distinct_failures() {
        let root = std::env::temp_dir();
        let mut timeout = LspClient::start_with_args(
            Path::new("pwsh"),
            &root,
            &["-NoProfile", "-Command", "Start-Sleep -Seconds 60"],
        )
        .unwrap();
        assert!(matches!(
            timeout.request(
                "fixture/timeout",
                serde_json::json!({}),
                Duration::from_millis(100)
            ),
            Err(AdapterFailure::ResourceLimit)
        ));
        drop(timeout);

        let mut crashed = LspClient::start_with_args(
            Path::new("pwsh"),
            &root,
            &["-NoProfile", "-Command", "exit 9"],
        )
        .unwrap();
        assert!(matches!(
            crashed.request(
                "fixture/crash",
                serde_json::json!({}),
                Duration::from_secs(2)
            ),
            Err(AdapterFailure::Unavailable)
        ));
    }

    #[test]
    fn pinned_rust_analyzer_confirms_only_adjudicated_same_file_reference() {
        let adapter = RustAnalyzerSemanticAdapter::discover_pinned()
            .unwrap()
            .with_limits(8, 16);
        let root = std::env::temp_dir().join(format!(
            "star-rust-analyzer-{}-{}",
            std::process::id(),
            Sha256Hash::digest(b"same-file-reference")
                .as_str()
                .trim_start_matches("sha256:")
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='star_ra_fixture'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        let text = "mod helper;\npub fn target() -> usize { 1 }\npub fn caller() -> usize { target() + helper::external() }\n";
        let helper_text = "pub fn external() -> usize { 2 }\n";
        fs::write(root.join("src/lib.rs"), text).unwrap();
        fs::write(root.join("src/helper.rs"), helper_text).unwrap();
        let file = source(text);
        let helper_file = source_at("src/helper.rs", helper_text);
        let prepared =
            run_rust_analyzer(&adapter.executable, &root, &[&file, &helper_file], 16).unwrap();
        let analysis = &prepared.get("src/lib.rs").unwrap().analysis;
        assert!(
            analysis
                .definitions
                .iter()
                .any(|definition| definition.qualified_name == "target")
        );
        assert!(analysis.references.iter().any(|reference| {
            reference.target_name == "target"
                && reference.resolution == star_contracts::management::SymbolResolution::Resolved
        }));
        assert!(analysis.references.iter().any(|reference| {
            reference.target_name == "external"
                && reference.resolution == star_contracts::management::SymbolResolution::Unresolved
        }));
        assert!(analysis.limitations.iter().any(|limitation| {
            limitation.code == "INDEX_RUST_ANALYZER_CROSS_FILE_TARGET_DEFERRED"
        }));
        assert_eq!(adapter.language_id(), "rust");
    }
}
