use super::ProviderRegistryError;
use std::error::Error;

impl Error for ProviderRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}
