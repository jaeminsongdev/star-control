use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{ArtifactKind, StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "1.0.0";
const VALIDATION_DECISION_SCHEMA: &str = "validation-decision.schema.json";
const VALIDATION_RUN_SCHEMA: &str = "validation-run.schema.json";
const APPROVAL_REQUEST_SCHEMA: &str = "approval-request.schema.json";
const APPROVAL_RESPONSE_SCHEMA: &str = "approval-response.schema.json";
const REVIEW_PACK_HANDOFF_SCHEMA: &str = "review-pack-handoff.schema.json";
const SENTINEL_APPROVAL_SCHEMA: &str = "approval.schema.json";
const SENTINEL_REVIEW_PACK_SCHEMA: &str = "review-pack.schema.json";
const VALIDATION_DECISION_FILE: &str = "validation-decision.json";
const VALIDATION_RUNS_FILE: &str = "validation_runs.json";
const APPROVAL_REQUEST_FILE: &str = "approval-request.json";
const APPROVAL_RESPONSE_PATH: &str = "approvals/approval-response.json";
const REVIEW_PACK_HANDOFF_FILE: &str = "handoff.json";
const REVIEW_PACK_JSON_PATH: &str = "review-packs/review_pack.json";
const REVIEW_PACK_MARKDOWN_PATH: &str = "review-packs/review_pack.md";
const SENTINEL_TOOL_OUTPUT_DIR: &str = "star-sentinel";
const SENTINEL_APPROVAL_PATH: &str = "tool-output/star-sentinel/approval.json";
const SENTINEL_REVIEW_PACK_JSON_PATH: &str = "tool-output/star-sentinel/review_pack.json";
const SENTINEL_REVIEW_PACK_MARKDOWN_PATH: &str = "tool-output/star-sentinel/review_pack.md";

#[derive(Debug)]
pub enum ValidationEngineError {
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
    },
    State(StateStoreError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    ProviderOutputMissing {
        path: PathBuf,
    },
    ApprovalResponseMissing {
        path: PathBuf,
    },
    ApprovalResponseNotApproved {
        response: String,
    },
    ApprovalResponseMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ValidationEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "failed to load schema {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed {
                path,
                schema_path,
                errors,
            } => write!(
                formatter,
                "schema validation failed for {} against {} with {} error(s)",
                path.display(),
                schema_path.display(),
                errors.len()
            ),
            Self::State(source) => write!(formatter, "state store failed: {}", source),
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::InvalidJson { path, source } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::MissingField { path, field } => {
                write!(formatter, "missing field {} in {}", field, path.display())
            }
            Self::InvalidFieldType {
                path,
                field,
                expected,
            } => write!(
                formatter,
                "invalid field type for {} in {}, expected {}",
                field,
                path.display(),
                expected
            ),
            Self::ProviderOutputMissing { path } => {
                write!(formatter, "provider output missing at {}", path.display())
            }
            Self::ApprovalResponseMissing { path } => {
                write!(formatter, "approval response missing at {}", path.display())
            }
            Self::ApprovalResponseNotApproved { response } => {
                write!(formatter, "approval response is not approved: {}", response)
            }
            Self::ApprovalResponseMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "approval response {} mismatch: expected {}, got {}",
                field, expected, actual
            ),
        }
    }
}

impl Error for ValidationEngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ValidationEngineError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationContext {
    job_id: String,
    stage: String,
    task_id: String,
    requested_at: String,
}

impl ValidationContext {
    pub fn new(
        job_id: impl Into<String>,
        stage: impl Into<String>,
        task_id: impl Into<String>,
        requested_at: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            stage: stage.into(),
            task_id: task_id.into(),
            requested_at: requested_at.into(),
        }
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationOutcome {
    validation_run: Value,
    decision: Value,
    approval_request: Option<Value>,
    handoff: Option<Value>,
}

impl ValidationOutcome {
    pub fn validation_run(&self) -> &Value {
        &self.validation_run
    }

    pub fn decision(&self) -> &Value {
        &self.decision
    }

    pub fn approval_request(&self) -> Option<&Value> {
        self.approval_request.as_ref()
    }

    pub fn handoff(&self) -> Option<&Value> {
        self.handoff.as_ref()
    }

    pub fn next_state(&self) -> Option<&str> {
        self.decision.get("next_state").and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrittenValidationArtifacts {
    validation_run_ref: Value,
    decision_ref: Value,
    approval_request_ref: Option<Value>,
    handoff_ref: Option<Value>,
    state: Value,
}

impl WrittenValidationArtifacts {
    pub fn validation_run_ref(&self) -> &Value {
        &self.validation_run_ref
    }

    pub fn decision_ref(&self) -> &Value {
        &self.decision_ref
    }

    pub fn approval_request_ref(&self) -> Option<&Value> {
        self.approval_request_ref.as_ref()
    }

    pub fn handoff_ref(&self) -> Option<&Value> {
        self.handoff_ref.as_ref()
    }

    pub fn state(&self) -> &Value {
        &self.state
    }
}

pub struct ValidationEngine<'a> {
    state_store: &'a StateStore,
    core_schema_root: PathBuf,
    sentinel_schema_root: PathBuf,
}

impl<'a> ValidationEngine<'a> {
    pub fn new(
        state_store: &'a StateStore,
        core_schema_root: impl AsRef<Path>,
        sentinel_schema_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            state_store,
            core_schema_root: core_schema_root.as_ref().to_path_buf(),
            sentinel_schema_root: sentinel_schema_root.as_ref().to_path_buf(),
        }
    }

    pub fn ensure_provider_response(
        &self,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<(), ValidationEngineError> {
        let path = self.state_store.resolve_job_path(
            job_id,
            &format!("provider-output/{}/response.json", provider_instance_id),
        )?;
        if path.is_file() {
            Ok(())
        } else {
            Err(ValidationEngineError::ProviderOutputMissing { path })
        }
    }

    pub fn evaluate_star_sentinel_gate(
        &self,
        context: &ValidationContext,
        approval: &Value,
        review_pack: Option<&Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        if let Err(error) = self.validate_sentinel_schema(
            approval,
            SENTINEL_APPROVAL_SCHEMA,
            SENTINEL_APPROVAL_PATH,
        ) {
            return self.failed_outcome(
                context,
                "star_sentinel_output_invalid",
                vec![diagnostic_for_error("star-sentinel.output.invalid", &error)],
            );
        }

        let approval_path = Path::new(SENTINEL_APPROVAL_PATH);
        let task_id = required_string(approval, approval_path, "task_id")?;
        let decision = required_string(approval, approval_path, "decision")?;
        let reasons = string_array(approval, approval_path, "reasons")?;
        let diagnostics = approval
            .get("diagnostics")
            .cloned()
            .unwrap_or_else(|| json!([]));

        if task_id != context.task_id {
            return self.failed_outcome(
                context,
                "star_sentinel_task_mismatch",
                vec![json!({
                    "rule_id": "star-sentinel.output.task_mismatch",
                    "severity": "block",
                    "message": format!(
                        "approval task_id {} did not match expected {}",
                        task_id, context.task_id
                    )
                })],
            );
        }

        if decision == "AUTO_PASS" && has_block_diagnostic(&diagnostics) {
            let mut failed_diagnostics = diagnostics_array(&diagnostics);
            failed_diagnostics.push(json!({
                "rule_id": "star-sentinel.output.inconsistent",
                "severity": "block",
                "message": "approval decision AUTO_PASS included a block diagnostic"
            }));
            return self.failed_outcome(
                context,
                "star_sentinel_output_inconsistent",
                failed_diagnostics,
            );
        }

        match decision {
            "AUTO_PASS" => {
                self.normal_outcome(context, decision, "VALIDATED", reasons, diagnostics, None)
            }
            "HUMAN_REVIEW" => {
                let review_pack = match review_pack {
                    Some(review_pack) => review_pack,
                    None => {
                        return self.failed_outcome(
                            context,
                            "review_pack_missing",
                            vec![json!({
                                "rule_id": "validation.review_pack.missing",
                                "severity": "block",
                                "message": "HUMAN_REVIEW requires a review pack handoff"
                            })],
                        )
                    }
                };
                if let Err(error) = self.validate_sentinel_schema(
                    review_pack,
                    SENTINEL_REVIEW_PACK_SCHEMA,
                    SENTINEL_REVIEW_PACK_JSON_PATH,
                ) {
                    return self.failed_outcome(
                        context,
                        "review_pack_invalid",
                        vec![diagnostic_for_error(
                            "star-sentinel.review_pack.invalid",
                            &error,
                        )],
                    );
                }
                self.normal_outcome(
                    context,
                    decision,
                    "WAITING_APPROVAL",
                    reasons,
                    diagnostics,
                    Some(review_pack),
                )
            }
            "BLOCK" => {
                let review_pack = match review_pack {
                    Some(review_pack) => review_pack,
                    None => {
                        return self.failed_outcome(
                            context,
                            "review_pack_missing",
                            vec![json!({
                                "rule_id": "validation.review_pack.missing",
                                "severity": "block",
                                "message": "BLOCK requires a review pack handoff"
                            })],
                        )
                    }
                };
                if let Err(error) = self.validate_sentinel_schema(
                    review_pack,
                    SENTINEL_REVIEW_PACK_SCHEMA,
                    SENTINEL_REVIEW_PACK_JSON_PATH,
                ) {
                    return self.failed_outcome(
                        context,
                        "review_pack_invalid",
                        vec![diagnostic_for_error(
                            "star-sentinel.review_pack.invalid",
                            &error,
                        )],
                    );
                }
                self.normal_outcome(
                    context,
                    decision,
                    "BLOCKED",
                    reasons,
                    diagnostics,
                    Some(review_pack),
                )
            }
            _ => self.failed_outcome(
                context,
                "approval_decision_invalid",
                vec![json!({
                    "rule_id": "star-sentinel.output.decision_invalid",
                    "severity": "block",
                    "message": format!("unsupported approval decision {}", decision)
                })],
            ),
        }
    }

    pub fn write_outcome(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<WrittenValidationArtifacts, ValidationEngineError> {
        self.validate_core_schema(
            outcome.validation_run(),
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;
        self.validate_core_schema(
            outcome.decision(),
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;

        let validation_run_ref = self.write_or_reference_validation_run(context, outcome)?;
        let decision_ref = self.state_store.write_validation_json(
            context.job_id(),
            VALIDATION_DECISION_FILE,
            outcome.decision(),
        )?;

        let handoff_ref = if let Some(handoff) = outcome.handoff() {
            self.validate_core_schema(
                handoff,
                REVIEW_PACK_HANDOFF_SCHEMA,
                &format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE),
            )?;
            Some(self.state_store.write_review_pack_json(
                context.job_id(),
                REVIEW_PACK_HANDOFF_FILE,
                handoff,
            )?)
        } else {
            None
        };

        let approval_request_ref = if let Some(approval_request) = outcome.approval_request() {
            self.validate_core_schema(
                approval_request,
                APPROVAL_REQUEST_SCHEMA,
                &format!("approvals/{}", APPROVAL_REQUEST_FILE),
            )?;
            Some(self.state_store.write_approval_json(
                context.job_id(),
                APPROVAL_REQUEST_FILE,
                approval_request,
            )?)
        } else {
            None
        };

        let state = self.update_run_state(
            context,
            outcome,
            &validation_run_ref,
            &decision_ref,
            approval_request_ref.as_ref(),
            handoff_ref.as_ref(),
        )?;
        self.append_gate_events(context, outcome)?;

        Ok(WrittenValidationArtifacts {
            validation_run_ref,
            decision_ref,
            approval_request_ref,
            handoff_ref,
            state,
        })
    }

    pub fn ensure_approval_response_allows_next_stage(
        &self,
        context: &ValidationContext,
    ) -> Result<Value, ValidationEngineError> {
        let path = self
            .state_store
            .resolve_job_path(context.job_id(), APPROVAL_RESPONSE_PATH)?;
        if !path.is_file() {
            return Err(ValidationEngineError::ApprovalResponseMissing { path });
        }
        let response = read_json_file(&path)?;
        self.validate_core_schema(&response, APPROVAL_RESPONSE_SCHEMA, APPROVAL_RESPONSE_PATH)?;
        ensure_response_field_matches(&response, "job_id", context.job_id())?;
        ensure_response_field_matches(&response, "stage", context.stage())?;
        ensure_response_field_matches(&response, "task_id", context.task_id())?;
        let response_value =
            required_string(&response, Path::new(APPROVAL_RESPONSE_PATH), "response")?;
        if response_value == "approved" {
            Ok(response)
        } else {
            Err(ValidationEngineError::ApprovalResponseNotApproved {
                response: response_value.to_string(),
            })
        }
    }

    fn write_or_reference_validation_run(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<Value, ValidationEngineError> {
        let relative_path = format!(
            "tool-output/{}/{}",
            SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
        );
        let resolved = self
            .state_store
            .resolve_job_path(context.job_id(), &relative_path)?;
        if resolved.exists() {
            let existing = read_json_file(&resolved)?;
            self.validate_core_schema(&existing, VALIDATION_RUN_SCHEMA, &relative_path)?;
            self.state_store
                .artifact_ref(
                    context.job_id(),
                    &relative_path,
                    ArtifactKind::ToolOutput,
                    SENTINEL_TOOL_OUTPUT_DIR,
                    Some("specs/schemas/validation-run.schema.json"),
                    Some("validation run output"),
                )
                .map_err(ValidationEngineError::from)
        } else {
            self.state_store
                .write_tool_json(
                    context.job_id(),
                    SENTINEL_TOOL_OUTPUT_DIR,
                    VALIDATION_RUNS_FILE,
                    outcome.validation_run(),
                )
                .map_err(ValidationEngineError::from)
        }
    }

    fn normal_outcome(
        &self,
        context: &ValidationContext,
        decision: &str,
        next_state: &str,
        reasons: Vec<String>,
        diagnostics: Value,
        review_pack: Option<&Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        let needs_approval = decision == "HUMAN_REVIEW" || decision == "BLOCK";
        let review_pack_path = needs_approval.then_some(REVIEW_PACK_MARKDOWN_PATH);
        let approval_request_path = needs_approval.then_some("approvals/approval-request.json");
        let decision_artifact = build_validation_decision(
            context,
            decision,
            reasons.clone(),
            diagnostics.clone(),
            next_state,
            review_pack_path,
            approval_request_path,
        );
        self.validate_core_schema(
            &decision_artifact,
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;

        let approval_request = if needs_approval {
            Some(build_approval_request(
                context,
                decision,
                reasons,
                diagnostics.clone(),
                review_pack,
            ))
        } else {
            None
        };
        if let Some(approval_request) = approval_request.as_ref() {
            self.validate_core_schema(
                approval_request,
                APPROVAL_REQUEST_SCHEMA,
                &format!("approvals/{}", APPROVAL_REQUEST_FILE),
            )?;
        }

        let handoff = if needs_approval {
            Some(build_review_pack_handoff(context, decision, review_pack))
        } else {
            None
        };
        if let Some(handoff) = handoff.as_ref() {
            self.validate_core_schema(
                handoff,
                REVIEW_PACK_HANDOFF_SCHEMA,
                &format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE),
            )?;
        }

        let validation_run = build_validation_run(context, next_state);
        self.validate_core_schema(
            &validation_run,
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;

        Ok(ValidationOutcome {
            validation_run,
            decision: decision_artifact,
            approval_request,
            handoff,
        })
    }

    fn failed_outcome(
        &self,
        context: &ValidationContext,
        reason: &str,
        diagnostics: Vec<Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        let decision = build_validation_decision(
            context,
            "BLOCK",
            vec![reason.to_string()],
            Value::Array(diagnostics),
            "FAILED",
            None,
            None,
        );
        self.validate_core_schema(
            &decision,
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;
        let validation_run = build_validation_run(context, "FAILED");
        self.validate_core_schema(
            &validation_run,
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;
        Ok(ValidationOutcome {
            validation_run,
            decision,
            approval_request: None,
            handoff: None,
        })
    }

    fn update_run_state(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
        validation_run_ref: &Value,
        decision_ref: &Value,
        approval_request_ref: Option<&Value>,
        handoff_ref: Option<&Value>,
    ) -> Result<Value, ValidationEngineError> {
        let mut state = self.state_store.load_state(context.job_id())?;
        set_object_field(
            &mut state,
            "state",
            Value::String(outcome.next_state().unwrap_or("FAILED").to_string()),
        )?;
        set_object_field(
            &mut state,
            "current_stage",
            Value::String(context.stage().to_string()),
        )?;
        set_object_field(
            &mut state,
            "updated_at",
            Value::String(context.requested_at.clone()),
        )?;
        set_object_field(
            &mut state,
            "latest_event_id",
            Value::String(format!(
                "{}-{}-gate-decided",
                context.job_id().to_lowercase(),
                context.stage()
            )),
        )?;
        set_object_field(
            &mut state,
            "next_action",
            Value::String(
                next_action_for_state(outcome.next_state().unwrap_or("FAILED")).to_string(),
            ),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_validation_run", context.stage()),
            validation_run_ref,
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_validation_decision", context.stage()),
            decision_ref,
        )?;
        if let Some(approval_request_ref) = approval_request_ref {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_approval_request", context.stage()),
                approval_request_ref,
            )?;
        }
        if let Some(handoff_ref) = handoff_ref {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_review_pack_handoff", context.stage()),
                handoff_ref,
            )?;
        }
        push_history(
            &mut state,
            json!({
                "stage": context.stage(),
                "task_id": context.task_id(),
                "decision": outcome.decision()["decision"],
                "next_state": outcome.decision()["next_state"]
            }),
        )?;
        self.state_store.save_state(context.job_id(), &state)?;
        Ok(state)
    }

    fn append_gate_events(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<(), ValidationEngineError> {
        self.append_event(
            context,
            "VALIDATION_RECORDED",
            "Validation run recorded",
            vec![format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            )],
            json!({ "status": outcome.validation_run()["status"] }),
        )?;
        self.append_event(
            context,
            "GATE_DECIDED",
            "Validation gate decision recorded",
            vec![format!("validation/{}", VALIDATION_DECISION_FILE)],
            json!({
                "decision": outcome.decision()["decision"],
                "next_state": outcome.decision()["next_state"]
            }),
        )?;
        if outcome.handoff().is_some() {
            self.append_event(
                context,
                "REVIEW_PACK_CREATED",
                "Review pack handoff recorded",
                vec![format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE)],
                json!({ "decision": outcome.decision()["decision"] }),
            )?;
        }
        if outcome.approval_request().is_some() {
            self.append_event(
                context,
                "APPROVAL_REQUESTED",
                "Human approval requested",
                vec![format!("approvals/{}", APPROVAL_REQUEST_FILE)],
                json!({ "decision": outcome.decision()["decision"] }),
            )?;
        }
        Ok(())
    }

    fn append_event(
        &self,
        context: &ValidationContext,
        event_type: &str,
        message: &str,
        artifact_paths: Vec<String>,
        details: Value,
    ) -> Result<(), ValidationEngineError> {
        let event = json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": format!(
                "{}-{}-{}",
                context.job_id().to_lowercase(),
                context.stage(),
                event_type.to_lowercase().replace('_', "-")
            ),
            "job_id": context.job_id(),
            "type": event_type,
            "created_at": context.requested_at,
            "stage": context.stage(),
            "state": "",
            "message": message,
            "artifact_paths": artifact_paths,
            "details": details
        });
        self.state_store.append_event(context.job_id(), &event)?;
        Ok(())
    }

    fn validate_core_schema(
        &self,
        value: &Value,
        schema_file: &str,
        relative_path: &str,
    ) -> Result<(), ValidationEngineError> {
        validate_schema_value(value, &self.core_schema_root, schema_file, relative_path)
    }

    fn validate_sentinel_schema(
        &self,
        value: &Value,
        schema_file: &str,
        relative_path: &str,
    ) -> Result<(), ValidationEngineError> {
        validate_schema_value(
            value,
            &self.sentinel_schema_root,
            schema_file,
            relative_path,
        )
    }
}

fn build_validation_decision(
    context: &ValidationContext,
    decision: &str,
    reasons: Vec<String>,
    diagnostics: Value,
    next_state: &str,
    review_pack_path: Option<&str>,
    approval_request_path: Option<&str>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "source": "star-sentinel/gate",
        "reasons": reasons,
        "diagnostics": diagnostics,
        "next_state": next_state,
        "review_pack_path": review_pack_path,
        "approval_request_path": approval_request_path
    })
}

fn build_approval_request(
    context: &ValidationContext,
    decision: &str,
    reasons: Vec<String>,
    diagnostics: Value,
    review_pack: Option<&Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "reasons": reasons,
        "changed_files": array_of_strings_from(review_pack, "changed_files"),
        "risks": array_of_strings_from(review_pack, "risks"),
        "diagnostics": diagnostics,
        "review_pack_path": REVIEW_PACK_MARKDOWN_PATH,
        "requested_at": context.requested_at,
        "requested_by": "validation-engine"
    })
}

fn build_review_pack_handoff(
    context: &ValidationContext,
    decision: &str,
    review_pack: Option<&Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "source_json_path": SENTINEL_REVIEW_PACK_JSON_PATH,
        "source_markdown_path": SENTINEL_REVIEW_PACK_MARKDOWN_PATH,
        "canonical_json_path": REVIEW_PACK_JSON_PATH,
        "canonical_markdown_path": REVIEW_PACK_MARKDOWN_PATH,
        "created_at": context.requested_at,
        "questions_for_human": array_of_strings_from(review_pack, "questions_for_human")
    })
}

fn build_validation_run(context: &ValidationContext, next_state: &str) -> Value {
    let status = match next_state {
        "FAILED" => "ERROR",
        "BLOCKED" => "FAIL",
        _ => "PASS",
    };
    let exit_code = if status == "PASS" { 0 } else { 1 };
    json!({
        "id": format!(
            "{}-{}-star-sentinel-gate",
            context.job_id().to_lowercase(),
            context.stage()
        ),
        "command": "star-sentinel gate",
        "status": status,
        "exit_code": exit_code,
        "started_at": context.requested_at,
        "finished_at": context.requested_at,
        "log_path": SENTINEL_APPROVAL_PATH
    })
}

fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
    relative_path: &str,
) -> Result<(), ValidationEngineError> {
    let schema_path = schema_root.join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ValidationEngineError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ValidationEngineError::SchemaValidationFailed {
            path: PathBuf::from(relative_path),
            schema_path,
            errors: result.errors,
        })
    }
}

fn read_json_file(path: &Path) -> Result<Value, ValidationEngineError> {
    let content = fs::read_to_string(path).map_err(|source| ValidationEngineError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ValidationEngineError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

fn required_string<'a>(
    value: &'a Value,
    path: &Path,
    field: &str,
) -> Result<&'a str, ValidationEngineError> {
    let Some(field_value) = value.get(field) else {
        return Err(ValidationEngineError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        });
    };
    field_value
        .as_str()
        .ok_or_else(|| ValidationEngineError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

fn ensure_response_field_matches(
    response: &Value,
    field: &str,
    expected: &str,
) -> Result<(), ValidationEngineError> {
    let actual = required_string(response, Path::new(APPROVAL_RESPONSE_PATH), field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationEngineError::ApprovalResponseMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ValidationEngineError> {
    let Some(field_value) = value.get(field) else {
        return Err(ValidationEngineError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        });
    };
    let Some(items) = field_value.as_array() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array".to_string(),
        });
    };
    Ok(items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect())
}

fn array_of_strings_from(value: Option<&Value>, field: &str) -> Vec<String> {
    value
        .and_then(|value| value.get(field))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn diagnostics_array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

fn has_block_diagnostic(diagnostics: &Value) -> bool {
    diagnostics
        .as_array()
        .map(|items| {
            items.iter().any(|item| {
                item.get("severity")
                    .and_then(Value::as_str)
                    .is_some_and(|severity| severity == "block")
            })
        })
        .unwrap_or(false)
}

fn diagnostic_for_error(rule_id: &str, error: &ValidationEngineError) -> Value {
    json!({
        "rule_id": rule_id,
        "severity": "block",
        "message": error.to_string()
    })
}

fn next_action_for_state(state: &str) -> &'static str {
    match state {
        "VALIDATED" => "continue",
        "WAITING_APPROVAL" => "await_approval",
        "BLOCKED" => "manual_intervention",
        _ => "inspect_validation_failure",
    }
}

fn set_object_field(
    value: &mut Value,
    field: &str,
    field_value: Value,
) -> Result<(), ValidationEngineError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    object.insert(field.to_string(), field_value);
    Ok(())
}

fn push_history(value: &mut Value, entry: Value) -> Result<(), ValidationEngineError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    let history = object
        .entry("history")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(history_items) = history.as_array_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "history".to_string(),
            expected: "array".to_string(),
        });
    };
    history_items.push(entry);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn auto_pass_maps_to_validated_and_writes_core_artifacts() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();
        fixture.write_provider_response();
        fixture
            .engine()
            .ensure_provider_response("J-0001", "fake-default")
            .expect("provider response");

        let outcome = fixture
            .engine()
            .evaluate_star_sentinel_gate(&context(), &approval("AUTO_PASS"), None)
            .expect("evaluate");
        assert_eq!(outcome.next_state(), Some("VALIDATED"));

        let written = fixture
            .engine()
            .write_outcome(&context(), &outcome)
            .expect("write outcome");

        assert_eq!(
            written.decision_ref()["path"],
            "validation/validation-decision.json"
        );
        assert_eq!(written.validation_run_ref()["kind"], "tool_output");
        assert_eq!(written.state()["state"], "VALIDATED");
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/validation/validation-decision.json")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/validation_runs.json")
            .is_file());
        let events = fixture.store.read_events("J-0001").expect("events");
        assert!(events.iter().any(|event| event["type"] == "GATE_DECIDED"));
        assert!(events
            .iter()
            .any(|event| event["type"] == "VALIDATION_RECORDED"));
    }

    #[test]
    fn human_review_maps_to_waiting_approval_and_writes_handoff() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();

        let outcome = fixture
            .engine()
            .evaluate_star_sentinel_gate(
                &context(),
                &approval("HUMAN_REVIEW"),
                Some(&review_pack("HUMAN_REVIEW")),
            )
            .expect("evaluate");

        assert_eq!(outcome.next_state(), Some("WAITING_APPROVAL"));
        assert!(outcome.approval_request().is_some());
        assert!(outcome.handoff().is_some());

        let written = fixture
            .engine()
            .write_outcome(&context(), &outcome)
            .expect("write outcome");

        assert_eq!(written.state()["state"], "WAITING_APPROVAL");
        assert_eq!(
            written.approval_request_ref().expect("approval ref")["path"],
            "approvals/approval-request.json"
        );
        assert_eq!(
            written.handoff_ref().expect("handoff ref")["path"],
            "review-packs/handoff.json"
        );
    }

    #[test]
    fn block_maps_to_blocked() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();

        let outcome = fixture
            .engine()
            .evaluate_star_sentinel_gate(
                &context(),
                &approval("BLOCK"),
                Some(&review_pack("BLOCK")),
            )
            .expect("evaluate");

        assert_eq!(outcome.next_state(), Some("BLOCKED"));
        assert_eq!(outcome.validation_run()["status"], "FAIL");
    }

    #[test]
    fn invalid_approval_output_maps_to_failed() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();

        let outcome = fixture
            .engine()
            .evaluate_star_sentinel_gate(
                &context(),
                &json!({
                    "schema_version": "1.0.0",
                    "task_id": "p0-task-demo",
                    "reasons": [],
                    "diagnostics": []
                }),
                None,
            )
            .expect("failed outcome");

        assert_eq!(outcome.next_state(), Some("FAILED"));
        assert_eq!(
            outcome.decision()["reasons"][0],
            "star_sentinel_output_invalid"
        );
        assert_eq!(outcome.validation_run()["status"], "ERROR");
    }

    #[test]
    fn auto_pass_with_block_diagnostic_maps_to_failed() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();
        let approval = json!({
            "schema_version": "1.0.0",
            "task_id": "p0-task-demo",
            "decision": "AUTO_PASS",
            "reasons": [],
            "diagnostics": [
                {
                    "rule_id": "scope.allowed_paths",
                    "severity": "block"
                }
            ]
        });

        let outcome = fixture
            .engine()
            .evaluate_star_sentinel_gate(&context(), &approval, None)
            .expect("failed outcome");

        assert_eq!(outcome.next_state(), Some("FAILED"));
        assert_eq!(
            outcome.decision()["reasons"][0],
            "star_sentinel_output_inconsistent"
        );
    }

    #[test]
    fn missing_provider_response_is_an_error() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();

        let error = fixture
            .engine()
            .ensure_provider_response("J-0001", "fake-default")
            .unwrap_err();

        assert!(matches!(
            error,
            ValidationEngineError::ProviderOutputMissing { .. }
        ));
    }

    #[test]
    fn missing_approval_response_blocks_next_stage() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();

        let error = fixture
            .engine()
            .ensure_approval_response_allows_next_stage(&context())
            .unwrap_err();

        assert!(matches!(
            error,
            ValidationEngineError::ApprovalResponseMissing { .. }
        ));
    }

    #[test]
    fn approved_response_allows_next_stage() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();
        fixture
            .store
            .write_approval_json(
                "J-0001",
                "approval-response.json",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": "J-0001",
                    "stage": "validate",
                    "task_id": "p0-task-demo",
                    "response": "approved",
                    "reviewer": "human",
                    "responded_at": "2026-07-01T00:00:00Z",
                    "reason": "approved for test",
                    "allowed_next_stage": "report",
                    "constraints": []
                }),
            )
            .expect("write response");

        let response = fixture
            .engine()
            .ensure_approval_response_allows_next_stage(&context())
            .expect("approved");

        assert_eq!(response["response"], "approved");
    }

    #[test]
    fn approval_response_task_mismatch_blocks_next_stage() {
        let fixture = Fixture::new();
        fixture.create_job_with_state();
        fixture
            .store
            .write_approval_json(
                "J-0001",
                "approval-response.json",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": "J-0001",
                    "stage": "validate",
                    "task_id": "different-task",
                    "response": "approved",
                    "reviewer": "human",
                    "responded_at": "2026-07-01T00:00:00Z",
                    "reason": "approved for test"
                }),
            )
            .expect("write response");

        let error = fixture
            .engine()
            .ensure_approval_response_allows_next_stage(&context())
            .unwrap_err();

        assert!(matches!(
            error,
            ValidationEngineError::ApprovalResponseMismatch { .. }
        ));
    }

    struct Fixture {
        project: PathBuf,
        store: StateStore,
        core_schema_root: PathBuf,
        sentinel_schema_root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let project = temp_project();
            let root = repo_root();
            let core_schema_root = root.join("specs/schemas");
            let sentinel_schema_root = root.join("builtin-tools/star-sentinel/schemas");
            let store = StateStore::open(&project, &core_schema_root).expect("store");
            Self {
                project,
                store,
                core_schema_root,
                sentinel_schema_root,
            }
        }

        fn engine(&self) -> ValidationEngine<'_> {
            ValidationEngine::new(
                &self.store,
                &self.core_schema_root,
                &self.sentinel_schema_root,
            )
        }

        fn create_job_with_state(&self) {
            self.store
                .create_job("validate p0 output", "validation-engine", Vec::new())
                .expect("job");
            self.store
                .save_state("J-0001", &state("J-0001", "VALIDATING"))
                .expect("state");
        }

        fn write_provider_response(&self) {
            self.store
                .write_provider_json(
                    "J-0001",
                    "fake-default",
                    "response.json",
                    &json!({ "ok": true }),
                )
                .expect("provider response");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.project).ok();
        }
    }

    fn context() -> ValidationContext {
        ValidationContext::new("J-0001", "validate", "p0-task-demo", "2026-07-01T00:00:00Z")
    }

    fn approval(decision: &str) -> Value {
        json!({
            "schema_version": "1.0.0",
            "task_id": "p0-task-demo",
            "decision": decision,
            "reasons": ["schema_change_requires_approval"],
            "diagnostics": [
                {
                    "rule_id": "dependency.requires_approval",
                    "severity": if decision == "BLOCK" { "block" } else { "warn" }
                }
            ],
            "required_human_actions": [
                "Review HUMAN_REVIEW diagnostics and record approval before continuing."
            ]
        })
    }

    fn review_pack(decision: &str) -> Value {
        json!({
            "schema_version": "1.0.0",
            "task_id": "p0-task-demo",
            "decision": decision,
            "summary": "Dependency-related files changed and require explicit review before proceeding.",
            "changed_files": ["Cargo.toml"],
            "risks": ["dependency_addition"],
            "validations": [
                {
                    "command": "policy:p0",
                    "result": if decision == "BLOCK" { "blocked" } else { "requires_human_review" }
                }
            ],
            "unverified_claims": [],
            "diagnostics": [
                {
                    "rule_id": "dependency.requires_approval",
                    "severity": if decision == "BLOCK" { "block" } else { "warn" }
                }
            ],
            "source_artifacts": [
                "tool-output/star-sentinel/approval.json"
            ],
            "questions_for_human": [
                "Is this dependency approved?"
            ],
            "review_pack_markdown": "# Review Pack\n\nDependency-related files changed and require explicit review before proceeding."
        })
    }

    fn state(job_id: &str, state: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "state": state,
            "current_stage": "validate",
            "updated_at": "2026-07-01T00:00:00Z",
            "workers": {},
            "artifacts": {},
            "next_action": "run_validation",
            "history": []
        })
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("package parent")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn temp_project() -> PathBuf {
        let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "star-control-validation-{}-{}-{}",
            std::process::id(),
            counter,
            nanos
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }
}
