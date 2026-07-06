mod catalog;

use super::super::ChangeType;
use catalog::keyword_rules;

pub(super) fn matched_change_types(haystack: &str) -> (Vec<ChangeType>, Vec<String>) {
    let mut change_types = Vec::new();
    let mut reasons = Vec::new();

    for rule in keyword_rules() {
        if contains_any(haystack, rule.needles) {
            change_types.push(rule.change_type);
            reasons.push(rule.reason.to_string());
        }
    }

    (change_types, reasons)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}
