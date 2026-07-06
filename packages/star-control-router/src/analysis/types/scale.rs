#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Size {
    Small = 1,
    Medium = 2,
    Large = 3,
    Critical = 4,
}

impl Size {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Small => "SMALL",
            Self::Medium => "MEDIUM",
            Self::Large => "LARGE",
            Self::Critical => "CRITICAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Risk {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl Risk {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}
