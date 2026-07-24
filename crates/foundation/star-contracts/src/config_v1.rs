//! Typed, fingerprinted effective configuration shared by every product stage.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Sha256Hash, canonical::canonical_sha256};

pub const EFFECTIVE_CONFIG_V1_SCHEMA_ID: &str = "star.effective-config";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSourceKindV1 {
    BuiltIn,
    PolicyProfile,
    User,
    Project,
    Goal,
    Command,
    PlatformConstraint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfigMergeStrategyV1 {
    Replace,
    DeepMerge,
    MostRestrictive,
    MinimumLimit,
    Intersection,
    Union,
    Immutable,
    PolicyAllowThenFalseWins,
    TrueWins,
    FalseWins,
    MaximumFloor,
    ExplicitWidening,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ConfigValueV1 {
    Boolean(bool),
    Integer(u64),
    String(String),
    StringSet(Vec<String>),
    Json(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigSourceRefV1 {
    pub source_kind: ConfigSourceKindV1,
    pub source_id: String,
    pub source_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfigEntryV1 {
    pub key: String,
    pub value: ConfigValueV1,
    pub merge_strategy: ConfigMergeStrategyV1,
    pub provenance: Vec<ConfigSourceRefV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigOverrideV1 {
    #[schemars(length(max = 160))]
    pub key: String,
    pub value: ConfigValueV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigLayerV1 {
    pub source: ConfigSourceRefV1,
    #[schemars(length(min = 1, max = 128))]
    pub overrides: Vec<ConfigOverrideV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfigV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub policy_profile_id: String,
    pub entries: Vec<EffectiveConfigEntryV1>,
    pub config_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EffectiveConfigError {
    #[error("effective config identity is invalid")]
    Identity,
    #[error("effective config entries are not canonical")]
    Entries,
    #[error("effective config provenance is invalid")]
    Provenance,
    #[error("effective config value is invalid")]
    Value,
    #[error("effective config fingerprint could not be computed")]
    Fingerprint,
    #[error("effective config layer conflicts with the declared merge strategy")]
    Merge,
}

impl EffectiveConfigV1 {
    pub fn seal(
        policy_profile_id: impl Into<String>,
        mut entries: Vec<EffectiveConfigEntryV1>,
    ) -> Result<Self, EffectiveConfigError> {
        let policy_profile_id = policy_profile_id.into();
        if !valid_identifier(&policy_profile_id) || entries.is_empty() {
            return Err(EffectiveConfigError::Identity);
        }
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        if entries.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(EffectiveConfigError::Entries);
        }
        for entry in &mut entries {
            validate_key(&entry.key)?;
            validate_value(&mut entry.value)?;
            validate_entry_value(&entry.key, &entry.value)?;
            if entry.provenance.is_empty() {
                return Err(EffectiveConfigError::Provenance);
            }
            entry.provenance.sort_by(|left, right| {
                (&left.source_kind, &left.source_id, &left.source_fingerprint).cmp(&(
                    &right.source_kind,
                    &right.source_id,
                    &right.source_fingerprint,
                ))
            });
            if entry.provenance.windows(2).any(|pair| pair[0] == pair[1])
                || entry.provenance.iter().any(|source| {
                    !valid_identifier(&source.source_id)
                        || source.source_fingerprint.as_str().is_empty()
                })
            {
                return Err(EffectiveConfigError::Provenance);
            }
        }
        let mut config = Self {
            schema_id: EFFECTIVE_CONFIG_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            policy_profile_id,
            entries,
            config_fingerprint: Sha256Hash::digest(b"star.effective-config.pending"),
        };
        config.config_fingerprint = config.expected_fingerprint()?;
        Ok(config)
    }

    pub fn verify(&self) -> Result<(), EffectiveConfigError> {
        let resealed = Self::seal(self.policy_profile_id.clone(), self.entries.clone())?;
        if self.schema_id != EFFECTIVE_CONFIG_V1_SCHEMA_ID
            || self.schema_version != 1
            || self.entries != resealed.entries
            || self.config_fingerprint != resealed.config_fingerprint
        {
            return Err(EffectiveConfigError::Fingerprint);
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValueV1> {
        self.entries
            .binary_search_by_key(&key, |entry| entry.key.as_str())
            .ok()
            .map(|index| &self.entries[index].value)
    }

    pub fn boolean(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(ConfigValueV1::Boolean(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn integer(&self, key: &str) -> Option<u64> {
        match self.get(key) {
            Some(ConfigValueV1::Integer(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(ConfigValueV1::String(value)) => Some(value),
            _ => None,
        }
    }

    pub fn string_set(&self, key: &str) -> Option<&[String]> {
        match self.get(key) {
            Some(ConfigValueV1::StringSet(value)) => Some(value),
            _ => None,
        }
    }

    pub fn validate_layer_shape(&self, layer: &ConfigLayerV1) -> Result<(), EffectiveConfigError> {
        if !valid_identifier(&layer.source.source_id)
            || layer.source.source_fingerprint.as_str().is_empty()
            || layer.source.source_kind == ConfigSourceKindV1::BuiltIn
            || layer.overrides.is_empty()
            || layer.overrides.len() > 128
        {
            return Err(EffectiveConfigError::Provenance);
        }
        let overrides =
            serde_json::to_value(&layer.overrides).map_err(|_| EffectiveConfigError::Value)?;
        if crate::canonical::jcs_bytes(&overrides)
            .map_err(|_| EffectiveConfigError::Value)?
            .len()
            > 64 * 1024
        {
            return Err(EffectiveConfigError::Value);
        }
        let mut overrides = layer.overrides.clone();
        overrides.sort_by(|left, right| left.key.cmp(&right.key));
        if overrides.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(EffectiveConfigError::Entries);
        }
        for config_override in overrides {
            validate_key(&config_override.key)?;
            let entry = self
                .entries
                .iter()
                .find(|entry| entry.key == config_override.key)
                .ok_or(EffectiveConfigError::Merge)?;
            if !source_can_override(&entry.key, layer.source.source_kind)
                || std::mem::discriminant(&entry.value)
                    != std::mem::discriminant(&config_override.value)
            {
                return Err(EffectiveConfigError::Merge);
            }
            let mut incoming = config_override.value;
            validate_value(&mut incoming)?;
            validate_entry_value(&entry.key, &incoming)?;
        }
        Ok(())
    }

    pub fn apply_layer(mut self, mut layer: ConfigLayerV1) -> Result<Self, EffectiveConfigError> {
        self.validate_layer_shape(&layer)?;
        layer
            .overrides
            .sort_by(|left, right| left.key.cmp(&right.key));
        for config_override in layer.overrides {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.key == config_override.key)
                .ok_or(EffectiveConfigError::Merge)?;
            let mut incoming = config_override.value.clone();
            validate_value(&mut incoming)?;
            entry.value = merge_value(
                &entry.key,
                entry.merge_strategy,
                &entry.value,
                &incoming,
                layer.source.source_kind,
            )?;
            entry.provenance.push(layer.source.clone());
        }
        let policy_profile_id = self
            .entries
            .iter()
            .find(|entry| entry.key == "policy_profile")
            .and_then(|entry| match &entry.value {
                ConfigValueV1::String(value) => Some(value.clone()),
                _ => None,
            })
            .ok_or(EffectiveConfigError::Merge)?;
        Self::seal(policy_profile_id, self.entries)
    }

    fn expected_fingerprint(&self) -> Result<Sha256Hash, EffectiveConfigError> {
        canonical_sha256(&serde_json::json!({
            "schema_id": EFFECTIVE_CONFIG_V1_SCHEMA_ID,
            "schema_version": 1,
            "policy_profile_id": self.policy_profile_id,
            "entries": self.entries,
        }))
        .map_err(|_| EffectiveConfigError::Fingerprint)
    }
}

fn source_can_override(key: &str, source_kind: ConfigSourceKindV1) -> bool {
    !matches!(source_kind, ConfigSourceKindV1::BuiltIn) && runtime_materialized_override_key(key)
}

/// Only values that are consumed by the current Controller/application path may
/// be changed by a derived layer. Reserved or fixed product keys remain in the
/// effective snapshot for auditability, but accepting an override that merely
/// changes the fingerprint would be an accepted-but-unmaterialized bug.
fn runtime_materialized_override_key(key: &str) -> bool {
    matches!(
        key,
        "policy_profile"
            | "default_work_profile"
            | "permissions.approval_ttl_ms"
            | "budgets.goal_wall_time_ms"
            | "budgets.stage_wall_time_ms"
            | "budgets.max_artifact_bytes"
            | "validation.command_timeout_ms"
            | "validation.max_log_bytes"
            | "validation.max_parallel_checks"
            | "scan.incremental"
            | "scan.include_untracked"
            | "scan.include_ignored"
            | "scan.follow_symlinks"
            | "scan.binary_mode"
            | "scan.max_file_bytes"
            | "scan.max_files"
            | "scan.max_total_bytes"
            | "scan.max_parallel_files"
            | "scan.include_paths"
            | "scan.exclude_paths_add"
            | "scan.hardcoding_rules_enabled"
            | "scan.hardcoding_include_tests"
            | "scan.hardcoding_include_fixtures"
            | "scan.hardcoding_include_docs_examples"
            | "scan.hardcoding_include_generated"
            | "scan.hardcoding_include_vendor"
            | "index.required_tier"
            | "index.max_tier"
            | "index.fallback_to_lower_tier"
            | "index.max_symbols"
            | "index.max_references"
            | "index.max_graph_edges"
            | "index.cross_project_edges"
            | "index_cache.enabled"
            | "index_cache.max_total_bytes"
            | "index_cache.retention_days"
            | "change_planning.max_graph_depth"
            | "change_planning.max_graph_nodes"
            | "change_planning.max_graph_edges"
            | "change_planning.max_downstream_projects"
            | "change_planning.max_check_candidates"
            | "change_planning.allow_cross_project_read"
            | "change_planning.allow_previous_success_reuse"
            | "failure_reproduction.max_rerun_attempts"
            | "security_supply_chain.default_max_age_hours"
            | "migration.default_strategy"
            | "migration.live_execute_action"
            | "migration.rollback_action"
            | "migration.max_resume_attempts"
            | "migration.max_additional_rehearsals"
            | "performance_build.default_warmup_runs"
            | "performance_build.default_measurement_runs"
            | "performance_build.minimum_measurement_runs"
            | "performance_build.max_additional_runs"
            | "performance_build.outlier_policy"
            | "language_platform_migration.cutover_action"
            | "release.publish_action"
            | "evaluation.default_mode"
            | "evaluation.max_attempts_per_case"
            | "management.keep_latest_successful_scans"
            | "management.incomplete_staging_retention_days"
            | "management.scan_detail_retention_days"
            | "vcs.use_worktree"
            | "vcs.merge_strategy"
            | "vcs.max_parallel_projects"
            | "vcs.max_active_worktrees"
            | "vcs.max_parallel_mutations_per_repository"
            | "vcs.max_parallel_local_merges"
            | "vcs.max_merge_queue_entries"
            | "vcs.worktree_disk_limit_bytes"
            | "remote.allowed_hosts"
            | "remote.require_clean_target"
            | "remote.max_parallel_writes"
    )
}

fn validate_entry_value(key: &str, value: &ConfigValueV1) -> Result<(), EffectiveConfigError> {
    match (key, value) {
        ("policy_profile", ConfigValueV1::String(value))
            if matches!(
                value.as_str(),
                "star.policy-profile.safe-default" | "star.policy-profile.personal-auto"
            ) =>
        {
            Ok(())
        }
        ("policy_profile", _) => Err(EffectiveConfigError::Value),
        ("default_work_profile", ConfigValueV1::StringSet(values))
            if values.len() <= 1
                && values.iter().all(|value| {
                    !value.is_empty()
                        && value.len() <= 128
                        && value.bytes().enumerate().all(|(index, byte)| {
                            byte.is_ascii_lowercase()
                                || (index > 0 && (byte.is_ascii_digit() || byte == b'_'))
                        })
                }) =>
        {
            Ok(())
        }
        ("default_work_profile", _) => Err(EffectiveConfigError::Value),
        ("routing.default_model_role", ConfigValueV1::String(value))
            if matches!(value.as_str(), "luna" | "sol" | "terra") =>
        {
            Ok(())
        }
        ("routing.default_model_role", _) => Err(EffectiveConfigError::Value),
        ("routing.unsupported_choice", ConfigValueV1::String(value))
            if matches!(value.as_str(), "fail" | "explain_and_fallback" | "ask") =>
        {
            Ok(())
        }
        ("routing.unsupported_choice", _) => Err(EffectiveConfigError::Value),
        ("migration.default_strategy", ConfigValueV1::String(value))
            if matches!(
                value.as_str(),
                "side_by_side" | "atomic_replace" | "transactional_in_place"
            ) =>
        {
            Ok(())
        }
        ("migration.default_strategy", _) => Err(EffectiveConfigError::Value),
        ("performance_build.default_warmup_runs", ConfigValueV1::Integer(value))
            if *value <= 100 =>
        {
            Ok(())
        }
        ("performance_build.default_warmup_runs", _) => Err(EffectiveConfigError::Value),
        ("performance_build.default_measurement_runs", ConfigValueV1::Integer(value))
            if (3..=100).contains(value) =>
        {
            Ok(())
        }
        ("performance_build.default_measurement_runs", _) => Err(EffectiveConfigError::Value),
        ("evaluation.default_mode", ConfigValueV1::String(value))
            if matches!(value.as_str(), "offline" | "replay" | "shadow") =>
        {
            Ok(())
        }
        ("evaluation.default_mode", _) => Err(EffectiveConfigError::Value),
        _ => Ok(()),
    }
}

fn merge_value(
    key: &str,
    strategy: ConfigMergeStrategyV1,
    current: &ConfigValueV1,
    incoming: &ConfigValueV1,
    source_kind: ConfigSourceKindV1,
) -> Result<ConfigValueV1, EffectiveConfigError> {
    if std::mem::discriminant(current) != std::mem::discriminant(incoming) {
        return Err(EffectiveConfigError::Merge);
    }
    match strategy {
        ConfigMergeStrategyV1::Replace => Ok(incoming.clone()),
        ConfigMergeStrategyV1::DeepMerge => match (current, incoming) {
            (ConfigValueV1::Json(current), ConfigValueV1::Json(incoming)) => Ok(
                ConfigValueV1::Json(deep_merge_json(current.clone(), incoming)),
            ),
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::MostRestrictive => match (current, incoming) {
            (ConfigValueV1::String(current), ConfigValueV1::String(incoming)) => {
                if current == incoming {
                    return Ok(ConfigValueV1::String(current.clone()));
                }
                let current_rank =
                    restrictive_rank(key, current).ok_or(EffectiveConfigError::Merge)?;
                let incoming_rank =
                    restrictive_rank(key, incoming).ok_or(EffectiveConfigError::Merge)?;
                if lower_constraint_source(source_kind) && incoming_rank < current_rank {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::String(if incoming_rank > current_rank {
                    incoming.clone()
                } else {
                    current.clone()
                }))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::MinimumLimit => match (current, incoming) {
            (ConfigValueV1::Integer(current), ConfigValueV1::Integer(incoming)) => {
                if lower_constraint_source(source_kind) && incoming > current {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::Integer((*current).min(*incoming)))
            }
            (ConfigValueV1::Json(current), ConfigValueV1::Json(incoming)) => {
                let current = optional_json_limit(current)?;
                let incoming = optional_json_limit(incoming)?;
                if lower_constraint_source(source_kind)
                    && match (current, incoming) {
                        (Some(current), Some(incoming)) => incoming > current,
                        (Some(_), None) => true,
                        _ => false,
                    }
                {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::Json(
                    current
                        .into_iter()
                        .chain(incoming)
                        .min()
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                ))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::Intersection => match (current, incoming) {
            (ConfigValueV1::StringSet(current), ConfigValueV1::StringSet(incoming)) => {
                if key == "scan.include_paths" && current.is_empty() {
                    return Ok(ConfigValueV1::StringSet(incoming.clone()));
                }
                if key == "scan.include_paths" && incoming.is_empty() {
                    return Ok(ConfigValueV1::StringSet(current.clone()));
                }
                let incoming = incoming.iter().collect::<std::collections::BTreeSet<_>>();
                let values = current
                    .iter()
                    .filter(|value| incoming.contains(value))
                    .cloned()
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::StringSet(values))
            }
            (ConfigValueV1::String(current), ConfigValueV1::String(incoming))
                if current == incoming =>
            {
                Ok(ConfigValueV1::String(current.clone()))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::Union => match (current, incoming) {
            (ConfigValueV1::StringSet(current), ConfigValueV1::StringSet(incoming)) => {
                let mut values = current.clone();
                values.extend(incoming.iter().cloned());
                Ok(ConfigValueV1::StringSet(values))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::Immutable => {
            if current == incoming {
                Ok(current.clone())
            } else {
                Err(EffectiveConfigError::Merge)
            }
        }
        ConfigMergeStrategyV1::PolicyAllowThenFalseWins | ConfigMergeStrategyV1::FalseWins => {
            match (current, incoming) {
                (ConfigValueV1::Boolean(current), ConfigValueV1::Boolean(incoming)) => {
                    if lower_constraint_source(source_kind) && !current && *incoming {
                        return Err(EffectiveConfigError::Merge);
                    }
                    Ok(ConfigValueV1::Boolean(*current && *incoming))
                }
                _ => Err(EffectiveConfigError::Merge),
            }
        }
        ConfigMergeStrategyV1::TrueWins => match (current, incoming) {
            (ConfigValueV1::Boolean(current), ConfigValueV1::Boolean(incoming)) => {
                if lower_constraint_source(source_kind) && *current && !incoming {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::Boolean(*current || *incoming))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::MaximumFloor => match (current, incoming) {
            (ConfigValueV1::Integer(current), ConfigValueV1::Integer(incoming)) => {
                if lower_constraint_source(source_kind) && incoming < current {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::Integer((*current).max(*incoming)))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
        ConfigMergeStrategyV1::ExplicitWidening => match (current, incoming) {
            (ConfigValueV1::Boolean(_), ConfigValueV1::Boolean(incoming))
                if matches!(
                    source_kind,
                    ConfigSourceKindV1::PolicyProfile | ConfigSourceKindV1::User
                ) =>
            {
                Ok(ConfigValueV1::Boolean(*incoming))
            }
            (ConfigValueV1::Boolean(current), ConfigValueV1::Boolean(incoming)) => {
                if !current && *incoming {
                    return Err(EffectiveConfigError::Merge);
                }
                Ok(ConfigValueV1::Boolean(*current && *incoming))
            }
            _ => Err(EffectiveConfigError::Merge),
        },
    }
}

fn lower_constraint_source(source_kind: ConfigSourceKindV1) -> bool {
    matches!(
        source_kind,
        ConfigSourceKindV1::Project
            | ConfigSourceKindV1::Goal
            | ConfigSourceKindV1::Command
            | ConfigSourceKindV1::PlatformConstraint
    )
}

fn optional_json_limit(value: &serde_json::Value) -> Result<Option<u64>, EffectiveConfigError> {
    if value.is_null() {
        Ok(None)
    } else {
        value.as_u64().map(Some).ok_or(EffectiveConfigError::Merge)
    }
}

fn deep_merge_json(
    mut current: serde_json::Value,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    match (&mut current, incoming) {
        (serde_json::Value::Object(current), serde_json::Value::Object(incoming)) => {
            for (key, value) in incoming {
                let merged = current
                    .remove(key)
                    .map(|current| deep_merge_json(current, value))
                    .unwrap_or_else(|| value.clone());
                current.insert(key.clone(), merged);
            }
            serde_json::Value::Object(current.clone())
        }
        _ => incoming.clone(),
    }
}

fn restrictive_rank(key: &str, value: &str) -> Option<u8> {
    if key == "policy_profile" {
        return match value {
            "star.policy-profile.personal-auto" => Some(0),
            "star.policy-profile.safe-default" => Some(1),
            _ => None,
        };
    }
    if key == "index.required_tier" {
        return match value {
            "text" => Some(0),
            "syntax" => Some(1),
            "semantic" => Some(2),
            _ => None,
        };
    }
    if key == "index.max_tier" {
        return match value {
            "semantic" => Some(0),
            "syntax" => Some(1),
            "text" => Some(2),
            _ => None,
        };
    }
    if key == "scan.binary_mode" {
        return match value {
            "metadata_only" => Some(0),
            "skip" => Some(1),
            _ => None,
        };
    }
    if key == "vcs.merge_strategy" {
        return match value {
            "review_then_merge" => Some(0),
            "manual" => Some(1),
            "never" => Some(2),
            _ => None,
        };
    }
    if key == "state.cleanup_trigger" {
        return match value {
            "startup_and_manual" => Some(0),
            "manual" => Some(1),
            _ => None,
        };
    }
    if key == "management.integrity_check_on_unclean_start" {
        return match value {
            "quick" => Some(0),
            "full" => Some(1),
            _ => None,
        };
    }
    if key == "validation.fail_on" {
        return match value {
            "fatal" => Some(0),
            "error" => Some(1),
            "warning" => Some(2),
            "info" => Some(3),
            _ => None,
        };
    }
    match value {
        "auto" => Some(0),
        "prompt" => Some(1),
        "deny" => Some(2),
        _ => None,
    }
}

fn validate_key(key: &str) -> Result<(), EffectiveConfigError> {
    if key.is_empty()
        || key.len() > 160
        || key.starts_with('.')
        || key.ends_with('.')
        || key.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
    {
        return Err(EffectiveConfigError::Entries);
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.contains('\0')
}

fn validate_value(value: &mut ConfigValueV1) -> Result<(), EffectiveConfigError> {
    match value {
        ConfigValueV1::String(value) if !valid_identifier(value) => {
            Err(EffectiveConfigError::Value)
        }
        ConfigValueV1::StringSet(values) => {
            values.sort();
            if values.len() > 256
                || values.windows(2).any(|pair| pair[0] == pair[1])
                || values.iter().any(|value| !valid_identifier(value))
            {
                return Err(EffectiveConfigError::Value);
            }
            Ok(())
        }
        ConfigValueV1::Json(value) => {
            if crate::canonical::jcs_bytes(value)
                .map_err(|_| EffectiveConfigError::Value)?
                .len()
                > 64 * 1024
                || json_depth(value) > 32
            {
                return Err(EffectiveConfigError::Value);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(values) => {
            1 + values.iter().map(json_depth).max().unwrap_or_default()
        }
        serde_json::Value::Object(values) => {
            1 + values.values().map(json_depth).max().unwrap_or_default()
        }
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(kind: ConfigSourceKindV1, id: &str) -> ConfigSourceRefV1 {
        ConfigSourceRefV1 {
            source_kind: kind,
            source_id: id.to_owned(),
            source_fingerprint: Sha256Hash::digest(id.as_bytes()),
        }
    }

    #[test]
    fn effective_config_is_sorted_typed_and_fingerprint_bound() {
        let config = EffectiveConfigV1::seal(
            "star.policy-profile.safe-default",
            vec![
                EffectiveConfigEntryV1 {
                    key: "validation.max_parallel_checks".to_owned(),
                    value: ConfigValueV1::Integer(4),
                    merge_strategy: ConfigMergeStrategyV1::MinimumLimit,
                    provenance: vec![source(ConfigSourceKindV1::User, "user-config")],
                },
                EffectiveConfigEntryV1 {
                    key: "change_planning.require_current_inputs".to_owned(),
                    value: ConfigValueV1::Boolean(true),
                    merge_strategy: ConfigMergeStrategyV1::TrueWins,
                    provenance: vec![source(ConfigSourceKindV1::BuiltIn, "product-default")],
                },
            ],
        )
        .unwrap();
        config.verify().unwrap();
        assert_eq!(
            config.entries[0].key,
            "change_planning.require_current_inputs"
        );
        assert_eq!(
            config.get("validation.max_parallel_checks"),
            Some(&ConfigValueV1::Integer(4))
        );

        let mut tampered = config;
        tampered.entries[0].value = ConfigValueV1::Boolean(false);
        assert_eq!(tampered.verify(), Err(EffectiveConfigError::Fingerprint));
    }

    #[test]
    fn invalid_keys_and_duplicate_provenance_fail_closed() {
        let duplicate = source(ConfigSourceKindV1::User, "user-config");
        assert_eq!(
            EffectiveConfigV1::seal(
                "star.policy-profile.safe-default",
                vec![EffectiveConfigEntryV1 {
                    key: "Validation.Bad".to_owned(),
                    value: ConfigValueV1::Boolean(true),
                    merge_strategy: ConfigMergeStrategyV1::Immutable,
                    provenance: vec![duplicate.clone(), duplicate],
                }],
            ),
            Err(EffectiveConfigError::Entries)
        );
    }

    #[test]
    fn lower_precedence_layers_apply_declared_merge_strategies_and_provenance() {
        let built_in = source(ConfigSourceKindV1::BuiltIn, "product-default");
        let base = EffectiveConfigV1::seal(
            "star.policy-profile.personal-auto",
            vec![
                EffectiveConfigEntryV1 {
                    key: "policy_profile".to_owned(),
                    value: ConfigValueV1::String("star.policy-profile.personal-auto".to_owned()),
                    merge_strategy: ConfigMergeStrategyV1::MostRestrictive,
                    provenance: vec![built_in.clone()],
                },
                EffectiveConfigEntryV1 {
                    key: "scan.max_files".to_owned(),
                    value: ConfigValueV1::Integer(200_000),
                    merge_strategy: ConfigMergeStrategyV1::MinimumLimit,
                    provenance: vec![built_in.clone()],
                },
                EffectiveConfigEntryV1 {
                    key: "scan.include_ignored".to_owned(),
                    value: ConfigValueV1::Boolean(true),
                    merge_strategy: ConfigMergeStrategyV1::ExplicitWidening,
                    provenance: vec![built_in.clone()],
                },
                EffectiveConfigEntryV1 {
                    key: "scan.exclude_paths_add".to_owned(),
                    value: ConfigValueV1::StringSet(vec!["main".to_owned()]),
                    merge_strategy: ConfigMergeStrategyV1::Union,
                    provenance: vec![built_in.clone()],
                },
                EffectiveConfigEntryV1 {
                    key: "state.artifact_root".to_owned(),
                    value: ConfigValueV1::String(".ai-runs/star-control".to_owned()),
                    merge_strategy: ConfigMergeStrategyV1::Immutable,
                    provenance: vec![built_in],
                },
            ],
        )
        .unwrap();
        let project_source = source(ConfigSourceKindV1::Project, "project-config");
        let merged = base
            .clone()
            .apply_layer(ConfigLayerV1 {
                source: project_source.clone(),
                overrides: vec![
                    ConfigOverrideV1 {
                        key: "policy_profile".to_owned(),
                        value: ConfigValueV1::String("star.policy-profile.safe-default".to_owned()),
                    },
                    ConfigOverrideV1 {
                        key: "scan.max_files".to_owned(),
                        value: ConfigValueV1::Integer(10_000),
                    },
                    ConfigOverrideV1 {
                        key: "scan.include_ignored".to_owned(),
                        value: ConfigValueV1::Boolean(false),
                    },
                    ConfigOverrideV1 {
                        key: "scan.exclude_paths_add".to_owned(),
                        value: ConfigValueV1::StringSet(vec!["release".to_owned()]),
                    },
                ],
            })
            .unwrap();
        assert_eq!(merged.policy_profile_id, "star.policy-profile.safe-default");
        assert_eq!(merged.integer("scan.max_files"), Some(10_000));
        assert_eq!(merged.boolean("scan.include_ignored"), Some(false));
        assert_eq!(
            merged.string_set("scan.exclude_paths_add"),
            Some(["main".to_owned(), "release".to_owned()].as_slice())
        );
        assert!(
            merged
                .entries
                .iter()
                .find(|entry| entry.key == "scan.max_files")
                .unwrap()
                .provenance
                .contains(&project_source)
        );
        assert_eq!(
            merged.clone().apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Goal, "widening-goal"),
                overrides: vec![ConfigOverrideV1 {
                    key: "policy_profile".to_owned(),
                    value: ConfigValueV1::String("star.policy-profile.personal-auto".to_owned(),),
                }],
            }),
            Err(EffectiveConfigError::Merge)
        );
        assert_eq!(
            merged.clone().apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Command, "widening-command"),
                overrides: vec![ConfigOverrideV1 {
                    key: "scan.max_files".to_owned(),
                    value: ConfigValueV1::Integer(20_000),
                }],
            }),
            Err(EffectiveConfigError::Merge)
        );
        assert_eq!(
            base.apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Project, "bad-project-config"),
                overrides: vec![ConfigOverrideV1 {
                    key: "state.artifact_root".to_owned(),
                    value: ConfigValueV1::String("elsewhere".to_owned()),
                }],
            }),
            Err(EffectiveConfigError::Merge)
        );

        for key in [
            "routing.default_model_role",
            "permissions.actions.local_write",
            "validation.fail_on",
            "vcs.protected_branches",
            "state.cleanup_trigger",
        ] {
            let value = match key {
                "permissions.actions.local_write" => ConfigValueV1::String("prompt".to_owned()),
                "validation.fail_on" => ConfigValueV1::String("error".to_owned()),
                "vcs.protected_branches" => ConfigValueV1::StringSet(vec!["main".to_owned()]),
                "state.cleanup_trigger" => ConfigValueV1::String("startup_and_manual".to_owned()),
                _ => ConfigValueV1::String("terra".to_owned()),
            };
            let entry = EffectiveConfigEntryV1 {
                key: key.to_owned(),
                value: value.clone(),
                merge_strategy: match key {
                    "vcs.protected_branches" => ConfigMergeStrategyV1::Union,
                    "state.cleanup_trigger" | "validation.fail_on" => {
                        ConfigMergeStrategyV1::MostRestrictive
                    }
                    _ => ConfigMergeStrategyV1::Replace,
                },
                provenance: vec![source(ConfigSourceKindV1::BuiltIn, "reserved-default")],
            };
            let reserved =
                EffectiveConfigV1::seal("star.policy-profile.safe-default", vec![entry]).unwrap();
            assert_eq!(
                reserved.apply_layer(ConfigLayerV1 {
                    source: source(ConfigSourceKindV1::Command, "reserved-command"),
                    overrides: vec![ConfigOverrideV1 {
                        key: key.to_owned(),
                        value,
                    }],
                }),
                Err(EffectiveConfigError::Merge),
                "{key} must not be accepted without a runtime consumer"
            );
        }
    }

    #[test]
    fn empty_scan_include_set_means_unrestricted_during_intersection() {
        let built_in = source(ConfigSourceKindV1::BuiltIn, "product-default");
        let base = EffectiveConfigV1::seal(
            "star.policy-profile.safe-default",
            vec![
                EffectiveConfigEntryV1 {
                    key: "policy_profile".to_owned(),
                    value: ConfigValueV1::String("star.policy-profile.safe-default".to_owned()),
                    merge_strategy: ConfigMergeStrategyV1::MostRestrictive,
                    provenance: vec![built_in.clone()],
                },
                EffectiveConfigEntryV1 {
                    key: "scan.include_paths".to_owned(),
                    value: ConfigValueV1::StringSet(Vec::new()),
                    merge_strategy: ConfigMergeStrategyV1::Intersection,
                    provenance: vec![built_in],
                },
            ],
        )
        .unwrap();
        let restricted = base
            .apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Project, "project-config"),
                overrides: vec![ConfigOverrideV1 {
                    key: "scan.include_paths".to_owned(),
                    value: ConfigValueV1::StringSet(vec!["crates/**".to_owned()]),
                }],
            })
            .unwrap();
        assert_eq!(
            restricted.string_set("scan.include_paths"),
            Some(["crates/**".to_owned()].as_slice())
        );
        let inherited = restricted
            .clone()
            .apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Goal, "goal-config"),
                overrides: vec![ConfigOverrideV1 {
                    key: "scan.include_paths".to_owned(),
                    value: ConfigValueV1::StringSet(Vec::new()),
                }],
            })
            .unwrap();
        assert_eq!(
            inherited.string_set("scan.include_paths"),
            Some(["crates/**".to_owned()].as_slice())
        );
        assert_eq!(
            restricted.apply_layer(ConfigLayerV1 {
                source: source(ConfigSourceKindV1::Command, "command-config"),
                overrides: vec![ConfigOverrideV1 {
                    key: "scan.include_paths".to_owned(),
                    value: ConfigValueV1::StringSet(vec!["apps/**".to_owned()]),
                }],
            }),
            Err(EffectiveConfigError::Merge)
        );
    }
}
