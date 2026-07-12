use std::time::{Duration, Instant};

use star_contracts::{
    canonical::{canonical_sha256, jcs_bytes},
    manifest::{ManifestSource, parse_manifest_v1},
};

struct DeterministicRng(u64);

impl DeterministicRng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn index(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper.max(1)
    }
}

fn fuzz_duration() -> Duration {
    Duration::from_secs(
        std::env::var("STAR_FUZZ_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1)
            .clamp(1, 600),
    )
}

#[test]
fn manifest_parser_bounded_arbitrary_byte_mutations_never_panic() {
    let seed = include_bytes!(
        "../../../../specs/fixtures/mcp/manifests/valid/tool-package-manifest-v1.toml"
    );
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0x9e37_79b9_7f4a_7c15);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let mut candidate = seed.to_vec();
        for _ in 0..=rng.index(16) {
            match rng.index(4) {
                0 if !candidate.is_empty() => {
                    let index = rng.index(candidate.len());
                    candidate[index] = rng.next() as u8;
                }
                1 if !candidate.is_empty() => {
                    candidate.remove(rng.index(candidate.len()));
                }
                2 if candidate.len() < 64 * 1024 => {
                    let index = rng.index(candidate.len() + 1);
                    candidate.insert(index, rng.next() as u8);
                }
                _ => candidate.truncate(rng.index(candidate.len() + 1)),
            }
        }
        let text = String::from_utf8_lossy(&candidate);
        let parsed = std::panic::catch_unwind(|| {
            let _ = parse_manifest_v1(&text, ManifestSource::User);
        });
        assert!(
            parsed.is_ok(),
            "manifest parser panicked at deterministic case {cases}"
        );
        cases += 1;
    }
    assert!(cases > 100);
}

#[test]
fn jcs_key_order_and_array_order_properties_are_stable_without_unicode_normalization() {
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0xd1b5_4a32_d192_ed03);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let a = rng.next() % 1_000_000;
        let b = rng.next() % 1_000_000;
        let left: serde_json::Value =
            serde_json::from_str(&format!(r#"{{"z":{b},"a":{a},"nested":{{"y":2,"x":1}}}}"#))
                .unwrap();
        let right: serde_json::Value =
            serde_json::from_str(&format!(r#"{{"nested":{{"x":1,"y":2}},"a":{a},"z":{b}}}"#))
                .unwrap();
        assert_eq!(jcs_bytes(&left).unwrap(), jcs_bytes(&right).unwrap());
        assert_eq!(
            canonical_sha256(&left).unwrap(),
            canonical_sha256(&right).unwrap()
        );

        let ordered = serde_json::json!({"values":[a,b]});
        let reversed = serde_json::json!({"values":[b,a]});
        if a != b {
            assert_ne!(
                canonical_sha256(&ordered).unwrap(),
                canonical_sha256(&reversed).unwrap(),
                "JCS must preserve array order"
            );
        }
        let composed = serde_json::json!({"value":format!("é-{a}")});
        let decomposed = serde_json::json!({"value":format!("e\u{301}-{a}")});
        assert_ne!(
            canonical_sha256(&composed).unwrap(),
            canonical_sha256(&decomposed).unwrap()
        );
        cases += 1;
    }
    assert!(cases > 100);
}
