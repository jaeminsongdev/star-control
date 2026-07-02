use serde_json::{json, Value};
use star_control_cli::{run_cli, CliConfig};
use star_control_state::StateStore;
use star_control_validation::{ValidationContext, ValidationEngine};
use star_sentinel::{
    build_approval_artifact, build_review_pack_artifact, read_p0_rule_registry,
    write_gate_artifacts, write_review_pack_artifacts, ChangedLines, Decision, P0Evaluator,
    ReviewValidation, SentinelTask,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn v0_fake_flow_auto_pass_reaches_done_report() {
    let fixture = SmokeFixture::new();
    let run = fixture.run_fake_cli("runtime code 구현");
    assert_eq!(run["data"]["state"], "IMPLEMENTED");

    let outcome = fixture.run_validation(
        "p0-auto-pass",
        ["src/**"],
        changed_lines_for("p0-auto-pass", "src/lib.rs", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "VALIDATED");
    fixture.write_done_report("J-0001", "AUTO_PASS smoke reached final report");

    let report = fixture.report_json("J-0001", "report");
    assert_eq!(report["data"]["report"]["stage"], "report");
    assert_eq!(report["data"]["report"]["status"], "DONE");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "DONE"
    );
}

#[test]
fn v0_fake_flow_human_review_waits_for_matching_approval() {
    let fixture = SmokeFixture::new();
    fixture.run_fake_cli("runtime code 구현");

    let outcome = fixture.run_validation(
        "p0-human-review",
        ["**"],
        changed_lines_for("p0-human-review", "Cargo.toml", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "WAITING_APPROVAL");
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/approvals/approval-request.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/review-packs/handoff.json")
        .is_file());

    let missing = fixture
        .validation_engine()
        .ensure_approval_response_allows_next_stage(&context("p0-human-review"))
        .unwrap_err();
    assert!(missing.to_string().contains("approval response missing"));

    fixture.write_approval_response("J-0001", "p0-human-review");
    let response = fixture
        .validation_engine()
        .ensure_approval_response_allows_next_stage(&context("p0-human-review"))
        .expect("approved response");
    assert_eq!(response["response"], "approved");

    fixture.write_done_report("J-0001", "HUMAN_REVIEW smoke completed after approval");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "DONE"
    );
}

#[test]
fn v0_fake_flow_block_stops_at_blocked_state() {
    let fixture = SmokeFixture::new();
    fixture.run_fake_cli("runtime code 구현");

    let outcome = fixture.run_validation(
        "p0-block",
        ["src/allowed/**"],
        changed_lines_for("p0-block", "src/other.rs", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "BLOCKED");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "BLOCKED"
    );
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/validation/validation-decision.json")
        .is_file());
}

struct SmokeFixture {
    project: PathBuf,
    repo_root: PathBuf,
    core_schema_root: PathBuf,
    sentinel_schema_root: PathBuf,
    store: StateStore,
}

impl SmokeFixture {
    fn new() -> Self {
        let project = temp_project();
        let repo_root = repo_root();
        let core_schema_root = repo_root.join("specs/schemas");
        let sentinel_schema_root = repo_root.join("builtin-tools/star-sentinel/schemas");
        let store = StateStore::open(&project, &core_schema_root).expect("state store");
        Self {
            project,
            repo_root,
            core_schema_root,
            sentinel_schema_root,
            store,
        }
    }

    fn run_fake_cli(&self, request: &str) -> Value {
        let config = CliConfig::new(&self.repo_root);
        let result = run_cli(
            [
                "run",
                "--project",
                self.project.to_str().expect("project path"),
                "--request",
                request,
                "--provider",
                "fake-default",
                "--json",
            ],
            &config,
        );
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        let output: Value = serde_json::from_str(&result.stdout).expect("run json");
        assert_eq!(output["command"], "run");
        assert_eq!(output["data"]["job_id"], "J-0001");
        assert!(self
            .project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .is_file());
        output
    }

    fn run_validation<const N: usize>(
        &self,
        task_id: &str,
        allowed_paths: [&str; N],
        changed_lines_value: Value,
    ) -> Value {
        let task_value = sentinel_task(task_id, allowed_paths);
        let task = SentinelTask::from_value(&task_value).expect("task");
        let changed_lines = ChangedLines::from_value(&changed_lines_value).expect("changed lines");
        self.store
            .write_tool_json("J-0001", "star-sentinel", "task.json", &task_value)
            .expect("write task");
        self.store
            .write_tool_json(
                "J-0001",
                "star-sentinel",
                "changed_lines.json",
                &changed_lines_value,
            )
            .expect("write changed lines");

        let registry = read_p0_rule_registry(
            self.repo_root
                .join("builtin-tools/star-sentinel/policies/p0-rule-registry.json"),
            &self.sentinel_schema_root,
        )
        .expect("registry");
        let result = P0Evaluator::new(registry)
            .evaluate(&task, &changed_lines)
            .expect("evaluate");
        write_gate_artifacts(
            &self.store,
            "J-0001",
            &task,
            &result,
            &self.sentinel_schema_root,
        )
        .expect("gate artifacts");

        let approval = build_approval_artifact(&task, &result);
        let review_pack = if result.decision == Decision::AutoPass {
            None
        } else {
            let review_pack = build_review_pack_artifact(
                &task,
                &changed_lines,
                &result,
                &[ReviewValidation::new(
                    "policy:p0",
                    validation_result_for_decision(result.decision),
                )],
            );
            write_review_pack_artifacts(
                &self.store,
                "J-0001",
                &review_pack,
                &self.sentinel_schema_root,
            )
            .expect("review pack artifacts");
            Some(review_pack)
        };

        let engine = self.validation_engine();
        engine
            .ensure_provider_response("J-0001", "fake-default")
            .expect("provider response");
        let context = context(task_id);
        let validation_outcome = engine
            .evaluate_star_sentinel_gate(&context, &approval, review_pack.as_ref())
            .expect("validation outcome");
        let written = engine
            .write_outcome(&context, &validation_outcome)
            .expect("write validation outcome");

        json!({
            "decision": validation_outcome.decision(),
            "validation_run_ref": written.validation_run_ref(),
            "decision_ref": written.decision_ref(),
            "state": written.state()
        })
    }

    fn validation_engine(&self) -> ValidationEngine<'_> {
        ValidationEngine::new(
            &self.store,
            &self.core_schema_root,
            &self.sentinel_schema_root,
        )
    }

    fn write_approval_response(&self, job_id: &str, task_id: &str) {
        self.store
            .write_approval_json(
                job_id,
                "approval-response.json",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": job_id,
                    "stage": "validate",
                    "task_id": task_id,
                    "response": "approved",
                    "reviewer": "integration-smoke",
                    "responded_at": "2026-07-01T00:00:00Z",
                    "reason": "approved for v0 fake integration smoke",
                    "allowed_next_stage": "report",
                    "constraints": []
                }),
            )
            .expect("approval response");
    }

    fn write_done_report(&self, job_id: &str, summary: &str) {
        self.store
            .save_report(
                job_id,
                "report-report",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": job_id,
                    "stage": "report",
                    "status": "DONE",
                    "changed_files": [],
                    "commands_run": [
                        {
                            "command": "star-control run",
                            "result": "PASS",
                            "evidence_path": ".ai-runs/J-0001/provider-output/fake-default/response.json"
                        },
                        {
                            "command": "star-sentinel gate",
                            "result": "PASS",
                            "evidence_path": ".ai-runs/J-0001/validation/validation-decision.json"
                        }
                    ],
                    "validation": [
                        {
                            "summary": summary,
                            "decision_path": ".ai-runs/J-0001/validation/validation-decision.json"
                        }
                    ],
                    "risks": [],
                    "blocked_reason": null,
                    "next_step": "v0 fake flow complete",
                    "artifacts": [
                        ".ai-runs/J-0001/reports/report-report.json",
                        ".ai-runs/J-0001/validation/validation-decision.json"
                    ]
                }),
            )
            .expect("save final report");
        let mut state = self.store.load_state(job_id).expect("state");
        state["state"] = json!("DONE");
        state["current_stage"] = json!("report");
        state["updated_at"] = json!("2026-07-01T00:00:00Z");
        state["next_action"] = json!("complete");
        self.store.save_state(job_id, &state).expect("save state");
    }

    fn report_json(&self, job_id: &str, stage: &str) -> Value {
        let config = CliConfig::new(&self.repo_root);
        let result = run_cli(
            [
                "report",
                "--project",
                self.project.to_str().expect("project path"),
                "--job",
                job_id,
                "--stage",
                stage,
                "--json",
            ],
            &config,
        );
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        serde_json::from_str(&result.stdout).expect("report json")
    }
}

impl Drop for SmokeFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.project).ok();
    }
}

fn context(task_id: &str) -> ValidationContext {
    ValidationContext::new("J-0001", "validate", task_id, "2026-07-01T00:00:00Z")
}

fn sentinel_task<const N: usize>(task_id: &str, allowed_paths: [&str; N]) -> Value {
    let allowed_paths = allowed_paths.into_iter().collect::<Vec<_>>();
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "goal": "v0 fake integration smoke",
        "allowed_paths": allowed_paths,
        "forbidden_paths": [],
        "forbidden_change_types": [],
        "required_validation": ["policy:p0"],
        "approval_required_changes": ["dependency"]
    })
}

fn changed_lines_for(task_id: &str, path: &str, change_type: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "files": [
            {
                "path": path,
                "change_type": change_type,
                "hunks": [
                    {
                        "old_start": 1,
                        "old_lines": 1,
                        "new_start": 1,
                        "new_lines": 1,
                        "lines": [
                            {
                                "kind": "added",
                                "new_line": 1,
                                "content": "smoke fixture line"
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

fn validation_result_for_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "passed",
        Decision::HumanReview => "requires_human_review",
        Decision::Block => "blocked",
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "star-control-v0-smoke-{}-{}",
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}
