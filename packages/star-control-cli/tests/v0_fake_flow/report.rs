use super::SmokeFixture;
use serde_json::json;

impl SmokeFixture {
    pub(crate) fn write_done_report(&self, job_id: &str, summary: &str) {
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
}
