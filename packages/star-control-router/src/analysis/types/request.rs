use super::{stages, ChangeType, PolicyProfile, Risk, RouteDecision, Size};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestAnalysis {
    pub(crate) change_types: Vec<ChangeType>,
    pub(crate) routing_reasons: Vec<String>,
    pub(crate) approval_reasons: Vec<String>,
    pub(crate) size: Size,
    pub(crate) risk: Risk,
    pub(crate) profile: PolicyProfile,
    pub(crate) decision: RouteDecision,
    pub(crate) requires_user_approval: bool,
}

impl RequestAnalysis {
    pub(crate) fn change_type_strings(&self) -> Vec<&'static str> {
        self.change_types
            .iter()
            .map(|change_type| change_type.as_str())
            .collect()
    }

    pub(crate) fn stages(&self) -> Vec<&'static str> {
        stages::stages_for(self.decision, &self.change_types)
    }
}
