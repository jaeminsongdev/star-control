use serde_json::{Map, Number, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

mod cloud;
mod fake;
mod local_process;

pub use cloud::{
    is_cloud_cli_manifest, is_cloud_provider_manifest, CloudCliProviderAdapter,
    CloudProviderPreflightAdapter,
};
pub use fake::{
    load_execution_request, ExecutionRequest, FakeProviderAdapter, FakeProviderSimulation,
    ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
pub use local_process::{LocalProcessCommandPolicy, LocalProcessProviderAdapter};

const PROVIDER_MANIFEST_SCHEMA: &str = "provider-manifest.schema.json";
const PROVIDER_INSTANCE_SCHEMA: &str = "provider-instance.schema.json";
const CAPABILITY_PROFILE_SCHEMA: &str = "capability-profile.schema.json";
const PROVIDER_REGISTRY_SCHEMA: &str = "provider-registry.schema.json";

#[derive(Debug)]
pub enum ProviderRegistryError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedFormat {
        path: PathBuf,
    },
    InvalidYamlSubset {
        path: PathBuf,
        line: usize,
        message: String,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    PathTraversalBlocked {
        path: String,
    },
    AbsoluteRegistryPathBlocked {
        path: String,
    },
    DuplicateProvider {
        provider_id: String,
    },
    DuplicateCapabilityProfile {
        provider_id: String,
    },
    DuplicateInstance {
        instance_id: String,
    },
    ProviderNotFound {
        provider_id: String,
    },
    InstanceNotFound {
        instance_id: String,
    },
    CapabilityProfileNotFound {
        provider_id: String,
    },
    RegistryManifestIdMismatch {
        registry_id: String,
        manifest_id: String,
        manifest_path: PathBuf,
    },
    RegistryCapabilityProviderMismatch {
        registry_id: String,
        capability_provider: String,
        capability_path: PathBuf,
    },
}

impl fmt::Display for ProviderRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse JSON {}: {}",
                    path.display(),
                    source
                )
            }
            Self::UnsupportedFormat { path } => {
                write!(
                    formatter,
                    "unsupported provider contract format: {}",
                    path.display()
                )
            }
            Self::InvalidYamlSubset {
                path,
                line,
                message,
            } => write!(
                formatter,
                "failed to parse Star-Control YAML subset {} at line {}: {}",
                path.display(),
                line,
                message
            ),
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "failed to load schema {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed {
                path,
                schema_path,
                errors,
            } => write!(
                formatter,
                "schema validation failed for {} against {} with {} error(s)",
                path.display(),
                schema_path.display(),
                errors.len()
            ),
            Self::MissingField { path, field } => {
                write!(formatter, "missing field {} in {}", field, path.display())
            }
            Self::InvalidFieldType {
                path,
                field,
                expected,
            } => write!(
                formatter,
                "invalid field type for {} in {}, expected {}",
                field,
                path.display(),
                expected
            ),
            Self::PathTraversalBlocked { path } => {
                write!(formatter, "registry path traversal blocked: {}", path)
            }
            Self::AbsoluteRegistryPathBlocked { path } => {
                write!(formatter, "absolute registry path blocked: {}", path)
            }
            Self::DuplicateProvider { provider_id } => {
                write!(formatter, "duplicate provider manifest: {}", provider_id)
            }
            Self::DuplicateCapabilityProfile { provider_id } => {
                write!(formatter, "duplicate capability profile: {}", provider_id)
            }
            Self::DuplicateInstance { instance_id } => {
                write!(formatter, "duplicate provider instance: {}", instance_id)
            }
            Self::ProviderNotFound { provider_id } => {
                write!(formatter, "provider not found: {}", provider_id)
            }
            Self::InstanceNotFound { instance_id } => {
                write!(formatter, "provider instance not found: {}", instance_id)
            }
            Self::CapabilityProfileNotFound { provider_id } => {
                write!(formatter, "capability profile not found: {}", provider_id)
            }
            Self::RegistryManifestIdMismatch {
                registry_id,
                manifest_id,
                manifest_path,
            } => write!(
                formatter,
                "registry provider id {} does not match manifest id {} at {}",
                registry_id,
                manifest_id,
                manifest_path.display()
            ),
            Self::RegistryCapabilityProviderMismatch {
                registry_id,
                capability_provider,
                capability_path,
            } => write!(
                formatter,
                "registry provider id {} does not match capability provider {} at {}",
                registry_id,
                capability_provider,
                capability_path.display()
            ),
        }
    }
}

impl Error for ProviderRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderManifest {
    id: String,
    kind: String,
    transport: String,
    adapter: String,
    path: PathBuf,
    value: Value,
}

impl ProviderManifest {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn transport(&self) -> &str {
        &self.transport
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInstance {
    id: String,
    provider_id: String,
    enabled: bool,
    routing_tags: Vec<String>,
    path: PathBuf,
    value: Value,
}

impl ProviderInstance {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn routing_tags(&self) -> &[String] {
        &self.routing_tags
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityProfile {
    provider_id: String,
    routing_tags: Vec<String>,
    path: PathBuf,
    value: Value,
}

impl CapabilityProfile {
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn routing_tags(&self) -> &[String] {
        &self.routing_tags
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn capability(&self, name: &str) -> Option<CapabilityValue<'_>> {
        self.value
            .pointer("/capability_profile/can")
            .and_then(Value::as_object)
            .and_then(|can| can.get(name))
            .and_then(CapabilityValue::from_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityValue<'a> {
    Bool(bool),
    Mode(&'a str),
}

impl<'a> CapabilityValue<'a> {
    pub fn from_value(value: &'a Value) -> Option<Self> {
        if let Some(flag) = value.as_bool() {
            return Some(Self::Bool(flag));
        }

        value.as_str().map(Self::Mode)
    }

    pub fn is_enabled(self) -> bool {
        match self {
            Self::Bool(flag) => flag,
            Self::Mode(mode) => matches!(mode, "true" | "partial" | "manual"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryEntry {
    id: String,
    manifest: String,
    capabilities: String,
}

impl ProviderRegistryEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn manifest(&self) -> &str {
        &self.manifest
    }

    pub fn capabilities(&self) -> &str {
        &self.capabilities
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRegistryDocument {
    schema_version: String,
    entries: Vec<ProviderRegistryEntry>,
    path: PathBuf,
    value: Value,
}

impl ProviderRegistryDocument {
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn entries(&self) -> &[ProviderRegistryEntry] {
        &self.entries
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderRegistry {
    manifests: BTreeMap<String, ProviderManifest>,
    capabilities: BTreeMap<String, CapabilityProfile>,
    instances: BTreeMap<String, ProviderInstance>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_manifest(
        &mut self,
        manifest: ProviderManifest,
    ) -> Result<(), ProviderRegistryError> {
        let provider_id = manifest.id().to_string();
        if self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::DuplicateProvider { provider_id });
        }

        self.manifests.insert(provider_id, manifest);
        Ok(())
    }

    pub fn register_capability_profile(
        &mut self,
        profile: CapabilityProfile,
    ) -> Result<(), ProviderRegistryError> {
        let provider_id = profile.provider_id().to_string();
        if !self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::ProviderNotFound { provider_id });
        }
        if self.capabilities.contains_key(&provider_id) {
            return Err(ProviderRegistryError::DuplicateCapabilityProfile { provider_id });
        }

        self.capabilities.insert(provider_id, profile);
        Ok(())
    }

    pub fn register_instance(
        &mut self,
        instance: ProviderInstance,
    ) -> Result<(), ProviderRegistryError> {
        let instance_id = instance.id().to_string();
        let provider_id = instance.provider_id().to_string();
        if !self.manifests.contains_key(&provider_id) {
            return Err(ProviderRegistryError::ProviderNotFound { provider_id });
        }
        if self.instances.contains_key(&instance_id) {
            return Err(ProviderRegistryError::DuplicateInstance { instance_id });
        }

        self.instances.insert(instance_id, instance);
        Ok(())
    }

    pub fn manifest(&self, provider_id: &str) -> Option<&ProviderManifest> {
        self.manifests.get(provider_id)
    }

    pub fn capability_profile(&self, provider_id: &str) -> Option<&CapabilityProfile> {
        self.capabilities.get(provider_id)
    }

    pub fn instance(&self, instance_id: &str) -> Option<&ProviderInstance> {
        self.instances.get(instance_id)
    }

    pub fn manifest_for_instance(
        &self,
        instance_id: &str,
    ) -> Result<&ProviderManifest, ProviderRegistryError> {
        let instance =
            self.instance(instance_id)
                .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;
        self.manifest(instance.provider_id()).ok_or_else(|| {
            ProviderRegistryError::ProviderNotFound {
                provider_id: instance.provider_id().to_string(),
            }
        })
    }

    pub fn capability_for_instance(
        &self,
        instance_id: &str,
    ) -> Result<&CapabilityProfile, ProviderRegistryError> {
        let instance =
            self.instance(instance_id)
                .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;
        self.capability_profile(instance.provider_id())
            .ok_or_else(|| ProviderRegistryError::CapabilityProfileNotFound {
                provider_id: instance.provider_id().to_string(),
            })
    }

    pub fn providers_by_kind(&self, kind: &str) -> Vec<&ProviderManifest> {
        self.manifests
            .values()
            .filter(|manifest| manifest.kind() == kind)
            .collect()
    }

    pub fn providers_by_transport(&self, transport: &str) -> Vec<&ProviderManifest> {
        self.manifests
            .values()
            .filter(|manifest| manifest.transport() == transport)
            .collect()
    }

    pub fn instances_for_provider(&self, provider_id: &str) -> Vec<&ProviderInstance> {
        self.instances
            .values()
            .filter(|instance| instance.provider_id() == provider_id)
            .collect()
    }

    pub fn enabled_instances(&self) -> Vec<&ProviderInstance> {
        self.instances
            .values()
            .filter(|instance| instance.enabled())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRegistryLoader {
    repo_root: PathBuf,
    schema_root: PathBuf,
}

impl ProviderRegistryLoader {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        let schema_root = repo_root.join("specs").join("schemas");
        Self {
            repo_root,
            schema_root,
        }
    }

    pub fn with_schema_root(
        repo_root: impl Into<PathBuf>,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            schema_root: schema_root.into(),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }

    pub fn load_manifest(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderManifest, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_MANIFEST_SCHEMA)?;

        Ok(ProviderManifest {
            id: required_string(&value, &path, "id")?,
            kind: required_string(&value, &path, "kind")?,
            transport: required_string(&value, &path, "transport")?,
            adapter: required_string(&value, &path, "adapter")?,
            path,
            value,
        })
    }

    pub fn load_instance(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderInstance, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_INSTANCE_SCHEMA)?;

        Ok(ProviderInstance {
            id: required_string(&value, &path, "id")?,
            provider_id: required_string(&value, &path, "provider")?,
            enabled: required_bool(&value, &path, "enabled")?,
            routing_tags: required_string_array(&value, &path, "routing_tags")?,
            path,
            value,
        })
    }

    pub fn load_capability_profile(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<CapabilityProfile, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, CAPABILITY_PROFILE_SCHEMA)?;

        Ok(CapabilityProfile {
            provider_id: required_string(&value, &path, "provider")?,
            routing_tags: pointer_string_array(&value, &path, "/capability_profile/routing_tags")?,
            path,
            value,
        })
    }

    pub fn load_registry_document(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProviderRegistryDocument, ProviderRegistryError> {
        let path = self.resolve_input_path(path.as_ref())?;
        let value = self.load_contract_value(&path)?;
        self.validate_contract(&value, &path, PROVIDER_REGISTRY_SCHEMA)?;

        let schema_version = match value.get("schema_version") {
            Some(Value::String(version)) => version.clone(),
            Some(Value::Number(version)) => version.to_string(),
            Some(_) => {
                return Err(ProviderRegistryError::InvalidFieldType {
                    path,
                    field: "schema_version".to_string(),
                    expected: "string or number".to_string(),
                });
            }
            None => {
                return Err(ProviderRegistryError::MissingField {
                    path,
                    field: "schema_version".to_string(),
                });
            }
        };

        let providers = value
            .get("providers")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
                path: path.clone(),
                field: "providers".to_string(),
                expected: "array".to_string(),
            })?;
        let mut entries = Vec::with_capacity(providers.len());
        for (index, provider) in providers.iter().enumerate() {
            let entry_path = format!("providers[{}]", index);
            entries.push(ProviderRegistryEntry {
                id: nested_required_string(provider, &path, &entry_path, "id")?,
                manifest: nested_required_string(provider, &path, &entry_path, "manifest")?,
                capabilities: nested_required_string(provider, &path, &entry_path, "capabilities")?,
            });
        }

        Ok(ProviderRegistryDocument {
            schema_version,
            entries,
            path,
            value,
        })
    }

    pub fn load_registry(
        &self,
        registry_path: impl AsRef<Path>,
        instance_paths: &[PathBuf],
    ) -> Result<ProviderRegistry, ProviderRegistryError> {
        let registry_document = self.load_registry_document(registry_path)?;
        let mut registry = ProviderRegistry::new();

        for entry in registry_document.entries() {
            let manifest_path = self.resolve_registry_entry_path(entry.manifest())?;
            let manifest = self.load_manifest(&manifest_path)?;
            if manifest.id() != entry.id() {
                return Err(ProviderRegistryError::RegistryManifestIdMismatch {
                    registry_id: entry.id().to_string(),
                    manifest_id: manifest.id().to_string(),
                    manifest_path,
                });
            }
            registry.register_manifest(manifest)?;

            let capability_path = self.resolve_registry_entry_path(entry.capabilities())?;
            let profile = self.load_capability_profile(&capability_path)?;
            if profile.provider_id() != entry.id() {
                return Err(ProviderRegistryError::RegistryCapabilityProviderMismatch {
                    registry_id: entry.id().to_string(),
                    capability_provider: profile.provider_id().to_string(),
                    capability_path,
                });
            }
            registry.register_capability_profile(profile)?;
        }

        for instance_path in instance_paths {
            let instance = self.load_instance(instance_path)?;
            registry.register_instance(instance)?;
        }

        Ok(registry)
    }

    pub fn load_fake_default_registry(&self) -> Result<ProviderRegistry, ProviderRegistryError> {
        self.load_registry(
            "examples/provider-contracts/provider-registry.example.json",
            &[PathBuf::from(
                "examples/provider-contracts/provider-instance.fake.example.json",
            )],
        )
    }

    fn load_contract_value(&self, path: &Path) -> Result<Value, ProviderRegistryError> {
        let content = fs::read_to_string(path).map_err(|source| ProviderRegistryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => serde_json::from_str(&content).map_err(|source| {
                ProviderRegistryError::InvalidJson {
                    path: path.to_path_buf(),
                    source,
                }
            }),
            Some("yaml") | Some("yml") => parse_star_control_yaml_subset(path, &content),
            _ => Err(ProviderRegistryError::UnsupportedFormat {
                path: path.to_path_buf(),
            }),
        }
    }

    fn validate_contract(
        &self,
        value: &Value,
        path: &Path,
        schema_file: &str,
    ) -> Result<(), ProviderRegistryError> {
        let schema_path = self.schema_root.join(schema_file);
        let schema = load_schema(&schema_path).map_err(|source| {
            ProviderRegistryError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            }
        })?;
        let result = validate_json(value, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ProviderRegistryError::SchemaValidationFailed {
                path: path.to_path_buf(),
                schema_path,
                errors: result.errors,
            })
        }
    }

    fn resolve_input_path(&self, path: &Path) -> Result<PathBuf, ProviderRegistryError> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            self.resolve_registry_entry_path(path.to_string_lossy().as_ref())
        }
    }

    fn resolve_registry_entry_path(&self, path: &str) -> Result<PathBuf, ProviderRegistryError> {
        let relative = Path::new(path);
        if relative.is_absolute() {
            return Err(ProviderRegistryError::AbsoluteRegistryPathBlocked {
                path: path.to_string(),
            });
        }
        for component in relative.components() {
            if matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            ) {
                return Err(ProviderRegistryError::PathTraversalBlocked {
                    path: path.to_string(),
                });
            }
        }

        Ok(self.repo_root.join(relative))
    }
}

fn required_string(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<String, ProviderRegistryError> {
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

fn required_bool(value: &Value, path: &Path, field: &str) -> Result<bool, ProviderRegistryError> {
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_bool()
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "boolean".to_string(),
        })
}

fn required_string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    let values = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array of string".to_string(),
        }
    })?;
    string_array_from_values(values, path, field)
}

fn pointer_string_array(
    value: &Value,
    path: &Path,
    pointer: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    let values = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: pointer.to_string(),
            expected: "array of string".to_string(),
        })?;
    string_array_from_values(values, path, pointer)
}

fn string_array_from_values(
    values: &[Value],
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ProviderRegistryError::InvalidFieldType {
                    path: path.to_path_buf(),
                    field: field.to_string(),
                    expected: "array of string".to_string(),
                }
            })
        })
        .collect()
}

fn nested_required_string(
    value: &Value,
    path: &Path,
    parent: &str,
    field: &str,
) -> Result<String, ProviderRegistryError> {
    let full_field = format!("{}.{}", parent, field);
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: full_field.clone(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: full_field,
            expected: "string".to_string(),
        })
}

#[derive(Debug, Clone)]
struct YamlLine {
    number: usize,
    indent: usize,
    text: String,
}

fn parse_star_control_yaml_subset(
    path: &Path,
    content: &str,
) -> Result<Value, ProviderRegistryError> {
    let mut lines = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let without_comment = strip_yaml_comment(raw_line);
        if without_comment.trim().is_empty() {
            continue;
        }
        if without_comment.starts_with('\t') {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: index + 1,
                message: "tabs are not supported".to_string(),
            });
        }

        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        lines.push(YamlLine {
            number: index + 1,
            indent,
            text: without_comment.trim().to_string(),
        });
    }

    if lines.is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    let mut cursor = 0;
    let value = parse_yaml_block(path, &lines, &mut cursor, lines[0].indent)?;
    if cursor != lines.len() {
        return Err(ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: lines[cursor].number,
            message: "unexpected trailing content".to_string(),
        });
    }

    Ok(value)
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, character) in line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_yaml_block(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let line = lines
        .get(*cursor)
        .ok_or_else(|| ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: 0,
            message: "unexpected end of document".to_string(),
        })?;

    if line.indent != indent {
        return Err(ProviderRegistryError::InvalidYamlSubset {
            path: path.to_path_buf(),
            line: line.number,
            message: format!("expected indent {}, got {}", indent, line.indent),
        });
    }

    if line.text.starts_with("- ") {
        parse_yaml_sequence(path, lines, cursor, indent)
    } else {
        parse_yaml_mapping(path, lines, cursor, indent)
    }
}

fn parse_yaml_mapping(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let mut object = Map::new();

    while let Some(line) = lines.get(*cursor) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: line.number,
                message: format!("unexpected nested indent {}", line.indent),
            });
        }
        if line.text.starts_with("- ") {
            break;
        }

        let (key, raw_value) = split_yaml_key_value(path, line)?;
        *cursor += 1;
        let value = if raw_value.is_empty() {
            if let Some(next_line) = lines.get(*cursor) {
                if next_line.indent <= indent {
                    Value::Null
                } else {
                    parse_yaml_block(path, lines, cursor, next_line.indent)?
                }
            } else {
                Value::Null
            }
        } else {
            parse_yaml_scalar(raw_value)
        };
        object.insert(key.to_string(), value);
    }

    Ok(Value::Object(object))
}

fn parse_yaml_sequence(
    path: &Path,
    lines: &[YamlLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Value, ProviderRegistryError> {
    let mut values = Vec::new();

    while let Some(line) = lines.get(*cursor) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ProviderRegistryError::InvalidYamlSubset {
                path: path.to_path_buf(),
                line: line.number,
                message: format!("unexpected nested indent {}", line.indent),
            });
        }
        if !line.text.starts_with("- ") {
            break;
        }

        let rest = line.text[2..].trim();
        *cursor += 1;
        if rest.is_empty() {
            if let Some(next_line) = lines.get(*cursor) {
                values.push(parse_yaml_block(path, lines, cursor, next_line.indent)?);
            } else {
                values.push(Value::Null);
            }
            continue;
        }

        if let Some((key, raw_value)) = split_inline_yaml_pair(rest) {
            let mut item = Map::new();
            let value = if raw_value.is_empty() {
                if let Some(next_line) = lines.get(*cursor) {
                    if next_line.indent <= indent {
                        Value::Null
                    } else {
                        parse_yaml_block(path, lines, cursor, next_line.indent)?
                    }
                } else {
                    Value::Null
                }
            } else {
                parse_yaml_scalar(raw_value)
            };
            item.insert(key.to_string(), value);

            while let Some(next_line) = lines.get(*cursor) {
                if next_line.indent <= indent {
                    break;
                }
                if next_line.text.starts_with("- ") {
                    break;
                }
                let nested_indent = next_line.indent;
                let nested = parse_yaml_mapping(path, lines, cursor, nested_indent)?;
                if let Value::Object(nested_map) = nested {
                    for (nested_key, nested_value) in nested_map {
                        item.insert(nested_key, nested_value);
                    }
                }
            }

            values.push(Value::Object(item));
        } else {
            values.push(parse_yaml_scalar(rest));
        }
    }

    Ok(Value::Array(values))
}

fn split_yaml_key_value<'a>(
    path: &Path,
    line: &'a YamlLine,
) -> Result<(&'a str, &'a str), ProviderRegistryError> {
    split_inline_yaml_pair(&line.text).ok_or_else(|| ProviderRegistryError::InvalidYamlSubset {
        path: path.to_path_buf(),
        line: line.number,
        message: "expected key: value mapping".to_string(),
    })
}

fn split_inline_yaml_pair(text: &str) -> Option<(&str, &str)> {
    let index = text.find(':')?;
    let key = text[..index].trim();
    if key.is_empty() {
        return None;
    }
    Some((key, text[index + 1..].trim()))
}

fn parse_yaml_scalar(raw_value: &str) -> Value {
    let value = raw_value.trim();
    if value.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if value.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if value.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Ok(number) = value.parse::<i64>() {
        return Value::Number(Number::from(number));
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return Value::String(value[1..value.len() - 1].to_string());
    }
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Value::String(value[1..value.len() - 1].to_string());
    }
    Value::String(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_fake_default_registry_from_json_contracts() {
        let loader = ProviderRegistryLoader::new(repo_root());
        let registry = loader
            .load_fake_default_registry()
            .expect("load fake default registry");

        let instance = registry
            .instance("fake-default")
            .expect("fake-default instance");
        assert_eq!(instance.provider_id(), "provider.fake");
        assert!(instance.enabled());

        let manifest = registry
            .manifest_for_instance("fake-default")
            .expect("manifest for fake-default");
        assert_eq!(manifest.id(), "provider.fake");
        assert_eq!(manifest.kind(), "fake_provider");
        assert_eq!(manifest.transport(), "manual");

        let profile = registry
            .capability_for_instance("fake-default")
            .expect("capability for fake-default");
        assert_eq!(
            profile.capability("read_repo"),
            Some(CapabilityValue::Bool(true))
        );
        assert_eq!(
            profile.capability("run_shell"),
            Some(CapabilityValue::Bool(false))
        );
    }

    #[test]
    fn loads_builtin_yaml_registry_and_fake_provider_contracts() {
        let loader = ProviderRegistryLoader::new(repo_root());
        let registry = loader
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[PathBuf::from(
                    "configs/provider-instances/fake-provider.example.yaml",
                )],
            )
            .expect("load builtin registry");

        let fake = registry.manifest("provider.fake").expect("fake provider");
        assert_eq!(fake.adapter(), "code_agent");
        assert_eq!(registry.providers_by_kind("fake_provider").len(), 1);
        assert!(registry.providers_by_transport("manual").len() >= 2);

        let instance = registry.instance("fake-default").expect("fake instance");
        assert_eq!(instance.provider_id(), "provider.fake");

        let profile = registry
            .capability_for_instance("fake-default")
            .expect("fake capability");
        assert!(profile.routing_tags().contains(&"test".to_string()));
        assert_eq!(
            profile.capability("return_json"),
            Some(CapabilityValue::Bool(true))
        );
    }

    #[test]
    fn rejects_instance_with_unknown_provider() {
        let loader = ProviderRegistryLoader::new(repo_root());
        let instance_path = write_temp_json(
            "unknown-provider-instance.json",
            &json!({
                "id": "unknown-default",
                "provider": "provider.unknown",
                "enabled": true,
                "limits": {
                    "timeout_seconds": 10,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["test"]
            }),
        );

        let error = loader
            .load_registry(
                "examples/provider-contracts/provider-registry.example.json",
                std::slice::from_ref(&instance_path),
            )
            .expect_err("unknown provider should fail");
        fs::remove_file(instance_path).ok();

        assert!(matches!(
            error,
            ProviderRegistryError::ProviderNotFound { provider_id } if provider_id == "provider.unknown"
        ));
    }

    #[test]
    fn rejects_registry_path_traversal() {
        let loader = ProviderRegistryLoader::new(repo_root());
        let error = loader
            .resolve_registry_entry_path("../outside/provider.yaml")
            .expect_err("path traversal should fail");

        assert!(matches!(
            error,
            ProviderRegistryError::PathTraversalBlocked { .. }
        ));
    }

    #[test]
    fn rejects_schema_invalid_manifest() {
        let loader = ProviderRegistryLoader::new(repo_root());
        let manifest_path = write_temp_json(
            "invalid-provider-manifest.json",
            &json!({
                "id": "provider.invalid",
                "name": "Invalid Provider",
                "kind": "not_a_kind",
                "transport": "manual",
                "adapter": "code_agent",
                "capabilities": {
                    "edit_files": false,
                    "run_shell": false,
                    "read_repo": true,
                    "apply_patch": false,
                    "structured_output": true,
                    "offline": true,
                    "requires_login_session": false
                },
                "risk": {
                    "can_modify_workspace": false,
                    "can_run_commands": false,
                    "requires_sandbox": false
                },
                "outputs": {
                    "parser": "invalid"
                }
            }),
        );

        let error = loader
            .load_manifest(&manifest_path)
            .expect_err("invalid manifest should fail schema validation");
        fs::remove_file(manifest_path).ok();

        assert!(matches!(
            error,
            ProviderRegistryError::SchemaValidationFailed { .. }
        ));
    }

    #[test]
    fn parses_star_control_yaml_subset() {
        let path = PathBuf::from("fixture.yaml");
        let value = parse_star_control_yaml_subset(
            &path,
            r#"
schema_version: 0.1.0
providers:
  - id: provider.fake
    manifest: builtin-providers/test/fake-provider/provider.yaml
    capabilities: builtin-providers/test/fake-provider/capabilities.yaml
capability_profile:
  can:
    run_shell: false
    return_json: partial
  routing_tags:
    - test
"#,
        )
        .expect("parse yaml subset");

        assert_eq!(value["schema_version"], "0.1.0");
        assert_eq!(value["providers"][0]["id"], "provider.fake");
        assert_eq!(value["capability_profile"]["can"]["run_shell"], false);
        assert_eq!(value["capability_profile"]["can"]["return_json"], "partial");
        assert_eq!(value["capability_profile"]["routing_tags"][0], "test");
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn write_temp_json(name: &str, value: &Value) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "star-control-provider-{}-{}-{}",
            std::process::id(),
            nanos,
            name
        ));
        fs::write(
            &path,
            serde_json::to_string_pretty(value).expect("serialize fixture"),
        )
        .expect("write fixture");
        path
    }
}
