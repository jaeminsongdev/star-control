use super::project::schema_root;
use crate::{UiBrowserShell, UiReadOnlyShell};
use star_control_api::{ApiControlService, ApiReadOnlyService};
use star_control_state::StateStore;

pub(crate) fn ui_with_store(store: StateStore) -> UiReadOnlyShell {
    let mut api = ApiReadOnlyService::new(schema_root());
    api.register_project_store("local", store)
        .expect("register project");
    UiReadOnlyShell::new(schema_root(), api)
}

pub(crate) fn browser_with_store(store: StateStore) -> UiBrowserShell {
    let mut api = ApiControlService::new(schema_root());
    api.register_project_store("local", store)
        .expect("register project");
    UiBrowserShell::new(schema_root(), api)
}
