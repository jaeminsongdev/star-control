use crate::{ExecutionRequest, ProviderInstance};
use serde_json::json;
use serde_json::Value;
use std::error::Error;
use std::fmt;

const DEFAULT_RESPONSES_PATH: &str = "/responses";
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
const DEFAULT_API: OpenAiCompatibleRequestApi = OpenAiCompatibleRequestApi::Responses;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatibleRequestApi {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiCompatiblePreparedRequest {
    api: OpenAiCompatibleRequestApi,
    method: String,
    url: String,
    body: Value,
}

impl OpenAiCompatiblePreparedRequest {
    pub fn api(&self) -> OpenAiCompatibleRequestApi {
        self.api
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn body(&self) -> &Value {
        &self.body
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenAiCompatibleRequestBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiCompatibleRequestError {
    MissingField {
        field: String,
    },
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
    UnsupportedApi {
        api: String,
    },
    EmptyField {
        field: String,
    },
}

impl fmt::Display for OpenAiCompatibleRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => {
                write!(
                    formatter,
                    "OpenAI-compatible request missing field {}",
                    field
                )
            }
            Self::InvalidFieldType { field, expected } => write!(
                formatter,
                "OpenAI-compatible request field {} must be {}",
                field, expected
            ),
            Self::UnsupportedApi { api } => {
                write!(formatter, "unsupported OpenAI-compatible API {}", api)
            }
            Self::EmptyField { field } => {
                write!(
                    formatter,
                    "OpenAI-compatible request field {} must not be empty",
                    field
                )
            }
        }
    }
}

impl Error for OpenAiCompatibleRequestError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatibleResponseKind {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatibleParsedResponse {
    kind: OpenAiCompatibleResponseKind,
    response_id: Option<String>,
    model: Option<String>,
    text: String,
    finish_reason: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
}

impl OpenAiCompatibleParsedResponse {
    pub fn kind(&self) -> OpenAiCompatibleResponseKind {
        self.kind
    }

    pub fn response_id(&self) -> Option<&str> {
        self.response_id.as_deref()
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn finish_reason(&self) -> Option<&str> {
        self.finish_reason.as_deref()
    }

    pub fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    pub fn output_tokens(&self) -> u64 {
        self.output_tokens
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_tokens
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenAiCompatibleResponseParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiCompatibleParseError {
    UnsupportedShape,
    MissingText {
        kind: OpenAiCompatibleResponseKind,
    },
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
}

impl fmt::Display for OpenAiCompatibleParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedShape => write!(
                formatter,
                "unsupported OpenAI-compatible response shape: expected Responses or Chat Completions JSON"
            ),
            Self::MissingText { kind } => write!(
                formatter,
                "OpenAI-compatible {:?} response did not contain text output",
                kind
            ),
            Self::InvalidFieldType { field, expected } => write!(
                formatter,
                "OpenAI-compatible response field {} must be {}",
                field, expected
            ),
        }
    }
}

impl Error for OpenAiCompatibleParseError {}

impl OpenAiCompatibleRequestBuilder {
    pub fn build(
        &self,
        request: &ExecutionRequest,
        instance: &ProviderInstance,
    ) -> Result<OpenAiCompatiblePreparedRequest, OpenAiCompatibleRequestError> {
        let api = request_api(instance.value())?;
        let base_url = required_string_pointer(instance.value(), "/endpoint/base_url")?;
        let model = required_string_pointer(instance.value(), "/endpoint/model")?;
        let url = endpoint_url(&base_url, api);
        let body = match api {
            OpenAiCompatibleRequestApi::Responses => json!({
                "model": model,
                "input": request.goal(),
                "stream": false
            }),
            OpenAiCompatibleRequestApi::ChatCompletions => json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": request.goal()
                    }
                ],
                "stream": false
            }),
        };

        Ok(OpenAiCompatiblePreparedRequest {
            api,
            method: "POST".to_string(),
            url,
            body,
        })
    }
}

impl OpenAiCompatibleResponseParser {
    pub fn parse(
        &self,
        value: &Value,
    ) -> Result<OpenAiCompatibleParsedResponse, OpenAiCompatibleParseError> {
        if value.get("choices").is_some() {
            parse_chat_completions(value)
        } else if value.get("output_text").is_some() || value.get("output").is_some() {
            parse_responses(value)
        } else {
            Err(OpenAiCompatibleParseError::UnsupportedShape)
        }
    }
}

fn request_api(value: &Value) -> Result<OpenAiCompatibleRequestApi, OpenAiCompatibleRequestError> {
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

fn required_string_pointer(
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

fn endpoint_url(base_url: &str, api: OpenAiCompatibleRequestApi) -> String {
    let path = match api {
        OpenAiCompatibleRequestApi::Responses => DEFAULT_RESPONSES_PATH,
        OpenAiCompatibleRequestApi::ChatCompletions => CHAT_COMPLETIONS_PATH,
    };
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

fn parse_responses(
    value: &Value,
) -> Result<OpenAiCompatibleParsedResponse, OpenAiCompatibleParseError> {
    let text = match optional_string(value, "output_text")? {
        Some(output_text) if !output_text.is_empty() => output_text,
        _ => aggregate_responses_output_text(value)?,
    };
    if text.is_empty() {
        return Err(OpenAiCompatibleParseError::MissingText {
            kind: OpenAiCompatibleResponseKind::Responses,
        });
    }
    let usage = value.get("usage").unwrap_or(&Value::Null);
    let input_tokens = optional_u64(usage, "input_tokens")?.unwrap_or(0);
    let output_tokens = optional_u64(usage, "output_tokens")?.unwrap_or(0);
    let total_tokens = optional_u64(usage, "total_tokens")?.unwrap_or(input_tokens + output_tokens);

    Ok(OpenAiCompatibleParsedResponse {
        kind: OpenAiCompatibleResponseKind::Responses,
        response_id: optional_string(value, "id")?,
        model: optional_string(value, "model")?,
        text,
        finish_reason: optional_string(value, "status")?,
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

fn parse_chat_completions(
    value: &Value,
) -> Result<OpenAiCompatibleParsedResponse, OpenAiCompatibleParseError> {
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_type("choices", "array"))?;
    let mut text_parts = Vec::new();
    let mut finish_reason = None;
    for (index, choice) in choices.iter().enumerate() {
        if finish_reason.is_none() {
            finish_reason = optional_string(choice, "finish_reason")?;
        }
        let message = choice
            .get("message")
            .ok_or_else(|| invalid_type(&format!("choices[{}].message", index), "object"))?;
        if let Some(content) = optional_string(message, "content")? {
            if !content.is_empty() {
                text_parts.push(content);
            }
        } else if let Some(refusal) = optional_string(message, "refusal")? {
            if !refusal.is_empty() {
                text_parts.push(refusal);
            }
        }
    }
    let text = text_parts.join("\n");
    if text.is_empty() {
        return Err(OpenAiCompatibleParseError::MissingText {
            kind: OpenAiCompatibleResponseKind::ChatCompletions,
        });
    }

    let usage = value.get("usage").unwrap_or(&Value::Null);
    let input_tokens = optional_u64(usage, "prompt_tokens")?.unwrap_or(0);
    let output_tokens = optional_u64(usage, "completion_tokens")?.unwrap_or(0);
    let total_tokens = optional_u64(usage, "total_tokens")?.unwrap_or(input_tokens + output_tokens);

    Ok(OpenAiCompatibleParsedResponse {
        kind: OpenAiCompatibleResponseKind::ChatCompletions,
        response_id: optional_string(value, "id")?,
        model: optional_string(value, "model")?,
        text,
        finish_reason,
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

fn aggregate_responses_output_text(value: &Value) -> Result<String, OpenAiCompatibleParseError> {
    let Some(output) = value.get("output") else {
        return Ok(String::new());
    };
    let items = output
        .as_array()
        .ok_or_else(|| invalid_type("output", "array"))?;
    let mut text_parts = Vec::new();
    for (item_index, item) in items.iter().enumerate() {
        let Some(content) = item.get("content") else {
            continue;
        };
        let content_items = content
            .as_array()
            .ok_or_else(|| invalid_type(&format!("output[{}].content", item_index), "array"))?;
        for (content_index, content_item) in content_items.iter().enumerate() {
            if content_item.get("type").and_then(Value::as_str) != Some("output_text") {
                continue;
            }
            let text = content_item
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    invalid_type(
                        &format!("output[{}].content[{}].text", item_index, content_index),
                        "string",
                    )
                })?;
            if !text.is_empty() {
                text_parts.push(text.to_string());
            }
        }
    }
    Ok(text_parts.join("\n"))
}

fn optional_string(
    value: &Value,
    field: &str,
) -> Result<Option<String>, OpenAiCompatibleParseError> {
    let Some(item) = value.get(field) else {
        return Ok(None);
    };
    if item.is_null() {
        return Ok(None);
    }
    item.as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| invalid_type(field, "string or null"))
}

fn optional_u64(value: &Value, field: &str) -> Result<Option<u64>, OpenAiCompatibleParseError> {
    let Some(item) = value.get(field) else {
        return Ok(None);
    };
    if item.is_null() {
        return Ok(None);
    }
    item.as_u64()
        .map(Some)
        .ok_or_else(|| invalid_type(field, "unsigned integer or null"))
}

fn invalid_type(field: &str, expected: &'static str) -> OpenAiCompatibleParseError {
    OpenAiCompatibleParseError::InvalidFieldType {
        field: field.to_string(),
        expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_responses_request_without_credentials() {
        let request = request_value("summarize the repository state");
        let instance = provider_instance(json!({
            "id": "openai-default",
            "provider": "provider.openai",
            "enabled": true,
            "credential_ref": "env:OPENAI_API_KEY",
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "endpoint": {
                "base_url": "https://api.openai.com/v1/",
                "model": "gpt-example"
            }
        }));

        let prepared = OpenAiCompatibleRequestBuilder
            .build(&request, &instance)
            .expect("build responses request");

        assert_eq!(prepared.api(), OpenAiCompatibleRequestApi::Responses);
        assert_eq!(prepared.method(), "POST");
        assert_eq!(prepared.url(), "https://api.openai.com/v1/responses");
        assert_eq!(
            prepared.body(),
            &json!({
                "model": "gpt-example",
                "input": "summarize the repository state",
                "stream": false
            })
        );
        let body_text = serde_json::to_string(prepared.body()).expect("serialize body");
        assert!(!body_text.contains("OPENAI_API_KEY"));
        assert!(!body_text.contains("credential_ref"));
    }

    #[test]
    fn builds_chat_completions_request_when_selected() {
        let request = request_value("write a short status");
        let instance = provider_instance(json!({
            "id": "openai-default",
            "provider": "provider.openai",
            "enabled": true,
            "credential_ref": "env:OPENAI_API_KEY",
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "endpoint": {
                "api": "chat_completions",
                "base_url": "https://api.openai.com/v1",
                "model": "gpt-example"
            }
        }));

        let prepared = OpenAiCompatibleRequestBuilder
            .build(&request, &instance)
            .expect("build chat completions request");

        assert_eq!(prepared.api(), OpenAiCompatibleRequestApi::ChatCompletions);
        assert_eq!(prepared.url(), "https://api.openai.com/v1/chat/completions");
        assert_eq!(
            prepared.body(),
            &json!({
                "model": "gpt-example",
                "messages": [
                    {
                        "role": "user",
                        "content": "write a short status"
                    }
                ],
                "stream": false
            })
        );
    }

    #[test]
    fn request_builder_requires_model_and_known_api() {
        let request = request_value("missing model");
        let missing_model = provider_instance(json!({
            "id": "openai-default",
            "provider": "provider.openai",
            "enabled": true,
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "endpoint": {
                "base_url": "https://api.openai.com/v1"
            }
        }));
        let error = OpenAiCompatibleRequestBuilder
            .build(&request, &missing_model)
            .expect_err("model is required");
        assert!(matches!(
            error,
            OpenAiCompatibleRequestError::MissingField { field } if field == "endpoint.model"
        ));

        let unsupported_api = provider_instance(json!({
            "id": "openai-default",
            "provider": "provider.openai",
            "enabled": true,
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "endpoint": {
                "api": "legacy_completions",
                "base_url": "https://api.openai.com/v1",
                "model": "gpt-example"
            }
        }));
        let error = OpenAiCompatibleRequestBuilder
            .build(&request, &unsupported_api)
            .expect_err("unsupported API should fail");
        assert!(matches!(
            error,
            OpenAiCompatibleRequestError::UnsupportedApi { api } if api == "legacy_completions"
        ));
    }

    #[test]
    fn parses_responses_api_output_text_shortcut_and_usage() {
        let parsed = OpenAiCompatibleResponseParser
            .parse(&json!({
                "id": "resp_123",
                "model": "gpt-example",
                "status": "completed",
                "output_text": "shortcut text",
                "output": [
                    {
                        "type": "reasoning",
                        "summary": []
                    },
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "array text"
                            }
                        ]
                    }
                ],
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 7,
                    "total_tokens": 18
                }
            }))
            .expect("parse responses response");

        assert_eq!(parsed.kind(), OpenAiCompatibleResponseKind::Responses);
        assert_eq!(parsed.response_id(), Some("resp_123"));
        assert_eq!(parsed.model(), Some("gpt-example"));
        assert_eq!(parsed.finish_reason(), Some("completed"));
        assert_eq!(parsed.text(), "shortcut text");
        assert_eq!(parsed.input_tokens(), 11);
        assert_eq!(parsed.output_tokens(), 7);
        assert_eq!(parsed.total_tokens(), 18);
    }

    #[test]
    fn parses_responses_api_output_array_without_assuming_first_item() {
        let parsed = OpenAiCompatibleResponseParser
            .parse(&json!({
                "id": "resp_456",
                "output": [
                    {
                        "type": "function_call",
                        "name": "ignored_tool"
                    },
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "first text"
                            },
                            {
                                "type": "refusal",
                                "refusal": "ignored refusal"
                            }
                        ]
                    },
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "second text"
                            }
                        ]
                    }
                ],
                "usage": {
                    "input_tokens": 3,
                    "output_tokens": 5
                }
            }))
            .expect("parse responses output array");

        assert_eq!(parsed.text(), "first text\nsecond text");
        assert_eq!(parsed.input_tokens(), 3);
        assert_eq!(parsed.output_tokens(), 5);
        assert_eq!(parsed.total_tokens(), 8);
    }

    #[test]
    fn parses_chat_completion_message_content_and_usage() {
        let parsed = OpenAiCompatibleResponseParser
            .parse(&json!({
                "id": "chatcmpl_123",
                "model": "gpt-example",
                "choices": [
                    {
                        "index": 0,
                        "finish_reason": "stop",
                        "message": {
                            "role": "assistant",
                            "content": "chat text"
                        }
                    }
                ],
                "usage": {
                    "prompt_tokens": 13,
                    "completion_tokens": 17,
                    "total_tokens": 30
                }
            }))
            .expect("parse chat completion");

        assert_eq!(parsed.kind(), OpenAiCompatibleResponseKind::ChatCompletions);
        assert_eq!(parsed.response_id(), Some("chatcmpl_123"));
        assert_eq!(parsed.text(), "chat text");
        assert_eq!(parsed.finish_reason(), Some("stop"));
        assert_eq!(parsed.input_tokens(), 13);
        assert_eq!(parsed.output_tokens(), 17);
        assert_eq!(parsed.total_tokens(), 30);
    }

    #[test]
    fn rejects_supported_shapes_without_text() {
        let error = OpenAiCompatibleResponseParser
            .parse(&json!({
                "id": "resp_no_text",
                "output": [
                    {
                        "type": "function_call",
                        "name": "only_tool"
                    }
                ]
            }))
            .expect_err("missing text should fail");

        assert!(matches!(
            error,
            OpenAiCompatibleParseError::MissingText {
                kind: OpenAiCompatibleResponseKind::Responses
            }
        ));
    }

    fn request_value(goal: &str) -> ExecutionRequest {
        ExecutionRequest::from_value(
            json!({
                "schema_version": "1.0.0",
                "request_id": "request-0001",
                "job_id": "J-0001",
                "stage": "implement",
                "provider_instance_id": "openai-default",
                "attempt_id": "attempt-0001",
                "workspec_path": "workspecs/implement.json",
                "created_at": "2026-06-28T00:00:00Z",
                "goal": goal,
                "allowed_scope": ["src/**", "tests/**"],
                "forbidden_actions": ["dependency_install", "file_delete"],
                "required_outputs": ["provider-output/openai-default/response.json"],
                "validation_requirements": ["policy:p0"],
                "context_pack": { "files": [] }
            }),
            "request.json",
            schema_root(),
        )
        .expect("execution request")
    }

    fn provider_instance(value: Value) -> ProviderInstance {
        ProviderInstance {
            id: "openai-default".to_string(),
            provider_id: "provider.openai".to_string(),
            enabled: true,
            routing_tags: vec!["cloud".to_string(), "api".to_string()],
            path: "openai-default.json".into(),
            value,
        }
    }

    fn schema_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .join("specs")
            .join("schemas")
    }
}
