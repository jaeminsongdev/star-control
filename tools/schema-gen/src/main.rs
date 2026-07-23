use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use star_contracts::{
    canonical::{Sha256Hash, canonical_sha256},
    ids::{CanonicalSourceId, FindingId, OccurrenceId, SymbolId},
    schema::generated_documents,
};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;
type GeneratedFile = (PathBuf, Vec<u8>);

const MANAGEMENT_SCHEMA_FILES: &[&str] = &[
    "task-invocation-v2.schema.json",
    "validation-run-v2.schema.json",
    "validation-result-v2.schema.json",
    "gate-decision-v2.schema.json",
    "evidence-bundle-v2.schema.json",
    "diagnostic-v2.schema.json",
    "baseline-v2.schema.json",
    "suppression-v2.schema.json",
    "disposition-v2.schema.json",
    "review-pack.schema.json",
    "rework-directive.schema.json",
    "validation-plan-v2.schema.json",
    "task-spec.schema.json",
    "scope-revision.schema.json",
    "change-set.schema.json",
    "impact-analysis.schema.json",
    "risk-path-descriptor.schema.json",
    "planning-bundle.schema.json",
    "goal-record.schema.json",
    "managed-registry-snapshot.schema.json",
    "managed-registry-manifest.schema.json",
    "managed-registry-fragment.schema.json",
    "managed-registry-snapshot-v2.schema.json",
    "registry-consistency-record.schema.json",
    "managed-declaration-change-intent.schema.json",
    "compatibility-report.schema.json",
    "project-contract-manifest.schema.json",
    "contract-surface-snapshot.schema.json",
    "compatibility-report-v2.schema.json",
    "documentation-snapshot.schema.json",
    "config-key-trace.schema.json",
    "environment-snapshot.schema.json",
    "project-doctor-report.schema.json",
    "clean-room-specification.schema.json",
    "dependency-security-input-manifest.schema.json",
    "failure-record.schema.json",
    "reproduction-pack-v2.schema.json",
    "regression-record.schema.json",
    "recovery-plan-v2.schema.json",
    "dependency-snapshot.schema.json",
    "supply-chain-snapshot.schema.json",
    "external-data-snapshot.schema.json",
    "dependency-update-plan.schema.json",
    "maintenance-radar-snapshot.schema.json",
    "project-migration-manifest.schema.json",
    "migration-plan-v2.schema.json",
    "migration-checkpoint-v2.schema.json",
    "migration-attempt.schema.json",
    "migration-validation-report.schema.json",
    "restore-verification-record.schema.json",
    "performance-workload-spec.schema.json",
    "performance-run.schema.json",
    "performance-comparison-v2.schema.json",
    "language-migration-plan.schema.json",
    "equivalence-report.schema.json",
    "cross-project-migration-handoff.schema.json",
    "multi-project-goal.schema.json",
    "cross-repo-change-bundle.schema.json",
    "change-bundle-participant-v2.schema.json",
    "worktree-record.schema.json",
    "overlap-analysis.schema.json",
    "merge-plan-v2.schema.json",
    "merge-queue-record.schema.json",
    "merge-conflict-record.schema.json",
    "project-merge-result.schema.json",
    "remote-state-snapshot-v2.schema.json",
    "remote-operation-record.schema.json",
    "change-bundle-release-handoff.schema.json",
    "clean-room-doctor-report.schema.json",
    "reproduction-pack.schema.json",
    "maintenance-radar.schema.json",
    "migration-run.schema.json",
    "performance-comparison.schema.json",
    "change-bundle.schema.json",
    "change-bundle-handoff.schema.json",
    "release-manifest-v2.schema.json",
    "release-asset-binding-v1.schema.json",
    "development-effect-receipt-v1.schema.json",
    "evaluation-run-v2.schema.json",
    "evaluation-catalog-item.schema.json",
    "rust-toolchain-binding.schema.json",
    "rust-style-policy-snapshot.schema.json",
    "rust-style-coverage-matrix.schema.json",
    "rust-style-step-execution.schema.json",
    "rust-style-policy-approval-request.schema.json",
    "rust-style-policy-approval-decision.schema.json",
    "development-profile-descriptor.schema.json",
    "development-profile-catalog-snapshot.schema.json",
    "development-profile-resolution.schema.json",
    "project.schema.json",
    "project-v1.schema.json",
    "project-checkout.schema.json",
    "project-catalog-snapshot.schema.json",
    "code-index-snapshot.schema.json",
    "project-v1-to-v2-migration-plan.schema.json",
    "project-v1-to-v2-migration-result.schema.json",
    "project-revision.schema.json",
    "workspace-snapshot.schema.json",
    "scan-run.schema.json",
    "rule.schema.json",
    "finding.schema.json",
    "occurrence.schema.json",
    "symbol.schema.json",
    "symbol-reference.schema.json",
    "canonical-source.schema.json",
    "suppression.schema.json",
    "baseline.schema.json",
    "disposition.schema.json",
    "change-plan.schema.json",
    "patch-set.schema.json",
    "change-recipe.schema.json",
    "validation-result.schema.json",
    "gate-decision.schema.json",
    "artifact-ref.schema.json",
    "management-store-status.schema.json",
    "management-active-set.schema.json",
    "management-backup-plan.schema.json",
    "management-backup-set-manifest.schema.json",
    "management-backup-apply-result.schema.json",
    "management-recovery-status.schema.json",
    "management-restore-plan.schema.json",
    "management-restore-apply-result.schema.json",
    "management-rebuild-plan.schema.json",
    "management-rebuild-apply-result.schema.json",
    "management-local-state-bundle.schema.json",
    "management-local-state-export-plan.schema.json",
    "management-local-state-export-result.schema.json",
    "management-local-state-import-plan.schema.json",
    "management-local-state-import-result.schema.json",
    "coordinated-operation.schema.json",
];

fn generated_files(root: &Path) -> DynResult<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    let mut manifest = Vec::new();
    let documents = generated_documents();
    for (name, document) in &documents {
        let bytes = serde_json::to_vec_pretty(&document)?;
        manifest
            .push(serde_json::json!({"file": name, "hash": Sha256Hash::digest(&bytes).as_str()}));
        files.push((root.join(name), bytes));
    }
    files.push((
        root.parent()
            .ok_or("schema output has no parent")?
            .join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"schema_version": 1, "files": manifest}))?,
    ));
    let fixture_root = if root.file_name().and_then(|value| value.to_str()) == Some("v1")
        && root
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("schemas")
    {
        root.parent()
            .and_then(Path::parent)
            .ok_or("schema output has no specs ancestor")?
            .join("fixtures/management/v1")
    } else {
        root.join("_fixtures/management/v1")
    };
    for (name, schema) in documents
        .iter()
        .filter(|(name, _)| MANAGEMENT_SCHEMA_FILES.contains(name))
    {
        let stem = name.trim_end_matches(".schema.json");
        let minimal = sample_from_schema(schema, schema, true, None)?;
        let full = sample_from_schema(schema, schema, false, None)?;
        let mut invalid = minimal.clone();
        invalid
            .as_object_mut()
            .ok_or("management fixture root is not an object")?
            .insert("unexpected".to_owned(), Value::Bool(true));
        let mut future = minimal.clone();
        let current_version = minimal
            .get("schema_version")
            .or_else(|| full.get("schema_version"))
            .and_then(Value::as_u64)
            .unwrap_or(1);
        future
            .as_object_mut()
            .ok_or("management fixture root is not an object")?
            .insert(
                "schema_version".to_owned(),
                Value::from(current_version + 1),
            );
        validate_fixture_set(name, schema, &minimal, &full, &invalid, &future)?;
        for (fixture_name, value) in [
            ("minimal.json", minimal),
            ("full.json", full),
            ("invalid.json", invalid),
            ("future.json", future),
        ] {
            files.push((
                fixture_root.join(stem).join(fixture_name),
                serde_json::to_vec_pretty(&value)?,
            ));
        }
    }
    let identity_payload = serde_json::json!({
        "project_id":"prj_00000000000000000000000000",
        "project_relative_path":"src/lib.rs",
        "rule_id":"star.rule.trailing-whitespace",
        "rule_version":"1.0.0",
        "scan_config_fingerprint":format!("sha256:{}", "1".repeat(64)),
        "source_content_sha256":Sha256Hash::digest(b"fn main() {}\n"),
    });
    let identity_fingerprint = canonical_sha256(&serde_json::json!({
        "algorithm":"star.management-fingerprint-golden",
        "contract_version":1,
        "payload":identity_payload,
    }))?;
    let golden = serde_json::json!({
        "schema_version":1,
        "algorithm":"star.management-fingerprint-golden",
        "contract_version":1,
        "payload":identity_payload,
        "identity_fingerprint":identity_fingerprint,
        "derived_ids":{
            "canonical_source_id":CanonicalSourceId::from_fingerprint(&identity_fingerprint),
            "symbol_id":SymbolId::from_fingerprint(&identity_fingerprint),
            "finding_id":FindingId::from_fingerprint(&identity_fingerprint),
            "occurrence_id":OccurrenceId::from_fingerprint(&identity_fingerprint),
        }
    });
    files.push((
        fixture_root.join("fingerprint-golden.json"),
        serde_json::to_vec_pretty(&golden)?,
    ));
    Ok(files)
}

fn validate_fixture_set(
    name: &str,
    schema: &Value,
    minimal: &Value,
    full: &Value,
    invalid: &Value,
    future: &Value,
) -> DynResult<()> {
    let validator = jsonschema::draft202012::options().build(schema)?;
    if let Some(error) = validator.iter_errors(minimal).next() {
        return Err(format!("{name} minimal fixture is invalid: {error}").into());
    }
    if let Some(error) = validator.iter_errors(full).next() {
        return Err(format!("{name} full fixture is invalid: {error}").into());
    }
    if validator.is_valid(invalid) || validator.is_valid(future) {
        return Err("generated invalid or future management fixture was accepted".into());
    }
    Ok(())
}

fn sample_from_schema(
    schema: &Value,
    root: &Value,
    minimal: bool,
    property_name: Option<&str>,
) -> DynResult<Value> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .ok_or("fixture generator only supports local Schema references")?;
        return sample_from_schema(
            root.pointer(pointer).ok_or("Schema reference is missing")?,
            root,
            minimal,
            property_name,
        );
    }
    if let Some(value) = schema.get("const") {
        return Ok(value.clone());
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if property_name == Some("decision")
            && values
                .iter()
                .any(|value| value.as_str() == Some("human_review"))
        {
            return Ok(Value::String("human_review".to_owned()));
        }
        return values.first().cloned().ok_or_else(|| "empty enum".into());
    }
    if property_name == Some("source_artifact_ref")
        && ["oneOf", "anyOf"].iter().any(|keyword| {
            schema
                .get(keyword)
                .and_then(Value::as_array)
                .is_some_and(|options| {
                    options
                        .iter()
                        .any(|option| option.get("type") == Some(&Value::String("null".to_owned())))
                })
        })
    {
        return Ok(Value::Null);
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(options) = schema.get(keyword).and_then(Value::as_array) {
            let selected = options
                .iter()
                .find(|option| option.get("type") != Some(&Value::String("null".to_owned())))
                .or_else(|| options.first())
                .ok_or("empty union")?;
            return sample_from_schema(selected, root, minimal, property_name);
        }
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let selected = types
            .iter()
            .filter_map(Value::as_str)
            .find(|value| *value != "null")
            .or_else(|| types.iter().filter_map(Value::as_str).next())
            .ok_or("empty type union")?;
        let mut narrowed = schema.clone();
        narrowed
            .as_object_mut()
            .ok_or("type union schema is not an object")?
            .insert("type".to_owned(), Value::String(selected.to_owned()));
        return sample_from_schema(&narrowed, root, minimal, property_name);
    }
    let schema_type = schema
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| schema.get("properties").map(|_| "object"))
        .unwrap_or("string");
    match schema_type {
        "object" => {
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let required: std::collections::BTreeSet<_> = schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut object = Map::new();
            for (name, property) in properties {
                if minimal && !required.contains(name.as_str()) {
                    continue;
                }
                object.insert(
                    name.clone(),
                    sample_from_schema(&property, root, minimal, Some(&name))?,
                );
            }
            if object.is_empty()
                && !minimal
                && let Some(additional) = schema.get("additionalProperties")
                && additional.is_object()
            {
                object.insert(
                    "fixture".to_owned(),
                    sample_from_schema(additional, root, minimal, Some("fixture"))?,
                );
            }
            Ok(Value::Object(object))
        }
        "array" => {
            let minimum = schema.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            let count = if minimal { minimum } else { minimum.max(1) };
            if count == 0 {
                return Ok(Value::Array(Vec::new()));
            }
            let Some(items) = schema.get("items") else {
                return Ok(Value::Array(Vec::new()));
            };
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(sample_from_schema(items, root, minimal, property_name)?);
            }
            Ok(Value::Array(values))
        }
        "integer" | "number" => Ok(schema
            .get("minimum")
            .cloned()
            .unwrap_or_else(|| Value::from(1))),
        "boolean" => Ok(Value::Bool(false)),
        "null" => Ok(Value::Null),
        "string" => Ok(Value::String(sample_string(schema, property_name))),
        _ => Err(format!("unsupported fixture schema type: {schema_type}").into()),
    }
}

fn sample_string(schema: &Value, property_name: Option<&str>) -> String {
    if schema.get("format").and_then(Value::as_str) == Some("date-time") {
        return "2026-01-01T00:00:00Z".to_owned();
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        if pattern.starts_with("^sha256:") {
            return format!("sha256:{}", "0".repeat(64));
        }
        if let Some(prefix) = pattern
            .strip_prefix('^')
            .and_then(|value| value.split('[').next())
            && prefix.ends_with('_')
        {
            let (character, length) = if pattern.contains("{52}") {
                ('a', 52)
            } else {
                ('0', 26)
            };
            return format!("{prefix}{}", character.to_string().repeat(length));
        }
    }
    let name = property_name.unwrap_or_default();
    if name.contains("sha256") || name.contains("fingerprint") || name.ends_with("_hash") {
        return format!("sha256:{}", "0".repeat(64));
    }
    let id_prefix = if name.contains("managed_registry_snapshot") {
        Some("mrs_")
    } else if name.contains("registry_consistency_record") {
        Some("rcr_")
    } else if name.contains("project_catalog_snapshot") {
        Some("pcs_")
    } else if name.contains("code_index_snapshot") {
        Some("cix_")
    } else if name.contains("project_revision")
        || name.contains("source_revision")
        || name == "scope_revision"
        || name.ends_with("revision_id")
    {
        Some("prv_")
    } else if name.contains("workspace_snapshot") {
        Some("wsp_")
    } else if name.contains("scan_run") || name.contains("scan_id") {
        Some("scn_")
    } else if name.contains("finding") {
        Some("fnd_")
    } else if name.contains("occurrence") {
        Some("occ_")
    } else if name.contains("symbol_reference") {
        Some("srf_")
    } else if name.contains("symbol") {
        Some("sym_")
    } else if name.contains("canonical_source")
        || name.contains("source_id")
        || name.contains("generated_from")
    {
        Some("src_")
    } else if name.contains("suppression") {
        Some("sup_")
    } else if name.contains("baseline") {
        Some("bas_")
    } else if name.contains("disposition") {
        Some("dsp_")
    } else if name.contains("change_plan") {
        Some("cpl_")
    } else if name.contains("patch_set") {
        Some("pat_")
    } else if name.contains("validation_result") {
        Some("vrs_")
    } else if name.contains("validation_run") {
        Some("val_")
    } else if name == "gate_id" {
        Some("gat_")
    } else if name == "goal_id" {
        Some("gol_")
    } else if name == "run_id" {
        Some("run_")
    } else if name == "stage_id" {
        Some("stg_")
    } else if name.contains("diagnostic_id") {
        Some("dia_")
    } else if name.contains("waiver_id") {
        Some("wav_")
    } else if name.contains("gate_decision") {
        Some("gtd_")
    } else if name.contains("artifact") {
        Some("art_")
    } else if name.contains("root_binding") {
        Some("rtb_")
    } else if name.contains("checkout") {
        Some("cko_")
    } else if name.contains("generation") {
        Some("gen_")
    } else if name.contains("backup_set") {
        Some("bks_")
    } else if name.contains("recovery_plan") {
        Some("rcp_")
    } else if name.contains("bundle_id") {
        Some("lsb_")
    } else if name.contains("coordinated_operation") || name == "operation_id" {
        Some("cop_")
    } else if name.contains("store_id") {
        Some("mst_")
    } else if name == "project_id" {
        Some("prj_")
    } else {
        None
    };
    if let Some(prefix) = id_prefix {
        let length = if matches!(
            prefix,
            "mrs_"
                | "rcr_"
                | "pcs_"
                | "cix_"
                | "prv_"
                | "wsp_"
                | "fnd_"
                | "occ_"
                | "srf_"
                | "sym_"
                | "src_"
        ) {
            52
        } else {
            26
        };
        let character = if length == 52 { "a" } else { "0" };
        return format!("{prefix}{}", character.repeat(length));
    }
    match name {
        name if name.contains("path") => "src/lib.rs".to_owned(),
        "created_at" | "updated_at" | "started_at" | "finished_at" | "captured_at"
        | "decided_at" | "observed_at" => "2026-01-01T00:00:00Z".to_owned(),
        _ => "fixture".to_owned(),
    }
}

fn write_generated(root: &Path) -> DynResult<()> {
    for (path, bytes) in generated_files(root)? {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn check_generated(root: &Path) -> DynResult<()> {
    let generated = generated_files(root)?;
    let expected_schema_paths: BTreeSet<_> = generated
        .iter()
        .filter(|(path, _)| path.parent() == Some(root))
        .map(|(path, _)| path.clone())
        .collect();
    let mut drift = Vec::new();
    for (path, expected) in &generated {
        match fs::read(path) {
            Ok(actual) if actual == *expected => {}
            Ok(_) => drift.push(format!("changed: {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drift.push(format!("missing: {}", path.display()));
            }
            Err(error) => return Err(error.into()),
        }
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && !expected_schema_paths.contains(&path)
        {
            drift.push(format!("stale: {}", path.display()));
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!("generated schema drift:\n{}", drift.join("\n")).into())
    }
}

fn main() -> DynResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let default_root = PathBuf::from("specs/schemas/v1");
    match args.as_slice() {
        [] => write_generated(&default_root),
        [flag] if flag == "--check" => check_generated(&default_root),
        [root] => write_generated(Path::new(root)),
        _ => Err("usage: star-schema-gen [--check | output-directory]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_mode_detects_missing_changed_and_stale_schema_files_without_writing() {
        let parent = std::env::temp_dir().join(format!("star-schema-gen-{}", std::process::id()));
        let root = parent.join(format!("v1-{}", star_contracts::ids::RequestId::new()));
        write_generated(&root).unwrap();
        check_generated(&root).unwrap();

        let first = generated_files(&root).unwrap().remove(0).0;
        fs::write(&first, b"changed").unwrap();
        assert!(check_generated(&root).is_err());

        write_generated(&root).unwrap();
        fs::write(root.join("stale.schema.json"), b"{}").unwrap();
        assert!(check_generated(&root).is_err());
    }
}
