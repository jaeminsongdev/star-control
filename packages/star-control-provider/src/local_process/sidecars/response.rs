use crate::fake::provider_output_path;
use crate::local_process::constants::{FORBIDDEN_ACTION_EVIDENCE_PREFIX, STDERR_FILE, STDOUT_FILE};
use crate::local_process::policy::LocalProcessCommandPolicy;
use crate::local_process::runner::LocalProcessRunResult;
use crate::provider_cost::COST_METRIC_FILE;
use crate::ExecutionRequest;
use serde_json::{json, Value};

pub(crate) fn response_value(
    request: &ExecutionRequest,
    policy: &LocalProcessCommandPolicy,
    process_result: &LocalProcessRunResult,
    wall_time_ms: u64,
    redaction_artifacts: &[String],
) -> Value {
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), "response.json");
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);
    let (status, summary, error) = match process_result {
        LocalProcessRunResult::Exited { status } if status.success() => (
            "success",
            "local process completed with exit code 0".to_string(),
            Value::Null,
        ),
        LocalProcessRunResult::Exited { status } => {
            let exit_code = status.code();
            (
                "failed",
                format!(
                    "local process exited with code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                json!({
                    "kind": "local_process_exit",
                    "exit_code": exit_code
                }),
            )
        }
        LocalProcessRunResult::TimedOut => (
            "timeout",
            format!(
                "local process timed out after {} second(s)",
                policy.timeout_seconds()
            ),
            json!({
                "kind": "local_process_timeout",
                "timeout_seconds": policy.timeout_seconds()
            }),
        ),
        LocalProcessRunResult::Cancelled { phase } => (
            "cancelled",
            "local process cancelled by RunState".to_string(),
            json!({
                "kind": "local_process_cancelled",
                "phase": phase
            }),
        ),
        LocalProcessRunResult::BlockedForbiddenAction { evidence } => (
            "blocked",
            format!(
                "local process reported forbidden action evidence: {}",
                evidence.action
            ),
            json!({
                "kind": "local_process_forbidden_action",
                "action": evidence.action,
                "source": evidence.source,
                "evidence_prefix": FORBIDDEN_ACTION_EVIDENCE_PREFIX
            }),
        ),
        LocalProcessRunResult::LaunchFailed { message } => (
            "error",
            "local process failed to launch".to_string(),
            json!({
                "kind": "local_process_launch_failed",
                "message": message
            }),
        ),
        LocalProcessRunResult::WaitFailed { source } => (
            "error",
            "local process wait failed".to_string(),
            json!({
                "kind": "local_process_wait_failed",
                "message": source.to_string()
            }),
        ),
    };

    let mut artifacts = vec![
        response_path,
        stdout_path.clone(),
        stderr_path.clone(),
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
            "estimated_cost": 0,
            "currency": "USD",
            "input_tokens": 0,
            "output_tokens": 0,
            "wall_time_ms": wall_time_ms
        },
        "error": error
    })
}
