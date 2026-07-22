use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

use star_contracts::{
    Sha256Hash,
    management::ProjectPathRef,
    rust_style::{
        ClippySuggestion, RUST_STYLE_PIPELINE_ID, RUST_STYLE_PIPELINE_VERSION,
        RustAvailabilityState, RustByteEdit, RustCatalogLifecycle, RustCompleteness,
        RustCoverageExecution, RustSourceOwnership, RustStyleCoverageMatrix,
        RustStylePolicySnapshot, RustToolchainBinding, RustToolchainPinState,
        SuggestionApplicability,
    },
};
use star_domain::versioned_fingerprint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustFileSnapshot {
    pub path: ProjectPathRef,
    pub bytes: Vec<u8>,
    pub ownership: RustSourceOwnership,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustFileChange {
    pub path: ProjectPathRef,
    pub before_sha256: Sha256Hash,
    pub after_sha256: Sha256Hash,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideEffectSummary {
    pub changes: Vec<RustFileChange>,
    pub changed_bytes: u64,
    pub hunk_count: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RustStyleValidationError {
    #[error("RUST_TOOLCHAIN_UNRESOLVED")]
    ToolchainUnresolved,
    #[error("RUST_COMPONENT_UNAVAILABLE")]
    ComponentUnavailable,
    #[error("RUST_STYLE_CONFIG_AMBIGUOUS")]
    ConfigAmbiguous,
    #[error("RUST_STYLE_COVERAGE_INCOMPLETE")]
    CoverageIncomplete,
    #[error("RUST_CLIPPY_FIX_NOT_ALLOWED")]
    ClippyFixNotAllowed,
    #[error("RUST_CLIPPY_SUGGESTION_NOT_MACHINE_APPLICABLE")]
    SuggestionNotMachineApplicable,
    #[error("RUST_STYLE_SIDE_EFFECT_VIOLATION")]
    SideEffectViolation,
    #[error("RUST_STYLE_NON_IDEMPOTENT")]
    NonIdempotent,
    #[error("RUST_STYLE_AUTO_SCOPE_MISMATCH")]
    AutoScopeMismatch,
    #[error("RUST_STYLE_DIAGNOSTIC_UNPARSED")]
    DiagnosticUnparsed,
    #[error("RUST_STYLE_FINGERPRINT_FAILED")]
    Fingerprint,
}

pub fn validate_binding_policy_and_coverage(
    binding: &RustToolchainBinding,
    policy: &RustStylePolicySnapshot,
    coverage: &RustStyleCoverageMatrix,
) -> Result<(), RustStyleValidationError> {
    if binding.contract_version != 1
        || binding.completeness != RustCompleteness::Complete
        || binding.toolchain_pin_state != RustToolchainPinState::PinnedStable
        || binding.release.as_deref() != Some("1.96.0")
        || binding.channel != "1.96.0"
        || !binding.limitations.is_empty()
    {
        return Err(RustStyleValidationError::ToolchainUnresolved);
    }
    if [
        &binding.cargo,
        &binding.rustc,
        &binding.rustfmt,
        &binding.clippy_driver,
    ]
    .iter()
    .any(|tool| tool.component_state != RustAvailabilityState::Available)
        || binding
            .target_states
            .iter()
            .any(|target| target.state != RustAvailabilityState::Available)
    {
        return Err(RustStyleValidationError::ComponentUnavailable);
    }
    if policy.contract_version != 1
        || policy.pipeline_ref != format!("{RUST_STYLE_PIPELINE_ID}@{RUST_STYLE_PIPELINE_VERSION}")
        || policy.policy_completeness != RustCompleteness::Complete
        || !policy.limitations.is_empty()
        || policy.scope_paths.is_empty()
    {
        return Err(RustStyleValidationError::ConfigAmbiguous);
    }
    for entry in &policy.clippy_fix_allowlist {
        if !exact_lint_id(&entry.lint_id)
            || entry.required_applicability != SuggestionApplicability::MachineApplicable
            || entry.public_api_policy != "deny"
            || entry.lifecycle != RustCatalogLifecycle::Active
            || entry.clippy_release != "1.96.0"
            || entry.clippy_executable_sha256 != binding.clippy_driver.sha256
            || entry.corpus_ref.trim().is_empty()
            || entry.allowed_scope.is_empty()
        {
            return Err(RustStyleValidationError::ClippyFixNotAllowed);
        }
    }
    if coverage.contract_version != 1
        || coverage.completeness != RustCompleteness::Complete
        || !coverage.cfg_frontier.is_empty()
        || !coverage.conflicts.is_empty()
        || !coverage.limitations.is_empty()
    {
        return Err(RustStyleValidationError::CoverageIncomplete);
    }
    let cells = coverage
        .cells
        .iter()
        .map(|cell| (cell.cell_id.as_str(), cell))
        .collect::<BTreeMap<_, _>>();
    let required = coverage
        .required_cell_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if required.len() != coverage.required_cell_ids.len()
        || required.iter().any(|cell_id| {
            cells.get(cell_id).is_none_or(|cell| {
                cell.execution != RustCoverageExecution::Executed
                    || !cell.required_features_satisfied
            })
        })
    {
        return Err(RustStyleValidationError::CoverageIncomplete);
    }
    Ok(())
}

pub fn parse_clippy_json_lines(
    raw: &str,
    coverage_cell_id: &str,
    files: &[RustFileSnapshot],
) -> Result<Vec<ClippySuggestion>, RustStyleValidationError> {
    let by_path = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let mut suggestions = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-message") {
            continue;
        }
        let message = value
            .get("message")
            .and_then(serde_json::Value::as_object)
            .ok_or(RustStyleValidationError::DiagnosticUnparsed)?;
        let lint_id = message
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !lint_id.starts_with("clippy::") {
            continue;
        }
        let mut span_groups = Vec::new();
        if let Some(spans) = message.get("spans").and_then(serde_json::Value::as_array)
            && spans.iter().any(|span| {
                span.get("suggested_replacement")
                    .is_some_and(|value| !value.is_null())
            })
        {
            span_groups.push(spans.clone());
        }
        for child in message
            .get("children")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(spans) = child.get("spans").and_then(serde_json::Value::as_array)
                && spans.iter().any(|span| {
                    span.get("suggested_replacement")
                        .is_some_and(|value| !value.is_null())
                })
            {
                span_groups.push(spans.clone());
            }
        }
        for spans in span_groups {
            suggestions.push(parse_span_group(
                lint_id,
                coverage_cell_id,
                &spans,
                &by_path,
            )?);
        }
    }
    suggestions.sort_by(|left, right| {
        (&left.path, &left.lint_id, &left.suggestion_fingerprint).cmp(&(
            &right.path,
            &right.lint_id,
            &right.suggestion_fingerprint,
        ))
    });
    suggestions.dedup_by(|left, right| left.suggestion_fingerprint == right.suggestion_fingerprint);
    Ok(suggestions)
}

pub fn select_allowlisted_suggestions(
    suggestions: &[ClippySuggestion],
    policy: &RustStylePolicySnapshot,
    files: &[RustFileSnapshot],
) -> Result<Vec<ClippySuggestion>, RustStyleValidationError> {
    let hashes = files
        .iter()
        .map(|file| (file.path.as_str(), Sha256Hash::digest(&file.bytes)))
        .collect::<BTreeMap<_, _>>();
    let ownership = files
        .iter()
        .map(|file| (file.path.as_str(), file.ownership))
        .collect::<BTreeMap<_, _>>();
    let allowlist = policy
        .clippy_fix_allowlist
        .iter()
        .map(|entry| (entry.lint_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::new();
    for suggestion in suggestions {
        let Some(entry) = allowlist.get(suggestion.lint_id.as_str()) else {
            continue;
        };
        if suggestion.applicability != SuggestionApplicability::MachineApplicable {
            continue;
        }
        if suggestion.expansion_origin.is_some()
            || ownership.get(suggestion.path.as_str()) != Some(&RustSourceOwnership::Handwritten)
            || hashes.get(suggestion.path.as_str()) != Some(&suggestion.before_file_sha256)
            || !path_in_any_scope(&suggestion.path, &policy.scope_paths)
            || !path_in_any_scope(&suggestion.path, &entry.allowed_scope)
            || edits_overlap(&suggestion.edits)
        {
            continue;
        }
        selected.push(suggestion.clone());
    }
    selected.sort_by(|left, right| {
        (&left.path, &left.suggestion_fingerprint)
            .cmp(&(&right.path, &right.suggestion_fingerprint))
    });
    let mut ranges = BTreeMap::<&str, Vec<(u64, u64)>>::new();
    for suggestion in &selected {
        for edit in &suggestion.edits {
            ranges
                .entry(suggestion.path.as_str())
                .or_default()
                .push((edit.start_byte, edit.end_byte));
        }
    }
    if ranges.values_mut().any(|ranges| {
        ranges.sort();
        ranges.windows(2).any(|pair| pair[0].1 > pair[1].0)
    }) {
        return Err(RustStyleValidationError::SideEffectViolation);
    }
    Ok(selected)
}

pub fn validate_side_effects(
    before: &[RustFileSnapshot],
    after: &[RustFileSnapshot],
    policy: &RustStylePolicySnapshot,
) -> Result<SideEffectSummary, RustStyleValidationError> {
    let before_map = before
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    if before_map.keys().copied().collect::<Vec<_>>()
        != after_map.keys().copied().collect::<Vec<_>>()
    {
        return Err(RustStyleValidationError::SideEffectViolation);
    }
    let mut changes = Vec::new();
    let mut changed_bytes = 0_u64;
    for (path, before_file) in before_map {
        let after_file = after_map
            .get(path)
            .ok_or(RustStyleValidationError::SideEffectViolation)?;
        if before_file.bytes == after_file.bytes {
            continue;
        }
        if before_file.ownership != RustSourceOwnership::Handwritten
            || after_file.ownership != RustSourceOwnership::Handwritten
            || !path.ends_with(".rs")
            || !path_in_any_scope(&before_file.path, &policy.scope_paths)
        {
            return Err(RustStyleValidationError::SideEffectViolation);
        }
        changed_bytes = changed_bytes
            .saturating_add(before_file.bytes.len() as u64)
            .saturating_add(after_file.bytes.len() as u64);
        changes.push(RustFileChange {
            path: before_file.path.clone(),
            before_sha256: Sha256Hash::digest(&before_file.bytes),
            after_sha256: Sha256Hash::digest(&after_file.bytes),
            before_bytes: before_file.bytes.clone(),
            after_bytes: after_file.bytes.clone(),
        });
    }
    if changes.len() > policy.max_files as usize
        || changed_bytes > policy.max_changed_bytes
        || changes.len() > policy.max_hunks as usize
    {
        return Err(RustStyleValidationError::AutoScopeMismatch);
    }
    Ok(SideEffectSummary {
        hunk_count: changes.len() as u32,
        changes,
        changed_bytes,
    })
}

pub fn validate_clippy_fix_result(
    before: &[RustFileSnapshot],
    after: &[RustFileSnapshot],
    selected: &[ClippySuggestion],
) -> Result<(), RustStyleValidationError> {
    let mut expected = before.to_vec();
    let mut suggestions_by_path = BTreeMap::<&str, Vec<&ClippySuggestion>>::new();
    for suggestion in selected {
        suggestions_by_path
            .entry(suggestion.path.as_str())
            .or_default()
            .push(suggestion);
    }
    for file in &mut expected {
        let mut edits = suggestions_by_path
            .get(file.path.as_str())
            .into_iter()
            .flatten()
            .flat_map(|suggestion| suggestion.edits.iter())
            .collect::<Vec<_>>();
        edits.sort_by_key(|edit| Reverse(edit.start_byte));
        for edit in edits {
            let start = usize::try_from(edit.start_byte)
                .map_err(|_| RustStyleValidationError::SideEffectViolation)?;
            let end = usize::try_from(edit.end_byte)
                .map_err(|_| RustStyleValidationError::SideEffectViolation)?;
            if start > end || end > file.bytes.len() {
                return Err(RustStyleValidationError::SideEffectViolation);
            }
            file.bytes
                .splice(start..end, edit.replacement.as_bytes().iter().copied());
        }
    }
    if expected != after {
        return Err(RustStyleValidationError::SideEffectViolation);
    }
    Ok(())
}

pub fn snapshot_fingerprint(
    files: &[RustFileSnapshot],
) -> Result<Sha256Hash, RustStyleValidationError> {
    let entries = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path":file.path,
                "sha256":Sha256Hash::digest(&file.bytes),
                "ownership":file.ownership,
            })
        })
        .collect::<Vec<_>>();
    versioned_fingerprint("star.rust-style-filesystem", 1, &entries)
        .map_err(|_| RustStyleValidationError::Fingerprint)
}

fn parse_span_group(
    lint_id: &str,
    coverage_cell_id: &str,
    spans: &[serde_json::Value],
    files: &BTreeMap<&str, &RustFileSnapshot>,
) -> Result<ClippySuggestion, RustStyleValidationError> {
    let relevant = spans
        .iter()
        .filter(|span| {
            span.get("suggested_replacement")
                .is_some_and(|value| !value.is_null())
        })
        .collect::<Vec<_>>();
    let path_text = relevant
        .first()
        .and_then(|span| span.get("file_name"))
        .and_then(serde_json::Value::as_str)
        .ok_or(RustStyleValidationError::DiagnosticUnparsed)?
        .replace('\\', "/");
    if relevant.iter().any(|span| {
        span.get("file_name")
            .and_then(serde_json::Value::as_str)
            .map(|value| value.replace('\\', "/"))
            .as_deref()
            != Some(path_text.as_str())
    }) {
        return Err(RustStyleValidationError::SuggestionNotMachineApplicable);
    }
    let path = ProjectPathRef::parse(path_text)
        .map_err(|_| RustStyleValidationError::DiagnosticUnparsed)?;
    let file = files
        .get(path.as_str())
        .ok_or(RustStyleValidationError::DiagnosticUnparsed)?;
    let mut applicability = None;
    let mut edits = Vec::new();
    let mut expansion_origin = None;
    for span in relevant {
        let current_applicability = parse_applicability(
            span.get("suggestion_applicability")
                .and_then(serde_json::Value::as_str),
        );
        if applicability
            .replace(current_applicability)
            .is_some_and(|prior| prior != current_applicability)
        {
            return Err(RustStyleValidationError::SuggestionNotMachineApplicable);
        }
        if span.get("expansion").is_some_and(|value| !value.is_null()) {
            expansion_origin = Some("macro_expansion".to_owned());
        }
        edits.push(RustByteEdit {
            start_byte: span
                .get("byte_start")
                .and_then(serde_json::Value::as_u64)
                .ok_or(RustStyleValidationError::DiagnosticUnparsed)?,
            end_byte: span
                .get("byte_end")
                .and_then(serde_json::Value::as_u64)
                .ok_or(RustStyleValidationError::DiagnosticUnparsed)?,
            replacement: span
                .get("suggested_replacement")
                .and_then(serde_json::Value::as_str)
                .ok_or(RustStyleValidationError::DiagnosticUnparsed)?
                .to_owned(),
        });
    }
    edits.sort_by_key(|edit| (edit.start_byte, edit.end_byte));
    let applicability = applicability.unwrap_or(SuggestionApplicability::Unknown);
    let before_file_sha256 = Sha256Hash::digest(&file.bytes);
    let suggestion_fingerprint = versioned_fingerprint(
        "star.rust-clippy-suggestion",
        1,
        &serde_json::json!({
            "lint_id":lint_id,
            "coverage_cell_id":coverage_cell_id,
            "path":path,
            "before_file_sha256":before_file_sha256,
            "applicability":applicability,
            "edits":edits,
            "expansion_origin":expansion_origin,
        }),
    )
    .map_err(|_| RustStyleValidationError::Fingerprint)?;
    Ok(ClippySuggestion {
        lint_id: lint_id.to_owned(),
        coverage_cell_id: coverage_cell_id.to_owned(),
        path,
        before_file_sha256,
        applicability,
        edits,
        expansion_origin,
        suggestion_fingerprint,
    })
}

fn parse_applicability(value: Option<&str>) -> SuggestionApplicability {
    match value {
        Some("MachineApplicable") => SuggestionApplicability::MachineApplicable,
        Some("MaybeIncorrect") => SuggestionApplicability::MaybeIncorrect,
        Some("HasPlaceholders") => SuggestionApplicability::HasPlaceholders,
        Some("Unspecified") => SuggestionApplicability::Unspecified,
        _ => SuggestionApplicability::Unknown,
    }
}

fn edits_overlap(edits: &[RustByteEdit]) -> bool {
    edits
        .windows(2)
        .any(|pair| pair[0].end_byte > pair[1].start_byte)
}

fn exact_lint_id(value: &str) -> bool {
    let Some(name) = value.strip_prefix("clippy::") else {
        return false;
    };
    !name.is_empty()
        && ![
            "all",
            "correctness",
            "style",
            "pedantic",
            "restriction",
            "nursery",
            "cargo",
        ]
        .contains(&name)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn path_in_any_scope(path: &ProjectPathRef, scopes: &[ProjectPathRef]) -> bool {
    scopes.iter().any(|scope| {
        path == scope
            || path
                .as_str()
                .strip_prefix(scope.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}
