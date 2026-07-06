mod changed_lines;
mod constants;
mod error;
mod evaluator;
mod gate;
mod json_fields;
mod ledger;
mod model;
mod readers;
mod review_pack;
mod schema_io;
mod selfcheck;
mod task;

pub use changed_lines::{ChangedFile, ChangedHunk, ChangedLine, ChangedLines};
pub use constants::*;
pub use error::SentinelError;
pub use evaluator::{FixtureOutcome, P0Evaluator};
pub use gate::{
    build_approval_artifact, build_diagnostics_artifact, validate_approval_artifact,
    validate_diagnostics_artifact, write_gate_artifacts,
};
pub use ledger::{
    build_gate_ledger_event, ledger_events_jsonl, validate_ledger_events, write_ledger_artifact,
};
pub use model::{
    Decision, Diagnostic, DiagnosticLocation, EvaluationResult, GateArtifactRefs, LedgerEvent,
    P0RuleRegistry, ReviewPackArtifactRefs, ReviewValidation, RuleDefinition, SelfcheckReport,
    Severity,
};
pub use readers::{read_changed_lines, read_fixture_outcome, read_p0_rule_registry, read_task};
pub use review_pack::{
    build_review_pack_artifact, validate_review_pack_artifact, write_review_pack_artifacts,
};
pub use selfcheck::run_selfcheck;
pub use task::SentinelTask;

#[cfg(test)]
mod tests;
