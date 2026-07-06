use crate::constants::JOB_SCHEMA;
use crate::contract::{optional_string_array, required_string, validate_contract};
use crate::RouterError;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct JobSpec {
    pub(crate) job_id: String,
    pub(crate) project_root: String,
    pub(crate) request_text: String,
    pub(crate) user_constraints: Vec<String>,
    pub(crate) value: Value,
}

impl JobSpec {
    pub fn from_value(
        value: Value,
        source_path: impl Into<PathBuf>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, RouterError> {
        let source_path = source_path.into();
        validate_contract(&value, &source_path, schema_root.as_ref(), JOB_SCHEMA)?;
        Ok(Self {
            job_id: required_string(&value, &source_path, "job_id")?,
            project_root: required_string(&value, &source_path, "project_root")?,
            request_text: required_string(&value, &source_path, "request_text")?,
            user_constraints: optional_string_array(&value, &source_path, "user_constraints")?,
            value,
        })
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    pub fn request_text(&self) -> &str {
        &self.request_text
    }

    pub fn user_constraints(&self) -> &[String] {
        &self.user_constraints
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouterDecision {
    pub(crate) value: Value,
}

impl RouterDecision {
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteSpec {
    pub(crate) value: Value,
}

impl RouteSpec {
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn decision(&self) -> Option<&str> {
        self.value.get("decision").and_then(Value::as_str)
    }

    pub fn policy_profile(&self) -> Option<&str> {
        self.value.get("policy_profile").and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkSpec {
    pub(crate) stage: String,
    pub(crate) value: Value,
}

impl WorkSpec {
    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouterOutput {
    pub(crate) decision: RouterDecision,
    pub(crate) route: RouteSpec,
    pub(crate) workspecs: BTreeMap<String, WorkSpec>,
}

impl RouterOutput {
    pub fn decision(&self) -> &RouterDecision {
        &self.decision
    }

    pub fn route(&self) -> &RouteSpec {
        &self.route
    }

    pub fn workspecs(&self) -> &BTreeMap<String, WorkSpec> {
        &self.workspecs
    }

    pub fn workspec(&self, stage: &str) -> Option<&WorkSpec> {
        self.workspecs.get(stage)
    }
}
