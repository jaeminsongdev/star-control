pub(crate) const SCHEMA_VERSION: &str = "1.0.0";
pub(crate) const RELEASE_READINESS_SCHEMA: &str = "release-readiness.schema.json";

pub const RELEASE_READINESS_PATH: &str = "release/release-readiness.json";
pub const RELEASE_REVIEW_PACK_MARKDOWN_FILE: &str = "release-review-pack.md";
pub const RELEASE_REVIEW_PACK_PATH: &str = "review-packs/release-review-pack.md";
pub const M9_REQUIRED_READINESS_CHECKS: &[&str] = &[
    "security-redaction",
    "audit-event-writer",
    "cost-budget-guard",
    "provider-conformance-hardening",
    "state-recovery-inspection",
    "release-readiness-writer",
    "release-readiness-api-read",
    "release-version-consistency",
    "release-evidence-file-checker",
    "release-profile-readiness",
    "release-readiness-ui-read",
    "release-readiness-cli-read",
    "release-review-pack",
    "recovery-command-surface",
    "destructive-actions-reserved",
    "release-automation-reserved",
];
pub const COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS: &[&str] = &[
    "m0-docs-decisions",
    "m1-runtime-foundation",
    "m2-provider-neutral-execution",
    "m3-validation-gate",
    "m4-v0-fake-e2e",
    "m5-local-provider",
    "m6-cloud-provider",
    "m7-daemon-api-control-plane",
    "m8-ui-shell",
    "m9-hardening-release-readiness",
    "full-local-validation",
    "remote-ci-evidence",
    "stacked-prs-clean",
    "reserved-actions-confirmed",
];
