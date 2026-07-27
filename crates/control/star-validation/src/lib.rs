//! Rule, Finding, ValidationResult, and GateDecision semantics.

pub mod permit;
pub mod planning;
pub mod process_executor;
pub mod rules;
pub mod runner;
pub mod rust_style;
pub mod sarif;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use star_contracts::{
    Sha256Hash,
    evidence::{
        ActorRef, ActorType, DocumentRef, GateDecision, GateDecisionKind, GateDecisionSchemaId,
        GatePolicySnapshot, GateScope, ProducerRef,
    },
    ids::{
        BaselineId, DispositionId, FindingId, GateId, OccurrenceId, ProjectId, ScanRunId,
        SuppressionId, ValidationResultId,
    },
    index::{
        HardcodingAssessment, HardcodingCandidate, SourceClass, SourceEntry,
        StructuralCloneCandidate,
    },
    management::{
        Baseline, BaselineStatus, CanonicalSource, Completeness, Confidence, Disposition,
        DispositionStatus, Finding, FindingLifecycle, Occurrence, PatchSet, ProjectRevision,
        RedactionState, Rule, RuleLifecycle, ScanRun, ScanStatus, Severity, SourceKind,
        SourceRange, Suppression, SuppressionStatus, Symbol, ValidationOutcome, ValidationResult,
    },
};
use star_domain::{validate_baseline, validate_suppression, versioned_fingerprint};
use star_project::FileObservation;
use thiserror::Error;

#[cfg(test)]
use star_contracts::management::WorkspaceSnapshot;

pub const TRAILING_WHITESPACE_RULE_ID: &str = "star.rule.trailing-whitespace";
pub const TRAILING_WHITESPACE_RECIPE_ID: &str = "star.recipe.remove-trailing-whitespace";
pub const HARDCODING_CANDIDATE_RULE_ID: &str = "star.rule.hardcoding-candidate";
pub const STRUCTURAL_CLONE_CANDIDATE_RULE_ID: &str = "star.rule.structural-clone-candidate";

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("validation input graph is inconsistent")]
    InconsistentGraph,
    #[error("validation fingerprint failed")]
    Fingerprint,
}

pub fn trailing_whitespace_rule() -> Result<Rule, ValidationError> {
    let definition_fingerprint = versioned_fingerprint(
        "star.rule-definition",
        1,
        &serde_json::json!({
            "rule_id":TRAILING_WHITESPACE_RULE_ID,
            "rule_version":"1.0.0",
            "identity_anchor":"symbol",
            "message_code":"TRAILING_WHITESPACE",
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    Ok(Rule {
        schema_id: "star.rule".to_owned(),
        schema_version: 1,
        rule_id: TRAILING_WHITESPACE_RULE_ID.to_owned(),
        rule_version: "1.0.0".to_owned(),
        definition_fingerprint,
        title: "Trailing whitespace".to_owned(),
        category: "source-hygiene".to_owned(),
        default_severity: Severity::Warning,
        default_confidence: Confidence::High,
        supported_languages: vec!["text".to_owned()],
        source_kinds: vec![SourceKind::File],
        analyzer_ref: "builtin.trailing-whitespace.v1".to_owned(),
        parameter_schema_ref: "star.rule.trailing-whitespace.parameters.v1".to_owned(),
        identity_contract_version: 1,
        identity_anchor: "symbol".to_owned(),
        redaction_contract_version: 1,
        remediation_recipe_refs: vec![TRAILING_WHITESPACE_RECIPE_ID.to_owned()],
        lifecycle: RuleLifecycle::Active,
    })
}

pub fn hardcoding_candidate_rule() -> Result<Rule, ValidationError> {
    let definition_fingerprint = versioned_fingerprint(
        "star.rule-definition",
        1,
        &serde_json::json!({
            "rule_id":HARDCODING_CANDIDATE_RULE_ID,
            "rule_version":"1.0.0",
            "identity_anchor":"canonical_source_and_candidate",
            "message_code":"HARDCODING_CANDIDATE",
            "automatic_confirmed_defect":false,
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    Ok(Rule {
        schema_id: "star.rule".to_owned(),
        schema_version: 1,
        rule_id: HARDCODING_CANDIDATE_RULE_ID.to_owned(),
        rule_version: "1.0.0".to_owned(),
        definition_fingerprint,
        title: "Hardcoding candidate".to_owned(),
        category: "maintainability".to_owned(),
        default_severity: Severity::Info,
        default_confidence: Confidence::Medium,
        supported_languages: vec!["text".to_owned()],
        source_kinds: vec![SourceKind::File],
        analyzer_ref: "builtin.hardcoding-candidate.v1".to_owned(),
        parameter_schema_ref: "star.rule.hardcoding-candidate.parameters.v1".to_owned(),
        identity_contract_version: 1,
        identity_anchor: "canonical_source_and_candidate".to_owned(),
        redaction_contract_version: 1,
        remediation_recipe_refs: Vec::new(),
        lifecycle: RuleLifecycle::Active,
    })
}

pub fn structural_clone_candidate_rule() -> Result<Rule, ValidationError> {
    let definition_fingerprint = versioned_fingerprint(
        "star.rule-definition",
        1,
        &serde_json::json!({
            "rule_id":STRUCTURAL_CLONE_CANDIDATE_RULE_ID,
            "rule_version":"1.0.0",
            "identity_anchor":"normalized-token-fingerprint-and-owner-set",
            "message_code":"STRUCTURAL_CLONE_CANDIDATE",
            "automatic_confirmed_defect":false,
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    Ok(Rule {
        schema_id: "star.rule".to_owned(),
        schema_version: 1,
        rule_id: STRUCTURAL_CLONE_CANDIDATE_RULE_ID.to_owned(),
        rule_version: "1.0.0".to_owned(),
        definition_fingerprint,
        title: "Exact structural clone candidate".to_owned(),
        category: "code-quality".to_owned(),
        default_severity: Severity::Info,
        default_confidence: Confidence::Medium,
        supported_languages: vec!["rust".to_owned()],
        source_kinds: vec![SourceKind::File],
        analyzer_ref: "builtin.structural-clone.exact-token.v1".to_owned(),
        parameter_schema_ref: "star.rule.structural-clone-candidate.parameters.v1".to_owned(),
        identity_contract_version: 1,
        identity_anchor: "normalized-token-fingerprint-and-owner-set".to_owned(),
        redaction_contract_version: 1,
        remediation_recipe_refs: Vec::new(),
        lifecycle: RuleLifecycle::Active,
    })
}

pub struct FindingProjection {
    pub findings: Vec<Finding>,
    pub occurrences: Vec<Occurrence>,
    pub rule_set_fingerprint: Sha256Hash,
}

/// Projects safe SARIF candidates into the existing source-derived Finding
/// model. A caller must supply the current source-entry generation; paths are
/// matched only within that generation and provider correlation text is never
/// persisted directly.
pub fn project_sarif_findings(
    project_id: &ProjectId,
    project_revision_id: &star_contracts::ids::ProjectRevisionId,
    workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    scan_run_id: &ScanRunId,
    source_entries: &[SourceEntry],
    candidates: &[sarif::SarifFindingCandidate],
    evidence_refs: &[star_contracts::evidence::ArtifactRef],
) -> Result<FindingProjection, ValidationError> {
    let sources_by_path = source_entries
        .iter()
        .filter(|source| source.owner_project_id == *project_id && source.analysis_eligible)
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut projection = FindingProjection {
        findings: Vec::new(),
        occurrences: Vec::new(),
        rule_set_fingerprint: Sha256Hash::digest(b"star.sarif-2.1.0-projection.v1"),
    };
    for candidate in candidates {
        if candidate.locations.is_empty() {
            continue;
        }
        let correlation_hash = Sha256Hash::digest(candidate.correlation_key.as_bytes());
        let finding_fingerprint = versioned_fingerprint(
            "star.identity.finding",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "rule_id":candidate.rule_id,
                "rule_version":"sarif-2.1.0",
                "identity_contract_version":1,
                "identity_anchor":"sarif-correlation",
                "identity_tokens":[correlation_hash],
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        let finding_id = FindingId::from_fingerprint(&finding_fingerprint);
        let mut occurrence_ids = Vec::new();
        for location in &candidate.locations {
            if location.path.project_id != *project_id {
                return Err(ValidationError::InconsistentGraph);
            }
            let source = sources_by_path
                .get(location.path.path.as_str())
                .copied()
                .ok_or(ValidationError::InconsistentGraph)?;
            let range = SourceRange {
                start_line: location.start.line,
                start_column: location.start.column,
                end_line: location
                    .end
                    .as_ref()
                    .map_or(location.start.line, |end| end.line),
                end_column: location
                    .end
                    .as_ref()
                    .map_or(location.start.column, |end| end.column),
            };
            let occurrence_fingerprint = versioned_fingerprint(
                "star.identity.occurrence",
                1,
                &serde_json::json!({
                    "finding_id":finding_id,
                    "workspace_snapshot_id":workspace_snapshot_id,
                    "source_content_sha256":source.content_sha256,
                    "location_range":range,
                    "evidence_key":correlation_hash,
                }),
            )
            .map_err(|_| ValidationError::Fingerprint)?;
            let occurrence_id = OccurrenceId::from_fingerprint(&occurrence_fingerprint);
            occurrence_ids.push(occurrence_id.clone());
            projection.occurrences.push(Occurrence {
                schema_id: "star.occurrence".to_owned(),
                schema_version: 1,
                occurrence_id,
                occurrence_fingerprint,
                finding_id: finding_id.clone(),
                scan_run_id: scan_run_id.clone(),
                project_revision_id: project_revision_id.clone(),
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                canonical_source_id: source.canonical_source_id.clone(),
                source_content_sha256: source.content_sha256.clone(),
                location_path: source.path.clone(),
                location_range: range,
                symbol_id: None,
                message_parameters: BTreeMap::new(),
                evidence_refs: evidence_refs.to_vec(),
                observed_at: Utc::now(),
                redaction_state: RedactionState::Redacted,
            });
        }
        occurrence_ids.sort();
        occurrence_ids.dedup();
        let severity = match candidate.severity {
            star_contracts::evidence::DiagnosticSeverity::Info => Severity::Info,
            star_contracts::evidence::DiagnosticSeverity::Warning => Severity::Warning,
            star_contracts::evidence::DiagnosticSeverity::Error => Severity::Error,
            star_contracts::evidence::DiagnosticSeverity::Critical => Severity::Critical,
        };
        let content_fingerprint = versioned_fingerprint(
            "star.finding-content",
            1,
            &serde_json::json!({
                "finding_fingerprint":finding_fingerprint,
                "occurrence_ids":occurrence_ids,
                "severity":severity,
                "confidence":Confidence::Medium,
                "provider":"sarif-2.1.0",
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        projection.findings.push(Finding {
            schema_id: "star.finding".to_owned(),
            schema_version: 1,
            finding_id,
            finding_fingerprint,
            project_id: project_id.clone(),
            rule_id: candidate.rule_id.clone(),
            rule_version: "sarif-2.1.0".to_owned(),
            identity_anchor: "sarif-correlation".to_owned(),
            identity_tokens: vec![correlation_hash.to_string()],
            title_code: "EXTERNAL_STATIC_ANALYSIS_TITLE".to_owned(),
            message_code: "EXTERNAL_STATIC_ANALYSIS".to_owned(),
            severity,
            confidence: Confidence::Medium,
            lifecycle: FindingLifecycle::Open,
            first_observed_scan_id: scan_run_id.clone(),
            last_observed_scan_id: scan_run_id.clone(),
            current_occurrence_ids: occurrence_ids,
            active_disposition_id: None,
            active_suppression_ids: Vec::new(),
            content_fingerprint,
        });
    }
    projection
        .findings
        .sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    projection
        .occurrences
        .sort_by(|left, right| left.occurrence_id.cmp(&right.occurrence_id));
    Ok(projection)
}

#[allow(clippy::too_many_arguments)]
pub fn analyze_builtin_findings(
    project_id: &ProjectId,
    revision: &ProjectRevision,
    workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    scan_run_id: &ScanRunId,
    files: &[FileObservation],
    sources: &[CanonicalSource],
    symbols: &[Symbol],
    hardcoding_candidates: &[HardcodingCandidate],
    structural_clone_candidates: &[StructuralCloneCandidate],
) -> Result<FindingProjection, ValidationError> {
    let mut projection = analyze_trailing_whitespace(
        project_id,
        revision,
        workspace_snapshot_id,
        scan_run_id,
        files,
        sources,
        symbols,
    )?;
    let hardcoding_rule = hardcoding_candidate_rule()?;
    let structural_clone_rule = structural_clone_candidate_rule()?;
    let trailing_rule = trailing_whitespace_rule()?;
    let symbol_by_source: BTreeMap<_, _> = symbols
        .iter()
        .map(|symbol| (symbol.canonical_source_id.clone(), symbol))
        .collect();
    for candidate in hardcoding_candidates {
        if candidate.rule_id != HARDCODING_CANDIDATE_RULE_ID || candidate.rule_version != "1.0.0" {
            return Err(ValidationError::InconsistentGraph);
        }
        let finding_fingerprint = versioned_fingerprint(
            "star.identity.finding",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "rule_id":candidate.rule_id,
                "identity_contract_version":1,
                "identity_anchor":candidate.canonical_source_id,
                "identity_tokens":[candidate.candidate_key.clone()],
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        let finding_id = FindingId::from_fingerprint(&finding_fingerprint);
        let occurrence_fingerprint = versioned_fingerprint(
            "star.identity.occurrence",
            1,
            &serde_json::json!({
                "finding_id":finding_id,
                "workspace_snapshot_id":workspace_snapshot_id,
                "source_content_sha256":candidate.source_content_sha256,
                "location_range":candidate.source_range,
                "evidence_key":candidate.candidate_key,
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        let occurrence_id = OccurrenceId::from_fingerprint(&occurrence_fingerprint);
        let mut message_parameters = BTreeMap::from([
            ("assessment".to_owned(), enum_label(&candidate.assessment)?),
            ("category".to_owned(), enum_label(&candidate.category)?),
            ("confidence".to_owned(), candidate.confidence.clone()),
            ("length_bucket".to_owned(), candidate.length_bucket.clone()),
            ("literal_shape".to_owned(), candidate.literal_shape.clone()),
            (
                "matched_predicate".to_owned(),
                candidate.matched_predicate.clone(),
            ),
        ]);
        message_parameters.insert(
            "false_positive_guards".to_owned(),
            candidate.false_positive_guards.join(","),
        );
        projection.occurrences.push(Occurrence {
            schema_id: "star.occurrence".to_owned(),
            schema_version: 1,
            occurrence_id: occurrence_id.clone(),
            occurrence_fingerprint,
            finding_id: finding_id.clone(),
            scan_run_id: scan_run_id.clone(),
            project_revision_id: revision.project_revision_id.clone(),
            workspace_snapshot_id: workspace_snapshot_id.clone(),
            canonical_source_id: candidate.canonical_source_id.clone(),
            source_content_sha256: candidate.source_content_sha256.clone(),
            location_path: candidate.source_ref.clone(),
            location_range: candidate.source_range.clone(),
            symbol_id: symbol_by_source
                .get(&candidate.canonical_source_id)
                .map(|symbol| symbol.symbol_id.clone()),
            message_parameters,
            evidence_refs: Vec::new(),
            observed_at: Utc::now(),
            redaction_state: RedactionState::Redacted,
        });
        let severity = if candidate.assessment == HardcodingAssessment::Warning {
            Severity::Warning
        } else {
            Severity::Info
        };
        let confidence = match candidate.confidence.as_str() {
            "high" => Confidence::High,
            "low" => Confidence::Low,
            _ => Confidence::Medium,
        };
        let content_fingerprint = versioned_fingerprint(
            "star.finding-content",
            1,
            &serde_json::json!({
                "finding_fingerprint":finding_fingerprint,
                "occurrence_ids":[occurrence_id.clone()],
                "severity":severity,
                "confidence":confidence,
                "assessment":candidate.assessment,
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        projection.findings.push(Finding {
            schema_id: "star.finding".to_owned(),
            schema_version: 1,
            finding_id,
            finding_fingerprint,
            project_id: project_id.clone(),
            rule_id: HARDCODING_CANDIDATE_RULE_ID.to_owned(),
            rule_version: "1.0.0".to_owned(),
            identity_anchor: candidate.canonical_source_id.as_str().to_owned(),
            identity_tokens: vec![candidate.candidate_key.clone()],
            title_code: "HARDCODING_CANDIDATE_TITLE".to_owned(),
            message_code: "HARDCODING_CANDIDATE".to_owned(),
            severity,
            confidence,
            lifecycle: FindingLifecycle::Open,
            first_observed_scan_id: scan_run_id.clone(),
            last_observed_scan_id: scan_run_id.clone(),
            current_occurrence_ids: vec![occurrence_id],
            active_disposition_id: None,
            active_suppression_ids: Vec::new(),
            content_fingerprint,
        });
    }
    append_structural_clone_findings(
        &mut projection,
        project_id,
        revision,
        workspace_snapshot_id,
        scan_run_id,
        symbols,
        structural_clone_candidates,
    )?;
    projection
        .findings
        .sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    projection
        .occurrences
        .sort_by(|left, right| left.occurrence_id.cmp(&right.occurrence_id));
    projection.rule_set_fingerprint = versioned_fingerprint(
        "star.rule-set",
        1,
        &serde_json::json!([
            {
                "rule_id":trailing_rule.rule_id,
                "rule_version":trailing_rule.rule_version,
                "definition_fingerprint":trailing_rule.definition_fingerprint,
            },
            {
                "rule_id":hardcoding_rule.rule_id,
                "rule_version":hardcoding_rule.rule_version,
                "definition_fingerprint":hardcoding_rule.definition_fingerprint,
            },
            {
                "rule_id":structural_clone_rule.rule_id,
                "rule_version":structural_clone_rule.rule_version,
                "definition_fingerprint":structural_clone_rule.definition_fingerprint,
            }
        ]),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    Ok(projection)
}

#[allow(clippy::too_many_arguments)]
fn append_structural_clone_findings(
    projection: &mut FindingProjection,
    project_id: &ProjectId,
    revision: &ProjectRevision,
    workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    scan_run_id: &ScanRunId,
    symbols: &[Symbol],
    candidates: &[StructuralCloneCandidate],
) -> Result<(), ValidationError> {
    let mut groups = BTreeMap::<(String, String, String), Vec<&StructuralCloneCandidate>>::new();
    for candidate in candidates {
        if candidate.language_id != "rust"
            || !matches!(
                candidate.source_class,
                SourceClass::Source | SourceClass::Test
            )
            || candidate.normalized_token_count < 12
            || candidate.owning_symbol_identity.trim().is_empty()
            || candidate.redaction_state
                != star_contracts::index::HardcodingRedactionState::ShapeOnly
        {
            return Err(ValidationError::InconsistentGraph);
        }
        let cohort = enum_label(&candidate.source_class)?;
        groups
            .entry((
                cohort,
                candidate.structural_kind.clone(),
                candidate.normalized_token_fingerprint.to_string(),
            ))
            .or_default()
            .push(candidate);
    }
    for ((cohort, structural_kind, normalized_token_fingerprint), mut group) in groups {
        if group.len() < 2 {
            continue;
        }
        group.sort_by(|left, right| left.candidate_key.cmp(&right.candidate_key));
        let mut owners = group
            .iter()
            .map(|candidate| candidate.owning_symbol_identity.clone())
            .collect::<Vec<_>>();
        owners.sort();
        let identity_tokens = vec![
            normalized_token_fingerprint.clone(),
            structural_kind.clone(),
            cohort.clone(),
            owners.join(","),
        ];
        let finding_fingerprint = versioned_fingerprint(
            "star.identity.finding",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "rule_id":STRUCTURAL_CLONE_CANDIDATE_RULE_ID,
                "identity_contract_version":1,
                "identity_anchor":"normalized-token-fingerprint-and-owner-set",
                "identity_tokens":identity_tokens,
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        let finding_id = FindingId::from_fingerprint(&finding_fingerprint);
        let mut occurrence_ids = Vec::with_capacity(group.len());
        for candidate in &group {
            let occurrence_fingerprint = versioned_fingerprint(
                "star.identity.occurrence",
                1,
                &serde_json::json!({
                    "finding_id":finding_id,
                    "workspace_snapshot_id":workspace_snapshot_id,
                    "source_content_sha256":candidate.source_content_sha256,
                    "location_range":candidate.source_range,
                    "evidence_key":candidate.candidate_key,
                }),
            )
            .map_err(|_| ValidationError::Fingerprint)?;
            let occurrence_id = OccurrenceId::from_fingerprint(&occurrence_fingerprint);
            occurrence_ids.push(occurrence_id.clone());
            projection.occurrences.push(Occurrence {
                schema_id: "star.occurrence".to_owned(),
                schema_version: 1,
                occurrence_id,
                occurrence_fingerprint,
                finding_id: finding_id.clone(),
                scan_run_id: scan_run_id.clone(),
                project_revision_id: revision.project_revision_id.clone(),
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                canonical_source_id: candidate.canonical_source_id.clone(),
                source_content_sha256: candidate.source_content_sha256.clone(),
                location_path: candidate.source_ref.clone(),
                location_range: candidate.source_range.clone(),
                symbol_id: symbols
                    .iter()
                    .find(|symbol| {
                        symbol.canonical_source_id == candidate.canonical_source_id
                            && symbol.qualified_name == candidate.owning_symbol_identity
                    })
                    .map(|symbol| symbol.symbol_id.clone()),
                message_parameters: BTreeMap::from([
                    ("clone_count".to_owned(), group.len().to_string()),
                    ("cohort".to_owned(), cohort.clone()),
                    ("structural_kind".to_owned(), structural_kind.clone()),
                    (
                        "normalized_token_count".to_owned(),
                        candidate.normalized_token_count.to_string(),
                    ),
                ]),
                evidence_refs: Vec::new(),
                observed_at: Utc::now(),
                redaction_state: RedactionState::Redacted,
            });
        }
        let content_fingerprint = versioned_fingerprint(
            "star.finding-content",
            1,
            &serde_json::json!({
                "finding_fingerprint":finding_fingerprint,
                "occurrence_ids":occurrence_ids,
                "severity":Severity::Info,
                "confidence":Confidence::Medium,
                "automatic_confirmed_defect":false,
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        projection.findings.push(Finding {
            schema_id: "star.finding".to_owned(),
            schema_version: 1,
            finding_id,
            finding_fingerprint,
            project_id: project_id.clone(),
            rule_id: STRUCTURAL_CLONE_CANDIDATE_RULE_ID.to_owned(),
            rule_version: "1.0.0".to_owned(),
            identity_anchor: "normalized-token-fingerprint-and-owner-set".to_owned(),
            identity_tokens,
            title_code: "STRUCTURAL_CLONE_CANDIDATE_TITLE".to_owned(),
            message_code: "STRUCTURAL_CLONE_CANDIDATE".to_owned(),
            severity: Severity::Info,
            confidence: Confidence::Medium,
            lifecycle: FindingLifecycle::Open,
            first_observed_scan_id: scan_run_id.clone(),
            last_observed_scan_id: scan_run_id.clone(),
            current_occurrence_ids: occurrence_ids,
            active_disposition_id: None,
            active_suppression_ids: Vec::new(),
            content_fingerprint,
        });
    }
    Ok(())
}

fn enum_label(value: &impl serde::Serialize) -> Result<String, ValidationError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(ValidationError::InconsistentGraph)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionEvaluation {
    pub baseline_ids: Vec<BaselineId>,
    pub suppression_ids: Vec<SuppressionId>,
    pub disposition_ids: Vec<DispositionId>,
    pub suppression_ids_by_finding: BTreeMap<FindingId, Vec<SuppressionId>>,
    pub disposition_id_by_finding: BTreeMap<FindingId, DispositionId>,
    pub stale_decision_refs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_decisions(
    project_id: &ProjectId,
    project_revision: &star_contracts::ids::ProjectRevisionId,
    scan_config_fingerprint: &Sha256Hash,
    rule_set_fingerprint: &Sha256Hash,
    findings: &[Finding],
    occurrences: &[Occurrence],
    baselines: &[Baseline],
    suppressions: &[Suppression],
    dispositions: &[Disposition],
    now: DateTime<Utc>,
) -> DecisionEvaluation {
    let mut evaluation = DecisionEvaluation::default();
    for baseline in baselines {
        let applicable = baseline.project_id == *project_id
            && baseline.status == BaselineStatus::Active
            && baseline.scan_config_fingerprint == *scan_config_fingerprint
            && baseline.rule_set_fingerprint == *rule_set_fingerprint
            && validate_baseline(baseline).is_ok();
        if applicable {
            evaluation.baseline_ids.push(baseline.baseline_id.clone());
        } else if baseline.status == BaselineStatus::Active {
            evaluation.stale_decision_refs.push(format!(
                "{}@{}",
                baseline.baseline_id.as_str(),
                baseline.revision
            ));
        }
    }
    for suppression in suppressions {
        let constraints_match = suppression.project_id == *project_id
            && suppression.status == SuppressionStatus::Active
            && suppression.expires_at.is_none_or(|expiry| expiry > now)
            && suppression
                .source_revision_constraint
                .as_ref()
                .is_none_or(|revision| revision == project_revision)
            && suppression
                .config_fingerprint_constraint
                .as_ref()
                .is_none_or(|fingerprint| fingerprint == scan_config_fingerprint)
            && validate_suppression(suppression).is_ok();
        if !constraints_match {
            if suppression.status == SuppressionStatus::Active {
                evaluation.stale_decision_refs.push(format!(
                    "{}@{}",
                    suppression.suppression_id.as_str(),
                    suppression.revision
                ));
            }
            continue;
        }
        let mut matched = false;
        for finding in findings {
            if suppression_matches(suppression, finding, occurrences) {
                evaluation
                    .suppression_ids_by_finding
                    .entry(finding.finding_id.clone())
                    .or_default()
                    .push(suppression.suppression_id.clone());
                matched = true;
            }
        }
        if matched {
            evaluation
                .suppression_ids
                .push(suppression.suppression_id.clone());
        }
    }
    let finding_by_id: BTreeMap<_, _> = findings
        .iter()
        .map(|finding| (finding.finding_id.clone(), finding))
        .collect();
    for disposition in dispositions {
        let active = disposition.status == DispositionStatus::Active
            && disposition.expires_at.is_none_or(|expiry| expiry > now)
            && disposition
                .scope_revision
                .as_ref()
                .is_none_or(|revision| revision == project_revision)
            && finding_by_id
                .get(&disposition.finding_id)
                .is_some_and(|finding| {
                    finding.finding_fingerprint == disposition.finding_fingerprint
                });
        if active {
            evaluation.disposition_id_by_finding.insert(
                disposition.finding_id.clone(),
                disposition.disposition_id.clone(),
            );
            evaluation
                .disposition_ids
                .push(disposition.disposition_id.clone());
        } else if disposition.status == DispositionStatus::Active {
            evaluation.stale_decision_refs.push(format!(
                "{}@{}",
                disposition.disposition_id.as_str(),
                disposition.revision
            ));
        }
    }
    evaluation.baseline_ids.sort();
    evaluation.baseline_ids.dedup();
    evaluation.suppression_ids.sort();
    evaluation.suppression_ids.dedup();
    evaluation.disposition_ids.sort();
    evaluation.disposition_ids.dedup();
    for ids in evaluation.suppression_ids_by_finding.values_mut() {
        ids.sort();
        ids.dedup();
    }
    evaluation.stale_decision_refs.sort();
    evaluation.stale_decision_refs.dedup();
    evaluation
}

pub fn apply_decision_projection(findings: &mut [Finding], evaluation: &DecisionEvaluation) {
    for finding in findings {
        finding.active_suppression_ids = evaluation
            .suppression_ids_by_finding
            .get(&finding.finding_id)
            .cloned()
            .unwrap_or_default();
        finding.active_disposition_id = evaluation
            .disposition_id_by_finding
            .get(&finding.finding_id)
            .cloned();
    }
}

fn suppression_matches(
    suppression: &Suppression,
    finding: &Finding,
    occurrences: &[Occurrence],
) -> bool {
    let Some((kind, value)) = suppression.selector.split_once(':') else {
        return false;
    };
    match kind {
        "finding" => value == finding.finding_fingerprint.as_str(),
        "rule" => value == finding.rule_id,
        "symbol" => value == finding.identity_anchor,
        "path" => occurrences.iter().any(|occurrence| {
            occurrence.finding_id == finding.finding_id
                && project_glob_matches(value, occurrence.location_path.as_str())
        }),
        _ => false,
    }
}

fn project_glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut memo = vec![vec![None; value.len() + 1]; pattern.len() + 1];
    fn matches(
        pattern: &[u8],
        value: &[u8],
        pattern_index: usize,
        value_index: usize,
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(result) = memo[pattern_index][value_index] {
            return result;
        }
        let result = if pattern_index == pattern.len() {
            value_index == value.len()
        } else if pattern[pattern_index] == b'*' {
            let recursive = pattern.get(pattern_index + 1) == Some(&b'*');
            let next_pattern = pattern_index + if recursive { 2 } else { 1 };
            matches(pattern, value, next_pattern, value_index, memo)
                || (value_index < value.len()
                    && (recursive || value[value_index] != b'/')
                    && matches(pattern, value, pattern_index, value_index + 1, memo))
        } else if value_index < value.len()
            && (pattern[pattern_index] == value[value_index]
                || (pattern[pattern_index] == b'?' && value[value_index] != b'/'))
        {
            matches(pattern, value, pattern_index + 1, value_index + 1, memo)
        } else {
            false
        };
        memo[pattern_index][value_index] = Some(result);
        result
    }
    matches(pattern, value, 0, 0, &mut memo)
}

pub fn analyze_trailing_whitespace(
    project_id: &ProjectId,
    revision: &ProjectRevision,
    workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    scan_run_id: &ScanRunId,
    files: &[FileObservation],
    sources: &[CanonicalSource],
    symbols: &[Symbol],
) -> Result<FindingProjection, ValidationError> {
    let rule = trailing_whitespace_rule()?;
    let rule_set_fingerprint = versioned_fingerprint(
        "star.rule-set",
        1,
        &serde_json::json!([{
            "rule_id":rule.rule_id,
            "rule_version":rule.rule_version,
            "definition_fingerprint":rule.definition_fingerprint,
        }]),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    let source_by_path: BTreeMap<_, _> = sources
        .iter()
        .filter_map(|source| source.path.as_ref().map(|path| (path.clone(), source)))
        .collect();
    let symbol_by_source: BTreeMap<_, _> = symbols
        .iter()
        .map(|symbol| (symbol.canonical_source_id.clone(), symbol))
        .collect();
    let mut findings = Vec::new();
    let mut occurrences = Vec::new();
    for file in files {
        let Some(text) = file.text.as_deref() else {
            continue;
        };
        let source = source_by_path
            .get(&file.path)
            .ok_or(ValidationError::InconsistentGraph)?;
        let symbol = symbol_by_source
            .get(&source.canonical_source_id)
            .ok_or(ValidationError::InconsistentGraph)?;
        let finding_fingerprint = versioned_fingerprint(
            "star.identity.finding",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "rule_id":TRAILING_WHITESPACE_RULE_ID,
                "identity_contract_version":1,
                "identity_anchor":symbol.symbol_id,
                "identity_tokens":[],
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        let finding_id = FindingId::from_fingerprint(&finding_fingerprint);
        let mut current_occurrence_ids = Vec::new();
        for (index, line) in text.split_inclusive('\n').enumerate() {
            let without_newline = line.strip_suffix('\n').unwrap_or(line);
            let without_cr = without_newline
                .strip_suffix('\r')
                .unwrap_or(without_newline);
            let trimmed = without_cr.trim_end_matches([' ', '\t']);
            let trailing_bytes = without_cr.len() - trimmed.len();
            if trailing_bytes == 0 {
                continue;
            }
            let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            let start_column = u32::try_from(trimmed.chars().count() + 1).unwrap_or(u32::MAX);
            let end_column = u32::try_from(without_cr.chars().count() + 1).unwrap_or(u32::MAX);
            let occurrence_fingerprint = versioned_fingerprint(
                "star.identity.occurrence",
                1,
                &serde_json::json!({
                    "finding_id":finding_id,
                    "workspace_snapshot_id":workspace_snapshot_id,
                    "source_content_sha256":file.content_sha256,
                    "location_range":{
                        "start_line":line_number,
                        "start_column":start_column,
                        "end_line":line_number,
                        "end_column":end_column,
                    },
                    "evidence_key":"trailing-whitespace",
                }),
            )
            .map_err(|_| ValidationError::Fingerprint)?;
            let occurrence_id = OccurrenceId::from_fingerprint(&occurrence_fingerprint);
            current_occurrence_ids.push(occurrence_id.clone());
            occurrences.push(Occurrence {
                schema_id: "star.occurrence".to_owned(),
                schema_version: 1,
                occurrence_id,
                occurrence_fingerprint,
                finding_id: finding_id.clone(),
                scan_run_id: scan_run_id.clone(),
                project_revision_id: revision.project_revision_id.clone(),
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                canonical_source_id: source.canonical_source_id.clone(),
                source_content_sha256: file.content_sha256.clone(),
                location_path: file.path.clone(),
                location_range: SourceRange {
                    start_line: line_number,
                    start_column,
                    end_line: line_number,
                    end_column,
                },
                symbol_id: Some(symbol.symbol_id.clone()),
                message_parameters: BTreeMap::from([(
                    "trailing_byte_count".to_owned(),
                    trailing_bytes.to_string(),
                )]),
                evidence_refs: vec![],
                observed_at: Utc::now(),
                redaction_state: RedactionState::NotNeeded,
            });
        }
        if current_occurrence_ids.is_empty() {
            continue;
        }
        let content_fingerprint = versioned_fingerprint(
            "star.finding-content",
            1,
            &serde_json::json!({
                "finding_fingerprint":finding_fingerprint,
                "occurrence_ids":current_occurrence_ids,
                "severity":"warning",
                "confidence":"high",
            }),
        )
        .map_err(|_| ValidationError::Fingerprint)?;
        findings.push(Finding {
            schema_id: "star.finding".to_owned(),
            schema_version: 1,
            finding_id,
            finding_fingerprint,
            project_id: project_id.clone(),
            rule_id: TRAILING_WHITESPACE_RULE_ID.to_owned(),
            rule_version: "1.0.0".to_owned(),
            identity_anchor: symbol.symbol_id.as_str().to_owned(),
            identity_tokens: vec![],
            title_code: "TRAILING_WHITESPACE_TITLE".to_owned(),
            message_code: "TRAILING_WHITESPACE".to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::High,
            lifecycle: FindingLifecycle::Open,
            first_observed_scan_id: scan_run_id.clone(),
            last_observed_scan_id: scan_run_id.clone(),
            current_occurrence_ids,
            active_disposition_id: None,
            active_suppression_ids: vec![],
            content_fingerprint,
        });
    }
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    occurrences.sort_by(|left, right| left.occurrence_id.cmp(&right.occurrence_id));
    Ok(FindingProjection {
        findings,
        occurrences,
        rule_set_fingerprint,
    })
}

pub fn validate_patch_result(
    patch_set: &PatchSet,
    scan_run: &ScanRun,
    current_findings: &[Finding],
    decisions: &DecisionEvaluation,
) -> Result<(ValidationResult, GateDecision), ValidationError> {
    validate_patch_result_with_plan(
        patch_set,
        scan_run,
        current_findings,
        decisions,
        "star.validation.trailing-whitespace.v1",
    )
}

pub fn validate_patch_result_with_plan(
    patch_set: &PatchSet,
    scan_run: &ScanRun,
    current_findings: &[Finding],
    decisions: &DecisionEvaluation,
    validation_plan_ref: &str,
) -> Result<(ValidationResult, GateDecision), ValidationError> {
    if validation_plan_ref.trim().is_empty() {
        return Err(ValidationError::InconsistentGraph);
    }
    let started_at = Utc::now();
    let unresolved: Vec<_> = patch_set
        .affected_finding_ids
        .iter()
        .filter(|finding_id| {
            current_findings
                .iter()
                .any(|finding| &finding.finding_id == *finding_id)
        })
        .cloned()
        .collect();
    let complete = scan_run.status == ScanStatus::Succeeded;
    let outcome = if !complete {
        ValidationOutcome::Incomplete
    } else if unresolved.is_empty() {
        ValidationOutcome::Pass
    } else {
        ValidationOutcome::Fail
    };
    let completeness = if complete {
        Completeness::Complete
    } else {
        Completeness::Partial
    };
    let result_fingerprint = versioned_fingerprint(
        "star.validation-result",
        1,
        &serde_json::json!({
            "patch_set_id":patch_set.patch_set_id,
            "scan_run_id":scan_run.scan_run_id,
            "validation_plan_ref":validation_plan_ref,
            "outcome":outcome,
            "unresolved":unresolved,
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    let validation_result_id = ValidationResultId::new();
    let result = ValidationResult {
        schema_id: "star.validation-result".to_owned(),
        schema_version: 1,
        validation_result_id: validation_result_id.clone(),
        subject_kind: "patch_set".to_owned(),
        subject_id: patch_set.patch_set_id.as_str().to_owned(),
        project_id: patch_set.project_id.clone(),
        project_revision_id: scan_run.project_revision_id.clone(),
        workspace_snapshot_id: scan_run.workspace_snapshot_id.clone(),
        validation_plan_ref: validation_plan_ref.to_owned(),
        validation_run_refs: vec![scan_run.scan_run_id.as_str().to_owned()],
        effective_config_fingerprint: scan_run.effective_config_fingerprint.clone(),
        outcome,
        completeness,
        finding_refs: unresolved.clone(),
        diagnostic_refs: vec![],
        artifact_refs: scan_run.artifact_refs.clone(),
        result_fingerprint: result_fingerprint.clone(),
        started_at,
        finished_at: Utc::now(),
    };
    let gate_decision = match result.outcome {
        ValidationOutcome::Pass if result.completeness == Completeness::Complete => {
            GateDecisionKind::AutoPass
        }
        ValidationOutcome::Incomplete => GateDecisionKind::Block,
        _ => GateDecisionKind::Block,
    };
    let policy_fingerprint = versioned_fingerprint(
        "star.gate-policy-input",
        1,
        &serde_json::json!({
            "effective_config_fingerprint":scan_run.effective_config_fingerprint,
            "baseline_ids":decisions.baseline_ids,
            "suppression_ids":decisions.suppression_ids,
            "disposition_ids":decisions.disposition_ids,
            "stale_decision_refs":decisions.stale_decision_refs,
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    let gate_fingerprint = versioned_fingerprint(
        "star.gate-decision",
        1,
        &serde_json::json!({
            "subject":patch_set.patch_fingerprint,
            "validation_result":result_fingerprint,
            "unresolved":unresolved,
            "outcome":gate_decision,
            "policy_fingerprint":policy_fingerprint,
        }),
    )
    .map_err(|_| ValidationError::Fingerprint)?;
    let reason_codes = {
        let mut reasons = vec![
            match gate_decision {
                GateDecisionKind::AutoPass => "REQUIRED_VALIDATION_PASSED",
                GateDecisionKind::HumanReview => "HUMAN_REVIEW_REQUIRED",
                GateDecisionKind::Block => "REQUIRED_VALIDATION_BLOCKED",
            }
            .to_owned(),
        ];
        if !decisions.stale_decision_refs.is_empty() {
            reasons.push("STALE_DECISION_IGNORED".to_owned());
        }
        reasons
    };
    let decided_at = Utc::now();
    let management_extension = serde_json::json!({
        "subject_kind":"patch_set",
        "subject_id":patch_set.patch_set_id,
        "project_revision_id":scan_run.project_revision_id,
        "workspace_snapshot_id":scan_run.workspace_snapshot_id,
        "subject_fingerprint":patch_set.patch_fingerprint,
        "baseline_ids":decisions.baseline_ids,
        "suppression_ids":decisions.suppression_ids,
        "disposition_ids":decisions.disposition_ids,
        "validation_result_ids":[validation_result_id],
        "unresolved_finding_ids":unresolved,
        "policy_fingerprint":policy_fingerprint,
        "effective_config_fingerprint":scan_run.effective_config_fingerprint,
        "reason_codes":reason_codes,
    });
    let decision = GateDecision {
        schema_id: GateDecisionSchemaId::GateDecision,
        schema_version: 1,
        gate_id: GateId::from_stable_bytes(gate_fingerprint.as_str().as_bytes()),
        revision: 1,
        created_at: decided_at,
        updated_at: decided_at,
        producer: ProducerRef {
            component: "star-validation".to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: option_env!("STAR_CONTROL_BUILD_ID")
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_owned(),
            platform: std::env::consts::OS.to_owned(),
        },
        extensions: BTreeMap::from([("star.management".to_owned(), management_extension)]),
        scope: GateScope::Merge {
            project_id: patch_set.project_id.clone(),
            revision: patch_set.change_plan_revision,
        },
        decision: gate_decision,
        required_run_refs: vec![],
        satisfied_run_refs: vec![],
        blocking_diagnostic_refs: vec![],
        waivers: vec![],
        omissions: vec![],
        remaining_risks: vec![],
        policy_snapshot: GatePolicySnapshot {
            policy_ref: DocumentRef {
                schema_id: "star.gate-policy-input".to_owned(),
                document_id: policy_fingerprint.as_str().to_owned(),
                revision: 1,
                sha256: policy_fingerprint.clone(),
            },
            policy_sha256: policy_fingerprint,
            thresholds: BTreeMap::from([(
                "effective_config_fingerprint".to_owned(),
                serde_json::Value::String(scan_run.effective_config_fingerprint.to_string()),
            )]),
        },
        decided_by: ActorRef {
            actor_type: ActorType::Controller,
            actor_id: "star-controller".to_owned(),
            display_name: "Star-Control Controller".to_owned(),
            auth_source: "current_user_controller".to_owned(),
        },
    };
    decision
        .validate_against(&[])
        .map_err(|_| ValidationError::InconsistentGraph)?;
    Ok((result, decision))
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        evidence::{
            ArtifactKind, ArtifactRef, DiagnosticSeverity, LocationRef, ProducerRef,
            ProjectPathKind, RedactionStatus, RetentionClass, TextPosition,
        },
        ids::{CanonicalSourceId, CheckoutId, ProjectRevisionId, SymbolId, WorkspaceSnapshotId},
        index::SourceClass,
        management::{ProjectPathRef, Sensitivity},
    };

    fn test_producer() -> ProducerRef {
        ProducerRef {
            component: "star-validation-test".to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: "test".to_owned(),
            platform: std::env::consts::OS.to_owned(),
        }
    }

    #[test]
    fn finding_persists_codes_and_counts_but_never_source_line() {
        let project_id = ProjectId::new();
        let revision_id = ProjectRevisionId::new();
        let snapshot_id = WorkspaceSnapshotId::new();
        let scan_id = ScanRunId::new();
        let source_id = CanonicalSourceId::new();
        let symbol_id = SymbolId::new();
        let path = ProjectPathRef::parse("src/lib.rs").unwrap();
        let hash = Sha256Hash::digest(b"token=do-not-persist  \n");
        let files = vec![FileObservation {
            path: path.clone(),
            content_sha256: hash.clone(),
            size_bytes: 23,
            text: Some("token=do-not-persist  \n".to_owned()),
            language_id: Some("rust".to_owned()),
            line_count: 1,
        }];
        let revision = ProjectRevision {
            schema_id: "star.project-revision".to_owned(),
            schema_version: 1,
            project_revision_id: revision_id.clone(),
            project_id: project_id.clone(),
            revision_kind: star_contracts::management::RevisionKind::FilesystemManifest,
            vcs_object_format: None,
            commit_id: None,
            tree_id: None,
            manifest_fingerprint: Some(hash.clone()),
            captured_at: Utc::now(),
            completeness: Completeness::Complete,
            limitations: vec![],
        };
        let artifact = ArtifactRef {
            artifact_id: star_contracts::ids::ArtifactId::new(),
            kind: ArtifactKind::Manifest,
            project_id: Some(project_id.clone()),
            relative_path: ".ai-runs/manifest.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            sha256: hash.clone(),
            created_at: Utc::now(),
            producer: test_producer(),
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Run,
            source_artifact_ref: None,
        };
        let snapshot = WorkspaceSnapshot {
            schema_id: "star.workspace-snapshot".to_owned(),
            schema_version: 1,
            workspace_snapshot_id: snapshot_id.clone(),
            project_id: project_id.clone(),
            project_revision_id: revision_id,
            scope: vec!["**/*".to_owned()],
            entries_manifest_ref: artifact,
            entries_fingerprint: hash.clone(),
            dirty_summary: BTreeMap::new(),
            ignored_policy: "exclude".to_owned(),
            symlink_policy: "do_not_follow".to_owned(),
            captured_at: Utc::now(),
            completeness: Completeness::Complete,
            limitations: vec![],
        };
        let sources = vec![CanonicalSource {
            schema_id: "star.canonical-source".to_owned(),
            schema_version: 1,
            canonical_source_id: source_id.clone(),
            project_id: project_id.clone(),
            path: Some(path),
            source_kind: SourceKind::File,
            language_id: Some("rust".to_owned()),
            content_sha256: Some(hash.clone()),
            project_revision_id: Some(revision.project_revision_id.clone()),
            workspace_snapshot_id: Some(snapshot_id.clone()),
            generated_from_refs: vec![],
            sensitivity: Sensitivity::Internal,
        }];
        let symbols = vec![Symbol {
            schema_id: "star.symbol".to_owned(),
            schema_version: 1,
            symbol_id,
            project_id: project_id.clone(),
            canonical_source_id: source_id,
            language_id: "rust".to_owned(),
            symbol_kind: "file".to_owned(),
            qualified_name: "src/lib.rs".to_owned(),
            signature_fingerprint: None,
            declaration_range: SourceRange {
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            },
            visibility: None,
            workspace_snapshot_id: snapshot_id,
            scan_run_id: scan_id.clone(),
            content_fingerprint: hash,
        }];
        let projection = analyze_trailing_whitespace(
            &project_id,
            &revision,
            &snapshot.workspace_snapshot_id,
            &scan_id,
            &files,
            &sources,
            &symbols,
        )
        .unwrap();
        let persisted =
            serde_json::to_string(&(projection.findings, projection.occurrences)).unwrap();
        assert!(persisted.contains("trailing_byte_count"));
        assert!(!persisted.contains("do-not-persist"));
    }

    #[test]
    fn structural_clone_findings_are_cohort_isolated_and_location_independent() {
        let project_id = ProjectId::new();
        let revision = ProjectRevision {
            schema_id: "star.project-revision".to_owned(),
            schema_version: 1,
            project_revision_id: ProjectRevisionId::new(),
            project_id: project_id.clone(),
            revision_kind: star_contracts::management::RevisionKind::FilesystemManifest,
            vcs_object_format: None,
            commit_id: None,
            tree_id: None,
            manifest_fingerprint: Some(Sha256Hash::digest(b"clone-revision")),
            captured_at: Utc::now(),
            completeness: Completeness::Complete,
            limitations: Vec::new(),
        };
        let normalized = Sha256Hash::digest(b"redacted-normalized-token-stream");
        let candidate = |source: &[u8], owner: &str, source_class: SourceClass, line: u32| {
            StructuralCloneCandidate {
                candidate_key: Sha256Hash::digest(source).to_string(),
                canonical_source_id: CanonicalSourceId::from_stable_bytes(source),
                source_ref: ProjectPathRef::parse(format!("src/{owner}.rs")).unwrap(),
                source_content_sha256: Sha256Hash::digest(source),
                source_range: SourceRange {
                    start_line: line,
                    start_column: 1,
                    end_line: line + 2,
                    end_column: 2,
                },
                source_class,
                language_id: "rust".to_owned(),
                structural_kind: "function_body".to_owned(),
                normalized_token_fingerprint: normalized.clone(),
                normalized_token_count: 16,
                owning_symbol_identity: owner.to_owned(),
                redaction_state: star_contracts::index::HardcodingRedactionState::ShapeOnly,
                limitations: Vec::new(),
                content_fingerprint: Sha256Hash::digest(format!("candidate:{owner}").as_bytes()),
            }
        };
        let production = vec![
            candidate(b"one", "one", SourceClass::Source, 4),
            candidate(b"two", "two", SourceClass::Source, 9),
        ];
        let first = analyze_builtin_findings(
            &project_id,
            &revision,
            &WorkspaceSnapshotId::new(),
            &ScanRunId::new(),
            &[],
            &[],
            &[],
            &[],
            &production,
        )
        .unwrap();
        let production_finding = first
            .findings
            .iter()
            .find(|finding| finding.rule_id == STRUCTURAL_CLONE_CANDIDATE_RULE_ID)
            .unwrap();
        assert_eq!(production_finding.severity, Severity::Info);
        assert_eq!(production_finding.current_occurrence_ids.len(), 2);
        assert!(
            !serde_json::to_string(&(&first.findings, &first.occurrences))
                .unwrap()
                .contains("token-stream")
        );

        let moved = production
            .iter()
            .cloned()
            .map(|mut item| {
                item.source_range.start_line += 100;
                item.source_range.end_line += 100;
                item
            })
            .collect::<Vec<_>>();
        let moved_projection = analyze_builtin_findings(
            &project_id,
            &revision,
            &WorkspaceSnapshotId::new(),
            &ScanRunId::new(),
            &[],
            &[],
            &[],
            &[],
            &moved,
        )
        .unwrap();
        let moved_finding = moved_projection
            .findings
            .iter()
            .find(|finding| finding.rule_id == STRUCTURAL_CLONE_CANDIDATE_RULE_ID)
            .unwrap();
        assert_eq!(production_finding.finding_id, moved_finding.finding_id);

        let mixed = vec![
            candidate(b"one", "one", SourceClass::Source, 4),
            candidate(b"test-one", "test_one", SourceClass::Test, 4),
        ];
        let mixed_projection = analyze_builtin_findings(
            &project_id,
            &revision,
            &WorkspaceSnapshotId::new(),
            &ScanRunId::new(),
            &[],
            &[],
            &[],
            &[],
            &mixed,
        )
        .unwrap();
        assert!(
            mixed_projection
                .findings
                .iter()
                .all(|finding| finding.rule_id != STRUCTURAL_CLONE_CANDIDATE_RULE_ID)
        );
    }

    #[test]
    fn sarif_projection_binds_only_current_source_entries_and_redacts_correlation() {
        let project_id = ProjectId::new();
        let revision_id = ProjectRevisionId::new();
        let workspace_id = WorkspaceSnapshotId::new();
        let scan_id = ScanRunId::new();
        let source = SourceEntry {
            canonical_source_id: CanonicalSourceId::new(),
            path: ProjectPathRef::parse("src/lib.rs").unwrap(),
            content_sha256: Sha256Hash::digest(b"current-source"),
            size_bytes: 10,
            source_class: SourceClass::Source,
            facets: vec![],
            language_id: "rust".to_owned(),
            encoding: "utf-8".to_owned(),
            owner_project_id: project_id.clone(),
            owner_checkout_id: CheckoutId::new(),
            analysis_eligible: true,
            content_fingerprint: Sha256Hash::digest(b"entry"),
        };
        let candidate = sarif::SarifFindingCandidate {
            rule_id: "fixture.rule".to_owned(),
            correlation_key: "provider-token=must-not-persist".to_owned(),
            severity: DiagnosticSeverity::Warning,
            locations: vec![LocationRef {
                path: star_contracts::evidence::ProjectPathRef {
                    project_id: project_id.clone(),
                    path: "src/lib.rs".to_owned(),
                    path_kind: ProjectPathKind::File,
                },
                start: TextPosition { line: 2, column: 3 },
                end: None,
                symbol: None,
            }],
        };
        let projection = project_sarif_findings(
            &project_id,
            &revision_id,
            &workspace_id,
            &scan_id,
            std::slice::from_ref(&source),
            std::slice::from_ref(&candidate),
            &[],
        )
        .unwrap();
        assert_eq!(projection.findings.len(), 1);
        assert_eq!(projection.occurrences.len(), 1);
        assert!(
            !serde_json::to_string(&projection.findings)
                .unwrap()
                .contains("must-not-persist")
        );
        assert!(
            !serde_json::to_string(&projection.occurrences)
                .unwrap()
                .contains("must-not-persist")
        );
        let mut stale = candidate;
        stale.locations[0].path.path = "src/stale.rs".to_owned();
        assert!(
            project_sarif_findings(
                &project_id,
                &revision_id,
                &workspace_id,
                &scan_id,
                &[source],
                &[stale],
                &[],
            )
            .is_err()
        );
    }
}
