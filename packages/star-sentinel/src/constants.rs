pub const SENTINEL_TASK_SCHEMA: &str = "sentinel-task.schema.json";
pub const CHANGED_LINES_SCHEMA: &str = "changed-lines.schema.json";
pub const P0_RULE_REGISTRY_SCHEMA: &str = "p0-rule-registry.schema.json";
pub const FIXTURE_OUTCOME_SCHEMA: &str = "fixture-outcome.schema.json";
pub const DIAGNOSTIC_SCHEMA: &str = "diagnostic.schema.json";
pub const APPROVAL_SCHEMA: &str = "approval.schema.json";
pub const REVIEW_PACK_SCHEMA: &str = "review-pack.schema.json";
pub const LEDGER_EVENT_SCHEMA: &str = "ledger-event.schema.json";
pub const STAR_SENTINEL_TOOL_OUTPUT_DIR: &str = "star-sentinel";
pub const DIAGNOSTICS_FILE: &str = "diagnostics.json";
pub const APPROVAL_FILE: &str = "approval.json";
pub const REVIEW_PACK_JSON_FILE: &str = "review_pack.json";
pub const REVIEW_PACK_MARKDOWN_FILE: &str = "review_pack.md";
pub const LEDGER_FILE: &str = "ledger.jsonl";

pub const REQUIRED_MANIFEST_OUTPUTS: [&str; 8] = [
    "repo_map.json",
    "changed_lines.json",
    DIAGNOSTICS_FILE,
    "validation_runs.json",
    REVIEW_PACK_JSON_FILE,
    REVIEW_PACK_MARKDOWN_FILE,
    APPROVAL_FILE,
    LEDGER_FILE,
];

pub const RULE_SCOPE_ALLOWED_PATHS: &str = "task.scope.allowed_paths";
pub const RULE_TEST_NO_DELETION: &str = "test.no_deletion";
pub const RULE_DEPENDENCY_REQUIRES_APPROVAL: &str = "dependency.requires_approval";
pub const RULE_SECRET_NO_PLAINTEXT_SECRET: &str = "secret.no_plaintext_secret";
pub const RULE_VALIDATOR_NO_SELF_BYPASS: &str = "validator.no_self_bypass";

pub const P0_RULE_IDS: [&str; 5] = [
    RULE_SCOPE_ALLOWED_PATHS,
    RULE_TEST_NO_DELETION,
    RULE_DEPENDENCY_REQUIRES_APPROVAL,
    RULE_SECRET_NO_PLAINTEXT_SECRET,
    RULE_VALIDATOR_NO_SELF_BYPASS,
];
