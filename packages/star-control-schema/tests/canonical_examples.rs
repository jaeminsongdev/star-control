#[path = "canonical_examples/cases.rs"]
mod cases;

use cases::validation_cases;
use star_control_schema::validate_file;
use std::path::{Path, PathBuf};

#[test]
fn canonical_examples_validate_with_runtime_validator() {
    let root = repo_root();
    let mut errors = Vec::new();

    for case in validation_cases() {
        match validate_file(root.join(case.document_path), root.join(case.schema_path)) {
            Ok(result) if result.is_ok() => {}
            Ok(result) => {
                for error in result.errors {
                    errors.push(format!(
                        "{} against {}: {} {}",
                        case.document_path, case.schema_path, error.location, error.message
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "{} against {}: {}",
                case.document_path, case.schema_path, error
            )),
        }
    }

    assert!(
        errors.is_empty(),
        "runtime validator failed canonical examples:\n{}",
        errors.join("\n")
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
