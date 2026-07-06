use serde_json::Value;

pub(super) fn provider_result_artifacts(result: &Value, job_id: &str) -> Vec<String> {
    result
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|path| format!(".ai-runs/{}/{}", job_id, path))
        .collect()
}
