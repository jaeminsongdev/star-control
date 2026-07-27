//! SARIF 2.1.0 output normalization.
//!
//! This module deliberately accepts only the bounded subset needed to project
//! a registered analyzer result into the common validation diagnostic path.
//! It never retains provider message text: external messages may contain
//! source literals or secrets, so persisted diagnostics use fixed text.

use std::{collections::BTreeSet, path::Path};

use serde::{Deserialize, Serialize};

use star_contracts::{
    Sha256Hash,
    evidence::{
        DiagnosticConfidence, DiagnosticSeverity, DiagnosticStatus, LocationRef, ProjectPathKind,
        ProjectPathRef, TextPosition,
    },
    ids::ProjectId,
};

use crate::runner::RawDiagnostic;

const SARIF_VERSION: &str = "2.1.0";
const MAX_SARIF_BYTES: usize = 8 * 1024 * 1024;
const MAX_RUNS: usize = 32;
const MAX_RESULTS: usize = 10_000;
const MAX_LOCATIONS_PER_RESULT: usize = 32;
const MAX_RULE_ID_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SarifCompleteness {
    Complete,
    Partial,
    Unverified,
}

/// Provider-message-free information which is safe to bind to the current
/// source generation. It is deliberately distinct from `RawDiagnostic` so
/// the common diagnostic contract never becomes an external-provider cache.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SarifFindingCandidate {
    pub rule_id: String,
    pub correlation_key: String,
    pub severity: DiagnosticSeverity,
    pub locations: Vec<LocationRef>,
}

#[derive(Clone, Debug)]
pub struct SarifNormalization {
    pub diagnostics: Vec<RawDiagnostic>,
    pub candidates: Vec<SarifFindingCandidate>,
    pub completeness: SarifCompleteness,
    pub imported_count: usize,
    pub rejected_count: usize,
}

/// Normalizes a bounded SARIF 2.1.0 document. Parser and path failures are
/// represented as a single blocking diagnostic instead of being silently
/// discarded. A missing result location is allowed but marks the output
/// partial; a path escaping the project is rejected.
pub fn normalize_sarif_2_1(
    bytes: &[u8],
    project_id: &ProjectId,
    project_root: &Path,
) -> SarifNormalization {
    if bytes.len() > MAX_SARIF_BYTES {
        return rejected(
            "SARIF_RESOURCE_LIMIT",
            "The SARIF document exceeds the byte limit.",
        );
    }
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => value,
        Err(_) => return rejected("SARIF_MALFORMED", "The SARIF document is not valid JSON."),
    };
    let Some(object) = value.as_object() else {
        return rejected("SARIF_MALFORMED", "The SARIF document must be an object.");
    };
    if object.get("version").and_then(serde_json::Value::as_str) != Some(SARIF_VERSION) {
        return rejected(
            "SARIF_VERSION_UNSUPPORTED",
            "Only SARIF 2.1.0 is accepted by this registered output normalizer.",
        );
    }
    let Some(runs) = object.get("runs").and_then(serde_json::Value::as_array) else {
        return rejected("SARIF_MALFORMED", "The SARIF document has no runs array.");
    };
    if runs.len() > MAX_RUNS {
        return rejected(
            "SARIF_RESOURCE_LIMIT",
            "The SARIF document exceeds the run limit.",
        );
    }

    let mut diagnostics = Vec::new();
    let mut candidates = Vec::new();
    let mut imported_count = 0usize;
    let mut rejected_count = 0usize;
    let mut completeness = SarifCompleteness::Complete;
    let mut seen_correlation_keys = BTreeSet::new();
    for run in runs {
        let Some(run_object) = run.as_object() else {
            return rejected("SARIF_MALFORMED", "A SARIF run must be an object.");
        };
        if run_object
            .get("tool")
            .and_then(serde_json::Value::as_object)
            .and_then(|tool| tool.get("driver"))
            .and_then(serde_json::Value::as_object)
            .and_then(|driver| driver.get("name"))
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            return rejected(
                "SARIF_TOOL_IDENTITY_MISSING",
                "The SARIF run has no tool identity.",
            );
        }
        let Some(results) = run_object
            .get("results")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        if results.len() > MAX_RESULTS {
            return rejected(
                "SARIF_RESOURCE_LIMIT",
                "A SARIF run exceeds the result limit.",
            );
        }
        for result in results {
            let Some(result) = result.as_object() else {
                return rejected("SARIF_MALFORMED", "A SARIF result must be an object.");
            };
            let Some(rule_id) = result.get("ruleId").and_then(serde_json::Value::as_str) else {
                return rejected("SARIF_RULE_ID_MISSING", "A SARIF result has no rule ID.");
            };
            if !valid_rule_id(rule_id) {
                return rejected("SARIF_RULE_ID_INVALID", "A SARIF rule ID is invalid.");
            }
            if result
                .get("message")
                .and_then(serde_json::Value::as_object)
                .and_then(|message| message.get("text"))
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            {
                return rejected(
                    "SARIF_MESSAGE_MISSING",
                    "A SARIF result has no message text.",
                );
            }
            let locations = match normalize_locations(result, project_id, project_root) {
                Ok(locations) => locations,
                Err(code) => {
                    return rejected(code, "A SARIF location is outside the bound project.");
                }
            };
            if locations.is_empty() {
                completeness = SarifCompleteness::Partial;
            }
            let correlation_key = result
                .get("partialFingerprints")
                .and_then(serde_json::Value::as_object)
                .and_then(|fingerprints| fingerprints.get("primaryLocationLineHash"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty() && value.len() <= 512)
                .map(|value| {
                    format!(
                        "{rule_id}:partial:{}",
                        Sha256Hash::digest(value.as_bytes()).as_str()
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "{rule_id}:location:{}",
                        serde_json::to_string(&locations).unwrap_or_default()
                    )
                });
            if !seen_correlation_keys.insert(correlation_key.clone()) {
                continue;
            }
            let severity = match result.get("level").and_then(serde_json::Value::as_str) {
                None | Some("none") | Some("note") => DiagnosticSeverity::Info,
                Some("warning") => DiagnosticSeverity::Warning,
                Some("error") => DiagnosticSeverity::Error,
                Some(_) => {
                    rejected_count += 1;
                    completeness = SarifCompleteness::Partial;
                    continue;
                }
            };
            diagnostics.push(RawDiagnostic {
                code: format!("SARIF:{rule_id}"),
                title: format!("External static-analysis rule {rule_id}"),
                message: "A registered external analyzer reported this rule; provider message text is retained only in the redacted raw artifact."
                    .to_owned(),
                severity,
                confidence: DiagnosticConfidence::Medium,
                status: DiagnosticStatus::Suspected,
                blocking: false,
                package_id: None,
                workspace_id: None,
                locations,
            });
            candidates.push(SarifFindingCandidate {
                rule_id: rule_id.to_owned(),
                correlation_key,
                severity,
                locations: diagnostics
                    .last()
                    .map(|diagnostic| diagnostic.locations.clone())
                    .unwrap_or_default(),
            });
            imported_count += 1;
        }
    }
    SarifNormalization {
        diagnostics,
        candidates,
        completeness,
        imported_count,
        rejected_count,
    }
}

fn rejected(code: &str, message: &str) -> SarifNormalization {
    SarifNormalization {
        diagnostics: vec![RawDiagnostic {
            code: code.to_owned(),
            title: "SARIF output was rejected".to_owned(),
            message: message.to_owned(),
            severity: DiagnosticSeverity::Error,
            confidence: DiagnosticConfidence::High,
            status: DiagnosticStatus::Confirmed,
            blocking: true,
            package_id: None,
            workspace_id: None,
            locations: vec![],
        }],
        candidates: Vec::new(),
        completeness: SarifCompleteness::Unverified,
        imported_count: 0,
        rejected_count: 1,
    }
}

fn normalize_locations(
    result: &serde_json::Map<String, serde_json::Value>,
    project_id: &ProjectId,
    project_root: &Path,
) -> Result<Vec<LocationRef>, &'static str> {
    let Some(locations) = result.get("locations") else {
        return Ok(Vec::new());
    };
    let Some(locations) = locations.as_array() else {
        return Err("SARIF_MALFORMED");
    };
    if locations.len() > MAX_LOCATIONS_PER_RESULT {
        return Err("SARIF_RESOURCE_LIMIT");
    }
    let mut normalized = Vec::new();
    for location in locations {
        let Some(location) = location.as_object() else {
            return Err("SARIF_MALFORMED");
        };
        let Some(physical) = location
            .get("physicalLocation")
            .and_then(serde_json::Value::as_object)
        else {
            return Err("SARIF_LOCATION_MISSING");
        };
        let Some(uri) = physical
            .get("artifactLocation")
            .and_then(serde_json::Value::as_object)
            .and_then(|artifact| artifact.get("uri"))
            .and_then(serde_json::Value::as_str)
        else {
            return Err("SARIF_LOCATION_MISSING");
        };
        let path =
            normalize_project_uri(uri, project_root).ok_or("SARIF_LOCATION_OUTSIDE_PROJECT")?;
        let region = physical
            .get("region")
            .and_then(serde_json::Value::as_object);
        let line = region
            .and_then(|region| region.get("startLine"))
            .and_then(serde_json::Value::as_u64)
            .filter(|line| *line > 0 && *line <= u32::MAX as u64)
            .unwrap_or(1) as u32;
        let column = region
            .and_then(|region| region.get("startColumn"))
            .and_then(serde_json::Value::as_u64)
            .filter(|column| *column > 0 && *column <= u32::MAX as u64)
            .unwrap_or(1) as u32;
        let project_path = ProjectPathRef {
            project_id: project_id.clone(),
            path,
            path_kind: ProjectPathKind::File,
        };
        project_path
            .validate()
            .map_err(|_| "SARIF_LOCATION_OUTSIDE_PROJECT")?;
        normalized.push(LocationRef {
            path: project_path,
            start: TextPosition { line, column },
            end: None,
            symbol: None,
        });
    }
    Ok(normalized)
}

fn normalize_project_uri(uri: &str, project_root: &Path) -> Option<String> {
    let file_uri = uri.strip_prefix("file:///");
    let uri = match file_uri {
        Some(value) if has_windows_drive_prefix(value) => value.to_owned(),
        Some(value) => format!("/{value}"),
        None => uri.to_owned(),
    };
    if uri.contains("//") || uri.contains("://") {
        return None;
    }
    let root = project_root.to_string_lossy().replace('\\', "/");
    let uri = uri.replace('\\', "/");
    let relative = if uri.starts_with('/') || has_windows_drive_prefix(&uri) {
        let root = root.trim_end_matches('/');
        let prefix_matches = if cfg!(windows) {
            uri.len() > root.len()
                && uri
                    .get(..root.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
                && uri.as_bytes().get(root.len()) == Some(&b'/')
        } else {
            uri.strip_prefix(root)
                .is_some_and(|suffix| suffix.starts_with('/'))
        };
        prefix_matches.then(|| uri[root.len() + 1..].to_owned())?
    } else {
        uri
    };
    if relative.is_empty()
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(relative)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    value.len() >= 3
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value.as_bytes()[1] == b':'
        && value.as_bytes()[2] == b'/'
}

fn valid_rule_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RULE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_id() -> ProjectId {
        ProjectId::new()
    }

    #[test]
    fn valid_minimal_sarif_is_normalized_without_provider_message() {
        let root = Path::new("C:/workspace/project");
        let result = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.rule","level":"warning","message":{"text":"token=secret"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":2,"startColumn":4}}}]}]}]}"#,
            &project_id(),
            root,
        );
        assert_eq!(result.completeness, SarifCompleteness::Complete);
        assert_eq!(result.imported_count, 1);
        assert!(!result.diagnostics[0].message.contains("secret"));
        assert_eq!(result.diagnostics[0].locations[0].path.path, "src/lib.rs");
    }

    #[test]
    fn traversal_and_future_version_are_rejected_fail_closed() {
        let project = project_id();
        let root = Path::new("C:/workspace/project");
        let traversal = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.rule","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"../secret.rs"}}}]}]}]}"#,
            &project,
            root,
        );
        assert_eq!(traversal.completeness, SarifCompleteness::Unverified);
        assert_eq!(
            traversal.diagnostics[0].code,
            "SARIF_LOCATION_OUTSIDE_PROJECT"
        );
        let future = normalize_sarif_2_1(br#"{"version":"2.2.0","runs":[]}"#, &project, root);
        assert_eq!(future.completeness, SarifCompleteness::Unverified);
        assert_eq!(future.diagnostics[0].code, "SARIF_VERSION_UNSUPPORTED");
    }

    #[test]
    fn windows_absolute_uri_is_mapped_and_missing_location_stays_partial() {
        let project = project_id();
        let result = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.windows","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"C:\\workspace\\project\\src\\lib.rs"}}}]},{"ruleId":"fixture.unlocated","message":{"text":"x"}}]}]}"#,
            &project,
            Path::new("C:/workspace/project"),
        );
        assert_eq!(result.completeness, SarifCompleteness::Partial);
        assert_eq!(result.imported_count, 2);
        assert_eq!(result.diagnostics[0].locations[0].path.path, "src/lib.rs");
        assert!(result.diagnostics[1].locations.is_empty());

        let file_uri = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.file-uri","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///workspace/project/src/lib.rs"}}}]}]}]}"#,
            &project,
            Path::new("/workspace/project"),
        );
        assert_eq!(file_uri.completeness, SarifCompleteness::Complete);
        assert_eq!(file_uri.diagnostics[0].locations[0].path.path, "src/lib.rs");
    }

    #[test]
    fn partial_fingerprint_correlates_duplicate_alerts_deterministically() {
        let result = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.duplicate","message":{"text":"x"},"partialFingerprints":{"primaryLocationLineHash":"same"}},{"ruleId":"fixture.duplicate","message":{"text":"x"},"partialFingerprints":{"primaryLocationLineHash":"same"}}]}]}"#,
            &project_id(),
            Path::new("C:/workspace/project"),
        );
        assert_eq!(result.imported_count, 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.completeness, SarifCompleteness::Partial);
        assert_eq!(
            result.candidates[0].correlation_key,
            format!(
                "fixture.duplicate:partial:{}",
                Sha256Hash::digest(b"same").as_str()
            )
        );
        assert!(!result.candidates[0].correlation_key.ends_with(":same"));
    }

    #[test]
    fn multiple_runs_and_location_fallback_are_imported_deterministically() {
        let result = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"first"}},"results":[{"ruleId":"fixture.first","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/one.rs"}}}]}]},{"tool":{"driver":{"name":"second"}},"results":[{"ruleId":"fixture.second","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/two.rs"}}}]}]}]}"#,
            &project_id(),
            Path::new("C:/workspace/project"),
        );
        assert_eq!(result.completeness, SarifCompleteness::Complete);
        assert_eq!(result.imported_count, 2);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].code, "SARIF:fixture.first");
        assert_eq!(result.diagnostics[1].code, "SARIF:fixture.second");
    }

    #[test]
    fn missing_rule_and_oversized_result_are_rejected() {
        let project = project_id();
        let missing_rule = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"message":{"text":"x"}}]}]}"#,
            &project,
            Path::new("C:/workspace/project"),
        );
        assert_eq!(missing_rule.completeness, SarifCompleteness::Unverified);
        assert_eq!(missing_rule.diagnostics[0].code, "SARIF_RULE_ID_MISSING");

        let results = (0..=MAX_RESULTS)
            .map(|_| serde_json::json!({"ruleId":"fixture.limit","message":{"text":"x"}}))
            .collect::<Vec<_>>();
        let oversized = serde_json::json!({
            "version":"2.1.0",
            "runs":[{"tool":{"driver":{"name":"fixture"}},"results":results}]
        });
        let oversized = normalize_sarif_2_1(
            serde_json::to_vec(&oversized).unwrap().as_slice(),
            &project,
            Path::new("C:/workspace/project"),
        );
        assert_eq!(oversized.completeness, SarifCompleteness::Unverified);
        assert_eq!(oversized.diagnostics[0].code, "SARIF_RESOURCE_LIMIT");
    }

    #[test]
    fn byte_limit_and_contract_invalid_relative_paths_are_rejected() {
        let project = project_id();
        let oversized = vec![b' '; MAX_SARIF_BYTES + 1];
        let result = normalize_sarif_2_1(&oversized, &project, Path::new("C:/workspace/project"));
        assert_eq!(result.completeness, SarifCompleteness::Unverified);
        assert_eq!(result.diagnostics[0].code, "SARIF_RESOURCE_LIMIT");

        let drive_relative = normalize_sarif_2_1(
            br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"fixture.path","message":{"text":"x"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"C:secret.rs"}}}]}]}]}"#,
            &project,
            Path::new("C:/workspace/project"),
        );
        assert_eq!(drive_relative.completeness, SarifCompleteness::Unverified);
        assert_eq!(
            drive_relative.diagnostics[0].code,
            "SARIF_LOCATION_OUTSIDE_PROJECT"
        );
    }
}
