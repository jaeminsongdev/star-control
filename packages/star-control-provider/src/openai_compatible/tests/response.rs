use super::super::{
    OpenAiCompatibleParseError, OpenAiCompatibleResponseKind, OpenAiCompatibleResponseParser,
};
use serde_json::json;

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
