use crate::test_support::repo_root;
use crate::{CliConfig, CliRunResult};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn config() -> CliConfig {
    CliConfig::new(repo_root())
}

pub(super) fn path_arg(path: &Path) -> &str {
    path.to_str().expect("project path")
}

pub(super) fn json_output(result: &CliRunResult, label: &str) -> Value {
    serde_json::from_str(&result.stdout).expect(label)
}

pub(super) fn assert_success(result: &CliRunResult) {
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
}

pub(super) fn cleanup_project(project: PathBuf) {
    fs::remove_dir_all(project).ok();
}
