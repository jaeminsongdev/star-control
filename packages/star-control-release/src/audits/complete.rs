use crate::constants::COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS;
use crate::error::ReleaseReadinessError;
use crate::support::{
    check_status, normalize_complete_implementation_blockers, normalize_evidence_paths,
    normalized_complete_implementation_check_name, release_check,
};
use crate::writer::ReleaseReadinessWriter;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteImplementationAuditCheck {
    name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl CompleteImplementationAuditCheck {
    pub fn passed(
        name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            name: normalized_complete_implementation_check_name(name)?,
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
        let blockers = normalize_complete_implementation_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed complete implementation check requires at least one blocker"
                    .to_string(),
            });
        }
        Ok(Self {
            name: normalized_complete_implementation_check_name(name)?,
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
pub struct CompleteImplementationAuditBuilder;

impl CompleteImplementationAuditBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        audit_checks: Vec<CompleteImplementationAuditCheck>,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = Vec::with_capacity(audit_checks.len());
        let mut blockers = Vec::new();
        let mut seen = Vec::with_capacity(audit_checks.len());

        for audit_check in &audit_checks {
            if seen
                .iter()
                .any(|seen_name: &String| seen_name == audit_check.name())
            {
                blockers.push(format!(
                    "duplicate complete implementation check: {}",
                    audit_check.name()
                ));
            } else {
                seen.push(audit_check.name().to_string());
            }

            checks.push(audit_check.to_check());

            if !audit_check.is_passed() {
                for blocker in audit_check.blockers() {
                    blockers.push(format!("{}: {}", audit_check.name(), blocker));
                }
            }
        }

        for required_check in COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS {
            if !seen.iter().any(|seen_name| seen_name == required_check) {
                blockers.push(format!(
                    "missing complete implementation check: {}",
                    required_check
                ));
            }
        }

        if blockers.is_empty() {
            blockers.push(
                "release/deploy/publish and external repository settings remain reserved until explicit approval"
                    .to_string(),
            );
            writer.readiness(release_id, target, version, "reserved", checks, blockers)
        } else {
            writer.not_ready(release_id, target, version, checks, blockers)
        }
    }
}
