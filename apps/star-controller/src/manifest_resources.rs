//! Filesystem validation for manifest-owned JSON Schema resources.
//!
//! `star-contracts` owns the wire types and static rules.  This module keeps
//! path resolution and bounded file IO in the Controller, where the manifest
//! source directory and final filesystem identity are available.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use star_contracts::manifest::{ActionDescriptor, ToolPackageManifest};
use thiserror::Error;

const PACKAGE_SCHEMA_BYTES: u64 = 4 * 1024 * 1024;
const ACTION_RESOLVED_SCHEMA_BYTES: usize = 1024 * 1024;
const MAX_REF_DEPTH: usize = 64;

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

/// Validates every schema referenced by a parsed manifest.  All paths are
/// resolved relative to the final manifest parent and must remain under it
/// after canonicalization.  Files are cached by final path so the package byte
/// limit counts each source once.
pub fn validate_manifest_resources(
    manifest: &ToolPackageManifest,
    manifest_path: &Path,
) -> Result<(), ResourceValidationError> {
    let parent = manifest_path
        .parent()
        .ok_or(ResourceValidationError::Path)?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| ResourceValidationError::Io)?;
    let mut resolver = SchemaResolver {
        package_root: canonical_parent,
        documents: BTreeMap::new(),
        package_bytes: 0,
    };
    for action in &manifest.actions {
        resolver.validate_action(action)?;
    }
    Ok(())
}

struct SchemaResolver {
    package_root: PathBuf,
    documents: BTreeMap<PathBuf, serde_json::Value>,
    package_bytes: u64,
}

impl SchemaResolver {
    fn validate_action(
        &mut self,
        action: &ActionDescriptor,
    ) -> Result<(), ResourceValidationError> {
        let mut resolved_bytes = 0usize;
        for (path, input) in [
            (action.input_schema_file.as_deref(), true),
            (action.output_schema_file.as_deref(), false),
        ] {
            let Some(path) = path else { continue };
            let root = self.load_relative(Path::new(path))?;
            validate_schema_root(&root, input)?;
            let mut stack = Vec::new();
            let mut visited_nodes = BTreeSet::new();
            resolved_bytes = resolved_bytes
                .checked_add(self.resolve_size(
                    &root,
                    self.resolve_path(Path::new(path))?,
                    0,
                    &mut stack,
                    &mut visited_nodes,
                )?)
                .ok_or(ResourceValidationError::ActionSize)?;
            if resolved_bytes > ACTION_RESOLVED_SCHEMA_BYTES {
                return Err(ResourceValidationError::ActionSize);
            }
        }
        Ok(())
    }

    fn resolve_size(
        &mut self,
        value: &serde_json::Value,
        document_path: PathBuf,
        depth: usize,
        stack: &mut Vec<String>,
        visited_nodes: &mut BTreeSet<String>,
    ) -> Result<usize, ResourceValidationError> {
        if depth > MAX_REF_DEPTH {
            return Err(ResourceValidationError::Reference);
        }
        reject_forbidden_keywords(value)?;
        let mut size = serde_json::to_vec(value)
            .map_err(|_| ResourceValidationError::Json)?
            .len();
        match value {
            serde_json::Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                    let (target_path, fragment) =
                        self.resolve_reference(&document_path, reference)?;
                    let identity = format!("{}#{fragment}", target_path.display());
                    if stack.contains(&identity) {
                        return Err(ResourceValidationError::Reference);
                    }
                    if visited_nodes.insert(identity.clone()) {
                        stack.push(identity);
                        let document = self.load_final(&target_path)?;
                        let target = resolve_fragment(&document, fragment)?;
                        size = size
                            .checked_add(self.resolve_size(
                                &target,
                                target_path,
                                depth + 1,
                                stack,
                                visited_nodes,
                            )?)
                            .ok_or(ResourceValidationError::ActionSize)?;
                        stack.pop();
                    }
                }
                for (key, child) in object {
                    if key != "$ref" {
                        size = size
                            .checked_add(self.resolve_size(
                                child,
                                document_path.clone(),
                                depth,
                                stack,
                                visited_nodes,
                            )?)
                            .ok_or(ResourceValidationError::ActionSize)?;
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    size = size
                        .checked_add(self.resolve_size(
                            child,
                            document_path.clone(),
                            depth,
                            stack,
                            visited_nodes,
                        )?)
                        .ok_or(ResourceValidationError::ActionSize)?;
                }
            }
            _ => {}
        }
        Ok(size)
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
        let final_path = parent
            .join(relative)
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
        let metadata = fs::metadata(final_path).map_err(|_| ResourceValidationError::Io)?;
        self.package_bytes = self
            .package_bytes
            .checked_add(metadata.len())
            .ok_or(ResourceValidationError::PackageSize)?;
        if self.package_bytes > PACKAGE_SCHEMA_BYTES {
            return Err(ResourceValidationError::PackageSize);
        }
        let bytes = fs::read(final_path).map_err(|_| ResourceValidationError::Io)?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ResourceValidationError::Json)?;
        self.documents
            .insert(final_path.to_path_buf(), value.clone());
        Ok(value)
    }
}

fn validate_schema_root(
    root: &serde_json::Value,
    input: bool,
) -> Result<(), ResourceValidationError> {
    let object = root.as_object().ok_or(ResourceValidationError::Schema)?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("object") {
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
}
