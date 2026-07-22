use std::{fs, path::PathBuf, time::Instant};

use serde::Serialize;
use star_adapter_rust_index::RustSyntaxAdapter;
use star_contracts::{Sha256Hash, management::ProjectPathRef};
use star_project::{FileObservation, index::SyntaxAdapter};

#[derive(Serialize)]
struct Metric {
    samples_ms: Vec<u128>,
    p95_ms: u128,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    architecture: String,
    native_execution: bool,
    parser: String,
    grammar: String,
    file_count: usize,
    repetitions: usize,
    analysis: Metric,
    definition_count: usize,
    reference_count: usize,
    resolved_reference_count: usize,
    parse_failure_count: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file_count = 10_000_usize;
    let mut repetitions = 5_usize;
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--files" => {
                file_count = arguments
                    .next()
                    .ok_or("--files requires a value")?
                    .parse()?;
            }
            "--repetitions" => {
                repetitions = arguments
                    .next()
                    .ok_or("--repetitions requires a value")?
                    .parse()?;
            }
            "--output" => {
                output = Some(PathBuf::from(
                    arguments.next().ok_or("--output requires a value")?,
                ));
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if !(1_000..=100_000).contains(&file_count) || !(3..=20).contains(&repetitions) {
        return Err("files must be 1000..100000 and repetitions must be 3..20".into());
    }
    let sources = (0..file_count).map(source).collect::<Result<Vec<_>, _>>()?;
    let adapter = RustSyntaxAdapter;
    let mut samples = Vec::new();
    let mut definition_count = 0;
    let mut reference_count = 0;
    let mut resolved_reference_count = 0;
    let mut parse_failure_count = 0;
    for _ in 0..repetitions {
        let started = Instant::now();
        definition_count = 0;
        reference_count = 0;
        resolved_reference_count = 0;
        parse_failure_count = 0;
        for source in &sources {
            match adapter.analyze(source) {
                Ok(analysis) => {
                    definition_count += analysis.definitions.len();
                    reference_count += analysis.references.len();
                    resolved_reference_count += analysis
                        .references
                        .iter()
                        .filter(|reference| {
                            reference.resolution
                                == star_contracts::management::SymbolResolution::Resolved
                        })
                        .count();
                }
                Err(_) => parse_failure_count += 1,
            }
        }
        samples.push(started.elapsed().as_millis());
    }
    if parse_failure_count != 0 || resolved_reference_count != 0 {
        return Err("syntax corpus produced a parse failure or false confirmed reference".into());
    }
    let report = Report {
        schema_version: 1,
        architecture: std::env::consts::ARCH.to_owned(),
        native_execution: true,
        parser: "tree-sitter=0.26.11".to_owned(),
        grammar: "tree-sitter-rust=0.24.2".to_owned(),
        file_count,
        repetitions,
        analysis: metric(samples),
        definition_count,
        reference_count,
        resolved_reference_count,
        parse_failure_count,
    };
    let rendered = serde_json::to_vec_pretty(&report)?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, &rendered)?;
    }
    println!("{}", String::from_utf8(rendered)?);
    Ok(())
}

fn source(index: usize) -> Result<FileObservation, star_contracts::management::ProjectPathError> {
    let text = format!(
        "#[cfg(any())]\nmacro_rules! hidden_{index} {{ () => {{ 0usize }} }}\npub struct Item{index} {{ pub value: usize }}\nimpl Item{index} {{ pub fn value(&self) -> usize {{ self.value }} }}\npub fn call_{index}(item: Item{index}) -> usize {{ item.value() }}\n"
    );
    Ok(FileObservation {
        path: ProjectPathRef::parse(format!("src/bucket_{:04}/item_{index}.rs", index / 100))?,
        content_sha256: Sha256Hash::digest(text.as_bytes()),
        size_bytes: text.len() as u64,
        line_count: text.lines().count() as u32,
        text: Some(text),
        language_id: Some("rust".to_owned()),
    })
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
