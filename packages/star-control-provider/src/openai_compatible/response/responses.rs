use super::fields::{invalid_type, optional_string, optional_u64};
use super::{
    OpenAiCompatibleParseError, OpenAiCompatibleParsedResponse, OpenAiCompatibleResponseKind,
};
use serde_json::Value;

pub(super) fn parse_responses(
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
