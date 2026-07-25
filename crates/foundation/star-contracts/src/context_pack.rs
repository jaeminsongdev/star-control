//! Source-bound Context Pack contract for A03 and Codex execution.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CheckoutId, ContextPackId, ProjectId, Sha256Hash, StageId, canonical_sha256,
    evidence::DocumentRef,
    ids::{CodeIndexSnapshotId, ProjectCatalogSnapshotId, ProjectRevisionId, WorkspaceSnapshotId},
    index::{IndexFreshnessState, IndexTier},
    management::ProjectPathRef,
};

pub const CONTEXT_PACK_SCHEMA_ID: &str = "star.context-pack";
pub const CONTEXT_PACK_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextFreshnessPolicyV1 {
    RequireCurrent,
    AllowStaleWithWarning,
    PinnedSnapshot,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContextQualityStateV1 {
    Current,
    Stale,
    Partial,
    Unsupported,
    Unverified,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContextItemKindV1 {
    Source,
    Test,
    Docs,
    Config,
    Schema,
    Migration,
    Symbol,
    Contract,
    Guidance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceAuthorityV1 {
    CanonicalSource,
    ProjectPolicy,
    GeneratedProjection,
    ExternalReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSensitivityV1 {
    Public,
    Internal,
    Sensitive,
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextDeliveryV1 {
    ReferenceOnly,
    Inline,
    InlineRedacted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextProjectInputV1 {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub checkout_observation_fingerprint: Sha256Hash,
    pub workspace_entries_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextIndexSnapshotRefV1 {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub project_catalog_snapshot_id: ProjectCatalogSnapshotId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub required_tier: IndexTier,
    pub used_tier: IndexTier,
    pub freshness_states: Vec<IndexFreshnessState>,
    pub partition_fingerprints: Vec<Sha256Hash>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextItemV1 {
    pub kind: ContextItemKindV1,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub relative_path: ProjectPathRef,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub content_sha256: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_entity_key: Option<String>,
    pub requested_tier: IndexTier,
    pub used_tier: IndexTier,
    pub inclusion_reason: String,
    pub source_authority: ContextSourceAuthorityV1,
    pub freshness: ContextQualityStateV1,
    pub sensitivity: ContextSensitivityV1,
    pub delivery: ContextDeliveryV1,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextOmissionV1 {
    pub project_id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<ProjectPathRef>,
    pub reason_code: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextQualitySummaryV1 {
    pub state: ContextQualityStateV1,
    pub current_items: u32,
    pub stale_items: u32,
    pub partial_items: u32,
    pub unsupported_items: u32,
    pub tier_coverage: BTreeMap<IndexTier, u32>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextPackV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub context_pack_id: ContextPackId,
    pub revision: u64,
    pub stage_graph_ref: DocumentRef,
    pub stage_id: StageId,
    pub stage_revision: u64,
    pub task_spec_ref: DocumentRef,
    pub scope_revision_ref: DocumentRef,
    pub project_inputs: Vec<ContextProjectInputV1>,
    pub project_catalog_snapshot_ref: DocumentRef,
    pub code_index_snapshot_refs: Vec<ContextIndexSnapshotRefV1>,
    pub items: Vec<ContextItemV1>,
    pub token_budget: u32,
    pub estimated_tokens: u32,
    pub omissions: Vec<ContextOmissionV1>,
    pub quality_summary: ContextQualitySummaryV1,
    pub freshness_policy: ContextFreshnessPolicyV1,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub context_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextPackError {
    #[error("Context Pack identity or reference is invalid")]
    Identity,
    #[error("Context Pack source coverage is incomplete or inconsistent")]
    Coverage,
    #[error("Context Pack freshness or limitation policy is violated")]
    Freshness,
    #[error("Context Pack content is invalid")]
    Content,
    #[error("Context Pack fingerprint could not be calculated")]
    Fingerprint,
}

impl ContextPackV1 {
    pub fn seal(mut self) -> Result<Self, ContextPackError> {
        self.project_inputs.sort_by(|left, right| {
            (&left.project_id, &left.checkout_id).cmp(&(&right.project_id, &right.checkout_id))
        });
        self.code_index_snapshot_refs.sort_by(|left, right| {
            (&left.project_id, &left.checkout_id).cmp(&(&right.project_id, &right.checkout_id))
        });
        for reference in &mut self.code_index_snapshot_refs {
            reference.freshness_states.sort();
            reference.freshness_states.dedup();
            reference.partition_fingerprints.sort();
            reference.partition_fingerprints.dedup();
        }
        self.items.sort_by(|left, right| {
            (
                &left.project_id,
                &left.checkout_id,
                &left.relative_path,
                left.kind,
            )
                .cmp(&(
                    &right.project_id,
                    &right.checkout_id,
                    &right.relative_path,
                    right.kind,
                ))
        });
        for item in &mut self.items {
            normalize_strings(&mut item.limitations)?;
        }
        self.omissions.sort_by(|left, right| {
            (&left.project_id, &left.relative_path, &left.reason_code).cmp(&(
                &right.project_id,
                &right.relative_path,
                &right.reason_code,
            ))
        });
        normalize_strings(&mut self.quality_summary.limitations)?;
        self.validate_shape()?;
        self.context_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":CONTEXT_PACK_SCHEMA_ID,
            "version":CONTEXT_PACK_CONTRACT_VERSION,
            "value":{
                "context_pack_id":self.context_pack_id,
                "revision":self.revision,
                "stage_graph_ref":self.stage_graph_ref,
                "stage_id":self.stage_id,
                "stage_revision":self.stage_revision,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "project_inputs":self.project_inputs,
                "project_catalog_snapshot_ref":self.project_catalog_snapshot_ref,
                "code_index_snapshot_refs":self.code_index_snapshot_refs,
                "items":self.items,
                "token_budget":self.token_budget,
                "estimated_tokens":self.estimated_tokens,
                "omissions":self.omissions,
                "quality_summary":self.quality_summary,
                "freshness_policy":self.freshness_policy,
                "generated_at":self.generated_at,
                "expires_at":self.expires_at,
            }
        }))
        .map_err(|_| ContextPackError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), ContextPackError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(ContextPackError::Fingerprint);
        }
        Ok(())
    }

    pub fn is_current_at(&self, now: DateTime<Utc>) -> bool {
        self.generated_at <= now
            && now < self.expires_at
            && self.freshness_policy == ContextFreshnessPolicyV1::RequireCurrent
            && self.quality_summary.state == ContextQualityStateV1::Current
    }

    fn validate_shape(&self) -> Result<(), ContextPackError> {
        if self.schema_id != CONTEXT_PACK_SCHEMA_ID
            || self.schema_version != CONTEXT_PACK_CONTRACT_VERSION
            || self.revision == 0
            || self.stage_revision == 0
            || self.generated_at >= self.expires_at
            || self
                .expires_at
                .signed_duration_since(self.generated_at)
                .num_hours()
                > 24
            || !document_ref(&self.task_spec_ref, "star.task-spec")
            || !document_ref(&self.scope_revision_ref, "star.scope-revision")
            || !document_ref(&self.stage_graph_ref, "star.stage-graph")
            || !document_ref(
                &self.project_catalog_snapshot_ref,
                "star.project-catalog-snapshot",
            )
        {
            return Err(ContextPackError::Identity);
        }
        if self.project_inputs.is_empty()
            || self.project_inputs.len() > 64
            || self.code_index_snapshot_refs.len() != self.project_inputs.len()
            || self.items.is_empty()
            || self.items.len() > 2_048
            || self.omissions.len() > 2_048
            || self.token_budget == 0
            || self.estimated_tokens > self.token_budget
        {
            return Err(ContextPackError::Content);
        }
        let project_inputs = self
            .project_inputs
            .iter()
            .map(|input| ((input.project_id.clone(), input.checkout_id.clone()), input))
            .collect::<BTreeMap<_, _>>();
        let index_refs = self
            .code_index_snapshot_refs
            .iter()
            .map(|reference| {
                (
                    (reference.project_id.clone(), reference.checkout_id.clone()),
                    reference,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let item_identities = self
            .items
            .iter()
            .map(|item| {
                (
                    item.project_id.clone(),
                    item.checkout_id.clone(),
                    item.relative_path.clone(),
                    item.kind,
                    item.index_entity_key.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        if project_inputs.len() != self.project_inputs.len()
            || index_refs.len() != self.code_index_snapshot_refs.len()
            || project_inputs.keys().ne(index_refs.keys())
            || item_identities.len() != self.items.len()
            || self.code_index_snapshot_refs.iter().any(|reference| {
                let input = project_inputs
                    .get(&(reference.project_id.clone(), reference.checkout_id.clone()));
                reference.project_catalog_snapshot_id.as_str()
                    != self.project_catalog_snapshot_ref.document_id
                    || input.is_none_or(|input| {
                        reference.project_revision_id != input.project_revision_id
                            || reference.workspace_snapshot_id != input.workspace_snapshot_id
                    })
                    || reference.freshness_states.is_empty()
                    || reference.partition_fingerprints.is_empty()
                    || reference.used_tier < reference.required_tier
            })
            || self.items.iter().any(|item| {
                let input =
                    project_inputs.get(&(item.project_id.clone(), item.checkout_id.clone()));
                let index = index_refs.get(&(item.project_id.clone(), item.checkout_id.clone()));
                input.is_none_or(|input| {
                    item.project_revision_id != input.project_revision_id
                        || item.workspace_snapshot_id != input.workspace_snapshot_id
                }) || index.is_none_or(|index| item.used_tier > index.used_tier)
                    || item.used_tier < item.requested_tier
                    || !bounded_text(&item.inclusion_reason, 1_024)
                    || item
                        .index_entity_key
                        .as_deref()
                        .is_some_and(|value| !bounded_text(value, 256))
                    || (item.sensitivity == ContextSensitivityV1::Secret
                        && item.delivery != ContextDeliveryV1::ReferenceOnly)
            })
        {
            return Err(ContextPackError::Coverage);
        }
        let counts = self.items.iter().fold([0_u32; 4], |mut counts, item| {
            match item.freshness {
                ContextQualityStateV1::Current => counts[0] = counts[0].saturating_add(1),
                ContextQualityStateV1::Stale => counts[1] = counts[1].saturating_add(1),
                ContextQualityStateV1::Partial | ContextQualityStateV1::Unverified => {
                    counts[2] = counts[2].saturating_add(1)
                }
                ContextQualityStateV1::Unsupported => counts[3] = counts[3].saturating_add(1),
            }
            counts
        });
        if counts
            != [
                self.quality_summary.current_items,
                self.quality_summary.stale_items,
                self.quality_summary.partial_items,
                self.quality_summary.unsupported_items,
            ]
        {
            return Err(ContextPackError::Coverage);
        }
        let expected_tier_coverage =
            self.items
                .iter()
                .fold(BTreeMap::<IndexTier, u32>::new(), |mut coverage, item| {
                    let count = coverage.entry(item.used_tier).or_insert(0);
                    *count = count.saturating_add(1);
                    coverage
                });
        let expected_quality_state = if counts[3] > 0 {
            ContextQualityStateV1::Unsupported
        } else if self
            .items
            .iter()
            .any(|item| item.freshness == ContextQualityStateV1::Unverified)
        {
            ContextQualityStateV1::Unverified
        } else if counts[2] > 0 {
            ContextQualityStateV1::Partial
        } else if counts[1] > 0 {
            ContextQualityStateV1::Stale
        } else {
            ContextQualityStateV1::Current
        };
        if self.quality_summary.tier_coverage != expected_tier_coverage
            || self.quality_summary.state != expected_quality_state
        {
            return Err(ContextPackError::Coverage);
        }
        let non_current = counts[1..].iter().any(|count| *count > 0)
            || self.code_index_snapshot_refs.iter().any(|reference| {
                reference
                    .freshness_states
                    .iter()
                    .any(|state| *state != IndexFreshnessState::Current)
            });
        if self.freshness_policy == ContextFreshnessPolicyV1::RequireCurrent
            && (non_current
                || self.quality_summary.state != ContextQualityStateV1::Current
                || !self.quality_summary.limitations.is_empty())
        {
            return Err(ContextPackError::Freshness);
        }
        if self.freshness_policy != ContextFreshnessPolicyV1::RequireCurrent
            && non_current
            && self.omissions.is_empty()
            && self.quality_summary.limitations.is_empty()
        {
            return Err(ContextPackError::Freshness);
        }
        if self.omissions.iter().any(|omission| {
            !project_inputs
                .iter()
                .any(|((project_id, _), _)| project_id == &omission.project_id)
                || !bounded_token(&omission.reason_code, 96)
                || !bounded_text(&omission.reason, 1_024)
        }) {
            return Err(ContextPackError::Content);
        }
        Ok(())
    }
}

fn document_ref(reference: &DocumentRef, schema_id: &str) -> bool {
    reference.schema_id == schema_id
        && bounded_token(&reference.document_id, 192)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn normalize_strings(values: &mut Vec<String>) -> Result<(), ContextPackError> {
    values.sort();
    values.dedup();
    if values.len() > 256 || values.iter().any(|value| !bounded_text(value, 1_024)) {
        return Err(ContextPackError::Content);
    }
    Ok(())
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn reference(schema_id: &str, document_id: &str) -> DocumentRef {
        DocumentRef {
            schema_id: schema_id.to_owned(),
            document_id: document_id.to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(document_id.as_bytes()),
        }
    }

    fn pack() -> ContextPackV1 {
        let now = Utc::now();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let project_revision_id = ProjectRevisionId::from_stable_bytes(b"revision");
        let workspace_snapshot_id = WorkspaceSnapshotId::from_stable_bytes(b"workspace");
        let catalog_id = ProjectCatalogSnapshotId::from_stable_bytes(b"catalog");
        let index_id = CodeIndexSnapshotId::from_stable_bytes(b"index");
        ContextPackV1 {
            schema_id: CONTEXT_PACK_SCHEMA_ID.to_owned(),
            schema_version: CONTEXT_PACK_CONTRACT_VERSION,
            context_pack_id: ContextPackId::new(),
            revision: 1,
            stage_graph_ref: reference("star.stage-graph", "stage-graph"),
            stage_id: StageId::new(),
            stage_revision: 1,
            task_spec_ref: reference("star.task-spec", "task"),
            scope_revision_ref: reference("star.scope-revision", "scope"),
            project_inputs: vec![ContextProjectInputV1 {
                project_id: project_id.clone(),
                checkout_id: checkout_id.clone(),
                project_revision_id: project_revision_id.clone(),
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                checkout_observation_fingerprint: Sha256Hash::digest(b"checkout"),
                workspace_entries_fingerprint: Sha256Hash::digest(b"entries"),
            }],
            project_catalog_snapshot_ref: reference(
                "star.project-catalog-snapshot",
                catalog_id.as_str(),
            ),
            code_index_snapshot_refs: vec![ContextIndexSnapshotRefV1 {
                project_id: project_id.clone(),
                checkout_id: checkout_id.clone(),
                code_index_snapshot_id: index_id,
                project_catalog_snapshot_id: catalog_id,
                project_revision_id: project_revision_id.clone(),
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                required_tier: IndexTier::Text,
                used_tier: IndexTier::Syntax,
                freshness_states: vec![IndexFreshnessState::Current],
                partition_fingerprints: vec![Sha256Hash::digest(b"partition")],
                content_fingerprint: Sha256Hash::digest(b"index-content"),
            }],
            items: vec![ContextItemV1 {
                kind: ContextItemKindV1::Source,
                project_id,
                checkout_id,
                relative_path: ProjectPathRef::parse("src/lib.rs").unwrap(),
                project_revision_id,
                workspace_snapshot_id,
                content_sha256: Sha256Hash::digest(b"source"),
                index_entity_key: None,
                requested_tier: IndexTier::Text,
                used_tier: IndexTier::Syntax,
                inclusion_reason: "stage source".to_owned(),
                source_authority: ContextSourceAuthorityV1::CanonicalSource,
                freshness: ContextQualityStateV1::Current,
                sensitivity: ContextSensitivityV1::Internal,
                delivery: ContextDeliveryV1::ReferenceOnly,
                limitations: Vec::new(),
            }],
            token_budget: 8_000,
            estimated_tokens: 400,
            omissions: Vec::new(),
            quality_summary: ContextQualitySummaryV1 {
                state: ContextQualityStateV1::Current,
                current_items: 1,
                stale_items: 0,
                partial_items: 0,
                unsupported_items: 0,
                tier_coverage: BTreeMap::from([(IndexTier::Syntax, 1)]),
                limitations: Vec::new(),
            },
            freshness_policy: ContextFreshnessPolicyV1::RequireCurrent,
            generated_at: now,
            expires_at: now + Duration::minutes(15),
            context_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
    }

    #[test]
    fn context_pack_positive_seals_current_exact_sources() {
        let pack = pack().seal().unwrap();
        pack.verify().unwrap();
        assert!(pack.is_current_at(pack.generated_at));
    }

    #[test]
    fn context_pack_negative_secret_content_cannot_be_inlined() {
        let mut pack = pack();
        pack.items[0].sensitivity = ContextSensitivityV1::Secret;
        pack.items[0].delivery = ContextDeliveryV1::Inline;
        assert_eq!(pack.seal(), Err(ContextPackError::Coverage));
    }

    #[test]
    fn context_pack_failure_stale_source_cannot_satisfy_require_current() {
        let mut pack = pack();
        pack.items[0].freshness = ContextQualityStateV1::Stale;
        pack.quality_summary.current_items = 0;
        pack.quality_summary.stale_items = 1;
        pack.quality_summary.state = ContextQualityStateV1::Stale;
        assert_eq!(pack.seal(), Err(ContextPackError::Freshness));
    }

    #[test]
    fn context_pack_recovery_pinned_snapshot_preserves_limitation() {
        let mut pack = pack();
        pack.freshness_policy = ContextFreshnessPolicyV1::PinnedSnapshot;
        pack.items[0].freshness = ContextQualityStateV1::Stale;
        pack.quality_summary.current_items = 0;
        pack.quality_summary.stale_items = 1;
        pack.quality_summary.state = ContextQualityStateV1::Stale;
        pack.quality_summary.limitations = vec!["pinned_snapshot_differs_from_current".to_owned()];
        let sealed = pack.seal().unwrap();
        assert!(!sealed.is_current_at(sealed.generated_at));
        sealed.verify().unwrap();
    }

    #[test]
    fn context_pack_negative_rejects_cross_bound_source_revision() {
        let mut pack = pack();
        pack.items[0].project_revision_id = ProjectRevisionId::from_stable_bytes(b"other");
        assert_eq!(pack.seal(), Err(ContextPackError::Coverage));
    }

    #[test]
    fn context_pack_negative_rejects_duplicate_item_identity() {
        let mut pack = pack();
        pack.items.push(pack.items[0].clone());
        pack.quality_summary.current_items = 2;
        pack.quality_summary
            .tier_coverage
            .insert(IndexTier::Syntax, 2);
        assert_eq!(pack.seal(), Err(ContextPackError::Coverage));
    }

    #[test]
    fn context_pack_negative_recomputes_tier_coverage_and_quality_state() {
        let mut bad_coverage = pack();
        bad_coverage.quality_summary.tier_coverage = BTreeMap::from([(IndexTier::Text, 1)]);
        assert_eq!(bad_coverage.seal(), Err(ContextPackError::Coverage));

        let mut bad_state = pack();
        bad_state.quality_summary.state = ContextQualityStateV1::Partial;
        assert_eq!(bad_state.seal(), Err(ContextPackError::Coverage));
    }
}
