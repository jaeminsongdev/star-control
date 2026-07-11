//! Release gate for the MCP verification matrix.
//!
//! A real Rust test must carry its exact matrix ID in a `// matrix:` marker.
//! Missing IDs are reported and produce a non-zero exit; this tool never turns
//! an unimplemented matrix row into a passing/skipped test.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use regex::Regex;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let matrix = fs::read_to_string(root.join("docs/testing/mcp-verification-matrix.md"))
        .expect("matrix document reads");
    let id_pattern = Regex::new(r"MCP-[A-Z]+[0-9]{3}").expect("static regex");
    let expected: BTreeSet<_> = id_pattern
        .find_iter(&matrix)
        .map(|value| value.as_str().to_owned())
        .collect();
    let mut mapped = BTreeMap::new();
    scan_rust(&root.join("apps"), &id_pattern, &mut mapped);
    scan_rust(&root.join("crates"), &id_pattern, &mut mapped);
    let mapped_ids: BTreeSet<_> = mapped.keys().cloned().collect();
    let missing: Vec<_> = expected.difference(&mapped_ids).cloned().collect();
    println!(
        "{{\"expected\":{},\"mapped\":{},\"missing\":{:?}}}",
        expected.len(),
        mapped_ids.intersection(&expected).count(),
        missing
    );
    if !missing.is_empty() || expected.len() != 170 {
        std::process::exit(1);
    }
}

fn scan_rust(directory: &Path, pattern: &Regex, mapped: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_rust(&path, pattern, mapped);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            if let Ok(source) = fs::read_to_string(&path) {
                scan_source(&path, &source, pattern, mapped);
            }
        }
    }
}

fn scan_source(path: &Path, source: &str, pattern: &Regex, mapped: &mut BTreeMap<String, String>) {
    let function =
        Regex::new(r"^(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)").expect("static regex");
    let lines: Vec<_> = source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("// matrix:") {
            continue;
        }
        let Some(next) = lines[index + 1..]
            .iter()
            .find(|line| !line.trim().is_empty())
        else {
            continue;
        };
        let Some(captures) = function.captures(next.trim()) else {
            continue;
        };
        let attached_to_test = lines[..index]
            .iter()
            .rev()
            .take(3)
            .any(|line| line.contains("#[test]") || line.contains("#[tokio::test]"));
        if !attached_to_test {
            continue;
        }
        let name = format!("{}::{}", path.display(), &captures[1]);
        for id in pattern
            .find_iter(line)
            .map(|value| value.as_str().to_owned())
        {
            mapped.entry(id).or_insert_with(|| name.clone());
        }
    }
}
