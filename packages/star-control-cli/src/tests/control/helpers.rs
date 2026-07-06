use crate::{run_cli, CliConfig, CliRunResult};
use serde_json::Value;
use std::path::Path;

pub(super) fn parse_stdout_json(result: &CliRunResult, context: &str) -> Value {
    serde_json::from_str(&result.stdout).expect(context)
}

pub(super) fn assert_error_code(
    result: &CliRunResult,
    context: &str,
    expected_code: &str,
) -> Value {
    let error_json = parse_stdout_json(result, context);
    assert_eq!(error_json["error"]["code"], expected_code);
    error_json
}

pub(super) fn run_approve_with_constraint(project: &Path, config: &CliConfig) -> CliRunResult {
    run_cli(
        [
            "approve",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--response",
            "approved",
            "--reason",
            "approved by CLI test",
            "--constraint",
            "keep validation strict",
            "--json",
        ],
        config,
    )
}

pub(super) fn run_approve_without_constraint(project: &Path, config: &CliConfig) -> CliRunResult {
    run_cli(
        [
            "approve",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--response",
            "approved",
            "--reason",
            "approved by CLI test",
            "--json",
        ],
        config,
    )
}

pub(super) fn run_resume(project: &Path, config: &CliConfig) -> CliRunResult {
    run_cli(
        [
            "resume",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        config,
    )
}

pub(super) fn run_dry_run(project: &Path, config: &CliConfig) -> CliRunResult {
    run_cli(
        [
            "run",
            "--project",
            project.to_str().expect("project path"),
            "--request",
            "README 문서 수정",
            "--dry-run",
            "--json",
        ],
        config,
    )
}

pub(super) fn run_cancel(project: &Path, config: &CliConfig) -> CliRunResult {
    run_cli(
        [
            "cancel",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        config,
    )
}
