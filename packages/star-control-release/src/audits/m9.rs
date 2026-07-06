use crate::constants::M9_REQUIRED_READINESS_CHECKS;
use crate::error::ReleaseReadinessError;
use crate::support::{
    check_status, normalize_evidence_paths, normalize_m9_readiness_blockers,
    normalized_m9_readiness_check_name, release_check,
};
use crate::writer::ReleaseReadinessWriter;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M9ReadinessCheck {
    name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl M9ReadinessCheck {
    pub fn passed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            name: normalized_m9_readiness_check_name(name)?,
            passed: true,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers: Vec::new(),
        })
    }

    pub fn failed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
        blockers: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        let blockers = normalize_m9_readiness_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed M9 readiness check requires at least one blocker".to_string(),
            });
        }
        Ok(Self {
            name: normalized_m9_readiness_check_name(name)?,
            passed: false,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_passed(&self) -> bool {
        self.passed
    }

    pub fn evidence_paths(&self) -> &[String] {
        &self.evidence_paths
    }

    pub fn blockers(&self) -> &[String] {
        &self.blockers
    }

    fn to_check(&self) -> Value {
        release_check(
            &self.name,
            check_status(self.passed),
            self.evidence_paths.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct M9ReadinessAuditBuilder;

impl M9ReadinessAuditBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        readiness_checks: Vec<M9ReadinessCheck>,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = Vec::with_capacity(readiness_checks.len());
        let mut blockers = Vec::new();
        let mut seen = Vec::with_capacity(readiness_checks.len());

        for readiness_check in &readiness_checks {
            if seen
                .iter()
                .any(|seen_name: &String| seen_name == readiness_check.name())
            {
                blockers.push(format!(
                    "duplicate M9 readiness check: {}",
                    readiness_check.name()
                ));
            } else {
                seen.push(readiness_check.name().to_string());
            }

            checks.push(readiness_check.to_check());

            if !readiness_check.is_passed() {
                for blocker in readiness_check.blockers() {
                    blockers.push(format!("{}: {}", readiness_check.name(), blocker));
                }
            }
        }

        for required_check in M9_REQUIRED_READINESS_CHECKS {
            if !seen.iter().any(|seen_name| seen_name == required_check) {
                blockers.push(format!("missing M9 readiness check: {}", required_check));
            }
        }

        if blockers.is_empty() {
            blockers.push(
                "final release/deploy/publish remains reserved until explicit approval".to_string(),
            );
            writer.readiness(release_id, target, version, "reserved", checks, blockers)
        } else {
            writer.not_ready(release_id, target, version, checks, blockers)
        }
    }
}
