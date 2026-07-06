use super::path::artifact_path_for_stage;
use super::role::role_for_stage;
use crate::analysis::{ChangeType, RequestAnalysis};
use crate::constants::{SCHEMA_VERSION, WORKSPEC_SCHEMA};
use crate::contract::validate_contract;
use crate::types::{JobSpec, WorkSpec};
use crate::RouterError;
use serde_json::json;
use std::path::Path;

pub(crate) fn build_workspec_for_stage(
    job: &JobSpec,
    stage: &str,
    provider_instance_id: &str,
    analysis: &RequestAnalysis,
    schema_root: &Path,
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
    let artifact_path = artifact_path_for_stage(stage);
    validate_contract(
        &workspec,
        Path::new(&artifact_path),
        schema_root,
        WORKSPEC_SCHEMA,
    )?;
    Ok(WorkSpec {
        stage: stage.to_string(),
        value: workspec,
    })
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
