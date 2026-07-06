mod error;
mod loader;
mod types;
mod validator;

pub use error::{DocumentLoadError, FileValidationError, SchemaLoadError};
pub use loader::{load_document, load_schema, validate_file};
pub use types::{Schema, ValidationError, ValidationResult};
pub use validator::{assert_valid, validate_json};

#[cfg(test)]
mod tests;
