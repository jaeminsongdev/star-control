use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedArgs {
    pub(crate) command: String,
    pub(crate) subcommand: Option<String>,
    pub(crate) subject: Option<String>,
    pub(crate) project: Option<PathBuf>,
    pub(crate) job_id: Option<String>,
    pub(crate) request: Option<String>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) provider_instances: Vec<PathBuf>,
    pub(crate) stage: Option<String>,
    pub(crate) response: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) constraints: Vec<String>,
    pub(crate) release_readiness: bool,
    pub(crate) recovery_list: bool,
    pub(crate) action: Option<String>,
    pub(crate) recovery_approval: Option<String>,
    pub(crate) recovery_artifact: Option<String>,
    pub(crate) recovery_source: Option<String>,
    pub(crate) release_approval: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) json: bool,
    pub(crate) markdown: bool,
}

impl ParsedArgs {
    pub(super) fn new(command: String) -> Self {
        Self {
            command,
            subcommand: None,
            subject: None,
            project: None,
            job_id: None,
            request: None,
            entrypoint: None,
            provider: None,
            provider_instances: Vec::new(),
            stage: None,
            response: None,
            reason: None,
            constraints: Vec::new(),
            release_readiness: false,
            recovery_list: false,
            action: None,
            recovery_approval: None,
            recovery_artifact: None,
            recovery_source: None,
            release_approval: None,
            dry_run: false,
            json: false,
            markdown: false,
        }
    }

    pub(crate) fn has_recovery_source_selection(&self) -> bool {
        self.recovery_artifact.is_some() || self.recovery_source.is_some()
    }
}
