use serde_json::{json, Value};
use star_control_schema::{
    load_document, load_schema, validate_json, DocumentLoadError, SchemaLoadError,
};
use star_control_state::{StateStore, StateStoreError};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

pub const SENTINEL_TASK_SCHEMA: &str = "sentinel-task.schema.json";
pub const CHANGED_LINES_SCHEMA: &str = "changed-lines.schema.json";
pub const P0_RULE_REGISTRY_SCHEMA: &str = "p0-rule-registry.schema.json";
pub const FIXTURE_OUTCOME_SCHEMA: &str = "fixture-outcome.schema.json";
pub const DIAGNOSTIC_SCHEMA: &str = "diagnostic.schema.json";
pub const APPROVAL_SCHEMA: &str = "approval.schema.json";
pub const STAR_SENTINEL_TOOL_OUTPUT_DIR: &str = "star-sentinel";
pub const DIAGNOSTICS_FILE: &str = "diagnostics.json";
pub const APPROVAL_FILE: &str = "approval.json";

pub const RULE_SCOPE_ALLOWED_PATHS: &str = "task.scope.allowed_paths";
pub const RULE_TEST_NO_DELETION: &str = "test.no_deletion";
pub const RULE_DEPENDENCY_REQUIRES_APPROVAL: &str = "dependency.requires_approval";
pub const RULE_SECRET_NO_PLAINTEXT_SECRET: &str = "secret.no_plaintext_secret";
pub const RULE_VALIDATOR_NO_SELF_BYPASS: &str = "validator.no_self_bypass";

pub const P0_RULE_IDS: [&str; 5] = [
    RULE_SCOPE_ALLOWED_PATHS,
    RULE_TEST_NO_DELETION,
    RULE_DEPENDENCY_REQUIRES_APPROVAL,
    RULE_SECRET_NO_PLAINTEXT_SECRET,
    RULE_VALIDATOR_NO_SELF_BYPASS,
];

#[derive(Debug)]
pub enum SentinelError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    SchemaLoad {
        source: SchemaLoadError,
    },
    DocumentLoad {
        source: DocumentLoadError,
    },
    State {
        source: StateStoreError,
    },
    SchemaValidation {
        artifact: String,
        schema: String,
        errors: Vec<String>,
    },
    MissingField {
        artifact: String,
        field: String,
    },
    InvalidField {
        artifact: String,
        field: String,
        message: String,
    },
    Registry {
        message: String,
    },
}

impl fmt::Display for SentinelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::SchemaLoad { source } => write!(formatter, "schema load failed: {}", source),
            Self::DocumentLoad { source } => write!(formatter, "document load failed: {}", source),
            Self::State { source } => write!(formatter, "state store operation failed: {}", source),
            Self::SchemaValidation {
                artifact,
                schema,
                errors,
            } => write!(
                formatter,
                "{} failed {} validation with {} error(s)",
                artifact,
                schema,
                errors.len()
            ),
            Self::MissingField { artifact, field } => {
                write!(formatter, "{} missing required field {}", artifact, field)
            }
            Self::InvalidField {
                artifact,
                field,
                message,
            } => write!(
                formatter,
                "{} field {} is invalid: {}",
                artifact, field, message
            ),
            Self::Registry { message } => write!(formatter, "rule registry invalid: {}", message),
        }
    }
}

impl Error for SentinelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::SchemaLoad { source } => Some(source),
            Self::DocumentLoad { source } => Some(source),
            Self::State { source } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelTask {
    pub task_id: String,
    pub goal: String,
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub forbidden_change_types: Vec<String>,
    pub required_validation: Vec<String>,
    pub approval_required_changes: Vec<String>,
    pub notes: Option<String>,
}

impl SentinelTask {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        Ok(Self {
            task_id: required_string(value, "task_id", "SentinelTask")?,
            goal: required_string(value, "goal", "SentinelTask")?,
            allowed_paths: required_string_array(value, "allowed_paths", "SentinelTask")?,
            forbidden_paths: required_string_array(value, "forbidden_paths", "SentinelTask")?,
            forbidden_change_types: required_string_array(
                value,
                "forbidden_change_types",
                "SentinelTask",
            )?,
            required_validation: required_string_array(
                value,
                "required_validation",
                "SentinelTask",
            )?,
            approval_required_changes: optional_string_array(
                value,
                "approval_required_changes",
                "SentinelTask",
            )?,
            notes: optional_string(value, "notes", "SentinelTask")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLines {
    pub task_id: String,
    pub files: Vec<ChangedFile>,
}

impl ChangedLines {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        let files = required_array(value, "files", "ChangedLines")?
            .iter()
            .enumerate()
            .map(|(index, file)| {
                ChangedFile::from_value(file, &format!("ChangedLines.files[{}]", index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            task_id: required_string(value, "task_id", "ChangedLines")?,
            files,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub change_type: String,
    pub old_path: Option<String>,
    pub hunks: Vec<ChangedHunk>,
}

impl ChangedFile {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        let hunks = required_array(value, "hunks", artifact)?
            .iter()
            .enumerate()
            .map(|(index, hunk)| {
                ChangedHunk::from_value(hunk, &format!("{}.hunks[{}]", artifact, index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            path: required_string(value, "path", artifact)?,
            change_type: required_string(value, "change_type", artifact)?,
            old_path: optional_string(value, "old_path", artifact)?,
            hunks,
        })
    }

    fn changed_paths(&self) -> Vec<&str> {
        let mut paths = vec![self.path.as_str()];
        if let Some(old_path) = self.old_path.as_deref() {
            if old_path != self.path {
                paths.push(old_path);
            }
        }
        paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedHunk {
    pub old_start: i64,
    pub old_lines: i64,
    pub new_start: i64,
    pub new_lines: i64,
    pub lines: Vec<ChangedLine>,
}

impl ChangedHunk {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        let lines = required_array(value, "lines", artifact)?
            .iter()
            .enumerate()
            .map(|(index, line)| {
                ChangedLine::from_value(line, &format!("{}.lines[{}]", artifact, index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            old_start: required_integer(value, "old_start", artifact)?,
            old_lines: required_integer(value, "old_lines", artifact)?,
            new_start: required_integer(value, "new_start", artifact)?,
            new_lines: required_integer(value, "new_lines", artifact)?,
            lines,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLine {
    pub kind: String,
    pub old_line: Option<i64>,
    pub new_line: Option<i64>,
    pub content: String,
}

impl ChangedLine {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        Ok(Self {
            kind: required_string(value, "kind", artifact)?,
            old_line: optional_integer(value, "old_line", artifact)?,
            new_line: optional_integer(value, "new_line", artifact)?,
            content: required_string(value, "content", artifact)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Block,
}

impl Severity {
    fn parse(value: &str, artifact: &str, field: &str) -> Result<Self, SentinelError> {
        match value {
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "block" => Ok(Self::Block),
            _ => Err(invalid_field(
                artifact,
                field,
                "expected one of info, warn, block",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Decision {
    AutoPass,
    HumanReview,
    Block,
}

impl Decision {
    fn parse(value: &str, artifact: &str, field: &str) -> Result<Self, SentinelError> {
        match value {
            "AUTO_PASS" => Ok(Self::AutoPass),
            "HUMAN_REVIEW" => Ok(Self::HumanReview),
            "BLOCK" => Ok(Self::Block),
            _ => Err(invalid_field(
                artifact,
                field,
                "expected one of AUTO_PASS, HUMAN_REVIEW, BLOCK",
            )),
        }
    }

    fn default_for_severity(severity: Severity) -> Self {
        match severity {
            Severity::Block => Self::Block,
            Severity::Warn => Self::HumanReview,
            Severity::Info => Self::AutoPass,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "AUTO_PASS",
            Self::HumanReview => "HUMAN_REVIEW",
            Self::Block => "BLOCK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    pub rule_id: String,
    pub severity: Severity,
    pub description: String,
    pub decision_effect: Decision,
}

impl RuleDefinition {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        let rule_id = required_string(value, "rule_id", artifact)?;
        let severity = Severity::parse(
            &required_string(value, "severity", artifact)?,
            artifact,
            "severity",
        )?;
        let decision_effect = match optional_string(value, "decision_effect", artifact)? {
            Some(value) => Decision::parse(&value, artifact, "decision_effect")?,
            None => Decision::default_for_severity(severity),
        };

        Ok(Self {
            rule_id,
            severity,
            description: required_string(value, "description", artifact)?,
            decision_effect,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P0RuleRegistry {
    pub profile: String,
    pub rules: Vec<RuleDefinition>,
    by_id: HashMap<String, RuleDefinition>,
}

impl P0RuleRegistry {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        let profile = required_string(value, "profile", "P0RuleRegistry")?;
        let supported: HashSet<&str> = P0_RULE_IDS.into_iter().collect();
        let mut seen = HashSet::new();
        let mut rules = Vec::new();
        let mut by_id = HashMap::new();

        for (index, rule_value) in required_array(value, "rules", "P0RuleRegistry")?
            .iter()
            .enumerate()
        {
            let artifact = format!("P0RuleRegistry.rules[{}]", index);
            let rule = RuleDefinition::from_value(rule_value, &artifact)?;
            if !supported.contains(rule.rule_id.as_str()) {
                return Err(SentinelError::Registry {
                    message: format!("unsupported P0 rule id {}", rule.rule_id),
                });
            }
            if !seen.insert(rule.rule_id.clone()) {
                return Err(SentinelError::Registry {
                    message: format!("duplicate rule id {}", rule.rule_id),
                });
            }
            by_id.insert(rule.rule_id.clone(), rule.clone());
            rules.push(rule);
        }

        for expected in P0_RULE_IDS {
            if !seen.contains(expected) {
                return Err(SentinelError::Registry {
                    message: format!("missing required P0 rule id {}", expected),
                });
            }
        }

        Ok(Self {
            profile,
            rules,
            by_id,
        })
    }

    pub fn rule(&self, rule_id: &str) -> Option<&RuleDefinition> {
        self.by_id.get(rule_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub path: String,
    pub line: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub diagnostic_id: String,
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub locations: Vec<DiagnosticLocation>,
    pub evidence: Vec<String>,
    pub recommendation: Option<String>,
}

impl Diagnostic {
    pub fn to_value(&self) -> Value {
        let mut value = json!({
            "schema_version": "1.0.0",
            "diagnostic_id": self.diagnostic_id,
            "rule_id": self.rule_id,
            "severity": self.severity.as_str(),
            "message": self.message,
            "locations": self.locations.iter().map(|location| {
                match location.line {
                    Some(line) => json!({"path": location.path, "line": line}),
                    None => json!({"path": location.path}),
                }
            }).collect::<Vec<_>>(),
            "evidence": self.evidence,
        });

        if let Some(recommendation) = &self.recommendation {
            value["recommendation"] = json!(recommendation);
        }

        value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationResult {
    pub decision: Decision,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateArtifactRefs {
    pub diagnostics_ref: Value,
    pub approval_ref: Value,
}

#[derive(Debug, Clone)]
pub struct P0Evaluator {
    registry: P0RuleRegistry,
}

impl P0Evaluator {
    pub fn new(registry: P0RuleRegistry) -> Self {
        Self { registry }
    }

    pub fn evaluate(
        &self,
        task: &SentinelTask,
        changed_lines: &ChangedLines,
    ) -> Result<EvaluationResult, SentinelError> {
        if task.task_id != changed_lines.task_id {
            return Err(SentinelError::InvalidField {
                artifact: "ChangedLines".to_string(),
                field: "task_id".to_string(),
                message: format!(
                    "must match SentinelTask task_id {}, got {}",
                    task.task_id, changed_lines.task_id
                ),
            });
        }

        let mut diagnostics = Vec::new();
        for rule in &self.registry.rules {
            match rule.rule_id.as_str() {
                RULE_SCOPE_ALLOWED_PATHS => {
                    evaluate_allowed_paths(rule, task, changed_lines, &mut diagnostics)
                }
                RULE_TEST_NO_DELETION => {
                    evaluate_test_deletion(rule, changed_lines, &mut diagnostics)
                }
                RULE_DEPENDENCY_REQUIRES_APPROVAL => {
                    evaluate_dependency_changes(rule, changed_lines, &mut diagnostics)
                }
                RULE_SECRET_NO_PLAINTEXT_SECRET => {
                    evaluate_plaintext_secrets(rule, changed_lines, &mut diagnostics)
                }
                RULE_VALIDATOR_NO_SELF_BYPASS => {
                    evaluate_validator_self_bypass(rule, changed_lines, &mut diagnostics)
                }
                _ => {}
            }
        }

        for (index, diagnostic) in diagnostics.iter_mut().enumerate() {
            diagnostic.diagnostic_id = format!("P0-D{:04}", index + 1);
        }

        let decision = diagnostics
            .iter()
            .fold(Decision::AutoPass, |decision, diagnostic| {
                let effect = self
                    .registry
                    .rule(&diagnostic.rule_id)
                    .map(|rule| rule.decision_effect)
                    .unwrap_or_else(|| Decision::default_for_severity(diagnostic.severity));
                decision.max(effect)
            });

        Ok(EvaluationResult {
            decision,
            diagnostics,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureOutcome {
    pub fixture_id: String,
    pub profile: String,
    pub expected_decision: Decision,
    pub expected_diagnostics: Vec<Value>,
}

impl FixtureOutcome {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        Ok(Self {
            fixture_id: required_string(value, "fixture_id", "FixtureOutcome")?,
            profile: required_string(value, "profile", "FixtureOutcome")?,
            expected_decision: Decision::parse(
                &required_string(value, "expected_decision", "FixtureOutcome")?,
                "FixtureOutcome",
                "expected_decision",
            )?,
            expected_diagnostics: required_array(value, "expected_diagnostics", "FixtureOutcome")?
                .to_vec(),
        })
    }

    pub fn matches_result(&self, result: &EvaluationResult) -> bool {
        if self.expected_decision != result.decision {
            return false;
        }

        self.expected_diagnostics.iter().all(|expected| {
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic_matches_expected(diagnostic, expected))
        })
    }
}

pub fn build_diagnostics_artifact(result: &EvaluationResult) -> Value {
    Value::Array(
        result
            .diagnostics
            .iter()
            .map(Diagnostic::to_value)
            .collect(),
    )
}

pub fn build_approval_artifact(task: &SentinelTask, result: &EvaluationResult) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task.task_id,
        "decision": result.decision.as_str(),
        "reasons": approval_reasons(result),
        "diagnostics": build_diagnostics_artifact(result),
        "required_human_actions": required_human_actions(result.decision)
    })
}

pub fn validate_diagnostics_artifact(
    diagnostics: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    let Value::Array(items) = diagnostics else {
        return Err(SentinelError::InvalidField {
            artifact: DIAGNOSTICS_FILE.to_string(),
            field: "$".to_string(),
            message: "expected array of diagnostic objects".to_string(),
        });
    };

    for (index, diagnostic) in items.iter().enumerate() {
        validate_against_schema(
            diagnostic,
            schema_root.as_ref(),
            DIAGNOSTIC_SCHEMA,
            &format!("{}[{}]", DIAGNOSTICS_FILE, index),
        )?;
    }

    Ok(())
}

pub fn validate_approval_artifact(
    approval: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    validate_against_schema(
        approval,
        schema_root.as_ref(),
        APPROVAL_SCHEMA,
        APPROVAL_FILE,
    )
}

pub fn write_gate_artifacts(
    store: &StateStore,
    job_id: &str,
    task: &SentinelTask,
    result: &EvaluationResult,
    schema_root: impl AsRef<Path>,
) -> Result<GateArtifactRefs, SentinelError> {
    let diagnostics = build_diagnostics_artifact(result);
    validate_diagnostics_artifact(&diagnostics, schema_root.as_ref())?;
    let approval = build_approval_artifact(task, result);
    validate_approval_artifact(&approval, schema_root.as_ref())?;

    let diagnostics_ref = store
        .write_tool_json(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            DIAGNOSTICS_FILE,
            &diagnostics,
        )
        .map_err(|source| SentinelError::State { source })?;
    let approval_ref = store
        .write_tool_json(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            APPROVAL_FILE,
            &approval,
        )
        .map_err(|source| SentinelError::State { source })?;

    Ok(GateArtifactRefs {
        diagnostics_ref,
        approval_ref,
    })
}

pub fn read_task(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<SentinelTask, SentinelError> {
    let value = read_validated_json(path.as_ref(), schema_root.as_ref(), SENTINEL_TASK_SCHEMA)?;
    SentinelTask::from_value(&value)
}

pub fn read_changed_lines(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<ChangedLines, SentinelError> {
    let value = read_validated_json(path.as_ref(), schema_root.as_ref(), CHANGED_LINES_SCHEMA)?;
    ChangedLines::from_value(&value)
}

pub fn read_p0_rule_registry(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<P0RuleRegistry, SentinelError> {
    let value = read_validated_json(path.as_ref(), schema_root.as_ref(), P0_RULE_REGISTRY_SCHEMA)?;
    P0RuleRegistry::from_value(&value)
}

pub fn read_fixture_outcome(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<FixtureOutcome, SentinelError> {
    let value = read_validated_json(path.as_ref(), schema_root.as_ref(), FIXTURE_OUTCOME_SCHEMA)?;
    FixtureOutcome::from_value(&value)
}

fn evaluate_allowed_paths(
    rule: &RuleDefinition,
    task: &SentinelTask,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        for path in file.changed_paths() {
            if !path_is_allowed(path, &task.allowed_paths) {
                push_file_diagnostic(
                    diagnostics,
                    rule,
                    path,
                    "Changed file is outside task allowed_paths.",
                    vec![format!(
                        "path {} is not covered by task.allowed_paths",
                        path
                    )],
                    Some("Keep changes inside the task allowed_paths or request scope approval."),
                );
            }
        }
    }
}

fn evaluate_test_deletion(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if file.change_type == "deleted"
            && file.changed_paths().iter().any(|path| is_test_path(path))
        {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Test file deletion is not allowed in P0.",
                vec!["deleted changed file is classified as test coverage".to_string()],
                Some("Restore the test file or request explicit human review with replacement coverage."),
            );
        }
    }
}

fn evaluate_dependency_changes(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if file
            .changed_paths()
            .iter()
            .any(|path| is_dependency_path(path))
        {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Dependency manifest or lockfile changed and requires human review.",
                vec!["dependency-related artifact changed".to_string()],
                Some("Record approval before automatic progress continues."),
            );
        }
    }
}

fn evaluate_plaintext_secrets(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        for line in added_lines(file) {
            if is_plaintext_secret_candidate(&line.content) {
                push_line_diagnostic(
                    diagnostics,
                    rule,
                    &file.path,
                    line.new_line,
                    "Added line contains a plaintext secret candidate.",
                    vec![
                        "secret-like token detected in added line; raw value intentionally omitted"
                            .to_string(),
                    ],
                    Some("Remove the secret and use an approved credential reference."),
                );
            }
        }
    }
}

fn evaluate_validator_self_bypass(
    rule: &RuleDefinition,
    changed_lines: &ChangedLines,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for file in &changed_lines.files {
        if !file
            .changed_paths()
            .iter()
            .any(|path| is_validator_path(path))
        {
            continue;
        }

        if file.change_type == "deleted" {
            push_file_diagnostic(
                diagnostics,
                rule,
                &file.path,
                "Validator, policy, schema, or CI artifact deletion can bypass validation.",
                vec!["validation-related artifact was deleted".to_string()],
                Some("Restore the validator artifact or route through explicit review."),
            );
            continue;
        }

        for line in changed_content_lines(file) {
            if is_self_bypass_line(&line.content) {
                push_line_diagnostic(
                    diagnostics,
                    rule,
                    &file.path,
                    line.new_line.or(line.old_line),
                    "Validator-related change appears to bypass enforcement.",
                    vec![
                        "bypass-like validation change detected; raw line intentionally omitted"
                            .to_string(),
                    ],
                    Some("Keep validation strict or request explicit policy review."),
                );
            }
        }
    }
}

fn push_file_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    rule: &RuleDefinition,
    path: &str,
    message: &str,
    evidence: Vec<String>,
    recommendation: Option<&str>,
) {
    diagnostics.push(Diagnostic {
        diagnostic_id: String::new(),
        rule_id: rule.rule_id.clone(),
        severity: rule.severity,
        message: message.to_string(),
        locations: vec![DiagnosticLocation {
            path: normalize_path(path),
            line: None,
        }],
        evidence,
        recommendation: recommendation.map(str::to_string),
    });
}

fn push_line_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    rule: &RuleDefinition,
    path: &str,
    line: Option<i64>,
    message: &str,
    evidence: Vec<String>,
    recommendation: Option<&str>,
) {
    diagnostics.push(Diagnostic {
        diagnostic_id: String::new(),
        rule_id: rule.rule_id.clone(),
        severity: rule.severity,
        message: message.to_string(),
        locations: vec![DiagnosticLocation {
            path: normalize_path(path),
            line,
        }],
        evidence,
        recommendation: recommendation.map(str::to_string),
    });
}

fn read_validated_json(
    path: &Path,
    schema_root: &Path,
    schema_name: &str,
) -> Result<Value, SentinelError> {
    let value = load_document(path).map_err(|source| SentinelError::DocumentLoad { source })?;
    validate_against_schema(
        &value,
        schema_root,
        schema_name,
        &path.display().to_string(),
    )?;
    Ok(value)
}

fn validate_against_schema(
    value: &Value,
    schema_root: &Path,
    schema_name: &str,
    artifact: &str,
) -> Result<(), SentinelError> {
    let schema_path = schema_root.join(schema_name);
    let schema =
        load_schema(&schema_path).map_err(|source| SentinelError::SchemaLoad { source })?;
    let validation = validate_json(value, &schema);
    if validation.is_ok() {
        Ok(())
    } else {
        Err(SentinelError::SchemaValidation {
            artifact: artifact.to_string(),
            schema: schema_name.to_string(),
            errors: schema_errors(&validation),
        })
    }
}

fn schema_errors(validation: &star_control_schema::ValidationResult) -> Vec<String> {
    validation
        .errors
        .iter()
        .map(|error| format!("{}: {}", error.location, error.message))
        .collect()
}

fn required_string(value: &Value, field: &str, artifact: &str) -> Result<String, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| missing_field(artifact, field))
}

fn optional_string(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Option<String>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid_field(artifact, field, "expected string or null")),
    }
}

fn required_integer(value: &Value, field: &str, artifact: &str) -> Result<i64, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| missing_field(artifact, field))
}

fn optional_integer(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Option<i64>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| invalid_field(artifact, field, "expected integer")),
        Some(_) => Err(invalid_field(artifact, field, "expected integer or null")),
    }
}

fn required_array<'a>(
    value: &'a Value,
    field: &str,
    artifact: &str,
) -> Result<&'a Vec<Value>, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(artifact, field))
}

fn required_string_array(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Vec<String>, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(artifact, field))?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                invalid_field(
                    artifact,
                    &format!("{}[{}]", field, index),
                    "expected string",
                )
            })
        })
        .collect()
}

fn optional_string_array(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Vec<String>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    invalid_field(
                        artifact,
                        &format!("{}[{}]", field, index),
                        "expected string",
                    )
                })
            })
            .collect(),
        Some(_) => Err(invalid_field(artifact, field, "expected array")),
    }
}

fn missing_field(artifact: &str, field: &str) -> SentinelError {
    SentinelError::MissingField {
        artifact: artifact.to_string(),
        field: field.to_string(),
    }
}

fn invalid_field(artifact: &str, field: &str, message: &str) -> SentinelError {
    SentinelError::InvalidField {
        artifact: artifact.to_string(),
        field: field.to_string(),
        message: message.to_string(),
    }
}

fn added_lines(file: &ChangedFile) -> impl Iterator<Item = &ChangedLine> {
    file.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .filter(|line| line.kind == "added")
}

fn changed_content_lines(file: &ChangedFile) -> impl Iterator<Item = &ChangedLine> {
    file.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .filter(|line| line.kind == "added" || line.kind == "removed")
}

fn path_is_allowed(path: &str, allowed_paths: &[String]) -> bool {
    if allowed_paths.is_empty() {
        return false;
    }
    allowed_paths
        .iter()
        .any(|pattern| path_matches_pattern(pattern, path))
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern = normalize_path(pattern);
    let path = normalize_path(path);
    if pattern == "**" || pattern == "**/*" {
        return true;
    }
    wildcard_match(&pattern, &path)
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let mut table = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    table[0][0] = true;

    for pattern_index in 1..=pattern.len() {
        if pattern[pattern_index - 1] == '*' {
            table[pattern_index][0] = table[pattern_index - 1][0];
        }
    }

    for pattern_index in 1..=pattern.len() {
        for text_index in 1..=text.len() {
            table[pattern_index][text_index] = if pattern[pattern_index - 1] == '*' {
                table[pattern_index - 1][text_index] || table[pattern_index][text_index - 1]
            } else {
                table[pattern_index - 1][text_index - 1]
                    && pattern[pattern_index - 1] == text[text_index - 1]
            };
        }
    }

    table[pattern.len()][text.len()]
}

fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .trim_start_matches('/')
        .to_string()
}

fn is_test_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    let name = path.rsplit('/').next().unwrap_or(&path);
    path == "tests"
        || path.starts_with("tests/")
        || path.contains("/tests/")
        || path.contains("/__tests__/")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with("_test.rs")
        || name.ends_with("_test.go")
}

fn is_dependency_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    let name = path.rsplit('/').next().unwrap_or(&path);
    matches!(
        name,
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "pipfile"
            | "pipfile.lock"
            | "go.mod"
            | "go.sum"
            | "gemfile"
            | "gemfile.lock"
            | "composer.json"
            | "composer.lock"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.lockfile"
            | "packages.lock.json"
    ) || name.ends_with(".csproj")
}

fn is_validator_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    path.starts_with(".github/workflows/")
        || path.starts_with("scripts/ci/")
        || path == "scripts/test.ps1"
        || path.starts_with("builtin-tools/star-sentinel/policies/")
        || path.starts_with("builtin-tools/star-sentinel/schemas/")
        || path.starts_with("builtin-tools/star-sentinel/fixtures/")
        || path.starts_with("packages/star-sentinel/")
}

fn is_plaintext_secret_candidate(content: &str) -> bool {
    let trimmed = content.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("-----begin ") && lower.contains(" private key") {
        return true;
    }
    if contains_token_with_min_suffix(trimmed, "sk-", 12) {
        return true;
    }

    let key_names = [
        "api_key",
        "apikey",
        "secret",
        "token",
        "password",
        "private_key",
        "client_secret",
        "access_key",
    ];
    key_names.iter().any(|name| lower.contains(name))
        && (lower.contains('=') || lower.contains(':'))
        && !is_placeholder_secret(&lower)
}

fn contains_token_with_min_suffix(content: &str, marker: &str, min_suffix: usize) -> bool {
    let Some(index) = content.find(marker) else {
        return false;
    };
    content[index + marker.len()..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .count()
        >= min_suffix
}

fn is_placeholder_secret(lower: &str) -> bool {
    [
        "example",
        "placeholder",
        "redacted",
        "changeme",
        "todo",
        "****",
        "<secret>",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn is_self_bypass_line(content: &str) -> bool {
    let lower = content.trim().to_ascii_lowercase();
    lower.contains("bypass")
        || lower.contains("skip validation")
        || lower.contains("disable validation")
        || lower.contains("ignore validation")
        || lower.contains("continue-on-error: true")
        || lower.contains("allow_failure: true")
        || lower.contains("exit 0")
        || lower.contains("|| true")
        || lower.contains("set +e")
}

fn diagnostic_matches_expected(diagnostic: &Diagnostic, expected: &Value) -> bool {
    if let Some(rule_id) = expected.get("rule_id").and_then(Value::as_str) {
        if diagnostic.rule_id != rule_id {
            return false;
        }
    }
    if let Some(severity) = expected.get("severity").and_then(Value::as_str) {
        if diagnostic.severity.as_str() != severity {
            return false;
        }
    }
    if let Some(path) = expected.get("path").and_then(Value::as_str) {
        if !diagnostic
            .locations
            .iter()
            .any(|location| location.path == normalize_path(path))
        {
            return false;
        }
    }
    true
}

fn approval_reasons(result: &EvaluationResult) -> Vec<String> {
    if result.diagnostics.is_empty() {
        return vec!["No P0 diagnostics were produced.".to_string()];
    }

    let mut rules = BTreeSet::new();
    for diagnostic in &result.diagnostics {
        rules.insert((diagnostic.severity.as_str(), diagnostic.rule_id.as_str()));
    }

    rules
        .into_iter()
        .map(|(severity, rule_id)| format!("{} diagnostic from {}", severity, rule_id))
        .collect()
}

fn required_human_actions(decision: Decision) -> Vec<String> {
    match decision {
        Decision::AutoPass => Vec::new(),
        Decision::HumanReview => vec![
            "Review HUMAN_REVIEW diagnostics and record approval before continuing.".to_string(),
        ],
        Decision::Block => {
            vec!["Resolve BLOCK diagnostics before continuing automatically.".to_string()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_control_state::StateStore;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_builtin_registry() {
        let registry = builtin_registry();

        assert_eq!(registry.profile, "quick");
        assert!(registry.rule(RULE_SCOPE_ALLOWED_PATHS).is_some());
        assert_eq!(
            registry
                .rule(RULE_DEPENDENCY_REQUIRES_APPROVAL)
                .expect("dependency rule")
                .decision_effect,
            Decision::HumanReview
        );
    }

    #[test]
    fn scope_fixture_blocks_out_of_scope_change() {
        let registry = builtin_registry();
        let evaluator = P0Evaluator::new(registry);
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let changed_lines = changed_lines(json!([
            file("src/allowed/index.ts", "modified", json!([])),
            file("src/other/hidden.ts", "modified", json!([]))
        ]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert_eq!(result.decision, Decision::Block);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == RULE_SCOPE_ALLOWED_PATHS
                && diagnostic.severity == Severity::Block
                && diagnostic.locations[0].path == "src/other/hidden.ts"
        }));
        assert_diagnostics_schema_valid(&result.diagnostics);
    }

    #[test]
    fn dependency_change_requires_human_review() {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["**"]);
        let changed_lines = changed_lines(json!([file("Cargo.toml", "modified", json!([]))]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert_eq!(result.decision, Decision::HumanReview);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == RULE_DEPENDENCY_REQUIRES_APPROVAL
                && diagnostic.severity == Severity::Warn
        }));
    }

    #[test]
    fn test_file_deletion_blocks() {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["**"]);
        let changed_lines =
            changed_lines(json!([file("tests/runtime_test.rs", "deleted", json!([]))]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert_eq!(result.decision, Decision::Block);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == RULE_TEST_NO_DELETION));
    }

    #[test]
    fn plaintext_secret_blocks_without_echoing_raw_secret() {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["**"]);
        let changed_lines = changed_lines(json!([file(
            "src/config.ts",
            "modified",
            json!([
                {"kind": "added", "old_line": null, "new_line": 7, "content": "const api_key = \"sk-test1234567890\";"}
            ])
        )]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert_eq!(result.decision, Decision::Block);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule_id == RULE_SECRET_NO_PLAINTEXT_SECRET)
            .expect("secret diagnostic");
        let rendered = diagnostic.to_value().to_string();
        assert!(!rendered.contains("sk-test1234567890"));
    }

    #[test]
    fn validator_self_bypass_blocks() {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["**"]);
        let changed_lines = changed_lines(json!([file(
            ".github/workflows/ci.yml",
            "modified",
            json!([
                {"kind": "added", "old_line": null, "new_line": 12, "content": "continue-on-error: true"}
            ])
        )]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert_eq!(result.decision, Decision::Block);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == RULE_VALIDATOR_NO_SELF_BYPASS));
    }

    #[test]
    fn reads_schema_valid_json_inputs() {
        let temp_dir = temp_dir();
        let task_path = temp_dir.join("task.json");
        let changed_path = temp_dir.join("changed-lines.json");
        fs::write(&task_path, task_value(["src/**"]).to_string()).expect("write task");
        fs::write(
            &changed_path,
            changed_lines_value(json!([file("src/main.rs", "modified", json!([]))])).to_string(),
        )
        .expect("write changed lines");

        let task = read_task(&task_path, schema_root()).expect("read task");
        let changed_lines = read_changed_lines(&changed_path, schema_root()).expect("read changed");

        assert_eq!(task.task_id, "p0-task-demo");
        assert_eq!(changed_lines.files[0].path, "src/main.rs");
        fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn builtin_scope_fixture_outcome_matches_evaluation() {
        let outcome = read_fixture_outcome(
            repo_root().join(
                "builtin-tools/star-sentinel/examples/p0/fixture-outcome-scope-block.example.json",
            ),
            schema_root(),
        )
        .expect("fixture outcome");
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let changed_lines = changed_lines(json!([
            file("src/allowed/index.ts", "modified", json!([])),
            file("src/other/hidden.ts", "modified", json!([]))
        ]));

        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

        assert!(outcome.matches_result(&result));
    }

    #[test]
    fn builds_schema_valid_gate_artifacts_for_block() {
        let result = scope_block_result();
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let diagnostics = build_diagnostics_artifact(&result);
        let approval = build_approval_artifact(&task, &result);

        validate_diagnostics_artifact(&diagnostics, schema_root()).expect("diagnostics schema");
        validate_approval_artifact(&approval, schema_root()).expect("approval schema");
        assert_eq!(approval["decision"], "BLOCK");
        assert_eq!(
            approval["diagnostics"][0]["rule_id"],
            RULE_SCOPE_ALLOWED_PATHS
        );
    }

    #[test]
    fn builds_human_review_gate_for_dependency_change() {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["**"]);
        let changed_lines = changed_lines(json!([file("Cargo.toml", "modified", json!([]))]));
        let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");
        let approval = build_approval_artifact(&task, &result);

        validate_approval_artifact(&approval, schema_root()).expect("approval schema");
        assert_eq!(approval["decision"], "HUMAN_REVIEW");
        assert_eq!(
            approval["required_human_actions"][0],
            "Review HUMAN_REVIEW diagnostics and record approval before continuing."
        );
    }

    #[test]
    fn writes_gate_artifacts_to_state_store_tool_output() {
        let temp_project = temp_dir();
        let store =
            StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
        let job = store
            .create_job("validate p0 output", "star-sentinel", Vec::new())
            .expect("job");
        let job_id = job["job_id"].as_str().expect("job_id");
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let result = scope_block_result();

        let refs =
            write_gate_artifacts(&store, job_id, &task, &result, schema_root()).expect("write");

        assert_eq!(refs.diagnostics_ref["kind"], "tool_output");
        assert_eq!(
            refs.diagnostics_ref["path"],
            "tool-output/star-sentinel/diagnostics.json"
        );
        assert_eq!(
            refs.approval_ref["path"],
            "tool-output/star-sentinel/approval.json"
        );
        assert!(temp_project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
            .is_file());
        assert!(temp_project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
            .is_file());
        fs::remove_dir_all(temp_project).ok();
    }

    #[test]
    fn gate_writer_refuses_to_overwrite_existing_artifacts() {
        let temp_project = temp_dir();
        let store =
            StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
        let job = store
            .create_job("validate p0 output", "star-sentinel", Vec::new())
            .expect("job");
        let job_id = job["job_id"].as_str().expect("job_id");
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let result = scope_block_result();

        write_gate_artifacts(&store, job_id, &task, &result, schema_root()).expect("first write");
        let overwrite = write_gate_artifacts(&store, job_id, &task, &result, schema_root());

        assert!(matches!(overwrite, Err(SentinelError::State { .. })));
        fs::remove_dir_all(temp_project).ok();
    }

    fn builtin_registry() -> P0RuleRegistry {
        read_p0_rule_registry(
            repo_root().join("builtin-tools/star-sentinel/policies/p0-rule-registry.json"),
            schema_root(),
        )
        .expect("builtin registry")
    }

    fn scope_block_result() -> EvaluationResult {
        let evaluator = P0Evaluator::new(builtin_registry());
        let task = task_with_allowed_paths(["src/allowed/**"]);
        let changed_lines = changed_lines(json!([
            file("src/allowed/index.ts", "modified", json!([])),
            file("src/other/hidden.ts", "modified", json!([]))
        ]));
        evaluator.evaluate(&task, &changed_lines).expect("evaluate")
    }

    fn assert_diagnostics_schema_valid(diagnostics: &[Diagnostic]) {
        let schema =
            load_schema(schema_root().join("diagnostic.schema.json")).expect("diagnostic schema");
        for diagnostic in diagnostics {
            let result = validate_json(&diagnostic.to_value(), &schema);
            assert!(result.is_ok(), "{:?}", result.errors);
        }
    }

    fn task_with_allowed_paths<const N: usize>(allowed_paths: [&str; N]) -> SentinelTask {
        SentinelTask::from_value(&task_value(allowed_paths)).expect("task")
    }

    fn task_value<const N: usize>(allowed_paths: [&str; N]) -> Value {
        let allowed_paths: Vec<&str> = allowed_paths.into_iter().collect();
        json!({
            "schema_version": "1.0.0",
            "task_id": "p0-task-demo",
            "goal": "Validate P0 evaluator behavior.",
            "allowed_paths": allowed_paths,
            "forbidden_paths": [],
            "forbidden_change_types": [],
            "required_validation": [],
            "approval_required_changes": []
        })
    }

    fn changed_lines(files: Value) -> ChangedLines {
        ChangedLines::from_value(&changed_lines_value(files)).expect("changed lines")
    }

    fn changed_lines_value(files: Value) -> Value {
        json!({
            "schema_version": "1.0.0",
            "task_id": "p0-task-demo",
            "files": files
        })
    }

    fn file(path: &str, change_type: &str, lines: Value) -> Value {
        json!({
            "path": path,
            "change_type": change_type,
            "old_path": null,
            "hunks": [
                {
                    "old_start": 1,
                    "old_lines": 1,
                    "new_start": 1,
                    "new_lines": 1,
                    "lines": lines
                }
            ]
        })
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn schema_root() -> PathBuf {
        repo_root().join("builtin-tools/star-sentinel/schemas")
    }

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("star-sentinel-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }
}
