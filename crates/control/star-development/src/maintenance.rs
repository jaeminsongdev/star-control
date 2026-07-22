use std::collections::BTreeSet;

use star_contracts::{
    Sha256Hash,
    development::{
        EvidenceCompleteness, MAINTENANCE_RADAR_SCHEMA_ID, MaintenanceRadar,
        REPRODUCTION_PACK_SCHEMA_ID, RadarItem, ReproductionAttempt, ReproductionPack,
        ReproductionState,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, token};

#[derive(Clone, Debug)]
pub struct FailureObservation {
    pub producer_code: String,
    pub message: String,
    pub logical_owner: String,
    pub command_descriptor: String,
    pub environment_class: String,
}

pub fn failure_family_fingerprint(
    observation: &FailureObservation,
) -> Result<Sha256Hash, DevelopmentError> {
    if !token(&observation.producer_code, 160)
        || observation.logical_owner.trim().is_empty()
        || observation.command_descriptor.trim().is_empty()
        || observation.environment_class.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    fingerprint(
        "star.failure-family",
        &serde_json::json!({
            "producer_code":observation.producer_code,
            "message_template":normalize_failure_message(&observation.message),
            "logical_owner":observation.logical_owner,
            "command_descriptor":observation.command_descriptor,
            "environment_class":observation.environment_class,
            "normalization_version":1,
        }),
    )
}

pub fn build_reproduction_pack(
    family_fingerprint: Sha256Hash,
    subject_fingerprint: Sha256Hash,
    mut attempts: Vec<ReproductionAttempt>,
    external_blocker: Option<String>,
) -> Result<ReproductionPack, DevelopmentError> {
    attempts.sort_by_key(|attempt| attempt.attempt);
    if attempts
        .windows(2)
        .any(|pair| pair[0].attempt == pair[1].attempt)
        || attempts.iter().any(|attempt| attempt.attempt == 0)
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut limitations = Vec::new();
    let state = if let Some(blocker) = external_blocker {
        if blocker.trim().is_empty() {
            return Err(DevelopmentError::Invalid);
        }
        limitations.push(blocker);
        ReproductionState::BlockedExternal
    } else if attempts.is_empty() || attempts.iter().any(|attempt| !attempt.complete) {
        limitations.push("required_attempt_missing_or_incomplete".to_owned());
        ReproductionState::Unverified
    } else {
        let same_family = attempts
            .iter()
            .filter(|attempt| attempt.family_fingerprint == family_fingerprint)
            .collect::<Vec<_>>();
        if same_family.iter().any(|attempt| attempt.observed) {
            let environments = same_family
                .iter()
                .filter(|attempt| attempt.observed)
                .map(|attempt| &attempt.environment_fingerprint)
                .collect::<BTreeSet<_>>();
            if environments.len() == 1 {
                ReproductionState::Reproduced
            } else {
                limitations.push("environment_variance".to_owned());
                ReproductionState::PartiallyReproduced
            }
        } else if attempts.iter().any(|attempt| attempt.observed) {
            limitations.push("different_failure_family_observed".to_owned());
            ReproductionState::PartiallyReproduced
        } else {
            ReproductionState::NotReproduced
        }
    };
    let mut pack = ReproductionPack {
        schema_id: REPRODUCTION_PACK_SCHEMA_ID.to_owned(),
        schema_version: 1,
        family_fingerprint,
        subject_fingerprint,
        attempts,
        state,
        limitations,
        pack_fingerprint: placeholder(),
    };
    pack.pack_fingerprint = fingerprint(
        REPRODUCTION_PACK_SCHEMA_ID,
        &serde_json::json!({
            "family_fingerprint":pack.family_fingerprint,
            "subject_fingerprint":pack.subject_fingerprint,
            "attempts":pack.attempts,
            "state":pack.state,
            "limitations":pack.limitations,
        }),
    )?;
    Ok(pack)
}

pub fn build_radar(mut items: Vec<RadarItem>) -> Result<MaintenanceRadar, DevelopmentError> {
    if items.is_empty()
        || items.iter().any(|item| {
            !token(&item.item_id, 160)
                || item.subject.trim().is_empty()
                || item.source.trim().is_empty()
                || item.priority > 1_000
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    items.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.item_id.cmp(&right.item_id))
    });
    if items
        .windows(2)
        .any(|pair| pair[0].item_id == pair[1].item_id)
    {
        return Err(DevelopmentError::Conflict);
    }
    let mut limitations = items
        .iter()
        .filter(|item| !item.fresh)
        .map(|item| format!("stale_external_input:{}", item.item_id))
        .collect::<Vec<_>>();
    limitations.sort();
    let completeness = if limitations.is_empty() {
        EvidenceCompleteness::Complete
    } else {
        EvidenceCompleteness::Partial
    };
    let mut radar = MaintenanceRadar {
        schema_id: MAINTENANCE_RADAR_SCHEMA_ID.to_owned(),
        schema_version: 1,
        items,
        completeness,
        limitations,
        radar_fingerprint: placeholder(),
    };
    radar.radar_fingerprint = fingerprint(
        MAINTENANCE_RADAR_SCHEMA_ID,
        &serde_json::json!({
            "items":radar.items,
            "completeness":radar.completeness,
            "limitations":radar.limitations,
        }),
    )?;
    Ok(radar)
}

fn normalize_failure_message(message: &str) -> String {
    let mut output = String::new();
    let mut digit_run = false;
    for token in message.split_whitespace() {
        let looks_like_path =
            token.contains("\\") || token.starts_with('/') || token.get(1..2) == Some(":");
        if looks_like_path {
            if !output.is_empty() {
                output.push(' ');
            }
            output.push_str("<path>");
            digit_run = false;
            continue;
        }
        for character in token.chars() {
            if character.is_ascii_digit() {
                if !digit_run {
                    output.push_str("<n>");
                    digit_run = true;
                }
            } else {
                output.push(character.to_ascii_lowercase());
                digit_run = false;
            }
        }
        output.push(' ');
        digit_run = false;
    }
    output.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use star_contracts::development::{RadarItem, RadarKind};

    use super::*;

    #[test]
    fn failure_family_ignores_paths_pids_and_timestamps() {
        let base = FailureObservation {
            producer_code: "RUST_TEST_FAILED".to_owned(),
            message: "C:\\tmp\\a.rs pid 123 failed at 2026".to_owned(),
            logical_owner: "crate::tests::case".to_owned(),
            command_descriptor: "cargo.test.v1".to_owned(),
            environment_class: "windows-x64".to_owned(),
        };
        let mut changed = base.clone();
        changed.message = "/tmp/b.rs pid 999 failed at 2030".to_owned();
        assert_eq!(
            failure_family_fingerprint(&base).unwrap(),
            failure_family_fingerprint(&changed).unwrap()
        );
    }

    #[test]
    fn reproduction_and_radar_preserve_unverified_external_state() {
        let family = Sha256Hash::digest(b"family");
        let environment = Sha256Hash::digest(b"env");
        let pack = build_reproduction_pack(
            family.clone(),
            Sha256Hash::digest(b"subject"),
            vec![ReproductionAttempt {
                attempt: 1,
                family_fingerprint: family,
                environment_fingerprint: environment,
                input_fingerprint: Sha256Hash::digest(b"input"),
                complete: true,
                observed: true,
                duration_ms: 10,
            }],
            None,
        )
        .unwrap();
        assert_eq!(pack.state, ReproductionState::Reproduced);

        let radar = build_radar(vec![RadarItem {
            item_id: "advisory-1".to_owned(),
            kind: RadarKind::Security,
            subject: "crate-a".to_owned(),
            priority: 900,
            source: "registered-advisory-feed".to_owned(),
            source_fingerprint: Sha256Hash::digest(b"feed"),
            fresh: false,
            blocking: true,
        }])
        .unwrap();
        assert_eq!(radar.completeness, EvidenceCompleteness::Partial);
        assert!(!radar.limitations.is_empty());
    }
}
