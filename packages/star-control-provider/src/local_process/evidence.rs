use super::constants::{
    FORBIDDEN_ACTION_EVIDENCE_PREFIX, LOCAL_PROCESS_FORBIDDEN_ACTIONS, STDERR_FILE, STDOUT_FILE,
};
use crate::{ExecutionRequest, ProviderAdapterError};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct ForbiddenActionEvidence {
    pub(crate) action: String,
    pub(crate) source: &'static str,
}

pub(crate) fn forbidden_action_evidence(
    request: &ExecutionRequest,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<Option<ForbiddenActionEvidence>, ProviderAdapterError> {
    let forbidden_actions = forbidden_action_names(request);
    if forbidden_actions.is_empty() {
        return Ok(None);
    }

    if let Some(evidence) =
        forbidden_action_evidence_from_file(stdout_path, STDOUT_FILE, &forbidden_actions)?
    {
        return Ok(Some(evidence));
    }

    forbidden_action_evidence_from_file(stderr_path, STDERR_FILE, &forbidden_actions)
}

fn forbidden_action_names(request: &ExecutionRequest) -> BTreeSet<String> {
    let mut names = LOCAL_PROCESS_FORBIDDEN_ACTIONS
        .iter()
        .map(|action| action.to_string())
        .collect::<BTreeSet<_>>();
    if let Some(actions) = request
        .value()
        .get("forbidden_actions")
        .and_then(Value::as_array)
    {
        for action in actions.iter().filter_map(Value::as_str) {
            names.insert(action.to_ascii_lowercase());
        }
    }
    names
}

fn forbidden_action_evidence_from_file(
    path: &Path,
    source: &'static str,
    forbidden_actions: &BTreeSet<String>,
) -> Result<Option<ForbiddenActionEvidence>, ProviderAdapterError> {
    let content = fs::read_to_string(path).map_err(|io_source| ProviderAdapterError::Io {
        path: path.to_path_buf(),
        source: io_source,
    })?;
    Ok(forbidden_action_evidence_from_text(
        &content,
        source,
        forbidden_actions,
    ))
}

fn forbidden_action_evidence_from_text(
    content: &str,
    source: &'static str,
    forbidden_actions: &BTreeSet<String>,
) -> Option<ForbiddenActionEvidence> {
    content.lines().find_map(|line| {
        let marker = line.trim().strip_prefix(FORBIDDEN_ACTION_EVIDENCE_PREFIX)?;
        let action = marker.split_whitespace().next()?.to_ascii_lowercase();
        if forbidden_actions.contains(&action) {
            Some(ForbiddenActionEvidence { action, source })
        } else {
            None
        }
    })
}
