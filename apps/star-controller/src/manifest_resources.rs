//! Filesystem validation for manifest-owned JSON Schema resources.
//!
//! `star-contracts` owns the wire types and static rules.  This module keeps
//! path resolution and bounded file IO in the Controller, where the manifest
//! source directory and final filesystem identity are available.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use star_contracts::{
    Sha256Hash,
    manifest::{ActionDescriptor, ToolPackageManifest},
    parse_no_duplicate_keys,
};
use thiserror::Error;

const PACKAGE_SCHEMA_BYTES: u64 = 4 * 1024 * 1024;
const ACTION_RESOLVED_SCHEMA_BYTES: usize = 1024 * 1024;
const MAX_REF_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaLimits {
    pub package_bytes: u64,
    pub action_resolved_bytes: usize,
    pub max_ref_depth: usize,
}

impl Default for SchemaLimits {
    fn default() -> Self {
        Self {
            package_bytes: PACKAGE_SCHEMA_BYTES,
            action_resolved_bytes: ACTION_RESOLVED_SCHEMA_BYTES,
            max_ref_depth: MAX_REF_DEPTH,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceValidationError {
    #[error("schema path escapes the manifest package directory")]
    Path,
    #[error("schema file is missing or unreadable")]
    Io,
    #[error("schema JSON is invalid")]
    Json,
    #[error("remote, cyclic, or excessively deep schema reference")]
    Reference,
    #[error("package schema bytes exceed 4 MiB")]
    PackageSize,
    #[error("one action's resolved schema bytes exceed 1 MiB")]
    ActionSize,
    #[error("schema root or keyword violates the v1 subset")]
    Schema,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedActionSchemas {
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestResources {
    #[serde(default)]
    pub schema_hashes: BTreeMap<String, Sha256Hash>,
    #[serde(default)]
    pub action_schemas: BTreeMap<String, ResolvedActionSchemas>,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("value does not satisfy the resolved Draft 2020-12 Schema")]
pub struct SchemaInstanceError;

/// Validates every schema referenced by a parsed manifest.  All paths are
/// resolved relative to the final manifest parent and must remain under it
/// after canonicalization.  Files are cached by final path so the package byte
/// limit counts each source once.
pub fn validate_manifest_resources(
    manifest: &ToolPackageManifest,
    manifest_path: &Path,
) -> Result<(), ResourceValidationError> {
    load_manifest_resources(manifest, manifest_path).map(|_| ())
}

/// Loads the immutable Schema resources that belong to one package candidate.
/// References are fully inlined before the snapshot is published so runtime
/// validation never reopens mutable package files.
pub fn load_manifest_resources(
    manifest: &ToolPackageManifest,
    manifest_path: &Path,
) -> Result<ManifestResources, ResourceValidationError> {
    load_manifest_resources_with_limits(manifest, manifest_path, SchemaLimits::default())
}

pub fn load_manifest_resources_with_limits(
    manifest: &ToolPackageManifest,
    manifest_path: &Path,
    limits: SchemaLimits,
) -> Result<ManifestResources, ResourceValidationError> {
    let parent = manifest_path
        .parent()
        .ok_or(ResourceValidationError::Path)?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| ResourceValidationError::Io)?;
    let mut resolver = SchemaResolver {
        package_root: canonical_parent,
        documents: BTreeMap::new(),
        schema_hashes: BTreeMap::new(),
        action_schemas: BTreeMap::new(),
        package_bytes: 0,
        limits,
    };
    for action in &manifest.actions {
        let schemas = resolver.resolve_action(action)?;
        if let Some(input) = &schemas.input {
            for example in &action.examples {
                normalize_schema_arguments(input, &example.arguments)
                    .map_err(|_| ResourceValidationError::Schema)?;
            }
        }
        resolver
            .action_schemas
            .insert(action.tool_id.clone(), schemas);
    }
    Ok(ManifestResources {
        schema_hashes: resolver.schema_hashes,
        action_schemas: resolver.action_schemas,
    })
}

struct SchemaResolver {
    package_root: PathBuf,
    documents: BTreeMap<PathBuf, serde_json::Value>,
    schema_hashes: BTreeMap<String, Sha256Hash>,
    action_schemas: BTreeMap<String, ResolvedActionSchemas>,
    package_bytes: u64,
    limits: SchemaLimits,
}

impl SchemaResolver {
    fn resolve_action(
        &mut self,
        action: &ActionDescriptor,
    ) -> Result<ResolvedActionSchemas, ResourceValidationError> {
        let mut resolved_bytes = 0usize;
        let mut schemas = ResolvedActionSchemas::default();
        for (path, input) in [
            (action.input_schema_file.as_deref(), true),
            (action.output_schema_file.as_deref(), false),
        ] {
            let Some(path) = path else { continue };
            let root = self.load_relative(Path::new(path))?;
            validate_schema_root(&root, input)?;
            let mut stack = Vec::new();
            let resolved =
                self.resolve_schema(&root, self.resolve_path(Path::new(path))?, 0, &mut stack)?;
            jsonschema::draft202012::meta::validate(&resolved)
                .map_err(|_| ResourceValidationError::Schema)?;
            jsonschema::draft202012::options()
                .build(&resolved)
                .map_err(|_| ResourceValidationError::Schema)?;
            resolved_bytes = resolved_bytes
                .checked_add(
                    serde_json::to_vec(&resolved)
                        .map_err(|_| ResourceValidationError::Json)?
                        .len(),
                )
                .ok_or(ResourceValidationError::ActionSize)?;
            if resolved_bytes > self.limits.action_resolved_bytes {
                return Err(ResourceValidationError::ActionSize);
            }
            if input {
                schemas.input = Some(resolved);
            } else {
                schemas.output = Some(resolved);
            }
        }
        if let Some(input) = &schemas.input {
            validate_schema_bindings(action, input)?;
        }
        Ok(schemas)
    }

    fn resolve_schema(
        &mut self,
        value: &serde_json::Value,
        document_path: PathBuf,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Result<serde_json::Value, ResourceValidationError> {
        if depth > self.limits.max_ref_depth {
            return Err(ResourceValidationError::Reference);
        }
        reject_forbidden_keywords(value)?;
        match value {
            serde_json::Value::Object(object) => {
                if object.contains_key("$ref") {
                    let reference = object
                        .get("$ref")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(ResourceValidationError::Schema)?;
                    let (target_path, fragment) =
                        self.resolve_reference(&document_path, reference)?;
                    let identity = format!("{}#{fragment}", target_path.display());
                    if stack.contains(&identity) {
                        return Err(ResourceValidationError::Reference);
                    }
                    stack.push(identity);
                    let document = self.load_final(&target_path)?;
                    let target = resolve_fragment(&document, fragment)?;
                    let resolved_target =
                        self.resolve_schema(&target, target_path, depth + 1, stack)?;
                    stack.pop();

                    let mut siblings = serde_json::Map::new();
                    for (key, child) in object {
                        if key != "$ref" {
                            siblings.insert(
                                key.clone(),
                                self.resolve_schema(child, document_path.clone(), depth, stack)?,
                            );
                        }
                    }
                    if siblings.is_empty() {
                        Ok(resolved_target)
                    } else {
                        Ok(serde_json::json!({
                            "allOf": [resolved_target, serde_json::Value::Object(siblings)]
                        }))
                    }
                } else {
                    object
                        .iter()
                        .map(|(key, child)| {
                            self.resolve_schema(child, document_path.clone(), depth, stack)
                                .map(|child| (key.clone(), child))
                        })
                        .collect::<Result<serde_json::Map<_, _>, _>>()
                        .map(serde_json::Value::Object)
                }
            }
            serde_json::Value::Array(items) => items
                .iter()
                .map(|child| self.resolve_schema(child, document_path.clone(), depth, stack))
                .collect::<Result<Vec<_>, _>>()
                .map(serde_json::Value::Array),
            _ => Ok(value.clone()),
        }
    }

    fn resolve_reference(
        &self,
        current: &Path,
        reference: &str,
    ) -> Result<(PathBuf, String), ResourceValidationError> {
        if reference.starts_with("http:")
            || reference.starts_with("https:")
            || reference.starts_with("file:")
            || reference.starts_with("urn:")
            || reference.starts_with("//")
        {
            return Err(ResourceValidationError::Reference);
        }
        let (path, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        let target = if path.is_empty() {
            current.to_path_buf()
        } else {
            let parent = current.parent().ok_or(ResourceValidationError::Path)?;
            self.resolve_from(parent, Path::new(path))?
        };
        Ok((target, fragment.to_owned()))
    }

    fn resolve_path(&self, relative: &Path) -> Result<PathBuf, ResourceValidationError> {
        self.resolve_from(&self.package_root, relative)
    }

    fn resolve_from(
        &self,
        parent: &Path,
        relative: &Path,
    ) -> Result<PathBuf, ResourceValidationError> {
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ResourceValidationError::Path);
        }
        let unresolved = parent.join(relative);
        if !non_reparse_path(&self.package_root, &unresolved) {
            return Err(ResourceValidationError::Path);
        }
        let final_path = unresolved
            .canonicalize()
            .map_err(|_| ResourceValidationError::Io)?;
        if !final_path.starts_with(&self.package_root) || !final_path.is_file() {
            return Err(ResourceValidationError::Path);
        }
        Ok(final_path)
    }

    fn load_relative(
        &mut self,
        relative: &Path,
    ) -> Result<serde_json::Value, ResourceValidationError> {
        let final_path = self.resolve_path(relative)?;
        self.load_final(&final_path)
    }

    fn load_final(
        &mut self,
        final_path: &Path,
    ) -> Result<serde_json::Value, ResourceValidationError> {
        if let Some(value) = self.documents.get(final_path) {
            return Ok(value.clone());
        }
        let file = open_schema_file(final_path).map_err(|_| ResourceValidationError::Io)?;
        let metadata = file.metadata().map_err(|_| ResourceValidationError::Io)?;
        if !metadata.is_file() {
            return Err(ResourceValidationError::Path);
        }
        self.package_bytes = self
            .package_bytes
            .checked_add(metadata.len())
            .ok_or(ResourceValidationError::PackageSize)?;
        if self.package_bytes > self.limits.package_bytes {
            return Err(ResourceValidationError::PackageSize);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        (&file)
            .take(metadata.len() + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ResourceValidationError::Io)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(ResourceValidationError::Io);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| ResourceValidationError::Json)?;
        let value = parse_no_duplicate_keys(text).map_err(|_| ResourceValidationError::Json)?;
        let relative = final_path
            .strip_prefix(&self.package_root)
            .map_err(|_| ResourceValidationError::Path)?
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        self.schema_hashes
            .insert(relative, Sha256Hash::digest(&bytes));
        self.documents
            .insert(final_path.to_path_buf(), value.clone());
        Ok(value)
    }
}

#[cfg(windows)]
fn non_reparse_path(root: &Path, candidate: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let Ok(relative) = candidate.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .ok()
            .is_none_or(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        {
            return false;
        }
    }
    true
}

#[cfg(not(windows))]
fn non_reparse_path(root: &Path, candidate: &Path) -> bool {
    candidate.starts_with(root)
}

#[cfg(windows)]
fn open_schema_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ};

    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
}

#[cfg(not(windows))]
fn open_schema_file(path: &Path) -> std::io::Result<fs::File> {
    fs::File::open(path)
}

fn validate_schema_root(
    root: &serde_json::Value,
    input: bool,
) -> Result<(), ResourceValidationError> {
    let object = root.as_object().ok_or(ResourceValidationError::Schema)?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("object") {
        return Err(ResourceValidationError::Schema);
    }
    if let Some(dialect) = object.get("$schema")
        && dialect.as_str() != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return Err(ResourceValidationError::Schema);
    }
    if !object.contains_key("additionalProperties") {
        return Err(ResourceValidationError::Schema);
    }
    if input
        && object
            .get("additionalProperties")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err(ResourceValidationError::Schema);
    }
    reject_forbidden_keywords(root)
}

pub fn normalize_schema_arguments(
    schema: &serde_json::Value,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, SchemaInstanceError> {
    if !arguments.is_object() {
        return Err(SchemaInstanceError);
    }
    let mut normalized = arguments.clone();
    apply_schema_defaults(schema, &mut normalized);
    validate_schema_instance(schema, &normalized)?;
    Ok(normalized)
}

pub fn validate_schema_instance(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), SchemaInstanceError> {
    let validator = jsonschema::draft202012::options()
        .build(schema)
        .map_err(|_| SchemaInstanceError)?;
    validator
        .is_valid(instance)
        .then_some(())
        .ok_or(SchemaInstanceError)
}

fn apply_schema_defaults(schema: &serde_json::Value, instance: &mut serde_json::Value) {
    let Some(schema) = schema.as_object() else {
        return;
    };
    if let Some(all_of) = schema.get("allOf").and_then(serde_json::Value::as_array) {
        for branch in all_of {
            apply_schema_defaults(branch, instance);
        }
    }
    if let (Some(properties), Some(instance)) = (
        schema
            .get("properties")
            .and_then(serde_json::Value::as_object),
        instance.as_object_mut(),
    ) {
        for (name, property_schema) in properties {
            if !instance.contains_key(name)
                && let Some(default) = property_schema.get("default")
            {
                instance.insert(name.clone(), default.clone());
            }
            if let Some(value) = instance.get_mut(name) {
                apply_schema_defaults(property_schema, value);
            }
        }
    }
    if let (Some(items), Some(instance)) = (schema.get("items"), instance.as_array_mut()) {
        for item in instance {
            apply_schema_defaults(items, item);
        }
    }
}

fn reject_forbidden_keywords(value: &serde_json::Value) -> Result<(), ResourceValidationError> {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("number")
                || object.keys().any(|key| {
                    matches!(
                        key.as_str(),
                        "$dynamicRef" | "$recursiveRef" | "formatMinimum" | "formatMaximum"
                    )
                })
            {
                return Err(ResourceValidationError::Schema);
            }
            for child in object.values() {
                reject_forbidden_keywords(child)?;
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                reject_forbidden_keywords(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_schema_bindings(
    action: &ActionDescriptor,
    input_schema: &serde_json::Value,
) -> Result<(), ResourceValidationError> {
    for binding in &action.argv {
        for name in binding
            .input
            .iter()
            .chain(binding.when_input.iter())
            .chain(&binding.inputs)
        {
            if schema_property(input_schema, name).is_none() {
                return Err(ResourceValidationError::Schema);
            }
        }
        if let Some(name) = binding.when_input.as_deref() {
            let property =
                schema_property(input_schema, name).ok_or(ResourceValidationError::Schema)?;
            if binding
                .when_equals
                .as_ref()
                .is_none_or(|value| validate_schema_instance(property, value).is_err())
            {
                return Err(ResourceValidationError::Schema);
            }
        }
        let Some(name) = binding.input.as_deref() else {
            continue;
        };
        let property =
            schema_property(input_schema, name).ok_or(ResourceValidationError::Schema)?;
        let valid_type = match binding.kind.as_str() {
            "flag_if_true" | "flag_if_false" => schema_allows_type(property, "boolean"),
            "repeat" => {
                schema_allows_type(property, "array")
                    && property.get("items").is_some_and(schema_allows_cli_scalar)
            }
            "stdin_text" => schema_allows_type(property, "string"),
            "temp_file" => match binding.content_kind.as_deref().unwrap_or("text") {
                "text" | "base64" => schema_allows_type(property, "string"),
                "json" => true,
                _ => false,
            },
            "positional" | "option" | "joined" => schema_allows_cli_scalar(property),
            _ => true,
        };
        if !valid_type {
            return Err(ResourceValidationError::Schema);
        }
    }
    Ok(())
}

fn schema_property<'a>(schema: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .and_then(|properties| properties.get(name))
        .or_else(|| {
            schema
                .get("allOf")
                .and_then(serde_json::Value::as_array)
                .and_then(|branches| {
                    branches
                        .iter()
                        .find_map(|branch| schema_property(branch, name))
                })
        })
}

fn schema_allows_cli_scalar(schema: &serde_json::Value) -> bool {
    ["string", "integer", "boolean"]
        .into_iter()
        .any(|kind| schema_allows_type(schema, kind))
        || schema.get("enum").is_some_and(|values| {
            values.as_array().is_some_and(|values| {
                !values.is_empty()
                    && values.iter().all(|value| {
                        value.is_string() || value.is_i64() || value.is_u64() || value.is_boolean()
                    })
            })
        })
}

fn schema_allows_type(schema: &serde_json::Value, expected: &str) -> bool {
    schema.get("type").is_some_and(|kind| {
        kind.as_str() == Some(expected)
            || kind
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(expected)))
    }) || schema
        .get("allOf")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|branches| {
            branches
                .iter()
                .any(|branch| schema_allows_type(branch, expected))
        })
}

fn resolve_fragment(
    document: &serde_json::Value,
    fragment: String,
) -> Result<serde_json::Value, ResourceValidationError> {
    if fragment.is_empty() {
        return Ok(document.clone());
    }
    if !fragment.starts_with('/') {
        return Err(ResourceValidationError::Reference);
    }
    document
        .pointer(&fragment)
        .cloned()
        .ok_or(ResourceValidationError::Reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::manifest::{ManifestSource, parse_manifest_v1};

    fn fixture_with_schema(schema: &str) -> (PathBuf, ToolPackageManifest) {
        let directory = std::env::temp_dir().join(format!("star-schema-{}", star_ipc::nonce()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("input.json"), schema).unwrap();
        let source = include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
            .replace(
                "[[actions.parameters]]\nname = \"value\"\ntype = \"string\"\ndescription = \"Value to echo\"\nrequired = true\n",
                "input_schema_file = \"input.json\"\n",
            )
            .replace(
                "[[actions.argv]]\nkind = \"positional\"\ninput = \"value\"\n",
                "[[actions.argv]]\nkind = \"literal\"\nvalue = \"fixed\"\n",
            );
        let manifest = parse_manifest_v1(&source, ManifestSource::User).unwrap();
        let manifest_path = directory.join("package.toml");
        fs::write(&manifest_path, source).unwrap();
        (manifest_path, manifest)
    }

    #[test]
    // matrix: MCP-M005 MCP-M020
    fn schema_references_are_local_acyclic_bounded_and_object_rooted() {
        let (path, manifest) = fixture_with_schema(
            r#"{"type":"object","additionalProperties":false,"properties":{"value":{"type":"string"}}}"#,
        );
        assert!(validate_manifest_resources(&manifest, &path).is_ok());

        let (path, manifest) = fixture_with_schema(
            r#"{"type":"object","additionalProperties":false,"$ref":"https://example.invalid/schema"}"#,
        );
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Reference)
        );

        let (path, manifest) =
            fixture_with_schema(r##"{"type":"object","additionalProperties":false,"$ref":"#"}"##);
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Reference)
        );

        let (path, manifest) = fixture_with_schema(r#"{"type":"number"}"#);
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Schema)
        );

        let (path, mut manifest) =
            fixture_with_schema(r#"{"type":"object","additionalProperties":false}"#);
        fs::write(
            path.parent().unwrap().join("output.json"),
            r#"{"type":"number"}"#,
        )
        .unwrap();
        manifest.actions[0].output_schema_file = Some("output.json".to_owned());
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Schema),
            "output Schema must also have an object root and reject number"
        );
    }

    #[test]
    // matrix: MCP-M006 MCP-M023
    fn package_and_action_schema_byte_limits_fail_closed() {
        let large = format!(
            "{{\"type\":\"object\",\"additionalProperties\":false,\"description\":\"{}\"}}",
            "x".repeat(ACTION_RESOLVED_SCHEMA_BYTES)
        );
        let (path, manifest) = fixture_with_schema(&large);
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::ActionSize)
        );

        let (path, manifest) =
            fixture_with_schema(r#"{"type":"object","additionalProperties":false}"#);
        let giant = path.parent().unwrap().join("giant.json");
        fs::write(&giant, vec![b' '; (PACKAGE_SCHEMA_BYTES + 1) as usize]).unwrap();
        let mut manifest = manifest;
        manifest.actions[0].output_schema_file = Some("giant.json".to_owned());
        assert_eq!(
            validate_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::PackageSize)
        );
    }

    #[test]
    // matrix: MCP-M004 MCP-M005 MCP-H005 MCP-H009
    fn resolved_schema_snapshot_rejects_invalid_schema_and_applies_defaults() {
        let (path, manifest) = fixture_with_schema(
            r##"{
                "$schema":"https://json-schema.org/draft/2020-12/schema",
                "type":"object",
                "additionalProperties":false,
                "properties":{"value":{"$ref":"defs.json#/$defs/value"}}
            }"##,
        );
        fs::write(
            path.parent().unwrap().join("defs.json"),
            r#"{"$defs":{"value":{"type":"string","default":"resolved"}}}"#,
        )
        .unwrap();
        let resources = load_manifest_resources(&manifest, &path).unwrap();
        assert_eq!(resources.schema_hashes.len(), 2);
        let input = resources.action_schemas["user.fake.echo.run"]
            .input
            .as_ref()
            .unwrap();
        assert_eq!(
            normalize_schema_arguments(input, &serde_json::json!({})).unwrap(),
            serde_json::json!({"value":"resolved"})
        );
        assert!(normalize_schema_arguments(input, &serde_json::json!({"value":7})).is_err());

        let (path, manifest) = fixture_with_schema(
            r#"{"type":"object","additionalProperties":false,"properties":{"x":{"type":"not-a-json-schema-type"}}}"#,
        );
        assert_eq!(
            load_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Schema)
        );

        let (path, manifest) = fixture_with_schema(
            r#"{"type":"object","type":"object","additionalProperties":false}"#,
        );
        assert_eq!(
            load_manifest_resources(&manifest, &path),
            Err(ResourceValidationError::Json)
        );
    }
}
