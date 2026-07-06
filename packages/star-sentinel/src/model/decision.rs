use crate::json_fields::invalid_field;
use crate::SentinelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Block,
}

impl Severity {
    pub(super) fn parse(value: &str, artifact: &str, field: &str) -> Result<Self, SentinelError> {
        match value {
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "block" => Ok(Self::Block),
            _ => Err(invalid_field(
                artifact,
                field,
                "expected one of info, warn, block",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Decision {
    AutoPass,
    HumanReview,
    Block,
}

impl Decision {
    pub(crate) fn parse(value: &str, artifact: &str, field: &str) -> Result<Self, SentinelError> {
        match value {
            "AUTO_PASS" => Ok(Self::AutoPass),
            "HUMAN_REVIEW" => Ok(Self::HumanReview),
            "BLOCK" => Ok(Self::Block),
            _ => Err(invalid_field(
                artifact,
                field,
                "expected one of AUTO_PASS, HUMAN_REVIEW, BLOCK",
            )),
        }
    }

    pub(crate) fn default_for_severity(severity: Severity) -> Self {
        match severity {
            Severity::Block => Self::Block,
            Severity::Warn => Self::HumanReview,
            Severity::Info => Self::AutoPass,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "AUTO_PASS",
            Self::HumanReview => "HUMAN_REVIEW",
            Self::Block => "BLOCK",
        }
    }
}
