use std::time::{Duration, Instant};

use star_contracts::ipc::IPC_MAX_FRAME_BYTES;
use star_ipc::{decode_frame, encode_frame};

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
fn length_prefixed_ipc_length_truncation_utf8_and_json_mutations_are_bounded() {
    let deadline = Instant::now() + fuzz_duration();
    let mut rng = DeterministicRng(0xa076_1d64_78bd_642f);
    let mut cases = 0_u64;
    while Instant::now() < deadline {
        let value = serde_json::json!({
            "case": cases,
            "text": format!("{:016x}", rng.next()),
            "items": [rng.next() % 100, rng.next() % 100]
        });
        let payload = serde_json::to_vec(&value).unwrap();
        let frame = encode_frame(&payload).unwrap();
        assert_eq!(decode_frame(&frame).unwrap(), payload);

        let mut mutated = frame.clone();
        match rng.index(5) {
            0 => mutated.truncate(rng.index(mutated.len())),
            1 => mutated[..4].copy_from_slice(&(rng.next() as u32).to_le_bytes()),
            2 => mutated.extend_from_slice(&[rng.next() as u8; 3]),
            3 if mutated.len() > 4 => {
                let index = 4 + rng.index(mutated.len() - 4);
                mutated[index] = 0xff;
            }
            _ => mutated[0..4].copy_from_slice(&0_u32.to_le_bytes()),
        }
        let result = std::panic::catch_unwind(|| decode_frame(&mutated).map(|bytes| bytes.len()));
        assert!(
            result.is_ok(),
            "IPC decoder panicked at deterministic case {cases}"
        );
        if let Ok(Ok(length)) = result {
            assert_eq!(length + 4, mutated.len());
            assert!(length <= IPC_MAX_FRAME_BYTES);
            assert!(std::str::from_utf8(&mutated[4..]).is_ok());
        }
        cases += 1;
    }
    assert!(cases > 100);
    assert!(encode_frame(&vec![b'x'; IPC_MAX_FRAME_BYTES + 1]).is_err());
    assert!(decode_frame(&[]).is_err());
}
