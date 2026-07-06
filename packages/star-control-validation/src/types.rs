use serde_json::Value;

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

    pub fn requested_at(&self) -> &str {
        &self.requested_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationOutcome {
    pub(crate) validation_run: Value,
    pub(crate) decision: Value,
    pub(crate) approval_request: Option<Value>,
    pub(crate) handoff: Option<Value>,
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
    pub(crate) validation_run_ref: Value,
    pub(crate) decision_ref: Value,
    pub(crate) approval_request_ref: Option<Value>,
    pub(crate) handoff_ref: Option<Value>,
    pub(crate) state: Value,
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
