use super::SmokeFixture;
use serde_json::Value;
use star_control_cli::{run_cli, CliConfig};

impl SmokeFixture {
    pub(crate) fn run_fake_cli(&self, request: &str) -> Value {
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

    pub(crate) fn report_json(&self, job_id: &str, stage: &str) -> Value {
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
