mod approval;
mod builders;
mod gate;

use super::SmokeFixture;
use builders::sentinel_task;
pub(crate) use builders::{changed_lines_for, context};
use serde_json::{json, Value};
use star_control_validation::ValidationEngine;
use star_sentinel::{ChangedLines, SentinelTask};

impl SmokeFixture {
    pub(crate) fn run_validation<const N: usize>(
        &self,
        task_id: &str,
        allowed_paths: [&str; N],
        changed_lines_value: Value,
    ) -> Value {
        let task_value = sentinel_task(task_id, allowed_paths);
        let task = SentinelTask::from_value(&task_value).expect("task");
        let changed_lines = ChangedLines::from_value(&changed_lines_value).expect("changed lines");
        gate::write_sentinel_inputs(&self.store, &task_value, &changed_lines_value);

        let gate_artifacts = gate::evaluate_and_write(
            &self.store,
            &self.repo_root,
            &self.sentinel_schema_root,
            &task,
            &changed_lines,
        );

        let engine = self.validation_engine();
        engine
            .ensure_provider_response("J-0001", "fake-default")
            .expect("provider response");
        let context = context(task_id);
        let validation_outcome = engine
            .evaluate_star_sentinel_gate(
                &context,
                &gate_artifacts.approval,
                gate_artifacts.review_pack.as_ref(),
            )
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

    pub(crate) fn validation_engine(&self) -> ValidationEngine<'_> {
        ValidationEngine::new(
            &self.store,
            &self.core_schema_root,
            &self.sentinel_schema_root,
        )
    }

    pub(crate) fn write_approval_response(&self, job_id: &str, task_id: &str) {
        approval::write_approval_response(&self.store, job_id, task_id);
    }
}
