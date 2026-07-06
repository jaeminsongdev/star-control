pub(crate) const LOCAL_PROCESS_KIND: &str = "local_process_model";
pub(crate) const PROCESS_TRANSPORT: &str = "process";
pub(crate) const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
pub(crate) const MAX_TIMEOUT_SECONDS: u64 = 600;
pub(crate) const STDOUT_FILE: &str = "stdout.txt";
pub(crate) const STDERR_FILE: &str = "stderr.txt";
pub(crate) const FORBIDDEN_ACTION_EVIDENCE_PREFIX: &str = "STAR_CONTROL_FORBIDDEN_ACTION_EVIDENCE:";
pub(crate) const LOCAL_PROCESS_FORBIDDEN_ACTIONS: &[&str] = &[
    "dependency_install",
    "dependency_change",
    "workflow_change",
    "release_publish",
    "deploy",
    "credential_change",
    "external_account_change",
    "file_delete",
    "bulk_move",
    "test_delete",
    "test_skip_only_ignore",
    "assertion_weakening",
    "validator_self_bypass",
    "sensitive_data_output",
];
