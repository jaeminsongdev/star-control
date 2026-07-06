use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Job,
    State,
    EventLog,
    Route,
    WorkSpec,
    Report,
    ProviderOutput,
    ToolOutput,
    Approval,
    ReviewPack,
    Log,
    Other,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Job => "job",
            Self::State => "state",
            Self::EventLog => "event_log",
            Self::Route => "route",
            Self::WorkSpec => "workspec",
            Self::Report => "report",
            Self::ProviderOutput => "provider_output",
            Self::ToolOutput => "tool_output",
            Self::Approval => "approval",
            Self::ReviewPack => "review_pack",
            Self::Log => "log",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateStore {
    pub(crate) project_root: PathBuf,
    pub(crate) ai_runs_dir: PathBuf,
    pub(crate) schema_root: PathBuf,
}
