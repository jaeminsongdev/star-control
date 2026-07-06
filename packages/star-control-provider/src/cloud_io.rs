use crate::cloud_policy::cloud_policy_denied;
use crate::{ProviderAdapterError, ProviderInstance};
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};

pub(crate) fn create_new_output_file(path: &Path) -> Result<File, ProviderAdapterError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ProviderAdapterError::Io {
            path: path.to_path_buf(),
            source,
        })
}

pub(crate) fn offline_response_fixture_path(
    instance: &ProviderInstance,
) -> Result<Option<String>, ProviderAdapterError> {
    let Some(item) = instance
        .value()
        .pointer("/transport_config/offline_response_fixture")
    else {
        return Ok(None);
    };
    let Some(path) = item.as_str() else {
        return Err(cloud_policy_denied(
            instance.id(),
            "transport_config.offline_response_fixture must be a string",
        ));
    };
    if path.trim().is_empty() {
        return Err(cloud_policy_denied(
            instance.id(),
            "transport_config.offline_response_fixture must not be empty",
        ));
    }
    Ok(Some(path.to_string()))
}

pub(crate) fn live_api_call_requested(
    instance: &ProviderInstance,
) -> Result<bool, ProviderAdapterError> {
    let Some(item) = instance
        .value()
        .pointer("/transport_config/live_api_call_requested")
    else {
        return Ok(false);
    };
    let Some(value) = item.as_bool() else {
        return Err(cloud_policy_denied(
            instance.id(),
            "transport_config.live_api_call_requested must be a boolean",
        ));
    };
    Ok(value)
}

pub(crate) fn resolve_project_relative_path(
    project_root: &Path,
    relative_path: &str,
    provider_instance_id: &str,
) -> Result<PathBuf, ProviderAdapterError> {
    if relative_path.is_empty()
        || relative_path.contains('\0')
        || relative_path.contains(':')
        || Path::new(relative_path).is_absolute()
    {
        return Err(cloud_policy_denied(
            provider_instance_id,
            "offline response fixture path must be a project-relative path",
        ));
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(segment) if segment == ".git" => {
                return Err(cloud_policy_denied(
                    provider_instance_id,
                    "offline response fixture path must not reference .git",
                ));
            }
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(cloud_policy_denied(
                    provider_instance_id,
                    "offline response fixture path must not traverse outside the project",
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(cloud_policy_denied(
            provider_instance_id,
            "offline response fixture path must not be empty",
        ));
    }
    let resolved = project_root.join(normalized);
    if !resolved.starts_with(project_root) {
        return Err(cloud_policy_denied(
            provider_instance_id,
            "offline response fixture path must stay inside the project",
        ));
    }
    Ok(resolved)
}

pub(crate) fn read_json_file(path: &Path) -> Result<Value, ProviderAdapterError> {
    let content = fs::read_to_string(path).map_err(|source| ProviderAdapterError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ProviderAdapterError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), ProviderAdapterError> {
    let schema_path = schema_root.join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ProviderAdapterError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ProviderAdapterError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}
