use super::super::{
    OpenAiCompatibleRequestApi, OpenAiCompatibleRequestBuilder, OpenAiCompatibleRequestError,
};
use super::{provider_instance, request_value};
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
