pub(in crate::evaluator) fn is_plaintext_secret_candidate(content: &str) -> bool {
    let trimmed = content.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("-----begin ") && lower.contains(" private key") {
        return true;
    }
    if contains_token_with_min_suffix(trimmed, "sk-", 12) {
        return true;
    }

    let key_names = [
        "api_key",
        "apikey",
        "secret",
        "token",
        "password",
        "private_key",
        "client_secret",
        "access_key",
    ];
    key_names.iter().any(|name| lower.contains(name))
        && (lower.contains('=') || lower.contains(':'))
        && !is_placeholder_secret(&lower)
}

fn contains_token_with_min_suffix(content: &str, marker: &str, min_suffix: usize) -> bool {
    let Some(index) = content.find(marker) else {
        return false;
    };
    content[index + marker.len()..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .count()
        >= min_suffix
}

fn is_placeholder_secret(lower: &str) -> bool {
    [
        "example",
        "placeholder",
        "redacted",
        "changeme",
        "todo",
        "****",
        "<secret>",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}
