use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum SchemaLoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    RootNotObject {
        path: Option<PathBuf>,
        actual: String,
    },
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read schema {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse schema JSON {}: {}",
                    path.display(),
                    source
                )
            }
            Self::RootNotObject { path, actual } => {
                if let Some(path) = path {
                    write!(
                        formatter,
                        "schema root must be object in {}, got {}",
                        path.display(),
                        actual
                    )
                } else {
                    write!(formatter, "schema root must be object, got {}", actual)
                }
            }
        }
    }
}

impl Error for SchemaLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::RootNotObject { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum DocumentLoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for DocumentLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read document {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse document JSON {}: {}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for DocumentLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum FileValidationError {
    Schema(SchemaLoadError),
    Document(DocumentLoadError),
}

impl fmt::Display for FileValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(source) => write!(formatter, "schema load failed: {}", source),
            Self::Document(source) => write!(formatter, "document load failed: {}", source),
        }
    }
}

impl Error for FileValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Schema(source) => Some(source),
            Self::Document(source) => Some(source),
        }
    }
}

impl From<SchemaLoadError> for FileValidationError {
    fn from(source: SchemaLoadError) -> Self {
        Self::Schema(source)
    }
}

impl From<DocumentLoadError> for FileValidationError {
    fn from(source: DocumentLoadError) -> Self {
        Self::Document(source)
    }
}
