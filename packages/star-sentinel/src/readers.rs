use crate::changed_lines::ChangedLines;
use crate::constants::{
    CHANGED_LINES_SCHEMA, FIXTURE_OUTCOME_SCHEMA, P0_RULE_REGISTRY_SCHEMA, SENTINEL_TASK_SCHEMA,
};
use crate::evaluator::FixtureOutcome;
use crate::model::P0RuleRegistry;
use crate::schema_io::read_validated_json;
use crate::{SentinelError, SentinelTask};
use std::path::Path;

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
