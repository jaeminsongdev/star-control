//! Stage-scoped permission policy used by routing and effect execution.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    GoalId, PermissionPlanId, ProjectId, RunId, Sha256Hash, StageId, canonical_sha256,
    evidence::{CatalogRef, DocumentRef},
    management::ProjectPathRef,
};

pub const PERMISSION_PLAN_SCHEMA_ID: &str = "star.permission-plan";
pub const PERMISSION_PLAN_CONTRACT_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecisionV1 {
    Auto,
    Prompt,
    Deny,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PathPermissionKindV1 {
    AllowRead,
    DenyRead,
    AllowWrite,
    DenyWrite,
    DenyExternalTransfer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionPathRuleV1 {
    pub project_id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<ProjectPathRef>,
    pub kind: PathPermissionKindV1,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionProcessRuleV1 {
    pub executable_sha256: Sha256Hash,
    pub argument_prefix: Vec<String>,
    pub cwd_project_id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd_prefix: Option<ProjectPathRef>,
    pub allow_child_processes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionNetworkRuleV1 {
    pub target: String,
    pub operation: String,
    pub decision: PermissionDecisionV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionEnvironmentRuleV1 {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref_kind: Option<String>,
    pub decision: PermissionDecisionV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaidActionRulesV1 {
    pub evidence_basis: Vec<String>,
    pub unknown_cost_decision: PermissionDecisionV1,
    pub measured_limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionExternalConstraintsV1 {
    pub codex_approval_required: bool,
    pub codex_approval_policy: String,
    pub codex_sandbox_mode: String,
    pub administrator_required: bool,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionPlanV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub permission_plan_id: PermissionPlanId,
    pub revision: u64,
    pub goal_id: GoalId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub stage_revision: u64,
    pub stage_scope_hash: Sha256Hash,
    pub policy_profile_ref: CatalogRef,
    pub default_action: PermissionDecisionV1,
    pub action_policies: BTreeMap<String, PermissionDecisionV1>,
    pub path_rules: Vec<PermissionPathRuleV1>,
    pub process_rules: Vec<PermissionProcessRuleV1>,
    pub network_rules: Vec<PermissionNetworkRuleV1>,
    pub environment_rules: Vec<PermissionEnvironmentRuleV1>,
    pub paid_action_rules: PaidActionRulesV1,
    pub external_constraints: PermissionExternalConstraintsV1,
    pub effective_config_ref: DocumentRef,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub scope_hash: Sha256Hash,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermissionPlanError {
    #[error("PermissionPlan identity or time range is invalid")]
    Identity,
    #[error("PermissionPlan rules are invalid")]
    Rules,
    #[error("PermissionPlan cannot auto-approve an unknown paid action")]
    PaidUnknown,
    #[error("PermissionPlan fingerprint could not be calculated")]
    Fingerprint,
}

impl PermissionPlanV1 {
    pub fn seal(mut self) -> Result<Self, PermissionPlanError> {
        self.path_rules.sort_by(|left, right| {
            (&left.project_id, &left.path_prefix, left.kind).cmp(&(
                &right.project_id,
                &right.path_prefix,
                right.kind,
            ))
        });
        if self.path_rules.windows(2).any(|pair| {
            pair[0].project_id == pair[1].project_id
                && pair[0].path_prefix == pair[1].path_prefix
                && pair[0].kind == pair[1].kind
                && pair[0] != pair[1]
        }) {
            return Err(PermissionPlanError::Rules);
        }
        let path_kinds = self.path_rules.iter().fold(
            BTreeMap::<(ProjectId, Option<ProjectPathRef>), Vec<PathPermissionKindV1>>::new(),
            |mut grouped, rule| {
                grouped
                    .entry((rule.project_id.clone(), rule.path_prefix.clone()))
                    .or_default()
                    .push(rule.kind);
                grouped
            },
        );
        if path_kinds.values().any(|kinds| {
            (kinds.contains(&PathPermissionKindV1::AllowRead)
                && kinds.contains(&PathPermissionKindV1::DenyRead))
                || (kinds.contains(&PathPermissionKindV1::AllowWrite)
                    && kinds.contains(&PathPermissionKindV1::DenyWrite))
        }) {
            return Err(PermissionPlanError::Rules);
        }
        self.path_rules.dedup();
        self.process_rules.sort_by(|left, right| {
            (
                &left.cwd_project_id,
                &left.cwd_prefix,
                &left.executable_sha256,
                &left.argument_prefix,
            )
                .cmp(&(
                    &right.cwd_project_id,
                    &right.cwd_prefix,
                    &right.executable_sha256,
                    &right.argument_prefix,
                ))
        });
        if self.process_rules.windows(2).any(|pair| {
            pair[0].cwd_project_id == pair[1].cwd_project_id
                && pair[0].cwd_prefix == pair[1].cwd_prefix
                && pair[0].executable_sha256 == pair[1].executable_sha256
                && pair[0].argument_prefix == pair[1].argument_prefix
                && pair[0] != pair[1]
        }) {
            return Err(PermissionPlanError::Rules);
        }
        self.process_rules.dedup();
        self.network_rules.sort_by(|left, right| {
            (&left.target, &left.operation).cmp(&(&right.target, &right.operation))
        });
        if self.network_rules.windows(2).any(|pair| {
            pair[0].target == pair[1].target
                && pair[0].operation == pair[1].operation
                && pair[0] != pair[1]
        }) {
            return Err(PermissionPlanError::Rules);
        }
        self.network_rules.dedup();
        self.environment_rules
            .sort_by(|left, right| left.name.cmp(&right.name));
        if self
            .environment_rules
            .windows(2)
            .any(|pair| pair[0].name.eq_ignore_ascii_case(&pair[1].name))
        {
            return Err(PermissionPlanError::Rules);
        }
        normalize_strings(&mut self.paid_action_rules.evidence_basis, 128)?;
        normalize_strings(&mut self.external_constraints.limitations, 128)?;
        self.validate_shape()?;
        self.scope_hash = canonical_sha256(&serde_json::json!({
            "domain":"star.permission-scope",
            "version":PERMISSION_PLAN_CONTRACT_VERSION,
            "value":{
                "goal_id":self.goal_id,
                "run_id":self.run_id,
                "stage_id":self.stage_id,
                "stage_revision":self.stage_revision,
                "stage_scope_hash":self.stage_scope_hash,
                "policy_profile_ref":self.policy_profile_ref,
                "default_action":self.default_action,
                "action_policies":self.action_policies,
                "path_rules":self.path_rules,
                "process_rules":self.process_rules,
                "network_rules":self.network_rules,
                "environment_rules":self.environment_rules,
                "paid_action_rules":self.paid_action_rules,
                "external_constraints":self.external_constraints,
                "effective_config_ref":self.effective_config_ref,
            }
        }))
        .map_err(|_| PermissionPlanError::Fingerprint)?;
        self.plan_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":PERMISSION_PLAN_SCHEMA_ID,
            "version":PERMISSION_PLAN_CONTRACT_VERSION,
            "value":{
                "permission_plan_id":self.permission_plan_id,
                "revision":self.revision,
                "scope_hash":self.scope_hash,
                "created_at":self.created_at,
                "expires_at":self.expires_at,
            }
        }))
        .map_err(|_| PermissionPlanError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), PermissionPlanError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(PermissionPlanError::Fingerprint);
        }
        Ok(())
    }

    pub fn is_current_at(&self, now: DateTime<Utc>) -> bool {
        self.created_at <= now && now < self.expires_at
    }

    pub fn decision(&self, action_id: &str) -> PermissionDecisionV1 {
        self.action_policies
            .get(action_id)
            .copied()
            .unwrap_or(self.default_action)
    }

    pub fn allows_process(
        &self,
        executable_sha256: &Sha256Hash,
        project_id: &ProjectId,
        relative_cwd: Option<&ProjectPathRef>,
        arguments: &[&str],
        child_processes_required: bool,
    ) -> bool {
        self.process_rules.iter().any(|rule| {
            rule.executable_sha256 == *executable_sha256
                && rule.cwd_project_id == *project_id
                && path_contains(rule.cwd_prefix.as_ref(), relative_cwd)
                && arguments.starts_with(
                    &rule
                        .argument_prefix
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                )
                && (!child_processes_required || rule.allow_child_processes)
        })
    }

    pub fn allowed_environment_names(&self) -> Vec<String> {
        self.environment_rules
            .iter()
            .filter(|rule| rule.decision != PermissionDecisionV1::Deny)
            .map(|rule| rule.name.clone())
            .collect()
    }

    fn validate_shape(&self) -> Result<(), PermissionPlanError> {
        if self.schema_id != PERMISSION_PLAN_SCHEMA_ID
            || self.schema_version != PERMISSION_PLAN_CONTRACT_VERSION
            || self.revision == 0
            || self.stage_revision == 0
            || self.created_at >= self.expires_at
            || self
                .expires_at
                .signed_duration_since(self.created_at)
                .num_hours()
                > 24
            || self.stage_scope_hash == Sha256Hash::digest(b"")
            || !catalog_ref(&self.policy_profile_ref)
            || !document_ref(&self.effective_config_ref, "star.effective-config")
        {
            return Err(PermissionPlanError::Identity);
        }
        if self.action_policies.is_empty()
            || self.action_policies.len() > 256
            || self
                .action_policies
                .keys()
                .any(|action| !bounded_token(action, 128))
            || self.path_rules.len() > 1_024
            || self.process_rules.len() > 128
            || self.network_rules.len() > 128
            || self.environment_rules.len() > 128
            || self.path_rules.iter().any(|rule| {
                rule.path_prefix
                    .as_ref()
                    .is_some_and(|path| !bounded_text(path.as_str(), 4_096))
                    || !bounded_text(&rule.reason, 1_024)
            })
            || self.process_rules.iter().any(|rule| {
                rule.executable_sha256 == Sha256Hash::digest(b"")
                    || rule.argument_prefix.is_empty()
                    || rule.argument_prefix.len() > 32
                    || rule
                        .argument_prefix
                        .iter()
                        .any(|argument| !bounded_text(argument, 512))
                    || rule
                        .cwd_prefix
                        .as_ref()
                        .is_some_and(|path| !bounded_text(path.as_str(), 4_096))
            })
            || self.network_rules.iter().any(|rule| {
                !bounded_text(&rule.target, 512) || !bounded_token(&rule.operation, 128)
            })
            || self.environment_rules.iter().any(|rule| {
                !environment_name(&rule.name)
                    || rule
                        .secret_ref_kind
                        .as_deref()
                        .is_some_and(|kind| !bounded_token(kind, 128))
            })
            || !bounded_token(&self.external_constraints.codex_approval_policy, 96)
            || !bounded_token(&self.external_constraints.codex_sandbox_mode, 96)
        {
            return Err(PermissionPlanError::Rules);
        }
        if self.paid_action_rules.unknown_cost_decision == PermissionDecisionV1::Auto {
            return Err(PermissionPlanError::PaidUnknown);
        }
        Ok(())
    }
}

fn path_contains(allowed: Option<&ProjectPathRef>, observed: Option<&ProjectPathRef>) -> bool {
    match (allowed, observed) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(allowed), Some(observed)) => {
            observed.as_str() == allowed.as_str()
                || observed
                    .as_str()
                    .strip_prefix(allowed.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
    }
}

fn catalog_ref(reference: &CatalogRef) -> bool {
    bounded_token(&reference.catalog_id, 192)
        && reference.format_version > 0
        && bounded_token(&reference.item_version, 96)
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn document_ref(reference: &DocumentRef, schema_id: &str) -> bool {
    reference.schema_id == schema_id
        && bounded_token(&reference.document_id, 192)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn normalize_strings(
    values: &mut Vec<String>,
    max_items: usize,
) -> Result<(), PermissionPlanError> {
    values.sort();
    values.dedup();
    if values.len() > max_items || values.iter().any(|value| !bounded_text(value, 1_024)) {
        return Err(PermissionPlanError::Rules);
    }
    Ok(())
}

fn environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
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

    fn document(schema_id: &str, id: &str) -> DocumentRef {
        DocumentRef {
            schema_id: schema_id.to_owned(),
            document_id: id.to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(id.as_bytes()),
        }
    }

    fn plan() -> PermissionPlanV1 {
        let now = Utc::now();
        let project_id = ProjectId::new();
        PermissionPlanV1 {
            schema_id: PERMISSION_PLAN_SCHEMA_ID.to_owned(),
            schema_version: PERMISSION_PLAN_CONTRACT_VERSION,
            permission_plan_id: PermissionPlanId::new(),
            revision: 1,
            goal_id: GoalId::new(),
            run_id: RunId::new(),
            stage_id: StageId::new(),
            stage_revision: 1,
            stage_scope_hash: Sha256Hash::digest(b"stage-scope"),
            policy_profile_ref: CatalogRef {
                catalog_id: "star.policy-profile.safe-default".to_owned(),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: Sha256Hash::digest(b"policy"),
            },
            default_action: PermissionDecisionV1::Deny,
            action_policies: BTreeMap::from([
                (
                    "external.ai.execute".to_owned(),
                    PermissionDecisionV1::Prompt,
                ),
                ("paid_action".to_owned(), PermissionDecisionV1::Prompt),
            ]),
            path_rules: vec![PermissionPathRuleV1 {
                project_id: project_id.clone(),
                path_prefix: None,
                kind: PathPermissionKindV1::AllowRead,
                reason: "stage context".to_owned(),
            }],
            process_rules: vec![PermissionProcessRuleV1 {
                executable_sha256: Sha256Hash::digest(b"codex"),
                argument_prefix: vec!["app-server".to_owned()],
                cwd_project_id: project_id,
                cwd_prefix: None,
                allow_child_processes: true,
            }],
            network_rules: vec![PermissionNetworkRuleV1 {
                target: "codex-provider".to_owned(),
                operation: "execute".to_owned(),
                decision: PermissionDecisionV1::Prompt,
            }],
            environment_rules: vec![PermissionEnvironmentRuleV1 {
                name: "PATH".to_owned(),
                secret_ref_kind: None,
                decision: PermissionDecisionV1::Auto,
            }],
            paid_action_rules: PaidActionRulesV1 {
                evidence_basis: vec!["provider usage unavailable".to_owned()],
                unknown_cost_decision: PermissionDecisionV1::Prompt,
                measured_limit: None,
            },
            external_constraints: PermissionExternalConstraintsV1 {
                codex_approval_required: true,
                codex_approval_policy: "on-request".to_owned(),
                codex_sandbox_mode: "workspace-write".to_owned(),
                administrator_required: false,
                limitations: vec!["provider cost unavailable".to_owned()],
            },
            effective_config_ref: document("star.effective-config", "effective-config"),
            created_at: now,
            expires_at: now + Duration::minutes(30),
            scope_hash: Sha256Hash::digest(b"unsealed-scope"),
            plan_fingerprint: Sha256Hash::digest(b"unsealed-plan"),
        }
    }

    #[test]
    fn permission_plan_positive_seals_exact_stage_scope() {
        let plan = plan().seal().unwrap();
        plan.verify().unwrap();
        assert_eq!(
            plan.decision("external.ai.execute"),
            PermissionDecisionV1::Prompt
        );
    }

    #[test]
    fn permission_plan_negative_unknown_paid_action_cannot_be_auto() {
        let mut plan = plan();
        plan.paid_action_rules.unknown_cost_decision = PermissionDecisionV1::Auto;
        assert_eq!(plan.seal(), Err(PermissionPlanError::PaidUnknown));
    }

    #[test]
    fn permission_plan_failure_expired_plan_is_not_current() {
        let plan = plan().seal().unwrap();
        assert!(!plan.is_current_at(plan.expires_at));
    }

    #[test]
    fn permission_plan_recovery_default_deny_is_preserved() {
        let plan = plan().seal().unwrap();
        assert_eq!(plan.decision("remote.publish"), PermissionDecisionV1::Deny);
    }

    #[test]
    fn permission_plan_negative_rejects_ambiguous_rules() {
        let mut path_conflict = plan();
        let mut deny = path_conflict.path_rules[0].clone();
        deny.kind = PathPermissionKindV1::DenyRead;
        deny.reason = "same scope denied".to_owned();
        path_conflict.path_rules.push(deny);
        assert_eq!(path_conflict.seal(), Err(PermissionPlanError::Rules));

        let mut network_conflict = plan();
        let mut deny = network_conflict.network_rules[0].clone();
        deny.decision = PermissionDecisionV1::Deny;
        network_conflict.network_rules.push(deny);
        assert_eq!(network_conflict.seal(), Err(PermissionPlanError::Rules));
    }
}
