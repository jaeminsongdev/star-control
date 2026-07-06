#[path = "cases/config.rs"]
mod config;
#[path = "cases/core.rs"]
mod core;
#[path = "cases/provider_execution.rs"]
mod provider_execution;
#[path = "cases/sentinel.rs"]
mod sentinel;
#[path = "cases/surface.rs"]
mod surface;

#[derive(Clone, Copy)]
pub(crate) struct ValidationCase {
    pub(crate) schema_path: &'static str,
    pub(crate) document_path: &'static str,
}

pub(crate) fn validation_cases() -> impl Iterator<Item = ValidationCase> {
    core::CASES
        .iter()
        .chain(provider_execution::CASES)
        .chain(surface::CASES)
        .chain(config::CASES)
        .chain(sentinel::CASES)
        .copied()
}
