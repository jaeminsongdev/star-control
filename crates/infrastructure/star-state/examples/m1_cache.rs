use std::{fs, path::PathBuf, time::Instant};

use serde::Serialize;
use star_contracts::Sha256Hash;
use star_ports::{CodeIndexCache, StoredCodeIndexProjection};
use star_state::FileCodeIndexCache;

const MIB: u64 = 1024 * 1024;

#[derive(Serialize)]
struct Metric {
    samples_ms: Vec<u128>,
    p95_ms: u128,
}

#[derive(Serialize)]
struct CacheReport {
    schema_version: u32,
    architecture: String,
    native_execution: bool,
    protection: String,
    repetitions: usize,
    projection_serialized_bytes: u64,
    miss: Metric,
    store: Metric,
    hit: Metric,
    retained_entry_count: usize,
    retained_entry_bytes: u64,
    max_entries_per_project: usize,
    max_entry_bytes: u64,
    max_project_bytes: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut projection_path = None;
    let mut output_path = None;
    let mut repetitions = 5_usize;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--projection" => {
                projection_path = Some(PathBuf::from(
                    arguments.next().ok_or("--projection requires a value")?,
                ));
            }
            "--output" => {
                output_path = Some(PathBuf::from(
                    arguments.next().ok_or("--output requires a value")?,
                ));
            }
            "--repetitions" => {
                repetitions = arguments
                    .next()
                    .ok_or("--repetitions requires a value")?
                    .parse()?;
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if !(3..=20).contains(&repetitions) {
        return Err("repetitions must be 3..20".into());
    }
    let projection_path = projection_path.ok_or("--projection is required")?;
    let projection_bytes = fs::read(&projection_path)?;
    let projection: StoredCodeIndexProjection = serde_json::from_slice(&projection_bytes)?;
    let project_id = projection.snapshot.project_id.clone();
    let root = std::env::temp_dir().join(format!(
        "star-m1-cache-{}-{}",
        std::process::id(),
        project_id
    ));
    let cache = FileCodeIndexCache::open_with_limits(&root, 3, 256 * MIB, 512 * MIB)?;

    let mut miss_samples = Vec::new();
    let mut store_samples = Vec::new();
    let mut hit_samples = Vec::new();
    for iteration in 0..repetitions {
        let cache_key = Sha256Hash::digest(format!("m1-cache-{iteration}").as_bytes());

        let started = Instant::now();
        if cache.load(&project_id, &cache_key)?.is_some() {
            return Err("fresh cache key unexpectedly hit".into());
        }
        miss_samples.push(started.elapsed().as_millis());

        let started = Instant::now();
        cache.store(&project_id, &cache_key, &projection)?;
        store_samples.push(started.elapsed().as_millis());

        let started = Instant::now();
        let loaded = cache
            .load(&project_id, &cache_key)?
            .ok_or("stored cache entry was not readable")?;
        hit_samples.push(started.elapsed().as_millis());
        if loaded.snapshot.code_index_snapshot_id != projection.snapshot.code_index_snapshot_id {
            return Err("cache hit returned the wrong projection".into());
        }
    }

    let retained_entries = fs::read_dir(root.join(project_id.as_str()))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        })
        .collect::<Vec<_>>();
    let retained_entry_count = retained_entries.len();
    let retained_entry_bytes = retained_entries.iter().try_fold(0_u64, |total, entry| {
        Ok::<_, std::io::Error>(total.saturating_add(entry.metadata()?.len()))
    })?;
    if retained_entry_count > 3 {
        return Err("cache eviction limit was exceeded".into());
    }
    let report = CacheReport {
        schema_version: 2,
        architecture: std::env::consts::ARCH.to_owned(),
        native_execution: true,
        protection: "dpapi_current_user".to_owned(),
        repetitions,
        projection_serialized_bytes: projection_bytes.len() as u64,
        miss: metric(miss_samples),
        store: metric(store_samples),
        hit: metric(hit_samples),
        retained_entry_count,
        retained_entry_bytes,
        max_entries_per_project: 3,
        max_entry_bytes: 256 * MIB,
        max_project_bytes: 512 * MIB,
    };
    let rendered = serde_json::to_vec_pretty(&report)?;
    if let Some(output_path) = output_path {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, &rendered)?;
    }
    println!("{}", String::from_utf8(rendered)?);
    eprintln!("cache_root={}", root.display());
    Ok(())
}

fn metric(samples_ms: Vec<u128>) -> Metric {
    let mut ordered = samples_ms.clone();
    ordered.sort_unstable();
    let index = ((ordered.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(ordered.len().saturating_sub(1));
    Metric {
        samples_ms,
        p95_ms: ordered[index],
    }
}
