use serde_json::{json, Map, Value};
use star_control_provider::{CapabilityValue, ProviderRegistry, ProviderRegistryError};
use star_control_schema::{load_schema, validate_json, ValidationError};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

const JOB_SCHEMA: &str = "job.schema.json";
const ROUTE_SCHEMA: &str = "route.schema.json";
const ROUTER_DECISION_SCHEMA: &str = "router-decision.schema.json";
const WORKSPEC_SCHEMA: &str = "workspec.schema.json";
const SCHEMA_VERSION: &str = "1.0.0";
const FAKE_PROVIDER_ID: &str = "provider.fake";

#[derive(Debug)]
pub enum RouterError {
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
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
    ProviderRegistry(ProviderRegistryError),
    NoProviderAvailable {
        role: String,
        capability: String,
    },
}

impl fmt::Display for RouterError {
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
            Self::ProviderRegistry(source) => {
                write!(formatter, "provider registry error: {}", source)
            }
            Self::NoProviderAvailable { role, capability } => write!(
                formatter,
                "no provider available for role {} requiring {}",
                role, capability
            ),
        }
    }
}

impl Error for RouterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProviderRegistry(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ProviderRegistryError> for RouterError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::ProviderRegistry(source)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobSpec {
    job_id: String,
    project_root: String,
    request_text: String,
    user_constraints: Vec<String>,
    value: Value,
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
    value: Value,
}

impl RouterDecision {
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteSpec {
    value: Value,
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
    stage: String,
    value: Value,
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
    decision: RouterDecision,
    route: RouteSpec,
    workspecs: BTreeMap<String, WorkSpec>,
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

#[derive(Debug, Clone)]
pub struct RouterEngine<'a> {
    registry: &'a ProviderRegistry,
    schema_root: PathBuf,
}

impl<'a> RouterEngine<'a> {
    pub fn new(registry: &'a ProviderRegistry, schema_root: impl Into<PathBuf>) -> Self {
        Self {
            registry,
            schema_root: schema_root.into(),
        }
    }

    pub fn route(&self, job: &JobSpec) -> Result<RouterOutput, RouterError> {
        let analysis = RequestAnalysis::analyze(job);
        let provider_instance_id =
            self.select_fake_provider_instance("worker-impl", "return_json")?;
        let stages = analysis.stages();
        let assignments = assignments_for_stages(&stages, &provider_instance_id, analysis.profile);
        let workspec_paths = workspec_paths_for_stages(&stages);

        let decision_value = json!({
            "schema_version": SCHEMA_VERSION,
            "decision_id": decision_id(job.job_id()),
            "size": analysis.size.as_str(),
            "risk": analysis.risk.as_str(),
            "policy_profile": analysis.profile.as_str(),
            "decision": analysis.decision.as_str(),
            "requires_user_approval": analysis.requires_user_approval,
            "approval_reasons": analysis.approval_reasons,
            "change_types": analysis.change_type_strings(),
            "routing_reasons": analysis.routing_reasons,
            "recommended_stages": stages,
        });
        validate_contract(
            &decision_value,
            Path::new("router-decision.json"),
            &self.schema_root,
            ROUTER_DECISION_SCHEMA,
        )?;

        let route_value = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job.job_id(),
            "summary": summary(job.request_text()),
            "size": analysis.size.as_str(),
            "risk": analysis.risk.as_str(),
            "policy_profile": analysis.profile.as_str(),
            "decision": analysis.decision.as_str(),
            "change_types": analysis.change_type_strings(),
            "routing_reasons": analysis.routing_reasons,
            "stages": stages,
            "assignments": assignments,
            "requires_user_approval": analysis.requires_user_approval,
            "approval_reasons": analysis.approval_reasons,
            "workspecs": workspec_paths,
        });
        validate_contract(
            &route_value,
            Path::new("route.json"),
            &self.schema_root,
            ROUTE_SCHEMA,
        )?;

        let mut workspecs = BTreeMap::new();
        for stage in stages.iter().filter(|stage| **stage != "route") {
            let workspec = self.workspec_for_stage(job, stage, &provider_instance_id, &analysis)?;
            workspecs.insert(stage.to_string(), workspec);
        }

        Ok(RouterOutput {
            decision: RouterDecision {
                value: decision_value,
            },
            route: RouteSpec { value: route_value },
            workspecs,
        })
    }

    fn select_fake_provider_instance(
        &self,
        role: &str,
        capability: &str,
    ) -> Result<String, RouterError> {
        for instance in self.registry.enabled_instances() {
            let manifest = self.registry.manifest_for_instance(instance.id())?;
            if manifest.id() != FAKE_PROVIDER_ID {
                continue;
            }
            let profile = self.registry.capability_for_instance(instance.id())?;
            let has_capability = profile
                .capability(capability)
                .map(CapabilityValue::is_enabled)
                .unwrap_or(false);
            let offline = profile
                .capability("work_offline")
                .map(CapabilityValue::is_enabled)
                .unwrap_or(false);
            if has_capability && offline {
                return Ok(instance.id().to_string());
            }
        }

        Err(RouterError::NoProviderAvailable {
            role: role.to_string(),
            capability: capability.to_string(),
        })
    }

    fn workspec_for_stage(
        &self,
        job: &JobSpec,
        stage: &str,
        provider_instance_id: &str,
        analysis: &RequestAnalysis,
    ) -> Result<WorkSpec, RouterError> {
        let workspec = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job.job_id(),
            "stage": stage,
            "role": role_for_stage(stage),
            "provider": provider_instance_id,
            "provider_instance": provider_instance_id,
            "project_root": job.project_root(),
            "goal": job.request_text(),
            "allowed_scope": allowed_scope(&analysis.change_types),
            "forbidden_actions": forbidden_actions(&analysis.change_types),
            "required_outputs": [
                format!("provider-output/{}/response.json", provider_instance_id)
            ],
            "validation_requirements": [
                format!("policy:{}", analysis.profile.as_str())
            ],
            "context_pack": {
                "source": "router",
                "change_types": analysis.change_type_strings()
            }
        });
        validate_contract(
            &workspec,
            Path::new(&format!("workspecs/{}.json", stage)),
            &self.schema_root,
            WORKSPEC_SCHEMA,
        )?;
        Ok(WorkSpec {
            stage: stage.to_string(),
            value: workspec,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Size {
    Small = 1,
    Medium = 2,
    Large = 3,
    Critical = 4,
}

impl Size {
    fn as_str(self) -> &'static str {
        match self {
            Self::Small => "SMALL",
            Self::Medium => "MEDIUM",
            Self::Large => "LARGE",
            Self::Critical => "CRITICAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Risk {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl Risk {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyProfile {
    Quick,
    Near,
    Full,
    Security,
    Release,
    Validator,
}

impl PolicyProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Near => "near",
            Self::Full => "full",
            Self::Security => "security",
            Self::Release => "release",
            Self::Validator => "validator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteDecision {
    AutoPass,
    HumanReview,
    Block,
}

impl RouteDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "AUTO_PASS",
            Self::HumanReview => "HUMAN_REVIEW",
            Self::Block => "BLOCK",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ChangeType {
    DocsOnly,
    ExampleChange,
    SchemaChange,
    SchemaBreakingChange,
    RuntimeCodeChange,
    MultiPackageChange,
    ProviderContractChange,
    DependencyAddition,
    DependencyVersionChange,
    WorkflowChange,
    PublicApiChange,
    CredentialChange,
    SensitiveDataExposure,
    ReleaseChange,
    DeployChange,
    ValidatorSensitiveChange,
    ValidatorSelfBypass,
    FileDeletion,
    BulkMove,
    RiskPathChange,
    ExternalAccountChange,
    BudgetExceeded,
    UnknownHighRisk,
}

impl ChangeType {
    fn as_str(self) -> &'static str {
        match self {
            Self::DocsOnly => "docs_only",
            Self::ExampleChange => "example_change",
            Self::SchemaChange => "schema_change",
            Self::SchemaBreakingChange => "schema_breaking_change",
            Self::RuntimeCodeChange => "runtime_code_change",
            Self::MultiPackageChange => "multi_package_change",
            Self::ProviderContractChange => "provider_contract_change",
            Self::DependencyAddition => "dependency_addition",
            Self::DependencyVersionChange => "dependency_version_change",
            Self::WorkflowChange => "workflow_change",
            Self::PublicApiChange => "public_api_change",
            Self::CredentialChange => "credential_change",
            Self::SensitiveDataExposure => "sensitive_data_exposure",
            Self::ReleaseChange => "release_change",
            Self::DeployChange => "deploy_change",
            Self::ValidatorSensitiveChange => "validator_sensitive_change",
            Self::ValidatorSelfBypass => "validator_self_bypass",
            Self::FileDeletion => "file_deletion",
            Self::BulkMove => "bulk_move",
            Self::RiskPathChange => "risk_path_change",
            Self::ExternalAccountChange => "external_account_change",
            Self::BudgetExceeded => "budget_exceeded",
            Self::UnknownHighRisk => "unknown_high_risk",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RequestAnalysis {
    change_types: Vec<ChangeType>,
    routing_reasons: Vec<String>,
    approval_reasons: Vec<String>,
    size: Size,
    risk: Risk,
    profile: PolicyProfile,
    decision: RouteDecision,
    requires_user_approval: bool,
}

impl RequestAnalysis {
    fn analyze(job: &JobSpec) -> Self {
        let haystack = normalized_haystack(job);
        let mut change_types = Vec::new();
        let mut reasons = Vec::new();

        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["readme", "docs", "documentation", "문서"]),
            ChangeType::DocsOnly,
            "documentation change requested",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["example", "fixture", "예시", "샘플"]),
            ChangeType::ExampleChange,
            "example or fixture change requested",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["schema", "specs/schemas", "스키마"]),
            ChangeType::SchemaChange,
            "schema files or schema contract mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["breaking schema", "schema breaking", "breaking change"],
            ),
            ChangeType::SchemaBreakingChange,
            "breaking schema change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["implement", "runtime", "code", "rust", "구현", "코드"],
            ),
            ChangeType::RuntimeCodeChange,
            "runtime implementation requested",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["multi package", "workspace", "여러 package", "여러 패키지"],
            ),
            ChangeType::MultiPackageChange,
            "multi-package change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["provider contract", "provider registry"]),
            ChangeType::ProviderContractChange,
            "provider contract change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &[
                    "dependency",
                    "cargo.toml",
                    "package.json",
                    "의존성",
                    "패키지 설치",
                ],
            ),
            ChangeType::DependencyAddition,
            "dependency addition or package manager change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["dependency version", "version bump", "버전 변경"],
            ),
            ChangeType::DependencyVersionChange,
            "dependency version change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["workflow", ".github/workflows", "ci", "워크플로"],
            ),
            ChangeType::WorkflowChange,
            "workflow or CI change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["public api", "breaking api", "공개 api"]),
            ChangeType::PublicApiChange,
            "public API change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &[
                    "credential",
                    "api key",
                    "token",
                    "password",
                    "비밀번호",
                    "토큰",
                ],
            ),
            ChangeType::CredentialChange,
            "credential-sensitive change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &[
                    "print secret",
                    "show token",
                    "dump credential",
                    "민감정보 출력",
                    "비밀 출력",
                ],
            ),
            ChangeType::SensitiveDataExposure,
            "sensitive data exposure requested",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["release", "publish", "릴리즈", "배포 준비"]),
            ChangeType::ReleaseChange,
            "release change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["deploy", "deployment", "배포"]),
            ChangeType::DeployChange,
            "deploy change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &[
                    "validator",
                    "star sentinel",
                    "policy",
                    "scripts/ci",
                    "검증기",
                ],
            ),
            ChangeType::ValidatorSensitiveChange,
            "validator-sensitive contract changed",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &[
                    "disable validator",
                    "skip ci",
                    "bypass validator",
                    "검증 우회",
                ],
            ),
            ChangeType::ValidatorSelfBypass,
            "validator self-bypass requested",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["delete file", "remove file", "파일 삭제", "삭제"],
            ),
            ChangeType::FileDeletion,
            "file deletion mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["bulk move", "move all", "대량 이동"]),
            ChangeType::BulkMove,
            "bulk move mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["security path", "permission", "권한"]),
            ChangeType::RiskPathChange,
            "risk-sensitive path mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["external account", "github settings", "외부 계정"],
            ),
            ChangeType::ExternalAccountChange,
            "external account change mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(
                &haystack,
                &["budget exceeded", "quota exceeded", "예산 초과"],
            ),
            ChangeType::BudgetExceeded,
            "budget or quota risk mentioned",
        );
        push_if(
            &mut change_types,
            &mut reasons,
            contains_any(&haystack, &["unknown risk", "risky", "위험"]),
            ChangeType::UnknownHighRisk,
            "unknown high risk mentioned",
        );

        if change_types.is_empty() {
            change_types.push(ChangeType::RuntimeCodeChange);
            reasons.push("defaulted to runtime code change".to_string());
        }
        change_types.sort();
        change_types.dedup();

        let size = size_for(&change_types);
        let risk = risk_for(&change_types);
        let profile = profile_for(&change_types, size, risk);
        let requires_user_approval = requires_approval(&change_types);
        let blocks = blocks(&change_types);
        let decision = if blocks {
            RouteDecision::Block
        } else if requires_user_approval {
            RouteDecision::HumanReview
        } else {
            RouteDecision::AutoPass
        };
        let approval_reasons = approval_reasons_for(&change_types, profile, blocks);

        Self {
            change_types,
            routing_reasons: reasons,
            approval_reasons,
            size,
            risk,
            profile,
            decision,
            requires_user_approval,
        }
    }

    fn change_type_strings(&self) -> Vec<&'static str> {
        self.change_types
            .iter()
            .map(|change_type| change_type.as_str())
            .collect()
    }

    fn stages(&self) -> Vec<&'static str> {
        if self.decision == RouteDecision::Block {
            return vec!["route", "report"];
        }
        if self.change_types.iter().any(|change_type| {
            matches!(
                change_type,
                ChangeType::ReleaseChange | ChangeType::DeployChange
            )
        }) {
            return vec!["design", "validate", "review", "report"];
        }
        if self.change_types.iter().any(|change_type| {
            matches!(
                change_type,
                ChangeType::SchemaChange
                    | ChangeType::SchemaBreakingChange
                    | ChangeType::ValidatorSensitiveChange
                    | ChangeType::ValidatorSelfBypass
            )
        }) {
            return vec!["design", "implement", "validate", "review", "report"];
        }
        if self.change_types.contains(&ChangeType::RuntimeCodeChange) {
            return vec![
                "design",
                "implement",
                "validate",
                "review",
                "polish",
                "report",
            ];
        }
        vec!["implement", "validate", "review", "report"]
    }
}

fn normalized_haystack(job: &JobSpec) -> String {
    let constraints = job.user_constraints().join(" ");
    format!("{} {}", job.request_text(), constraints).to_lowercase()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn push_if(
    change_types: &mut Vec<ChangeType>,
    reasons: &mut Vec<String>,
    condition: bool,
    change_type: ChangeType,
    reason: &str,
) {
    if condition {
        change_types.push(change_type);
        reasons.push(reason.to_string());
    }
}

fn size_for(change_types: &[ChangeType]) -> Size {
    let mut size = Size::Small;
    for change_type in change_types {
        size = size.max(match change_type {
            ChangeType::RuntimeCodeChange | ChangeType::SchemaChange => Size::Medium,
            ChangeType::MultiPackageChange
            | ChangeType::ProviderContractChange
            | ChangeType::PublicApiChange => Size::Large,
            ChangeType::ReleaseChange
            | ChangeType::DeployChange
            | ChangeType::CredentialChange
            | ChangeType::SensitiveDataExposure
            | ChangeType::ValidatorSelfBypass
            | ChangeType::ExternalAccountChange => Size::Critical,
            ChangeType::DependencyAddition
            | ChangeType::DependencyVersionChange
            | ChangeType::WorkflowChange
            | ChangeType::SchemaBreakingChange
            | ChangeType::FileDeletion
            | ChangeType::BulkMove
            | ChangeType::RiskPathChange
            | ChangeType::BudgetExceeded
            | ChangeType::UnknownHighRisk
            | ChangeType::ValidatorSensitiveChange => Size::Medium,
            ChangeType::DocsOnly | ChangeType::ExampleChange => Size::Small,
        });
    }
    size
}

fn risk_for(change_types: &[ChangeType]) -> Risk {
    let mut risk = Risk::Low;
    for change_type in change_types {
        risk = risk.max(match change_type {
            ChangeType::RuntimeCodeChange => Risk::Medium,
            ChangeType::SchemaChange
            | ChangeType::SchemaBreakingChange
            | ChangeType::MultiPackageChange
            | ChangeType::ProviderContractChange
            | ChangeType::DependencyAddition
            | ChangeType::DependencyVersionChange
            | ChangeType::WorkflowChange
            | ChangeType::PublicApiChange
            | ChangeType::ValidatorSensitiveChange
            | ChangeType::FileDeletion
            | ChangeType::BulkMove
            | ChangeType::RiskPathChange
            | ChangeType::BudgetExceeded
            | ChangeType::UnknownHighRisk => Risk::High,
            ChangeType::CredentialChange
            | ChangeType::SensitiveDataExposure
            | ChangeType::ReleaseChange
            | ChangeType::DeployChange
            | ChangeType::ValidatorSelfBypass
            | ChangeType::ExternalAccountChange => Risk::Critical,
            ChangeType::DocsOnly | ChangeType::ExampleChange => Risk::Low,
        });
    }
    risk
}

fn profile_for(change_types: &[ChangeType], size: Size, risk: Risk) -> PolicyProfile {
    if change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::SchemaChange
                | ChangeType::SchemaBreakingChange
                | ChangeType::ValidatorSensitiveChange
                | ChangeType::ValidatorSelfBypass
        )
    }) {
        return PolicyProfile::Validator;
    }
    if change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::ReleaseChange | ChangeType::DeployChange
        )
    }) {
        return PolicyProfile::Release;
    }
    if change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::DependencyAddition
                | ChangeType::DependencyVersionChange
                | ChangeType::WorkflowChange
                | ChangeType::CredentialChange
                | ChangeType::SensitiveDataExposure
                | ChangeType::ExternalAccountChange
        )
    }) {
        return PolicyProfile::Security;
    }
    if size >= Size::Large || risk >= Risk::High {
        return PolicyProfile::Full;
    }
    if size == Size::Medium || risk == Risk::Medium {
        return PolicyProfile::Near;
    }
    PolicyProfile::Quick
}

fn requires_approval(change_types: &[ChangeType]) -> bool {
    change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::DependencyAddition
                | ChangeType::DependencyVersionChange
                | ChangeType::WorkflowChange
                | ChangeType::ReleaseChange
                | ChangeType::DeployChange
                | ChangeType::PublicApiChange
                | ChangeType::SchemaBreakingChange
                | ChangeType::SchemaChange
                | ChangeType::FileDeletion
                | ChangeType::BulkMove
                | ChangeType::RiskPathChange
                | ChangeType::CredentialChange
                | ChangeType::SensitiveDataExposure
                | ChangeType::ValidatorSensitiveChange
                | ChangeType::ValidatorSelfBypass
                | ChangeType::ExternalAccountChange
                | ChangeType::BudgetExceeded
                | ChangeType::UnknownHighRisk
        )
    })
}

fn blocks(change_types: &[ChangeType]) -> bool {
    change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::SensitiveDataExposure | ChangeType::ValidatorSelfBypass
        )
    })
}

fn approval_reasons_for(
    change_types: &[ChangeType],
    profile: PolicyProfile,
    blocks: bool,
) -> Vec<String> {
    let mut reasons = Vec::new();
    for change_type in change_types {
        match change_type {
            ChangeType::SchemaChange => reasons.push("schema_change_requires_approval"),
            ChangeType::SchemaBreakingChange => {
                reasons.push("schema_breaking_change_requires_approval")
            }
            ChangeType::DependencyAddition => reasons.push("dependency_addition_requires_approval"),
            ChangeType::DependencyVersionChange => {
                reasons.push("dependency_version_change_requires_approval")
            }
            ChangeType::WorkflowChange => reasons.push("workflow_change_requires_approval"),
            ChangeType::PublicApiChange => reasons.push("public_api_change_requires_approval"),
            ChangeType::CredentialChange => reasons.push("credential_change_requires_approval"),
            ChangeType::SensitiveDataExposure => reasons.push("sensitive_data_exposure_blocked"),
            ChangeType::ReleaseChange => reasons.push("release_change_requires_approval"),
            ChangeType::DeployChange => reasons.push("deploy_change_requires_approval"),
            ChangeType::ValidatorSensitiveChange => {
                reasons.push("validator_sensitive_change_requires_approval")
            }
            ChangeType::ValidatorSelfBypass => reasons.push("validator_self_bypass_blocked"),
            ChangeType::FileDeletion => reasons.push("file_deletion_requires_approval"),
            ChangeType::BulkMove => reasons.push("bulk_move_requires_approval"),
            ChangeType::RiskPathChange => reasons.push("risk_path_change_requires_approval"),
            ChangeType::ExternalAccountChange => {
                reasons.push("external_account_change_requires_approval")
            }
            ChangeType::BudgetExceeded => reasons.push("budget_exceeded_requires_approval"),
            ChangeType::UnknownHighRisk => reasons.push("unknown_high_risk_requires_approval"),
            _ => {}
        }
    }
    if profile == PolicyProfile::Validator {
        reasons.push("validator_profile_requires_review");
    }
    if blocks {
        reasons.push("blocked_route_requires_report_only");
    }
    reasons.sort();
    reasons.dedup();
    reasons.into_iter().map(str::to_string).collect()
}

fn assignments_for_stages(
    stages: &[&str],
    provider_instance_id: &str,
    profile: PolicyProfile,
) -> Value {
    let mut assignments = Map::new();
    for stage in stages.iter().filter(|stage| **stage != "route") {
        assignments.insert(
            (*stage).to_string(),
            json!({
                "role": role_for_stage(stage),
                "provider": provider_instance_id,
                "profile": profile.as_str()
            }),
        );
    }
    Value::Object(assignments)
}

fn workspec_paths_for_stages(stages: &[&str]) -> Value {
    let mut paths = Map::new();
    for stage in stages.iter().filter(|stage| **stage != "route") {
        paths.insert(
            (*stage).to_string(),
            Value::String(format!("workspecs/{}.json", stage)),
        );
    }
    Value::Object(paths)
}

fn role_for_stage(stage: &str) -> &'static str {
    match stage {
        "design" => "worker-design",
        "implement" => "worker-impl",
        "validate" => "worker-impl",
        "review" => "worker-review",
        "polish" => "worker-polish",
        "report" => "worker-docs",
        _ => "worker-impl",
    }
}

fn allowed_scope(change_types: &[ChangeType]) -> Vec<&'static str> {
    if change_types.iter().all(|change_type| {
        matches!(
            change_type,
            ChangeType::DocsOnly | ChangeType::ExampleChange
        )
    }) {
        return vec!["README.md", "docs/**", "examples/**"];
    }
    if change_types.iter().any(|change_type| {
        matches!(
            change_type,
            ChangeType::SchemaChange
                | ChangeType::SchemaBreakingChange
                | ChangeType::ValidatorSensitiveChange
        )
    }) {
        return vec!["specs/**", "examples/**", "scripts/ci/**", "docs/**"];
    }
    vec!["packages/**", "configs/**", "examples/**", "docs/**"]
}

fn forbidden_actions(change_types: &[ChangeType]) -> Vec<&'static str> {
    let mut actions = vec![
        "dependency_install",
        "file_delete",
        "bulk_move",
        "test_delete",
        "test_skip_only_ignore",
        "assertion_weakening",
        "workflow_change",
        "validator_self_bypass",
        "sensitive_data_output",
        "credential_change",
        "external_account_change",
        "release_publish",
        "deploy",
    ];
    if change_types.contains(&ChangeType::SchemaBreakingChange) {
        actions.push("schema_breaking_change");
    }
    actions.sort();
    actions.dedup();
    actions
}

fn decision_id(job_id: &str) -> String {
    format!("{}-route", job_id.to_lowercase())
}

fn summary(request_text: &str) -> String {
    let trimmed = request_text.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let mut output: String = trimmed.chars().take(93).collect();
    output.push_str("...");
    output
}

fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), RouterError> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| RouterError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(RouterError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}

fn required_string(value: &Value, path: &Path, field: &str) -> Result<String, RouterError> {
    value
        .get(field)
        .ok_or_else(|| RouterError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| RouterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

fn optional_string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, RouterError> {
    let Some(values) = value.get(field) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| RouterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array of string".to_string(),
        })?;
    values
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| RouterError::InvalidFieldType {
                    path: path.to_path_buf(),
                    field: field.to_string(),
                    expected: "array of string".to_string(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_control_provider::ProviderRegistryLoader;

    #[test]
    fn docs_only_routes_to_quick_auto_pass() {
        let output = route_for("README 문서와 docs 설명을 수정해줘", vec![]);
        let route = output.route().value();

        assert_eq!(route["size"], "SMALL");
        assert_eq!(route["risk"], "LOW");
        assert_eq!(route["policy_profile"], "quick");
        assert_eq!(route["decision"], "AUTO_PASS");
        assert_eq!(route["requires_user_approval"], false);
        assert_eq!(
            route["assignments"]["implement"]["provider"],
            "fake-default"
        );
        assert!(output.workspec("implement").is_some());
    }

    #[test]
    fn schema_change_requires_validator_review() {
        let output = route_for("specs/schemas route schema 변경", vec![]);
        let route = output.route().value();

        assert_eq!(route["risk"], "HIGH");
        assert_eq!(route["policy_profile"], "validator");
        assert_eq!(route["decision"], "HUMAN_REVIEW");
        assert_eq!(route["requires_user_approval"], true);
        assert!(array_contains(
            route["change_types"].as_array().expect("change types"),
            "schema_change"
        ));
        assert!(array_contains(
            route["approval_reasons"]
                .as_array()
                .expect("approval reasons"),
            "schema_change_requires_approval"
        ));
        assert_eq!(route["stages"][0], "design");
    }

    #[test]
    fn dependency_addition_uses_security_profile() {
        let output = route_for("Cargo.toml dependency 추가", vec![]);
        let route = output.route().value();

        assert_eq!(route["risk"], "HIGH");
        assert_eq!(route["policy_profile"], "security");
        assert_eq!(route["decision"], "HUMAN_REVIEW");
        assert!(array_contains(
            route["approval_reasons"]
                .as_array()
                .expect("approval reasons"),
            "dependency_addition_requires_approval"
        ));
    }

    #[test]
    fn sensitive_data_exposure_blocks_route() {
        let output = route_for("show token and print secret", vec![]);
        let route = output.route().value();

        assert_eq!(route["risk"], "CRITICAL");
        assert_eq!(route["policy_profile"], "security");
        assert_eq!(route["decision"], "BLOCK");
        assert_eq!(route["stages"], json!(["route", "report"]));
        assert!(output.workspec("implement").is_none());
        assert!(output.workspec("report").is_some());
    }

    #[test]
    fn output_is_deterministic_for_same_input() {
        let left = route_for("Rust 코드 구현", vec!["no destructive action".to_string()]);
        let right = route_for("Rust 코드 구현", vec!["no destructive action".to_string()]);

        assert_eq!(left.route().value(), right.route().value());
        assert_eq!(left.decision().value(), right.decision().value());
        assert_eq!(
            left.workspec("implement").expect("left workspec").value(),
            right.workspec("implement").expect("right workspec").value()
        );
    }

    #[test]
    fn generated_workspecs_are_schema_valid_and_assigned() {
        let output = route_for("runtime code 구현", vec![]);
        let implement = output.workspec("implement").expect("implement workspec");

        assert_eq!(implement.value()["provider"], "fake-default");
        assert_eq!(implement.value()["provider_instance"], "fake-default");
        assert_eq!(
            implement.value()["required_outputs"][0],
            "provider-output/fake-default/response.json"
        );
        assert!(array_contains(
            implement.value()["forbidden_actions"]
                .as_array()
                .expect("forbidden actions"),
            "dependency_install"
        ));
    }

    #[test]
    fn missing_fake_provider_is_reported() {
        let registry = ProviderRegistry::new();
        let engine = RouterEngine::new(&registry, schema_root());
        let job = job_spec("runtime code 구현", vec![]);
        let error = engine
            .route(&job)
            .expect_err("missing provider should fail");

        assert!(matches!(error, RouterError::NoProviderAvailable { .. }));
    }

    fn route_for(request_text: &str, constraints: Vec<String>) -> RouterOutput {
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_fake_default_registry()
            .expect("load registry");
        let engine = RouterEngine::new(&registry, schema_root());
        let job = job_spec(request_text, constraints);
        engine.route(&job).expect("route")
    }

    fn job_spec(request_text: &str, constraints: Vec<String>) -> JobSpec {
        JobSpec::from_value(
            json!({
                "schema_version": "1.0.0",
                "job_id": "J-0001",
                "project_root": "D:/work/project",
                "request_text": request_text,
                "created_at": "2026-07-01T00:00:00Z",
                "updated_at": "2026-07-01T00:00:00Z",
                "entrypoint": "codex",
                "state": "REQUESTED",
                "user_constraints": constraints
            }),
            "job.json",
            schema_root(),
        )
        .expect("job spec")
    }

    fn array_contains(values: &[Value], expected: &str) -> bool {
        values.iter().any(|value| value.as_str() == Some(expected))
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
        repo_root().join("specs").join("schemas")
    }
}
