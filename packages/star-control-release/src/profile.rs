use crate::consistency::ReleaseConsistencyResult;
use crate::error::ReleaseReadinessError;
use crate::support::{
    check_status, normalize_evidence_paths, normalize_profile_blockers, normalized_profile_name,
    release_check,
};
use crate::writer::ReleaseReadinessWriter;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseProfileValidation {
    profile_name: String,
    passed: bool,
    evidence_paths: Vec<String>,
    blockers: Vec<String>,
}

impl ReleaseProfileValidation {
    pub fn passed(
        profile_name: impl Into<String>,
        evidence_paths: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        Ok(Self {
            profile_name: normalized_profile_name(profile_name)?,
            passed: true,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers: Vec::new(),
        })
    }

    pub fn failed(
        profile_name: impl Into<String>,
        evidence_paths: Vec<String>,
        blockers: Vec<String>,
    ) -> Result<Self, ReleaseReadinessError> {
        let blockers = normalize_profile_blockers(blockers)?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "failed release profile validation requires at least one blocker"
                    .to_string(),
            });
        }
        Ok(Self {
            profile_name: normalized_profile_name(profile_name)?,
            passed: false,
            evidence_paths: normalize_evidence_paths(evidence_paths)?,
            blockers,
        })
    }

    pub fn profile_name(&self) -> &str {
        &self.profile_name
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
            "release-profile-passed",
            check_status(self.passed),
            self.evidence_paths.clone(),
        )
    }

    fn into_blockers(self) -> Vec<String> {
        self.blockers
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseProfileReadinessBuilder;

impl ReleaseProfileReadinessBuilder {
    pub fn build(
        &self,
        writer: &ReleaseReadinessWriter,
        release_id: impl Into<String>,
        target: impl Into<String>,
        version: impl Into<String>,
        profile: ReleaseProfileValidation,
        consistency: ReleaseConsistencyResult,
    ) -> Value {
        let release_id = release_id.into();
        let target = target.into();
        let version = version.into();
        let mut checks = vec![profile.to_check()];
        let (mut consistency_checks, consistency_blockers) = consistency.into_parts();
        checks.append(&mut consistency_checks);
        let mut blockers = profile.into_blockers();
        blockers.extend(consistency_blockers);

        if blockers.is_empty() {
            blockers.push(
                "release approval/signing/publish/deploy automation remains reserved".to_string(),
            );
            writer.readiness(release_id, target, version, "reserved", checks, blockers)
        } else {
            writer.not_ready(release_id, target, version, checks, blockers)
        }
    }
}
