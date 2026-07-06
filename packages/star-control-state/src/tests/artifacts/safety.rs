use super::super::{create_job, open_store, temp_project};
use crate::{ArtifactKind, StateStoreError};
use serde_json::json;
use std::fs;

#[test]
fn artifact_writers_reject_unsafe_names() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    assert!(matches!(
        store.write_provider_json("J-0001", "../fake", "request.json", &json!({})),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));
    assert!(matches!(
        store.write_tool_json("J-0001", "star-sentinel", "../diagnostics.json", &json!({})),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));
    assert!(matches!(
        store.artifact_ref(
            "J-0001",
            "/absolute/path.json",
            ArtifactKind::Other,
            "test",
            None,
            None,
        ),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));

    fs::remove_dir_all(project).ok();
}
