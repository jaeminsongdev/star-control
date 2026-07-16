use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use star_contracts::{
    ArtifactId, EvidenceBundleId, GateId, GoalId, ProjectId, RunId, Sha256Hash, TaskInvocationId,
    ValidationPlanId, ValidationRunId,
    evidence::{
        ActorRef, ActorType, ArtifactKind, ArtifactManifest, ArtifactManifestEntry, ArtifactRef,
        AuthoritativeGateState, CatalogRef, ChangeEvidenceRefs, Completeness,
        ContractInvariantError, DocumentRef, EvidenceBundle, EvidenceBundleSchemaId, GateDecision,
        GateDecisionKind, GateDecisionRef, GateDecisionSchemaId, GatePolicySnapshot, GateScope,
        OutputLimits, ProducerRef, ProjectPathKind, ProjectPathRef, RedactionStatus,
        RetentionClass, TaskInvocation, TerminationReason, ValidationOutcome, ValidationRun,
        ValidationRunRef, ValidationRunSchemaId,
    },
    schema::generated_documents,
};

fn at(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

fn hash(label: &str) -> Sha256Hash {
    Sha256Hash::digest(label.as_bytes())
}

fn producer() -> ProducerRef {
    ProducerRef {
        component: "star-controller".to_owned(),
        product_version: "0.1.0".to_owned(),
        build_id: "test-build".to_owned(),
        platform: "windows-x86_64".to_owned(),
    }
}

fn actor() -> ActorRef {
    ActorRef {
        actor_type: ActorType::Controller,
        actor_id: "star-controller".to_owned(),
        display_name: "Star Controller".to_owned(),
        auth_source: "local_ipc".to_owned(),
    }
}

fn document(schema_id: &str, document_id: &str) -> DocumentRef {
    DocumentRef {
        schema_id: schema_id.to_owned(),
        document_id: document_id.to_owned(),
        revision: 3,
        sha256: hash(document_id),
    }
}

fn catalog(catalog_id: &str) -> CatalogRef {
    CatalogRef {
        catalog_id: catalog_id.to_owned(),
        format_version: 1,
        item_version: "1.0.0".to_owned(),
        sha256: hash(catalog_id),
    }
}

fn artifact(kind: ArtifactKind, relative_path: &str) -> ArtifactRef {
    ArtifactRef {
        artifact_id: ArtifactId::new(),
        kind,
        project_id: Some(ProjectId::new()),
        relative_path: relative_path.to_owned(),
        media_type: "application/json".to_owned(),
        size_bytes: 128,
        sha256: hash(relative_path),
        created_at: at("2026-07-12T00:00:00Z"),
        producer: producer(),
        redaction_status: RedactionStatus::NotNeeded,
        retention_class: RetentionClass::Evidence,
        source_artifact_ref: None,
    }
}

fn invocation() -> TaskInvocation {
    TaskInvocation {
        invocation_id: TaskInvocationId::new(),
        tool_ref: catalog("tool.cargo"),
        executable: "cargo.exe".to_owned(),
        args: vec!["test".to_owned(), "--locked".to_owned()],
        cwd: ProjectPathRef {
            project_id: ProjectId::new(),
            path: ".".to_owned(),
            path_kind: ProjectPathKind::Directory,
        },
        env_refs: BTreeMap::new(),
        stdin_ref: None,
        timeout_ms: 60_000,
        permission_action: "process_run".to_owned(),
        idempotency_key: "validation:test".to_owned(),
        expected_exit_codes: BTreeSet::from([0]),
        output_limits: OutputLimits {
            stdout_bytes: 1_048_576,
            stderr_bytes: 1_048_576,
            artifact_bytes: 8_388_608,
        },
    }
}

fn passing_run() -> ValidationRun {
    ValidationRun {
        schema_id: ValidationRunSchemaId::ValidationRun,
        schema_version: 1,
        validation_run_id: ValidationRunId::new(),
        revision: 1,
        created_at: at("2026-07-12T00:00:00Z"),
        updated_at: at("2026-07-12T00:00:02Z"),
        producer: producer(),
        extensions: BTreeMap::new(),
        validation_plan_ref: document("star.validation-plan", "plan-1"),
        check_ref: catalog("check.cargo-test"),
        tool_ref: catalog("tool.cargo"),
        attempt: 1,
        invocation: invocation(),
        started_at: Some(at("2026-07-12T00:00:00Z")),
        finished_at: Some(at("2026-07-12T00:00:02Z")),
        outcome: ValidationOutcome::Pass,
        completeness: Completeness::Complete,
        exit_code: Some(0),
        termination_reason: Some(TerminationReason::Exited),
        diagnostic_refs: Vec::new(),
        stdout_ref: Some(artifact(ArtifactKind::Log, "runs/validation/stdout.log")),
        stderr_ref: None,
        result_artifact_refs: vec![artifact(
            ArtifactKind::Report,
            "runs/validation/report.json",
        )],
        observed_tool: None,
        cache: None,
    }
}

fn run_ref(run: &ValidationRun) -> ValidationRunRef {
    ValidationRunRef {
        validation_run_id: run.validation_run_id.clone(),
        revision: run.revision,
        sha256: hash(run.validation_run_id.as_str()),
    }
}

fn gate(decision: GateDecisionKind, required: Vec<ValidationRunRef>) -> GateDecision {
    GateDecision {
        schema_id: GateDecisionSchemaId::GateDecision,
        schema_version: 1,
        gate_id: GateId::new(),
        revision: 1,
        created_at: at("2026-07-12T00:00:03Z"),
        updated_at: at("2026-07-12T00:00:03Z"),
        producer: producer(),
        extensions: BTreeMap::new(),
        scope: GateScope::Goal {
            goal_id: GoalId::new(),
            run_id: RunId::new(),
            revision: 3,
        },
        decision,
        required_run_refs: required.clone(),
        satisfied_run_refs: required,
        blocking_diagnostic_refs: Vec::new(),
        waivers: Vec::new(),
        omissions: Vec::new(),
        remaining_risks: Vec::new(),
        policy_snapshot: GatePolicySnapshot {
            policy_ref: document("star.gate-policy", "policy-1"),
            policy_sha256: hash("policy-1"),
            thresholds: BTreeMap::new(),
        },
        decided_by: actor(),
    }
}

#[test]
fn schema_ids_are_exact_singletons_and_wrong_values_are_rejected() {
    let run = passing_run();
    let mut json = serde_json::to_value(&run).unwrap();
    assert_eq!(json["schema_id"], "star.validation-run");
    json["schema_id"] = serde_json::json!("adapter.validation-result");
    assert!(serde_json::from_value::<ValidationRun>(json).is_err());

    let ids: BTreeMap<_, _> = generated_documents()
        .into_iter()
        .map(|(name, value)| (name, value["$id"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(
        ids["validation-plan.schema.json"],
        "urn:star-control:schema:star.validation-plan:v1"
    );
    assert_eq!(
        ids["validation-run.schema.json"],
        "urn:star-control:schema:star.validation-run:v1"
    );
    assert_eq!(
        ids["gate-decision.schema.json"],
        "urn:star-control:schema:star.gate-decision:v1"
    );
    assert_eq!(
        ids["evidence-bundle.schema.json"],
        "urn:star-control:schema:star.evidence-bundle:v1"
    );
    assert_eq!(
        ids["diagnostic.schema.json"],
        "urn:star-control:schema:star.diagnostic:v1"
    );
}

#[test]
fn typed_ids_reject_a_different_contract_prefix_during_deserialization() {
    let validation_plan_id = ValidationPlanId::new();
    let plan_suffix = &validation_plan_id.as_str()[4..];
    assert!(serde_json::from_str::<ValidationPlanId>(&format!("\"val_{plan_suffix}\"")).is_err());
    let validation_run_id = ValidationRunId::new();
    let suffix = &validation_run_id.as_str()[4..];
    let wrong = format!("\"req_{suffix}\"");
    assert!(serde_json::from_str::<ValidationRunId>(&wrong).is_err());
    assert!(serde_json::from_str::<Sha256Hash>("\"sha256:not-a-digest\"").is_err());
}

#[test]
fn not_run_never_satisfies_a_required_check_or_auto_pass_gate() {
    let mut not_run = passing_run();
    not_run.outcome = ValidationOutcome::NotRun;
    not_run.started_at = None;
    not_run.finished_at = None;
    not_run.exit_code = None;
    not_run.termination_reason = None;
    not_run.observed_tool = None;

    assert_eq!(not_run.validate(), Ok(()));
    assert!(!not_run.satisfies_required_check());

    let decision = gate(GateDecisionKind::AutoPass, vec![run_ref(&not_run)]);
    assert_eq!(
        decision.validate_against(&[not_run]),
        Err(ContractInvariantError::UnsatisfiedValidationRun)
    );
}

#[test]
fn pass_requires_complete_exited_expected_exit_evidence() {
    let mut run = passing_run();
    assert_eq!(run.validate(), Ok(()));
    assert!(run.satisfies_required_check());

    run.completeness = Completeness::Partial;
    assert_eq!(run.validate(), Err(ContractInvariantError::InvalidPass));

    run.completeness = Completeness::Complete;
    run.exit_code = Some(1);
    assert_eq!(run.validate(), Err(ContractInvariantError::InvalidPass));
}

#[test]
fn gate_state_is_authoritative_and_is_not_inferred_from_passing_runs() {
    let run = passing_run();
    let reference = run_ref(&run);

    let review = gate(GateDecisionKind::HumanReview, vec![reference.clone()]);
    assert_eq!(review.validate_against(std::slice::from_ref(&run)), Ok(()));
    assert_eq!(
        review.authoritative_state(),
        AuthoritativeGateState::AwaitingHumanReview
    );

    let blocked = gate(GateDecisionKind::Block, vec![reference]);
    assert_eq!(blocked.validate_against(&[run]), Ok(()));
    assert_eq!(
        blocked.authoritative_state(),
        AuthoritativeGateState::Blocked
    );
}

#[test]
fn auto_pass_requires_the_exact_required_and_satisfied_run_set() {
    let run = passing_run();
    let reference = run_ref(&run);
    let mut decision = gate(GateDecisionKind::AutoPass, vec![reference]);
    assert_eq!(
        decision.validate_against(std::slice::from_ref(&run)),
        Ok(())
    );
    assert_eq!(
        decision.authoritative_state(),
        AuthoritativeGateState::Passed
    );

    decision.satisfied_run_refs.clear();
    assert_eq!(
        decision.validate_against(&[run]),
        Err(ContractInvariantError::IncompleteAutoPass)
    );
}

fn evidence_bundle(completeness: Completeness, missing_reasons: Vec<String>) -> EvidenceBundle {
    let manifest_ref = artifact(ArtifactKind::Manifest, "evidence/manifest.json");
    let manifest_entry = ArtifactManifestEntry {
        artifact_id: manifest_ref.artifact_id.clone(),
        sha256: manifest_ref.sha256.clone(),
        size_bytes: manifest_ref.size_bytes,
        redaction_status: manifest_ref.redaction_status,
    };
    EvidenceBundle {
        schema_id: EvidenceBundleSchemaId::EvidenceBundle,
        schema_version: 1,
        evidence_bundle_id: EvidenceBundleId::new(),
        revision: 1,
        created_at: at("2026-07-12T00:00:00Z"),
        updated_at: at("2026-07-12T00:00:03Z"),
        producer: producer(),
        extensions: BTreeMap::new(),
        goal_spec_ref: document("star.goal-spec", "goal-spec-1"),
        stage_graph_ref: document("star.stage-graph", "stage-graph-1"),
        final_revision_ref: document("star.project-revision", "revision-1"),
        stage_evidence: Vec::new(),
        change_evidence: ChangeEvidenceRefs {
            before_fingerprint: hash("before"),
            after_fingerprint: hash("after"),
            change_set_ref: document("star.change-set", "change-set-1"),
            changed_files_ref: artifact(ArtifactKind::ChangeSet, "changes/files.json"),
        },
        validation_plan_refs: vec![document("star.validation-plan", "plan-1")],
        validation_run_refs: Vec::new(),
        diagnostic_refs: Vec::new(),
        gate_decision_ref: GateDecisionRef {
            gate_id: GateId::new(),
            revision: 1,
            sha256: hash("gate-1"),
        },
        event_ranges: Vec::new(),
        cost_record_refs: Vec::new(),
        unmeasured_usage: Vec::new(),
        merge_result_ref: None,
        remaining_risks: Vec::new(),
        handoff_ref: None,
        artifact_manifest: ArtifactManifest {
            manifest_ref,
            artifacts: vec![manifest_entry],
        },
        completeness,
        missing_reasons,
    }
}

#[test]
fn evidence_bundle_makes_missing_evidence_explicit() {
    let complete = evidence_bundle(Completeness::Complete, Vec::new());
    assert_eq!(complete.validate(), Ok(()));

    let partial = evidence_bundle(
        Completeness::Partial,
        vec!["manual observation was not recorded".to_owned()],
    );
    assert_eq!(partial.validate(), Ok(()));

    let unlabelled_partial = evidence_bundle(Completeness::Partial, Vec::new());
    assert_eq!(
        unlabelled_partial.validate(),
        Err(ContractInvariantError::EvidenceCompleteness)
    );
}
