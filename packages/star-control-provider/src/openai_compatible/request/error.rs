use std::error::Error;
use std::fmt;

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
