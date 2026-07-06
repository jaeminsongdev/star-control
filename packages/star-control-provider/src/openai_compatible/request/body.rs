use super::OpenAiCompatibleRequestApi;
use serde_json::{json, Value};

pub(super) fn request_body(api: OpenAiCompatibleRequestApi, model: &str, goal: &str) -> Value {
    match api {
        OpenAiCompatibleRequestApi::Responses => json!({
            "model": model,
            "input": goal,
            "stream": false
        }),
        OpenAiCompatibleRequestApi::ChatCompletions => json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": goal
                }
            ],
            "stream": false
        }),
    }
}
