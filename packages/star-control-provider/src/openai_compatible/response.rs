mod chat;
mod fields;
mod responses;

use serde_json::Value;
use std::error::Error;
use std::fmt;

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

impl OpenAiCompatibleResponseParser {
    pub fn parse(
        &self,
        value: &Value,
    ) -> Result<OpenAiCompatibleParsedResponse, OpenAiCompatibleParseError> {
        if value.get("choices").is_some() {
            chat::parse_chat_completions(value)
        } else if value.get("output_text").is_some() || value.get("output").is_some() {
            responses::parse_responses(value)
        } else {
            Err(OpenAiCompatibleParseError::UnsupportedShape)
        }
    }
}
