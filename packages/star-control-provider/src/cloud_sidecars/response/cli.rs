use crate::cloud_cli::CloudCliRunResult;
use crate::cloud_constants::{
    CLI_TRANSPORT, COST_METRIC_FILE, PRIVACY_HANDOFF_FILE, RESPONSE_FILE, STDERR_FILE, STDOUT_FILE,
};
use crate::cloud_policy::{currency, estimated_cost};
use crate::fake::provider_output_path;
use crate::{ExecutionRequest, ProviderInstance, ProviderManifest};
use serde_json::{json, Value};

pub(crate) fn cli_response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    process_result: &CloudCliRunResult,
    wall_time_ms: u64,
    redaction_artifacts: &[String],
) -> Value {
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), RESPONSE_FILE);
    let privacy_path = provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE);
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);
    let (status, summary, error, exit_code) = match process_result {
        CloudCliRunResult::Exited { status } if status.success() => (
            "success",
            "cloud CLI provider completed with exit code 0".to_string(),
            Value::Null,
            status.code(),
        ),
        CloudCliRunResult::Exited { status } => {
            let exit_code = status.code();
            (
                "failed",
                format!(
                    "cloud CLI provider exited with code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                json!({
                    "kind": "cloud_cli_exit",
                    "exit_code": exit_code,
                    "provider_id": manifest.id()
                }),
                exit_code,
            )
        }
        CloudCliRunResult::TimedOut => (
            "timeout",
            "cloud CLI provider timed out".to_string(),
            json!({
                "kind": "cloud_cli_timeout",
                "provider_id": manifest.id()
            }),
            None,
        ),
        CloudCliRunResult::LaunchFailed { message } => (
            "error",
            "cloud CLI provider failed to launch".to_string(),
            json!({
                "kind": "cloud_cli_launch_failed",
                "message": message,
                "provider_id": manifest.id()
            }),
            None,
        ),
        CloudCliRunResult::WaitFailed { source } => (
            "error",
            "cloud CLI provider wait failed".to_string(),
            json!({
                "kind": "cloud_cli_wait_failed",
                "message": source.to_string(),
                "provider_id": manifest.id()
            }),
            None,
        ),
    };

    let mut artifacts = vec![
        response_path,
        stdout_path.clone(),
        stderr_path.clone(),
        privacy_path,
        cost_path,
    ];
    artifacts.extend(redaction_artifacts.iter().cloned());

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": status,
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "summary": summary,
        "changed_files": [],
        "artifacts": artifacts,
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": 0,
            "output_tokens": 0,
            "wall_time_ms": wall_time_ms,
            "exit_code": exit_code,
            "transport": CLI_TRANSPORT
        },
        "error": error
    })
}
