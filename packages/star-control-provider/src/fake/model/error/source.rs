use super::ProviderAdapterError;
use std::error::Error;

impl Error for ProviderAdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::Registry(source) => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}
