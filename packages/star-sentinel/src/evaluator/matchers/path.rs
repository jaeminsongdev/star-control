pub(in crate::evaluator) fn path_is_allowed(path: &str, allowed_paths: &[String]) -> bool {
    if allowed_paths.is_empty() {
        return false;
    }
    allowed_paths
        .iter()
        .any(|pattern| path_matches_pattern(pattern, path))
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern = normalize_path(pattern);
    let path = normalize_path(path);
    if pattern == "**" || pattern == "**/*" {
        return true;
    }
    wildcard_match(&pattern, &path)
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let mut table = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    table[0][0] = true;

    for pattern_index in 1..=pattern.len() {
        if pattern[pattern_index - 1] == '*' {
            table[pattern_index][0] = table[pattern_index - 1][0];
        }
    }

    for pattern_index in 1..=pattern.len() {
        for text_index in 1..=text.len() {
            table[pattern_index][text_index] = if pattern[pattern_index - 1] == '*' {
                table[pattern_index - 1][text_index] || table[pattern_index][text_index - 1]
            } else {
                table[pattern_index - 1][text_index - 1]
                    && pattern[pattern_index - 1] == text[text_index - 1]
            };
        }
    }

    table[pattern.len()][text.len()]
}

pub(crate) fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .trim_start_matches('/')
        .to_string()
}

pub(in crate::evaluator) fn is_test_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    let name = path.rsplit('/').next().unwrap_or(&path);
    path == "tests"
        || path.starts_with("tests/")
        || path.contains("/tests/")
        || path.contains("/__tests__/")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with("_test.rs")
        || name.ends_with("_test.go")
}

pub(in crate::evaluator) fn is_dependency_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    let name = path.rsplit('/').next().unwrap_or(&path);
    matches!(
        name,
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "pipfile"
            | "pipfile.lock"
            | "go.mod"
            | "go.sum"
            | "gemfile"
            | "gemfile.lock"
            | "composer.json"
            | "composer.lock"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.lockfile"
            | "packages.lock.json"
    ) || name.ends_with(".csproj")
}

pub(in crate::evaluator) fn is_validator_path(path: &str) -> bool {
    let path = normalize_path(path).to_ascii_lowercase();
    path.starts_with(".github/workflows/")
        || path.starts_with("scripts/ci/")
        || path == "scripts/test.ps1"
        || path.starts_with("builtin-tools/star-sentinel/policies/")
        || path.starts_with("builtin-tools/star-sentinel/schemas/")
        || path.starts_with("builtin-tools/star-sentinel/fixtures/")
        || path.starts_with("packages/star-sentinel/")
}
