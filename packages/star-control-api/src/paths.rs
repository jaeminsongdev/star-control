use crate::ApiError;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) struct ParsedPath {
    path: String,
    query: BTreeMap<String, String>,
}

impl ParsedPath {
    pub(crate) fn parse(raw_path: &str) -> Self {
        let (path, query) = raw_path.split_once('?').unwrap_or((raw_path, ""));
        let mut query_map = BTreeMap::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            query_map.insert(key.to_string(), value.to_string());
        }
        Self {
            path: path.to_string(),
            query: query_map,
        }
    }

    pub(crate) fn segments(&self) -> Vec<&str> {
        self.path
            .trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect()
    }

    pub(crate) fn query_value(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(String::as_str)
    }
}

pub(crate) fn validate_project_id(project_id: &str) -> Result<(), ApiError> {
    let valid = !project_id.is_empty()
        && project_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(ApiError::InvalidProjectId {
            project_id: project_id.to_string(),
        })
    }
}
