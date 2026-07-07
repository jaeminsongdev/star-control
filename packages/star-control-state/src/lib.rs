mod artifacts;
mod constants;
mod error;
mod events;
mod outputs;
mod paths;
mod recovery;
mod store;
mod types;

pub use error::StateStoreError;
pub use recovery::{
    JobSummary, RecoveryActionExecution, RecoveryActionPlan, RecoveryInspection, RecoveryIssue,
    RecoverySourceSelection, RECOVERY_ACTIONS,
};
pub use types::{ArtifactKind, StateStore};

pub(crate) use constants::SCHEMA_VERSION;

#[cfg(test)]
mod tests;
