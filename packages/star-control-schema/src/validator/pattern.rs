pub(super) fn known_pattern_matches(text: &str, pattern: &str) -> Option<bool> {
    match pattern {
        "^J-[0-9]{4,}$" => Some(matches_job_id(text)),
        "^[a-z0-9][a-z0-9.-]*$" => Some(matches_slug(text, ".-")),
        "^[a-z0-9][a-z0-9_.-]*$" => Some(matches_slug(text, "_.-")),
        "^provider\\.[a-z0-9][a-z0-9.-]*$" => Some(
            text.strip_prefix("provider.")
                .is_some_and(|provider_id| matches_slug(provider_id, ".-")),
        ),
        _ => None,
    }
}

fn matches_job_id(text: &str) -> bool {
    let Some(suffix) = text.strip_prefix("J-") else {
        return false;
    };
    suffix.len() >= 4 && suffix.chars().all(|character| character.is_ascii_digit())
}

fn matches_slug(text: &str, extra_allowed: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_lower_ascii_alnum(first)
        && chars
            .all(|character| is_lower_ascii_alnum(character) || extra_allowed.contains(character))
}

fn is_lower_ascii_alnum(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}
