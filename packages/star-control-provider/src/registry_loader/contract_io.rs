use super::ProviderRegistryLoader;
use crate::registry_error::ProviderRegistryError;
use crate::registry_yaml::parse_star_control_yaml_subset;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::fs;
use std::path::Path;

impl ProviderRegistryLoader {
    pub(super) fn load_contract_value(&self, path: &Path) -> Result<Value, ProviderRegistryError> {
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

    pub(super) fn validate_contract(
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
}
