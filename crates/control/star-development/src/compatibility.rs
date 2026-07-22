use std::collections::{BTreeMap, BTreeSet};

use star_contracts::{
    Sha256Hash,
    development::{
        CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID, COMPATIBILITY_REPORT_SCHEMA_ID, CleanRoomDoctorReport,
        CompatibilityFinding, CompatibilityOutcome, CompatibilityReport, ContractKind, DoctorCheck,
        DoctorCheckState, EvidenceCompleteness,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, token};

#[derive(Clone, Debug)]
pub struct DoctorObservation {
    pub check_id: String,
    pub observed: Option<String>,
    pub required: String,
    pub compatible: bool,
}

pub fn compare_contract(
    kind: ContractKind,
    before: &[u8],
    after: &[u8],
) -> Result<CompatibilityReport, DevelopmentError> {
    let before_sha256 = Sha256Hash::digest(before);
    let after_sha256 = Sha256Hash::digest(after);
    let mut findings = match kind {
        ContractKind::Api => compare_api(before, after)?,
        ContractKind::Schema => compare_schema(before, after)?,
        ContractKind::Config => compare_config(before, after)?,
        ContractKind::Docs => compare_docs(before, after)?,
    };
    findings.sort_by(|left, right| (&left.code, &left.subject).cmp(&(&right.code, &right.subject)));
    let outcome = if findings
        .iter()
        .any(|finding| finding.outcome == CompatibilityOutcome::Breaking)
    {
        CompatibilityOutcome::Breaking
    } else if findings
        .iter()
        .any(|finding| finding.outcome == CompatibilityOutcome::Unverified)
    {
        CompatibilityOutcome::Unverified
    } else if findings
        .iter()
        .any(|finding| finding.outcome == CompatibilityOutcome::HumanReview)
    {
        CompatibilityOutcome::HumanReview
    } else {
        CompatibilityOutcome::Compatible
    };
    let completeness = if outcome == CompatibilityOutcome::Unverified {
        EvidenceCompleteness::Unverified
    } else {
        EvidenceCompleteness::Complete
    };
    let mut report = CompatibilityReport {
        schema_id: COMPATIBILITY_REPORT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        kind,
        before_sha256,
        after_sha256,
        outcome,
        findings,
        completeness,
        report_fingerprint: placeholder(),
    };
    report.report_fingerprint = fingerprint(
        COMPATIBILITY_REPORT_SCHEMA_ID,
        &serde_json::json!({
            "kind":report.kind,
            "before_sha256":report.before_sha256,
            "after_sha256":report.after_sha256,
            "outcome":report.outcome,
            "findings":report.findings,
            "completeness":report.completeness,
        }),
    )?;
    Ok(report)
}

pub fn clean_room_doctor(
    mut observations: Vec<DoctorObservation>,
) -> Result<CleanRoomDoctorReport, DevelopmentError> {
    observations.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    if observations.is_empty()
        || observations
            .windows(2)
            .any(|pair| pair[0].check_id == pair[1].check_id)
        || observations
            .iter()
            .any(|item| !token(&item.check_id, 128) || item.required.trim().is_empty())
    {
        return Err(DevelopmentError::Invalid);
    }
    let checks = observations
        .into_iter()
        .map(|observation| DoctorCheck {
            check_id: observation.check_id,
            state: match observation.observed.as_ref() {
                None => DoctorCheckState::Unverified,
                Some(_) if observation.compatible => DoctorCheckState::Pass,
                Some(_) => DoctorCheckState::Block,
            },
            observed: observation
                .observed
                .unwrap_or_else(|| "not_observed".to_owned()),
            required: observation.required,
        })
        .collect::<Vec<_>>();
    let state = if checks
        .iter()
        .any(|check| check.state == DoctorCheckState::Block)
    {
        DoctorCheckState::Block
    } else if checks
        .iter()
        .any(|check| check.state == DoctorCheckState::Unverified)
    {
        DoctorCheckState::Unverified
    } else {
        DoctorCheckState::Pass
    };
    let mut report = CleanRoomDoctorReport {
        schema_id: CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        dependency_download: "deny".to_owned(),
        package_install: "deny".to_owned(),
        system_mutation: "deny".to_owned(),
        checks,
        state,
        report_fingerprint: placeholder(),
    };
    report.report_fingerprint = fingerprint(
        CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID,
        &serde_json::json!({
            "dependency_download":report.dependency_download,
            "package_install":report.package_install,
            "system_mutation":report.system_mutation,
            "checks":report.checks,
            "state":report.state,
        }),
    )?;
    Ok(report)
}

fn compare_api(before: &[u8], after: &[u8]) -> Result<Vec<CompatibilityFinding>, DevelopmentError> {
    let before = public_api(before)?;
    let after = public_api(after)?;
    let mut findings = Vec::new();
    for removed in before.difference(&after) {
        findings.push(finding(
            "API_PUBLIC_ITEM_REMOVED",
            removed,
            CompatibilityOutcome::Breaking,
            "A previously public declaration is absent.",
        ));
    }
    for added in after.difference(&before) {
        findings.push(finding(
            "API_PUBLIC_ITEM_ADDED",
            added,
            CompatibilityOutcome::Compatible,
            "A public declaration was added.",
        ));
    }
    Ok(findings)
}

fn public_api(bytes: &[u8]) -> Result<BTreeSet<String>, DevelopmentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DevelopmentError::Invalid)?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub ") || line.starts_with("pub("))
        .map(normalize_space)
        .collect())
}

fn compare_schema(
    before: &[u8],
    after: &[u8],
) -> Result<Vec<CompatibilityFinding>, DevelopmentError> {
    let before: serde_json::Value =
        serde_json::from_slice(before).map_err(|_| DevelopmentError::Invalid)?;
    let after: serde_json::Value =
        serde_json::from_slice(after).map_err(|_| DevelopmentError::Invalid)?;
    let before_properties = schema_properties(&before)?;
    let after_properties = schema_properties(&after)?;
    let before_required = schema_required(&before)?;
    let after_required = schema_required(&after)?;
    let mut findings = Vec::new();
    for property in before_properties.keys() {
        if !after_properties.contains_key(property) {
            findings.push(finding(
                "SCHEMA_PROPERTY_REMOVED",
                property,
                CompatibilityOutcome::Breaking,
                "A previously declared property is absent.",
            ));
        } else if schema_type(before_properties[property])
            != schema_type(after_properties[property])
        {
            findings.push(finding(
                "SCHEMA_PROPERTY_TYPE_CHANGED",
                property,
                CompatibilityOutcome::Breaking,
                "A property type changed.",
            ));
        }
    }
    for property in after_required.difference(&before_required) {
        findings.push(finding(
            "SCHEMA_REQUIRED_PROPERTY_ADDED",
            property,
            CompatibilityOutcome::Breaking,
            "A new required property rejects older producers.",
        ));
    }
    for property in after_properties.keys() {
        if !before_properties.contains_key(property) && !after_required.contains(property) {
            findings.push(finding(
                "SCHEMA_OPTIONAL_PROPERTY_ADDED",
                property,
                CompatibilityOutcome::Compatible,
                "An optional property was added.",
            ));
        }
    }
    Ok(findings)
}

fn schema_properties(
    value: &serde_json::Value,
) -> Result<BTreeMap<String, &serde_json::Value>, DevelopmentError> {
    value
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(name, value)| (name.clone(), value))
                .collect()
        })
        .ok_or(DevelopmentError::Invalid)
}

fn schema_required(value: &serde_json::Value) -> Result<BTreeSet<String>, DevelopmentError> {
    value
        .get("required")
        .map(|required| {
            required
                .as_array()
                .ok_or(DevelopmentError::Invalid)?
                .iter()
                .map(|name| {
                    name.as_str()
                        .map(str::to_owned)
                        .ok_or(DevelopmentError::Invalid)
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(BTreeSet::new()))
}

fn schema_type(value: &serde_json::Value) -> Option<&str> {
    value.get("type").and_then(serde_json::Value::as_str)
}

fn compare_config(
    before: &[u8],
    after: &[u8],
) -> Result<Vec<CompatibilityFinding>, DevelopmentError> {
    let before = flatten_toml(std::str::from_utf8(before).map_err(|_| DevelopmentError::Invalid)?)?;
    let after = flatten_toml(std::str::from_utf8(after).map_err(|_| DevelopmentError::Invalid)?)?;
    let mut findings = Vec::new();
    for (key, old) in &before {
        match after.get(key) {
            None => findings.push(finding(
                "CONFIG_KEY_REMOVED",
                key,
                CompatibilityOutcome::Breaking,
                "A supported config key was removed.",
            )),
            Some(new) if value_kind(old) != value_kind(new) => findings.push(finding(
                "CONFIG_VALUE_TYPE_CHANGED",
                key,
                CompatibilityOutcome::Breaking,
                "A config value type changed.",
            )),
            Some(new) if new != old => findings.push(finding(
                "CONFIG_DEFAULT_CHANGED",
                key,
                CompatibilityOutcome::HumanReview,
                "A config value changed and requires semantic review.",
            )),
            _ => {}
        }
    }
    Ok(findings)
}

fn flatten_toml(text: &str) -> Result<BTreeMap<String, toml::Value>, DevelopmentError> {
    let root: toml::Value = toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
    let mut output = BTreeMap::new();
    flatten_value("", &root, &mut output);
    Ok(output)
}

fn flatten_value(prefix: &str, value: &toml::Value, output: &mut BTreeMap<String, toml::Value>) {
    if let toml::Value::Table(table) = value {
        for (key, value) in table {
            let key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten_value(&key, value, output);
        }
    } else {
        output.insert(prefix.to_owned(), value.clone());
    }
}

fn value_kind(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

fn compare_docs(
    before: &[u8],
    after: &[u8],
) -> Result<Vec<CompatibilityFinding>, DevelopmentError> {
    let before = docs_contract_tokens(before)?;
    let after = docs_contract_tokens(after)?;
    Ok(before
        .difference(&after)
        .map(|removed| {
            finding(
                "DOCS_CONTRACT_REFERENCE_REMOVED",
                removed,
                CompatibilityOutcome::HumanReview,
                "A documented command, link, or anchor disappeared.",
            )
        })
        .collect())
}

fn docs_contract_tokens(bytes: &[u8]) -> Result<BTreeSet<String>, DevelopmentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DevelopmentError::Invalid)?;
    let mut tokens = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            tokens.insert(format!(
                "anchor:{}",
                normalize_space(trimmed.trim_start_matches('#'))
            ));
        }
        for part in trimmed.split('`').skip(1).step_by(2) {
            if !part.trim().is_empty() {
                tokens.insert(format!("code:{}", part.trim()));
            }
        }
        for part in trimmed.split("](").skip(1) {
            if let Some(target) = part.split(')').next() {
                tokens.insert(format!("link:{target}"));
            }
        }
    }
    Ok(tokens)
}

fn finding(
    code: &str,
    subject: &str,
    outcome: CompatibilityOutcome,
    summary: &str,
) -> CompatibilityFinding {
    CompatibilityFinding {
        code: code.to_owned(),
        subject: subject.to_owned(),
        outcome,
        summary: summary.to_owned(),
    }
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_schema_config_and_docs_changes_are_classified_without_guessing_pass() {
        let api =
            compare_contract(ContractKind::Api, b"pub fn old();\n", b"pub fn new();\n").unwrap();
        assert_eq!(api.outcome, CompatibilityOutcome::Breaking);

        let schema = compare_contract(
            ContractKind::Schema,
            br#"{"type":"object","properties":{"a":{"type":"string"}},"required":[]}"#,
            br#"{"type":"object","properties":{"a":{"type":"string"},"b":{"type":"integer"}},"required":["b"]}"#,
        )
        .unwrap();
        assert_eq!(schema.outcome, CompatibilityOutcome::Breaking);

        let config =
            compare_contract(ContractKind::Config, b"timeout=1\n", b"timeout=2\n").unwrap();
        assert_eq!(config.outcome, CompatibilityOutcome::HumanReview);

        let docs =
            compare_contract(ContractKind::Docs, b"# Run\n`star run`\n", b"# Start\n").unwrap();
        assert_eq!(docs.outcome, CompatibilityOutcome::HumanReview);
    }

    #[test]
    fn doctor_never_installs_or_downloads_and_missing_probe_is_unverified() {
        let report = clean_room_doctor(vec![
            DoctorObservation {
                check_id: "rust".to_owned(),
                observed: Some("1.96.0".to_owned()),
                required: "1.96.0".to_owned(),
                compatible: true,
            },
            DoctorObservation {
                check_id: "signer".to_owned(),
                observed: None,
                required: "authenticode".to_owned(),
                compatible: false,
            },
        ])
        .unwrap();
        assert_eq!(report.state, DoctorCheckState::Unverified);
        assert_eq!(report.dependency_download, "deny");
        assert_eq!(report.package_install, "deny");
        assert_eq!(report.system_mutation, "deny");
    }
}
