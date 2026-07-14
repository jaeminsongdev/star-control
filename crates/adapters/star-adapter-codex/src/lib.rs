//! Codex local Marketplace rendering and official CLI registration adapter.

#![cfg(windows)]

use std::{
    collections::BTreeMap,
    fs,
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use chrono::Utc;
use serde::Serialize;
use star_adapter_windows::{
    InstallationManager, WindowsAdapterError, atomic_write, atomic_write_json,
    canonical_fixed_directory, ensure_fixed_directory, load_codex_integration_record,
    normal_windows_path, open_regular_local_file,
};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    installation::{
        CODEX_INTEGRATION_RECORD_SCHEMA_ID, CodexIntegrationRecord, CodexIntegrationSummary,
        CodexRegistrationState, INSTALLATION_SCHEMA_VERSION,
    },
    parse_no_duplicate_keys,
};
use thiserror::Error;
use windows::Win32::{
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT, System::Threading::CREATE_NO_WINDOW,
};

pub const MARKETPLACE_NAME: &str = "star-control-local";
pub const PLUGIN_NAME: &str = "star-control";
const INTEGRATION_RECORD_FILE: &str = "integration-record.v1.json";
const TEMPLATE_RELATIVE_ROOT: &str = "integrations/codex-plugin-template/marketplace-root";
const MARKETPLACE_RELATIVE: &str = ".agents/plugins/marketplace.json";
const PLUGIN_ROOT_RELATIVE: &str = "plugins/star-control";
const PLUGIN_MANIFEST_RELATIVE: &str = "plugins/star-control/.codex-plugin/plugin.json";
const MCP_RELATIVE: &str = "plugins/star-control/.mcp.json";
const HOOKS_RELATIVE: &str = "plugins/star-control/hooks/hooks.json";
const SKILL_RELATIVE: &str = "plugins/star-control/skills/star-control-workflow/SKILL.md";
const SOURCE_FILE_MAX_BYTES: u64 = 512 * 1024;
const CODEX_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum CodexAdapterError {
    #[error("Windows installation state is not valid: {0}")]
    Installation(#[from] WindowsAdapterError),
    #[error("Codex Plugin template is missing, malformed or outside the allowed file set")]
    InvalidTemplate,
    #[error("rendered Codex Plugin does not satisfy the integration contract")]
    InvalidRenderedPlugin,
    #[error("Codex integration ownership could not be proven")]
    Ownership,
    #[error("Codex integration I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex integration JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationResult {
    pub action: String,
    pub local_state: String,
    pub record_path: String,
    pub marketplace_root: String,
    pub registration_state: CodexRegistrationState,
    pub requires_new_task: bool,
    pub hook_trust_required: bool,
    pub manual_commands: Vec<String>,
    pub manual_steps: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct IntegrationOptions {
    pub codex_executable: Option<PathBuf>,
    pub skip_register: bool,
}

#[derive(Clone, Debug)]
pub struct CodexIntegrationManager {
    installation: InstallationManager,
}

impl CodexIntegrationManager {
    pub fn for_current_user() -> Result<Self, CodexAdapterError> {
        Ok(Self::new(InstallationManager::for_current_user()?))
    }

    pub fn new(installation: InstallationManager) -> Self {
        Self { installation }
    }

    pub fn install(
        &self,
        install_root: &Path,
        options: &IntegrationOptions,
    ) -> Result<IntegrationResult, CodexAdapterError> {
        self.render_and_register("install", install_root, options)
    }

    pub fn repair(
        &self,
        install_root: &Path,
        options: &IntegrationOptions,
    ) -> Result<IntegrationResult, CodexAdapterError> {
        self.render_and_register("repair", install_root, options)
    }

    pub fn status(&self, install_root: &Path) -> Result<IntegrationResult, CodexAdapterError> {
        let installation = self.installation.status(install_root)?;
        let summary = installation
            .codex_integration
            .ok_or(CodexAdapterError::Ownership)?;
        let record_path = PathBuf::from(&summary.record_path);
        let record = load_codex_integration_record(&record_path)?;
        let expected_root = self.integration_root(&record.product_version);
        if !paths_equal(Path::new(&record.integration_root), &expected_root)
            || !paths_equal(Path::new(&record.install_root), install_root)
            || record.marketplace_name != MARKETPLACE_NAME
            || record.plugin_name != PLUGIN_NAME
        {
            return Err(CodexAdapterError::Ownership);
        }
        let rendered = read_rendered_files(Path::new(&record.marketplace_root))?;
        if rendered_hash(&rendered)? != record.render_sha256 {
            return Err(CodexAdapterError::InvalidRenderedPlugin);
        }
        validate_rendered(
            &rendered,
            Path::new(&record.install_root),
            &record.plugin_version,
        )?;
        Ok(result_from_record(
            "status",
            "verified",
            &record,
            record.manual_commands.clone(),
        ))
    }

    pub fn uninstall(
        &self,
        install_root: &Path,
        codex_executable: Option<&Path>,
    ) -> Result<IntegrationResult, CodexAdapterError> {
        let installation = self.installation.status(install_root)?;
        let Some(summary) = installation.codex_integration else {
            return Ok(IntegrationResult {
                action: "uninstall".to_owned(),
                local_state: "not_installed".to_owned(),
                record_path: String::new(),
                marketplace_root: String::new(),
                registration_state: CodexRegistrationState::Removed,
                requires_new_task: true,
                hook_trust_required: false,
                manual_commands: Vec::new(),
                manual_steps: vec!["Codex를 다시 시작하거나 새 작업을 연다.".to_owned()],
            });
        };
        let record_path = PathBuf::from(&summary.record_path);
        let mut record = load_codex_integration_record(&record_path)?;
        let expected_root = self.integration_root(&record.product_version);
        if !paths_equal(Path::new(&record.integration_root), &expected_root)
            || !paths_equal(Path::new(&record.install_root), install_root)
        {
            return Err(CodexAdapterError::Ownership);
        }
        let commands = deregistration_commands();
        let plugin_removed = run_codex(codex_executable, &commands[0]);
        let marketplace_removed = plugin_removed && run_codex(codex_executable, &commands[1]);
        if plugin_removed && marketplace_removed {
            remove_owned_tree(&expected_root, self.installation.local_data_root())?;
            self.installation
                .set_codex_integration(install_root, None)?;
            record.registration_state = CodexRegistrationState::Removed;
            Ok(IntegrationResult {
                action: "uninstall".to_owned(),
                local_state: "removed".to_owned(),
                record_path: normal_windows_path(&record_path)
                    .to_string_lossy()
                    .into_owned(),
                marketplace_root: record.marketplace_root,
                registration_state: CodexRegistrationState::Removed,
                requires_new_task: true,
                hook_trust_required: false,
                manual_commands: Vec::new(),
                manual_steps: vec![
                    "Codex 앱에서 남아 있는 star-control Plugin을 제거하고 새 작업을 연다."
                        .to_owned(),
                ],
            })
        } else {
            let manual_commands = commands
                .iter()
                .enumerate()
                .filter(|(index, _)| !plugin_removed || *index > 0)
                .map(|(_, args)| display_codex_command(codex_executable, args))
                .collect::<Vec<_>>();
            record.registration_state = CodexRegistrationState::ManualActionRequired;
            record.manual_commands = manual_commands.clone();
            record.updated_at = Utc::now();
            atomic_write_json(&record_path, &record)?;
            self.installation.set_codex_integration(
                install_root,
                Some(CodexIntegrationSummary {
                    record_path: normal_windows_path(&record_path)
                        .to_string_lossy()
                        .into_owned(),
                    registration_state: CodexRegistrationState::ManualActionRequired,
                }),
            )?;
            Ok(IntegrationResult {
                action: "uninstall".to_owned(),
                local_state: "preserved_until_deregistered".to_owned(),
                record_path: normal_windows_path(&record_path)
                    .to_string_lossy()
                    .into_owned(),
                marketplace_root: record.marketplace_root,
                registration_state: CodexRegistrationState::ManualActionRequired,
                requires_new_task: true,
                hook_trust_required: false,
                manual_commands,
                manual_steps: vec![
                    "Codex 앱에서 star-control Plugin을 제거한 뒤 Marketplace 제거 명령을 실행한다."
                        .to_owned(),
                    "등록 해제 확인 전에는 Marketplace source를 보존한다.".to_owned(),
                ],
            })
        }
    }

    fn render_and_register(
        &self,
        action: &str,
        install_root: &Path,
        options: &IntegrationOptions,
    ) -> Result<IntegrationResult, CodexAdapterError> {
        let installation = self.installation.status(install_root)?;
        let install_root = canonical_fixed_directory(Path::new(&installation.install_root))?;
        let rendered_install_root = normal_windows_path(&install_root);
        let source_root = install_root.join(TEMPLATE_RELATIVE_ROOT.replace('/', "\\"));
        let source = read_source_files(&source_root)?;
        let seed = serde_json::json!({
            "product_version": installation.product_version,
            "install_root": rendered_install_root.to_string_lossy(),
            "source": source.iter().map(|(path, bytes)| {
                (path.clone(), Sha256Hash::digest(bytes).to_string())
            }).collect::<BTreeMap<_, _>>()
        });
        let seed_hash = canonical_sha256(&seed).map_err(|_| CodexAdapterError::InvalidTemplate)?;
        let cachebuster = &seed_hash.as_str()[7..19];
        let base_version = installation
            .product_version
            .split_once('+')
            .map_or(installation.product_version.as_str(), |(base, _)| base);
        let plugin_version = format!("{base_version}+codex.{cachebuster}");
        semver::Version::parse(&plugin_version).map_err(|_| CodexAdapterError::InvalidTemplate)?;
        let rendered = render_files(&source, &rendered_install_root, &plugin_version)?;
        validate_rendered(&rendered, &rendered_install_root, &plugin_version)?;

        let integration_root =
            ensure_fixed_directory(&self.integration_root(&installation.product_version))?;
        let marketplace_path = integration_root.join("marketplace-root");
        if marketplace_path.exists() {
            remove_owned_tree(&marketplace_path, self.installation.local_data_root())?;
        }
        let marketplace_root = ensure_fixed_directory(&marketplace_path)?;
        write_rendered_files(&marketplace_root, &rendered)?;
        let render_sha256 = rendered_hash(&rendered)?;
        let rendered_marketplace_root = normal_windows_path(&marketplace_root);
        let commands = registration_commands(&rendered_marketplace_root);
        let registration_state = if options.skip_register {
            CodexRegistrationState::ManualActionRequired
        } else {
            let marketplace_added = run_codex(options.codex_executable.as_deref(), &commands[0]);
            let plugin_added = run_codex(options.codex_executable.as_deref(), &commands[1]);
            if marketplace_added && plugin_added {
                CodexRegistrationState::Registered
            } else {
                CodexRegistrationState::ManualActionRequired
            }
        };
        let manual_commands = if registration_state == CodexRegistrationState::Registered {
            Vec::new()
        } else {
            commands
                .iter()
                .map(|args| display_codex_command(options.codex_executable.as_deref(), args))
                .collect()
        };
        let record_path = integration_root.join(INTEGRATION_RECORD_FILE);
        let record = CodexIntegrationRecord {
            schema_id: CODEX_INTEGRATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: INSTALLATION_SCHEMA_VERSION,
            product_version: installation.product_version,
            install_root: rendered_install_root.to_string_lossy().into_owned(),
            integration_root: normal_windows_path(&integration_root)
                .to_string_lossy()
                .into_owned(),
            marketplace_root: rendered_marketplace_root.to_string_lossy().into_owned(),
            marketplace_name: MARKETPLACE_NAME.to_owned(),
            plugin_name: PLUGIN_NAME.to_owned(),
            plugin_version,
            render_sha256,
            registration_state,
            manual_commands: manual_commands.clone(),
            updated_at: Utc::now(),
        };
        atomic_write_json(&record_path, &record)?;
        self.installation.set_codex_integration(
            &install_root,
            Some(CodexIntegrationSummary {
                record_path: normal_windows_path(&record_path)
                    .to_string_lossy()
                    .into_owned(),
                registration_state,
            }),
        )?;
        Ok(result_from_record(
            action,
            "rendered",
            &record,
            manual_commands,
        ))
    }

    fn integration_root(&self, product_version: &str) -> PathBuf {
        self.installation
            .local_data_root()
            .join("integrations")
            .join("codex")
            .join(product_version)
    }
}

fn read_source_files(source_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, CodexAdapterError> {
    let mut files = BTreeMap::new();
    for relative in [
        MARKETPLACE_RELATIVE,
        PLUGIN_MANIFEST_RELATIVE,
        MCP_RELATIVE,
        HOOKS_RELATIVE,
        SKILL_RELATIVE,
    ] {
        files.insert(
            relative.to_owned(),
            read_regular_bounded(&source_root.join(relative.replace('/', "\\")))?,
        );
    }
    Ok(files)
}

fn render_files(
    source: &BTreeMap<String, Vec<u8>>,
    install_root: &Path,
    plugin_version: &str,
) -> Result<BTreeMap<String, Vec<u8>>, CodexAdapterError> {
    let mut rendered = source.clone();
    let plugin = strict_object(source, PLUGIN_MANIFEST_RELATIVE)?;
    let mut plugin = plugin;
    plugin.insert("version".to_owned(), plugin_version.into());
    rendered.insert(
        PLUGIN_MANIFEST_RELATIVE.to_owned(),
        pretty_json_object(plugin)?,
    );

    let mut mcp = strict_object(source, MCP_RELATIVE)?;
    let server = mcp
        .get_mut("mcpServers")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|servers| servers.get_mut("star-control"))
        .and_then(serde_json::Value::as_object_mut)
        .ok_or(CodexAdapterError::InvalidTemplate)?;
    server.insert(
        "command".to_owned(),
        install_root
            .join("star-mcp.exe")
            .to_string_lossy()
            .into_owned()
            .into(),
    );
    rendered.insert(MCP_RELATIVE.to_owned(), pretty_json_object(mcp)?);

    let mut hooks = strict_object(source, HOOKS_RELATIVE)?;
    let hook = hooks
        .get_mut("hooks")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|events| events.get_mut("SessionStart"))
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|groups| groups.first_mut())
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|group| group.get_mut("hooks"))
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|handlers| handlers.first_mut())
        .and_then(serde_json::Value::as_object_mut)
        .ok_or(CodexAdapterError::InvalidTemplate)?;
    let executable = install_root.join("star.exe").to_string_lossy().into_owned();
    hook.insert(
        "commandWindows".to_owned(),
        format!("\"{executable}\" hook session-start").into(),
    );
    rendered.insert(HOOKS_RELATIVE.to_owned(), pretty_json_object(hooks)?);
    Ok(rendered)
}

fn validate_rendered(
    rendered: &BTreeMap<String, Vec<u8>>,
    install_root: &Path,
    plugin_version: &str,
) -> Result<(), CodexAdapterError> {
    let marketplace = strict_object(rendered, MARKETPLACE_RELATIVE)?;
    let expected_marketplace_plugin_source = format!("./{PLUGIN_ROOT_RELATIVE}");
    if marketplace.get("name").and_then(|value| value.as_str()) != Some(MARKETPLACE_NAME)
        || marketplace
            .get("plugins")
            .and_then(|value| value.as_array())
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| plugin.get("name"))
            .and_then(|value| value.as_str())
            != Some(PLUGIN_NAME)
        || marketplace
            .get("plugins")
            .and_then(|value| value.as_array())
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| plugin.get("source"))
            .and_then(|value| value.get("source"))
            .and_then(|value| value.as_str())
            != Some("local")
        || marketplace
            .get("plugins")
            .and_then(|value| value.as_array())
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| plugin.get("source"))
            .and_then(|value| value.get("path"))
            .and_then(|value| value.as_str())
            != Some(expected_marketplace_plugin_source.as_str())
    {
        return Err(CodexAdapterError::InvalidRenderedPlugin);
    }
    let plugin = strict_object(rendered, PLUGIN_MANIFEST_RELATIVE)?;
    if plugin.get("name").and_then(|value| value.as_str()) != Some(PLUGIN_NAME)
        || plugin.get("version").and_then(|value| value.as_str()) != Some(plugin_version)
        || plugin.get("mcpServers").and_then(|value| value.as_str()) != Some("./.mcp.json")
        || plugin.contains_key("hooks")
    {
        return Err(CodexAdapterError::InvalidRenderedPlugin);
    }
    let mcp = strict_object(rendered, MCP_RELATIVE)?;
    let expected_mcp = install_root
        .join("star-mcp.exe")
        .to_string_lossy()
        .into_owned();
    if mcp
        .get("mcpServers")
        .and_then(|value| value.get("star-control"))
        .and_then(|value| value.get("command"))
        .and_then(|value| value.as_str())
        != Some(expected_mcp.as_str())
    {
        return Err(CodexAdapterError::InvalidRenderedPlugin);
    }
    let hooks = strict_object(rendered, HOOKS_RELATIVE)?;
    let expected_hook = format!(
        "\"{}\" hook session-start",
        install_root.join("star.exe").to_string_lossy()
    );
    if hooks
        .get("hooks")
        .and_then(|value| value.get("SessionStart"))
        .and_then(|value| value.as_array())
        .and_then(|groups| groups.first())
        .and_then(|group| group.get("hooks"))
        .and_then(|value| value.as_array())
        .and_then(|handlers| handlers.first())
        .and_then(|handler| handler.get("commandWindows"))
        .and_then(|value| value.as_str())
        != Some(expected_hook.as_str())
    {
        return Err(CodexAdapterError::InvalidRenderedPlugin);
    }
    let skill = rendered
        .get(SKILL_RELATIVE)
        .ok_or(CodexAdapterError::InvalidRenderedPlugin)?;
    if !skill.starts_with(b"---\n") && !skill.starts_with(b"---\r\n") {
        return Err(CodexAdapterError::InvalidRenderedPlugin);
    }
    Ok(())
}

fn write_rendered_files(
    marketplace_root: &Path,
    rendered: &BTreeMap<String, Vec<u8>>,
) -> Result<(), CodexAdapterError> {
    for (relative, bytes) in rendered {
        let destination = marketplace_root.join(relative.replace('/', "\\"));
        atomic_write(&destination, bytes)?;
    }
    Ok(())
}

fn read_rendered_files(
    marketplace_root: &Path,
) -> Result<BTreeMap<String, Vec<u8>>, CodexAdapterError> {
    let mut files = BTreeMap::new();
    for relative in [
        MARKETPLACE_RELATIVE,
        PLUGIN_MANIFEST_RELATIVE,
        MCP_RELATIVE,
        HOOKS_RELATIVE,
        SKILL_RELATIVE,
    ] {
        files.insert(
            relative.to_owned(),
            read_regular_bounded(&marketplace_root.join(relative.replace('/', "\\")))?,
        );
    }
    Ok(files)
}

fn rendered_hash(files: &BTreeMap<String, Vec<u8>>) -> Result<Sha256Hash, CodexAdapterError> {
    let index = files
        .iter()
        .map(|(path, bytes)| (path.clone(), Sha256Hash::digest(bytes).to_string()))
        .collect::<BTreeMap<_, _>>();
    canonical_sha256(&serde_json::to_value(index)?)
        .map_err(|_| CodexAdapterError::InvalidRenderedPlugin)
}

fn strict_object(
    files: &BTreeMap<String, Vec<u8>>,
    relative: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, CodexAdapterError> {
    let bytes = files
        .get(relative)
        .ok_or(CodexAdapterError::InvalidTemplate)?;
    let text = std::str::from_utf8(bytes).map_err(|_| CodexAdapterError::InvalidTemplate)?;
    parse_no_duplicate_keys(text)
        .map_err(|_| CodexAdapterError::InvalidTemplate)?
        .as_object()
        .cloned()
        .ok_or(CodexAdapterError::InvalidTemplate)
}

fn pretty_json_object(
    object: serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<u8>, CodexAdapterError> {
    let mut bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(object))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn registration_commands(marketplace_root: &Path) -> [Vec<String>; 2] {
    [
        vec![
            "plugin".to_owned(),
            "marketplace".to_owned(),
            "add".to_owned(),
            marketplace_root.to_string_lossy().into_owned(),
            "--json".to_owned(),
        ],
        vec![
            "plugin".to_owned(),
            "add".to_owned(),
            format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}"),
            "--json".to_owned(),
        ],
    ]
}

fn deregistration_commands() -> [Vec<String>; 2] {
    [
        vec![
            "plugin".to_owned(),
            "remove".to_owned(),
            format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}"),
            "--json".to_owned(),
        ],
        vec![
            "plugin".to_owned(),
            "marketplace".to_owned(),
            "remove".to_owned(),
            MARKETPLACE_NAME.to_owned(),
            "--json".to_owned(),
        ],
    ]
}

fn run_codex(executable: Option<&Path>, args: &[String]) -> bool {
    if let Some(path) = executable
        && open_regular_local_file(path).is_err()
    {
        return false;
    }
    let program = executable
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW.0);
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < CODEX_COMMAND_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn display_codex_command(executable: Option<&Path>, args: &[String]) -> String {
    let program = executable
        .map(|path| quote_argument(&path.to_string_lossy()))
        .unwrap_or_else(|| "codex".to_owned());
    std::iter::once(program)
        .chain(args.iter().map(|value| quote_argument(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_argument(value: &str) -> String {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_owned()
    }
}

fn result_from_record(
    action: &str,
    local_state: &str,
    record: &CodexIntegrationRecord,
    manual_commands: Vec<String>,
) -> IntegrationResult {
    let removal_pending = manual_commands
        .iter()
        .any(|command| command.contains(" plugin remove "));
    IntegrationResult {
        action: action.to_owned(),
        local_state: local_state.to_owned(),
        record_path: Path::new(&record.integration_root)
            .join(INTEGRATION_RECORD_FILE)
            .to_string_lossy()
            .into_owned(),
        marketplace_root: record.marketplace_root.clone(),
        registration_state: record.registration_state,
        requires_new_task: true,
        hook_trust_required: record.registration_state != CodexRegistrationState::Removed,
        manual_commands,
        manual_steps: if record.registration_state == CodexRegistrationState::Registered {
            vec![
                "Codex에서 새 작업을 연다.".to_owned(),
                "Codex /hooks에서 Star-Control SessionStart Hook을 검토하고 신뢰한다.".to_owned(),
            ]
        } else if removal_pending {
            vec![
                "manual_commands를 실행하거나 Codex Plugin 화면에서 star-control을 제거한다."
                    .to_owned(),
                "등록 해제 확인 전에는 Marketplace source를 보존한다.".to_owned(),
            ]
        } else {
            vec![
                "manual_commands를 실행하거나 Codex Plugin 화면에서 star-control을 설치한다."
                    .to_owned(),
                "새 작업을 열고 /hooks에서 SessionStart Hook을 검토하고 신뢰한다.".to_owned(),
            ]
        },
    }
}

fn read_regular_bounded(path: &Path) -> Result<Vec<u8>, CodexAdapterError> {
    let file = open_regular_local_file(path)?;
    let length = file.metadata()?.len();
    if length == 0 || length > SOURCE_FILE_MAX_BYTES {
        return Err(CodexAdapterError::InvalidTemplate);
    }
    use std::io::Read;
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(SOURCE_FILE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length {
        return Err(CodexAdapterError::InvalidTemplate);
    }
    Ok(bytes)
}

fn remove_owned_tree(path: &Path, local_data_root: &Path) -> Result<(), CodexAdapterError> {
    let path = canonical_fixed_directory(path)?;
    let local_data_root = canonical_fixed_directory(local_data_root)?;
    if !path.starts_with(local_data_root.join("integrations").join("codex")) {
        return Err(CodexAdapterError::Ownership);
    }
    reject_reparse_tree(&path)?;
    fs::remove_dir_all(path)?;
    Ok(())
}

fn reject_reparse_tree(path: &Path) -> Result<(), CodexAdapterError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(CodexAdapterError::Ownership);
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            reject_reparse_tree(&entry?.path())?;
        }
    }
    Ok(())
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_argv_is_exact_and_quotes_unicode_space_paths() {
        let root = Path::new(r"D:\도구 모음\Star-Control 연동");
        let commands = registration_commands(root);
        assert_eq!(
            commands[0],
            [
                "plugin",
                "marketplace",
                "add",
                r"D:\도구 모음\Star-Control 연동",
                "--json"
            ]
        );
        assert_eq!(
            commands[1],
            ["plugin", "add", "star-control@star-control-local", "--json"]
        );
        assert_eq!(
            display_codex_command(None, &commands[0]),
            "codex plugin marketplace add \"D:\\도구 모음\\Star-Control 연동\" --json"
        );
        assert_eq!(
            deregistration_commands()[0],
            [
                "plugin",
                "remove",
                "star-control@star-control-local",
                "--json"
            ]
        );
    }

    #[test]
    fn renderer_changes_only_owned_runtime_fields() {
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/codex-plugin-template/marketplace-root");
        let source = read_source_files(&source_root).unwrap();
        let install = Path::new(r"D:\도구\Star-Control 시험");
        let rendered = render_files(&source, install, "0.1.0+codex.0123456789ab").unwrap();
        validate_rendered(&rendered, install, "0.1.0+codex.0123456789ab").unwrap();
        assert_eq!(
            source.get(MARKETPLACE_RELATIVE),
            rendered.get(MARKETPLACE_RELATIVE)
        );
        assert_eq!(source.get(SKILL_RELATIVE), rendered.get(SKILL_RELATIVE));
        assert_ne!(source.get(MCP_RELATIVE), rendered.get(MCP_RELATIVE));
        assert_ne!(source.get(HOOKS_RELATIVE), rendered.get(HOOKS_RELATIVE));
    }

    #[test]
    fn source_and_rendered_plugin_have_closed_component_shapes() {
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/codex-plugin-template/marketplace-root");
        let source = read_source_files(&source_root).unwrap();
        assert_eq!(source.len(), 5);
        let marketplace = strict_object(&source, MARKETPLACE_RELATIVE).unwrap();
        let source_path = marketplace
            .get("plugins")
            .and_then(|value| value.as_array())
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| plugin.get("source"))
            .and_then(|value| value.get("path"))
            .and_then(|value| value.as_str())
            .unwrap();
        assert_eq!(source_path, format!("./{PLUGIN_ROOT_RELATIVE}"));
        let source_relative = source_path.strip_prefix("./").unwrap();
        assert_eq!(source_relative, PLUGIN_ROOT_RELATIVE);
        let plugin_root = source_root.join(source_relative.replace('/', "\\"));
        assert!(plugin_root.is_dir());
        assert_eq!(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            source_root.join(PLUGIN_MANIFEST_RELATIVE.replace('/', "\\"))
        );
        let plugin = strict_object(&source, PLUGIN_MANIFEST_RELATIVE).unwrap();
        assert!(!plugin.contains_key("hooks"));
        assert_eq!(plugin.get("mcpServers").unwrap(), "./.mcp.json");
    }

    #[test]
    fn persisted_manual_removal_is_not_reported_as_an_install_step() {
        let record = CodexIntegrationRecord {
            schema_id: CODEX_INTEGRATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: INSTALLATION_SCHEMA_VERSION,
            product_version: "0.1.0".to_owned(),
            install_root: r"D:\도구\Star-Control".to_owned(),
            integration_root: r"D:\state\integrations\codex\0.1.0".to_owned(),
            marketplace_root: r"D:\state\integrations\codex\0.1.0\marketplace-root".to_owned(),
            marketplace_name: MARKETPLACE_NAME.to_owned(),
            plugin_name: PLUGIN_NAME.to_owned(),
            plugin_version: "0.1.0+codex.0123456789ab".to_owned(),
            render_sha256: Sha256Hash::digest(b"rendered"),
            registration_state: CodexRegistrationState::ManualActionRequired,
            manual_commands: vec![
                "codex plugin remove star-control@star-control-local --json".to_owned(),
            ],
            updated_at: Utc::now(),
        };
        let result = result_from_record(
            "status",
            "verified",
            &record,
            record.manual_commands.clone(),
        );
        assert!(result.manual_steps[0].contains("제거"));
        assert!(!result.manual_steps[0].contains("설치"));
    }
}
