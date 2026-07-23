use std::{fs, path::PathBuf, process::Command, time::Instant};

use chrono::Utc;
use serde::Serialize;
use star_contracts::{
    Sha256Hash,
    ids::{CheckoutId, GenerationId, ProjectId, RootBindingId, ScanRunId},
    management::{
        CheckoutAttachmentState, CheckoutHeadState, CheckoutKind, IdentityScope, Project,
        ProjectCheckout, RegistrationState, RepositoryKind,
    },
};
use star_project::{
    ScanPolicy,
    catalog_snapshot::{CatalogSnapshotInput, DiscoveryConfig, build_project_catalog_snapshot},
    index::{CodeIndexBuildRequest, CodeIndexProjection, IndexPolicy, build_code_index},
    observe_project,
};

#[derive(Serialize)]
struct Metric {
    samples_ms: Vec<u128>,
    p95_ms: u128,
}

#[derive(Serialize)]
struct CorpusReport {
    schema_version: u32,
    architecture: String,
    native_execution: bool,
    repository_mode: String,
    dirty_worktree: bool,
    file_count: usize,
    total_source_bytes: u64,
    repetitions: usize,
    full_scan: Metric,
    incremental_unchanged: Metric,
    incremental_single_file: Metric,
    cache_miss_index_only: Metric,
    cache_hit_index_only: Metric,
    peak_working_set_bytes: u64,
    projection_serialized_bytes: usize,
    default_file_cache_eligible: bool,
    classification_partition_count: usize,
    unchanged_reused_partition_count: usize,
    changed_reused_partition_count: usize,
    changed_invalidated_partition_count: usize,
    semantic_unavailable_count: usize,
    bad_confirmed_definition_count: usize,
    bad_confirmed_reference_count: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file_count = 10_000_usize;
    let mut repetitions = 5_usize;
    let mut output = None;
    let mut projection_output = None;
    let mut repository_mode = "non_git".to_owned();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--files" => {
                file_count = arguments
                    .next()
                    .ok_or("--files requires a value")?
                    .parse()?
            }
            "--repetitions" => {
                repetitions = arguments
                    .next()
                    .ok_or("--repetitions requires a value")?
                    .parse()?
            }
            "--output" => {
                output = Some(PathBuf::from(
                    arguments.next().ok_or("--output requires a value")?,
                ))
            }
            "--projection-output" => {
                projection_output = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--projection-output requires a value")?,
                ))
            }
            "--repository" => {
                repository_mode = arguments.next().ok_or("--repository requires a value")?;
                if !matches!(repository_mode.as_str(), "non_git" | "git_dirty") {
                    return Err("--repository must be non_git or git_dirty".into());
                }
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if !(1_000..=100_000).contains(&file_count) || !(3..=20).contains(&repetitions) {
        return Err("files must be 1000..100000 and repetitions must be 3..20".into());
    }

    let fixture_root = std::env::temp_dir().join(format!(
        "star-m1-corpus-{}-{}",
        std::process::id(),
        CheckoutId::new()
    ));
    let source_root = fixture_root.join("source");
    fs::create_dir_all(&source_root)?;
    let mut total_source_bytes = 0_u64;
    for index in 0..file_count {
        let bucket = source_root.join(format!("crate_{:04}", index / 100));
        fs::create_dir_all(&bucket)?;
        let content = format!(
            "pub fn item_{index}(value: usize) -> usize {{ value.wrapping_add({index}) }}\n"
        );
        total_source_bytes += content.len() as u64;
        fs::write(bucket.join(format!("item_{index}.rs")), content)?;
    }
    if repository_mode == "git_dirty" {
        initialize_dirty_git_fixture(&source_root)?;
    }
    let repository_kind = if repository_mode == "git_dirty" {
        RepositoryKind::Git
    } else {
        RepositoryKind::None
    };
    let (project, checkout) = fixture_contracts(repository_kind);
    let catalog = build_project_catalog_snapshot(
        &[CatalogSnapshotInput {
            project: &project,
            checkout: &checkout,
            root: &source_root,
        }],
        &DiscoveryConfig::default(),
    )?;
    let scan_policy = ScanPolicy::default();
    let index_policy = IndexPolicy::default();

    let mut full_samples = Vec::new();
    let mut last_full = None;
    for _ in 0..repetitions {
        let started = Instant::now();
        let observation = observe_project(&project, &source_root, &scan_policy)?;
        let projection = build(
            &project,
            &checkout,
            &catalog,
            &observation,
            &index_policy,
            None,
        )?;
        full_samples.push(started.elapsed().as_millis());
        last_full = Some((observation, projection));
    }
    let (base_observation, base_projection) = last_full.ok_or("full scan did not run")?;

    let mut unchanged_samples = Vec::new();
    let mut unchanged_projection = None;
    for _ in 0..repetitions {
        let started = Instant::now();
        let observation = observe_project(&project, &source_root, &scan_policy)?;
        let projection = build(
            &project,
            &checkout,
            &catalog,
            &observation,
            &index_policy,
            Some(&base_projection),
        )?;
        unchanged_samples.push(started.elapsed().as_millis());
        unchanged_projection = Some(projection);
    }
    let unchanged_projection = unchanged_projection.ok_or("unchanged scan did not run")?;

    let changed_path = source_root.join("crate_0000/item_0.rs");
    fs::write(
        &changed_path,
        "pub fn changed_item(value: usize) -> usize { value.wrapping_mul(2) }\n",
    )?;
    let mut changed_samples = Vec::new();
    let mut changed_projection = None;
    for _ in 0..repetitions {
        let started = Instant::now();
        let observation = observe_project(&project, &source_root, &scan_policy)?;
        let projection = build(
            &project,
            &checkout,
            &catalog,
            &observation,
            &index_policy,
            Some(&base_projection),
        )?;
        changed_samples.push(started.elapsed().as_millis());
        changed_projection = Some(projection);
    }
    let changed_projection = changed_projection.ok_or("changed scan did not run")?;
    let projection_serialized_bytes = serde_json::to_vec(&base_projection)?.len();
    if let Some(projection_output) = projection_output {
        if let Some(parent) = projection_output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(projection_output, serde_json::to_vec(&base_projection)?)?;
    }

    let mut miss_samples = Vec::new();
    let mut hit_samples = Vec::new();
    for _ in 0..repetitions {
        let started = Instant::now();
        let _ = build(
            &project,
            &checkout,
            &catalog,
            &base_observation,
            &index_policy,
            None,
        )?;
        miss_samples.push(started.elapsed().as_millis());

        let started = Instant::now();
        let _ = build(
            &project,
            &checkout,
            &catalog,
            &base_observation,
            &index_policy,
            Some(&base_projection),
        )?;
        hit_samples.push(started.elapsed().as_millis());
    }

    let unchanged_reused_partition_count = unchanged_projection
        .snapshot
        .partitions
        .iter()
        .filter(|partition| partition.cache_hit)
        .count();
    let changed_reused_partition_count = changed_projection
        .snapshot
        .partitions
        .iter()
        .filter(|partition| partition.cache_hit)
        .count();
    let changed_invalidated_partition_count = changed_projection
        .snapshot
        .partitions
        .iter()
        .filter(|partition| partition.partition_key.starts_with("crate_0000/item_0.rs:"))
        .filter(|partition| !partition.cache_hit)
        .count();
    let report = CorpusReport {
        schema_version: 1,
        architecture: std::env::consts::ARCH.to_owned(),
        native_execution: true,
        repository_mode: repository_mode.clone(),
        dirty_worktree: repository_mode == "git_dirty",
        file_count,
        total_source_bytes,
        repetitions,
        full_scan: metric(full_samples),
        incremental_unchanged: metric(unchanged_samples),
        incremental_single_file: metric(changed_samples),
        cache_miss_index_only: metric(miss_samples),
        cache_hit_index_only: metric(hit_samples),
        peak_working_set_bytes: peak_working_set_bytes(),
        projection_serialized_bytes,
        default_file_cache_eligible: projection_serialized_bytes <= 256 * 1024 * 1024,
        classification_partition_count: base_projection
            .snapshot
            .partitions
            .iter()
            .filter(|partition| {
                partition.kind == star_contracts::index::IndexPartitionKind::Classification
            })
            .count(),
        unchanged_reused_partition_count,
        changed_reused_partition_count,
        changed_invalidated_partition_count,
        semantic_unavailable_count: base_projection
            .snapshot
            .limitations
            .iter()
            .filter(|limitation| limitation.code == "INDEX_SEMANTIC_UNAVAILABLE")
            .count(),
        bad_confirmed_definition_count: base_projection
            .entities
            .iter()
            .filter(|entity| entity.confidence == "confirmed_definition")
            .count(),
        bad_confirmed_reference_count: base_projection
            .references
            .iter()
            .filter(|reference| {
                reference.resolution == star_contracts::management::SymbolResolution::Resolved
            })
            .count(),
    };
    let rendered = serde_json::to_vec_pretty(&report)?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, &rendered)?;
    }
    println!("{}", String::from_utf8(rendered)?);
    eprintln!("fixture_root={}", fixture_root.display());
    Ok(())
}

fn fixture_contracts(repository_kind: RepositoryKind) -> (Project, ProjectCheckout) {
    let project_id = ProjectId::new();
    let checkout_id = CheckoutId::new();
    let fingerprint = Sha256Hash::digest(project_id.as_str().as_bytes());
    (
        Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id: project_id.clone(),
            identity_scope: IdentityScope::Local,
            display_name: "m1-corpus".to_owned(),
            repository_kind,
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: fingerprint.clone(),
            registration_state: RegistrationState::Attached,
            attached_checkout_ids: vec![checkout_id.clone()],
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        },
        ProjectCheckout {
            schema_id: "star.project-checkout".to_owned(),
            schema_version: 1,
            checkout_id,
            project_id,
            root_binding_id: Some(RootBindingId::new()),
            repository_kind,
            checkout_kind: if repository_kind == RepositoryKind::Git {
                CheckoutKind::MainWorktree
            } else {
                CheckoutKind::FilesystemRoot
            },
            repository_binding_id: (repository_kind == RepositoryKind::Git)
                .then(|| "fixture_repository_binding".to_owned()),
            worktree_binding_id: (repository_kind == RepositoryKind::Git)
                .then(|| "fixture_worktree_binding".to_owned()),
            object_format: (repository_kind == RepositoryKind::Git).then(|| "sha1".to_owned()),
            head_state: CheckoutHeadState::Unavailable,
            head_ref: None,
            head_commit_id: None,
            head_tree_id: None,
            upstream_ref: None,
            default_branch_hint: None,
            remote_identity: None,
            attachment_state: CheckoutAttachmentState::Attached,
            last_observed_at: Utc::now(),
            limitations: vec![],
            content_fingerprint: fingerprint,
        },
    )
}

fn initialize_dirty_git_fixture(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    run_git(root, &["init", "--quiet"])?;
    run_git(root, &["config", "user.name", "Star-Control Fixture"])?;
    run_git(root, &["config", "user.email", "fixture@invalid.local"])?;
    fs::write(root.join(".star-control-baseline"), "tracked baseline\n")?;
    run_git(root, &["add", ".star-control-baseline"])?;
    run_git(root, &["commit", "--quiet", "-m", "fixture baseline"])?;
    Ok(())
}

fn run_git(root: &std::path::Path, arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()?;
    if !status.success() {
        return Err(format!("git fixture command failed: {arguments:?}").into());
    }
    Ok(())
}

fn build(
    project: &Project,
    checkout: &ProjectCheckout,
    catalog: &star_contracts::index::ProjectCatalogSnapshot,
    observation: &star_project::ProjectObservation,
    policy: &IndexPolicy,
    previous: Option<&CodeIndexProjection>,
) -> Result<CodeIndexProjection, star_project::ProjectError> {
    build_code_index(&CodeIndexBuildRequest {
        project_root: None,
        project,
        checkout,
        catalog_snapshot: catalog,
        observation,
        scan_run_id: &ScanRunId::new(),
        generation_id: &GenerationId::new(),
        policy,
        syntax_adapters: &[],
        semantic_adapters: &[],
        scan_mode: star_contracts::index::IndexScanMode::Incremental,
        previous,
    })
}

fn metric(mut samples_ms: Vec<u128>) -> Metric {
    let mut ordered = samples_ms.clone();
    ordered.sort_unstable();
    let index = ((ordered.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(ordered.len().saturating_sub(1));
    Metric {
        samples_ms: std::mem::take(&mut samples_ms),
        p95_ms: ordered[index],
    }
}

#[cfg(windows)]
fn peak_working_set_bytes() -> u64 {
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    // SAFETY: both calls use the current pseudo handle and a correctly sized,
    // writable PROCESS_MEMORY_COUNTERS-compatible structure.
    let succeeded = unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    if succeeded == 0 {
        0
    } else {
        counters.peak_working_set_size as u64
    }
}

#[cfg(not(windows))]
fn peak_working_set_bytes() -> u64 {
    0
}
