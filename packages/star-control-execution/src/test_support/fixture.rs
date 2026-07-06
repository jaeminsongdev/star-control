mod cloud;
mod local_process;
mod workspec;

use super::helpers::{repo_root, schema_root, temp_project};
use crate::state::initial_state;
use crate::ExecutionEngine;
use star_control_provider::{FakeProviderAdapter, ProviderRegistry, ProviderRegistryLoader};
use star_control_router::{JobSpec, RouterEngine};
use star_control_state::StateStore;
use std::fs;
use std::path::PathBuf;

pub(crate) struct Fixture {
    pub(crate) project: PathBuf,
    pub(crate) store: StateStore,
    pub(crate) registry: ProviderRegistry,
    pub(crate) schemas: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let project = temp_project();
        let schemas = schema_root();
        let store = StateStore::open(&project, &schemas).expect("open store");
        let job = store
            .create_job("runtime code 구현", "codex", vec![])
            .expect("create job");
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_fake_default_registry()
            .expect("load registry");
        let router = RouterEngine::new(&registry, &schemas);
        let job_spec = JobSpec::from_value(job.clone(), "job.json", &schemas).expect("job spec");
        let output = router.route(&job_spec).expect("route");
        store
            .save_route("J-0001", output.route().value())
            .expect("save route");
        for (stage, workspec) in output.workspecs() {
            store
                .save_workspec("J-0001", stage, workspec.value())
                .expect("save workspec");
        }
        store
            .save_state(
                "J-0001",
                &initial_state(
                    "J-0001",
                    "implement",
                    job["created_at"].as_str().unwrap_or("created"),
                ),
            )
            .expect("save state");
        Self {
            project,
            store,
            registry,
            schemas,
        }
    }

    pub(crate) fn engine(&self, adapter: FakeProviderAdapter) -> ExecutionEngine<'_> {
        ExecutionEngine::new(&self.store, &self.registry, &self.schemas).with_fake_adapter(adapter)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.project).ok();
    }
}
