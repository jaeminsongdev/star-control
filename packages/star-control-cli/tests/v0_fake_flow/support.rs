use star_control_state::StateStore;
use std::path::PathBuf;

mod cli;
mod fixture;
mod report;
mod validation;

pub(crate) use validation::{changed_lines_for, context};

pub(crate) struct SmokeFixture {
    pub(crate) project: PathBuf,
    repo_root: PathBuf,
    core_schema_root: PathBuf,
    sentinel_schema_root: PathBuf,
    pub(crate) store: StateStore,
}
