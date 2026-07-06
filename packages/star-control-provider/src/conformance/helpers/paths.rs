use super::super::error::ProviderConformanceError;

pub(crate) fn check_path_equals(
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), ProviderConformanceError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProviderConformanceError::FieldMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

pub(crate) fn check_provider_relative_path(
    field: &str,
    path: &str,
    provider_instance_id: &str,
) -> Result<(), ProviderConformanceError> {
    if path.is_empty() {
        return invalid_path(field, path, "path is empty");
    }
    if path.contains('\\') {
        return invalid_path(
            field,
            path,
            "backslash is not a canonical artifact separator",
        );
    }
    if path.starts_with('/') {
        return invalid_path(field, path, "absolute paths are not allowed");
    }
    if path.split('/').any(|segment| {
        segment.is_empty() || segment == "." || segment == ".." || segment.contains(':')
    }) {
        return invalid_path(field, path, "path must use normalized relative segments");
    }
    let expected_prefix = format!("provider-output/{}/", provider_instance_id);
    if !path.starts_with(&expected_prefix) {
        return invalid_path(
            field,
            path,
            "path must stay inside provider output directory",
        );
    }
    Ok(())
}

pub(crate) fn check_safe_segment(field: &str, value: &str) -> Result<(), ProviderConformanceError> {
    if value.is_empty() {
        return invalid_path(field, value, "segment is empty");
    }
    if value.contains('\0')
        || value.contains(':')
        || value.contains('/')
        || value.contains('\\')
        || value == "."
        || value == ".."
        || value == ".git"
    {
        return invalid_path(field, value, "segment is not safe for artifact paths");
    }
    Ok(())
}

fn invalid_path<T>(field: &str, path: &str, reason: &str) -> Result<T, ProviderConformanceError> {
    Err(ProviderConformanceError::InvalidArtifactPath {
        field: field.to_string(),
        path: path.to_string(),
        reason: reason.to_string(),
    })
}

pub(crate) fn provider_path(provider_instance_id: &str, file_name: &str) -> String {
    format!("provider-output/{}/{}", provider_instance_id, file_name)
}
