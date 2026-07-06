mod routine;
mod safety;

use super::super::super::ChangeType;

pub(super) struct KeywordRule {
    pub(super) change_type: ChangeType,
    pub(super) needles: &'static [&'static str],
    pub(super) reason: &'static str,
}

impl KeywordRule {
    const fn new(
        change_type: ChangeType,
        needles: &'static [&'static str],
        reason: &'static str,
    ) -> Self {
        Self {
            change_type,
            needles,
            reason,
        }
    }
}

pub(super) fn keyword_rules() -> impl Iterator<Item = &'static KeywordRule> {
    routine::ROUTINE_KEYWORD_RULES
        .iter()
        .chain(safety::SAFETY_KEYWORD_RULES.iter())
}
