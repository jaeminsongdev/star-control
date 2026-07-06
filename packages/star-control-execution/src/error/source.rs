use super::ExecutionError;
use std::error::Error;

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProviderRegistry(source) => Some(source),
            Self::ProviderAdapter(source) => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}
