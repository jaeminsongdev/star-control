use super::fields::{invalid_type, optional_string, optional_u64};
use super::{
    OpenAiCompatibleParseError, OpenAiCompatibleParsedResponse, OpenAiCompatibleResponseKind,
};
use serde_json::Value;

pub(super) fn parse_chat_completions(
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
