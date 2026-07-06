use super::super::{create_job, open_store, state, temp_project};
use crate::ArtifactKind;
use std::fs;

#[test]
fn registers_artifact_ref_in_run_state() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    let mut state = state("J-0001", "REQUESTED");
    let route_ref = store
        .artifact_ref(
            "J-0001",
            "route.json",
            ArtifactKind::Route,
            "router",
            Some("specs/schemas/route.schema.json"),
            Some("RouteSpec artifact"),
        )
        .expect("artifact ref");
    store
        .register_artifact_ref(&mut state, "route", &route_ref)
        .expect("register artifact ref");
    store.save_state("J-0001", &state).expect("save state");

    let loaded = store.load_state("J-0001").expect("load state");
    assert_eq!(loaded["artifacts"]["route"]["path"], "route.json");
    assert_eq!(loaded["artifacts"]["route"]["kind"], "route");

    fs::remove_dir_all(project).ok();
}
