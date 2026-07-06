#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyProfile {
    Quick,
    Near,
    Full,
    Security,
    Release,
    Validator,
}

impl PolicyProfile {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Near => "near",
            Self::Full => "full",
            Self::Security => "security",
            Self::Release => "release",
            Self::Validator => "validator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteDecision {
    AutoPass,
    HumanReview,
    Block,
}

impl RouteDecision {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "AUTO_PASS",
            Self::HumanReview => "HUMAN_REVIEW",
            Self::Block => "BLOCK",
        }
    }
}
