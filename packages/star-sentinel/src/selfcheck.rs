mod aliases;
mod contracts;
mod manifest;
mod yaml;

use crate::model::SelfcheckReport;
use aliases::check_legacy_alias_locations;
use contracts::{check_fixture_files, check_p0_registry, check_schema_files};
use manifest::check_manifest_outputs;
use std::path::Path;

pub fn run_selfcheck(repo_root: impl AsRef<Path>) -> SelfcheckReport {
    let repo_root = repo_root.as_ref();
    let schema_root = repo_root.join("builtin-tools/star-sentinel/schemas");
    let manifest_path = repo_root.join("builtin-tools/star-sentinel/tool.yaml");
    let registry_path =
        repo_root.join("builtin-tools/star-sentinel/policies/p0-rule-registry.json");
    let fixtures_root = repo_root.join("builtin-tools/star-sentinel/fixtures/p0");
    let mut diagnostics = Vec::new();

    check_manifest_outputs(&manifest_path, &mut diagnostics);
    check_p0_registry(&registry_path, &schema_root, &mut diagnostics);
    check_schema_files(&schema_root, &mut diagnostics);
    check_fixture_files(&fixtures_root, &mut diagnostics);
    check_legacy_alias_locations(repo_root, &manifest_path, &mut diagnostics);

    SelfcheckReport {
        ok: diagnostics.is_empty(),
        diagnostics,
    }
}
