use super::StateStoreError;
use std::error::Error;

impl Error for StateStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AiRunsNotWritable { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::AtomicWriteFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}
