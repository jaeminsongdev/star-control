use super::*;

#[test]
fn selfcheck_passes_for_repository_contracts() {
    let report = run_selfcheck(repo_root());

    assert!(report.ok, "{:?}", report.diagnostics);
}

#[test]
fn selfcheck_reports_missing_manifest_output() {
    let temp_repo = temp_dir();
    copy_dir(
        &repo_root().join("builtin-tools/star-sentinel"),
        &temp_repo.join("builtin-tools/star-sentinel"),
    );
    let manifest = temp_repo.join("builtin-tools/star-sentinel/tool.yaml");
    let content = fs::read_to_string(&manifest)
        .expect("manifest")
        .replace("  - ledger.jsonl\n", "");
    fs::write(&manifest, content).expect("write manifest");

    let report = run_selfcheck(&temp_repo);

    assert!(!report.ok);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("manifest outputs missing ledger.jsonl")));
    fs::remove_dir_all(temp_repo).ok();
}
