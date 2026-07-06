use super::{
    OpenAiCompatibleRequestApi, OpenAiCompatibleRequestError, CHAT_COMPLETIONS_PATH, DEFAULT_API,
    DEFAULT_RESPONSES_PATH,
};
use serde_json::Value;

pub(super) fn request_api(
    value: &Value,
) -> Result<OpenAiCompatibleRequestApi, OpenAiCompatibleRequestError> {
    let Some(item) = value.pointer("/endpoint/api") else {
        return Ok(DEFAULT_API);
    };
    let Some(api) = item.as_str() else {
        return Err(OpenAiCompatibleRequestError::InvalidFieldType {
            field: "endpoint.api".to_string(),
            expected: "string",
        });
    };
    match api {
        "responses" => Ok(OpenAiCompatibleRequestApi::Responses),
        "chat_completions" | "chat-completions" => Ok(OpenAiCompatibleRequestApi::ChatCompletions),
        other => Err(OpenAiCompatibleRequestError::UnsupportedApi {
            api: other.to_string(),
        }),
    }
}

pub(super) fn required_string_pointer(
    value: &Value,
    pointer: &str,
) -> Result<String, OpenAiCompatibleRequestError> {
    let Some(item) = value.pointer(pointer) else {
        return Err(OpenAiCompatibleRequestError::MissingField {
            field: pointer.trim_start_matches('/').replace('/', "."),
        });
    };
    let field = pointer.trim_start_matches('/').replace('/', ".");
    let Some(text) = item.as_str() else {
        return Err(OpenAiCompatibleRequestError::InvalidFieldType {
            field,
            expected: "string",
        });
    };
    if text.is_empty() {
        return Err(OpenAiCompatibleRequestError::EmptyField { field });
    }
    Ok(text.to_string())
}

pub(super) fn endpoint_url(base_url: &str, api: OpenAiCompatibleRequestApi) -> String {
    let path = match api {
        OpenAiCompatibleRequestApi::Responses => DEFAULT_RESPONSES_PATH,
        OpenAiCompatibleRequestApi::ChatCompletions => CHAT_COMPLETIONS_PATH,
    };
    format!("{}{}", base_url.trim_end_matches('/'), path)
}
