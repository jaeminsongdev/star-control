use star_control_execution::ExecutionError;
use star_control_provider::ProviderRegistryError;
use star_control_release::ReleaseReadinessError;
use star_control_router::RouterError;
use star_control_state::StateStoreError;
use star_sentinel::SentinelError;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum CliError {
    InvalidInput {
        command: String,
        message: String,
    },
    MissingArtifact {
        command: String,
        message: String,
        artifact_paths: Vec<String>,
    },
    ProviderExecution {
        command: String,
        message: String,
    },
    State {
        command: String,
        source: StateStoreError,
    },
    Router {
        command: String,
        source: RouterError,
    },
    ProviderRegistry {
        command: String,
        source: ProviderRegistryError,
    },
    Sentinel {
        command: String,
        source: SentinelError,
    },
    ReleaseReadiness {
        command: String,
        source: ReleaseReadinessError,
    },
    Execution {
        command: String,
        source: ExecutionError,
    },
    Internal {
        command: String,
        message: String,
    },
}

impl CliError {
    pub(crate) fn command(&self) -> &str {
        match self {
            Self::InvalidInput { command, .. }
            | Self::MissingArtifact { command, .. }
            | Self::ProviderExecution { command, .. }
            | Self::State { command, .. }
            | Self::Router { command, .. }
            | Self::ProviderRegistry { command, .. }
            | Self::Sentinel { command, .. }
            | Self::ReleaseReadiness { command, .. }
            | Self::Execution { command, .. }
            | Self::Internal { command, .. } => command,
        }
    }

    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput { .. } => 2,
            Self::MissingArtifact { .. } | Self::State { .. } => 3,
            Self::ProviderExecution { .. } | Self::Execution { .. } => 4,
            Self::Router { .. }
            | Self::ProviderRegistry { .. }
            | Self::Sentinel { .. }
            | Self::Internal { .. } => 5,
            Self::ReleaseReadiness { .. } => 5,
        }
    }

    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "InvalidInput",
            Self::MissingArtifact { .. } => "MissingArtifact",
            Self::ProviderExecution { .. } => "ProviderExecutionFailed",
            Self::State { .. } => "StateReadFailed",
            Self::Router { .. } => "RouteFailed",
            Self::ProviderRegistry { .. } => "ProviderRegistryFailed",
            Self::Sentinel { .. } => "StarSentinelFailed",
            Self::ReleaseReadiness { .. } => "ReleaseReadinessReadFailed",
            Self::Execution { .. } => "ExecutionFailed",
            Self::Internal { .. } => "InternalError",
        }
    }

    pub(crate) fn category(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "input",
            Self::MissingArtifact { .. } | Self::State { .. } => "state-store",
            Self::ProviderExecution { .. } | Self::Execution { .. } => "provider-execution",
            Self::Router { .. } => "router",
            Self::ProviderRegistry { .. } => "provider-registry",
            Self::Sentinel { .. } => "star-sentinel",
            Self::ReleaseReadiness { .. } => "release-readiness",
            Self::Internal { .. } => "internal",
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::InvalidInput { message, .. }
            | Self::MissingArtifact { message, .. }
            | Self::ProviderExecution { message, .. }
            | Self::Internal { message, .. } => message.clone(),
            Self::State { source, .. } => source.to_string(),
            Self::Router { source, .. } => source.to_string(),
            Self::ProviderRegistry { source, .. } => source.to_string(),
            Self::Sentinel { source, .. } => source.to_string(),
            Self::ReleaseReadiness { source, .. } => source.to_string(),
            Self::Execution { source, .. } => source.to_string(),
        }
    }

    pub(crate) fn artifact_paths(&self) -> Vec<String> {
        match self {
            Self::MissingArtifact { artifact_paths, .. } => artifact_paths.clone(),
            _ => Vec::new(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message())
    }
}

impl Error for CliError {}
