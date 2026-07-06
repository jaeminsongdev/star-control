use crate::ExecutionRequest;
use serde_json::Value;

pub(super) fn render_arg(arg: &str, request: &ExecutionRequest, request_ref: &Value) -> String {
    let request_path = request_ref
        .get("path")
        .and_then(Value::as_str)
        .map(|path| format!(".ai-runs/{}/{}", request.job_id(), path))
        .unwrap_or_else(|| request.workspec_path().to_string());
    arg.replace("{{request_path}}", &request_path)
        .replace("{{job_id}}", request.job_id())
        .replace("{{stage}}", request.stage())
        .replace("{{goal}}", request.goal())
}
