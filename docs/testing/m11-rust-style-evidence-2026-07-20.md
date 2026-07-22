# P-0052 M11 Rust 자동 교정 제품 Slice 증거

## 계약과 pipeline

- `RustToolchainBinding`, `RustStylePolicySnapshot`, `RustStyleCoverageMatrix`, `RustStyleStepExecution` v1 공개 계약과 generated fixture
- exact stable Rust 1.96, installed cargo/rustc/rustfmt/clippy identity, complete target/coverage만 auto candidate 허용
- pipeline 고정 순서: `rustfmt_first(5) → clippy_allowlisted_fix(6) → rustfmt_final(7) → idempotence_replay(11) → candidate_validate(12)`
- Clippy fix는 exact `clippy::<lint>`, active allowlist, exact release/hash, `MachineApplicable`, handwritten in-scope `.rs`, before hash와 byte-range를 모두 요구
- actual Clippy after byte가 selected suggestion 적용 결과와 정확히 같지 않으면 candidate 전체 차단
- final candidate는 forward/reverse artifact와 immutable `PatchSet`을 생성하고 full mutation pipeline의 second-run operation 0을 요구
- `safe_default` exact candidate approval과 `personal_auto` standing grant 후속 permit을 분리하며 permit은 single-use
- partial/outcome-unknown/post Gate failure는 rollback success로 덮지 않고 `recovery_required`
- `star style rust inspect|check|prepare|auto-apply`는 Controller single writer와 같은 application service를 사용하며 raw Cargo argv나 filesystem locator를 공개 payload로 받지 않음
- candidate artifact는 workspace/package scope와 `safe_default|personal_auto`를 봉인한다. apply 전·후에 exact toolchain·Cargo/rustfmt/Clippy config·policy·coverage·adapter fingerprint를 재검증하고 drift면 source mutation 전 차단하거나 reverse artifact로 rollback한다.
- `personal_auto`는 current scan의 persisted `star.validation.rust-style-pre-apply-v1` authoritative Gate를 소비하고, post apply는 `star.validation.rust-style-v1` Gate 증거를 저장한다.

## Corpus

`specs/corpus/rust-style/multicrate`는 다음을 포함한다.

- workspace의 app + proc-macro crate
- build script
- default/named feature와 required-feature binary
- target cfg branch
- generated/vendor sentinel

실제 fixed adapter smoke는 짧은 Star-owned mirror와 외부 owned `CARGO_TARGET_DIR`, offline mode에서 `cargo fmt --all -- --check`와 `cargo clippy --workspace --features rust-style-app/cli --all-targets --offline --message-format=json --no-deps`를 실행했다. `--all-features`는 사용하지 않았고 project `.star-control/rust-style.toml`이 compatible하다고 선언한 feature union만 선택했다. source snapshot은 실행 전후 byte-exact 동일했다.

첫 long-path mirror는 Windows linker `LNK1104`를 실제 재현했다. runtime root 아래 opaque hash 기반 짧은 owned path로 격리 layout을 변경한 뒤 같은 Corpus를 다시 실행해 통과했으며 사용자 source나 `target/`을 정리하지 않았다.

ARM64는 실기 성공으로 표시하지 않는다. `scripts/release/cross-build-arm64.ps1`가 exact Rust 1.96 toolchain에서 root workspace를 cross-build한 뒤 같은 nested multi-crate corpus를 `aarch64-pc-windows-msvc` 대상으로 `cargo check`와 Clippy `--all-targets --features rust-style-app/cli --offline --no-deps -D warnings`로 분석한다. build script·proc-macro는 host에서, target cfg·required-feature binary는 ARM64 target에서 검사되며 결과는 계속 `native_unverified`다.

## 검증

- `cargo test --locked -p star-application --lib`: Rust pipeline·approval·recovery 포함 PASS
- `cargo test --locked -p star-execution rust_style::tests::actual_rustfmt_and_clippy_check_multicrate_corpus_without_source_write -- --nocapture`: PASS
- `cargo test -p star-application rust_style_runtime::tests --offline -- --nocapture`: exact Rust 1.96 small workspace inspect/check/prepare/apply/rollback와 multi-crate feature/build-script/proc-macro no-op PASS
- `cargo test -p star-application personal_auto_rust_style_uses_persisted_pre_and_post_gates --offline -- --nocapture`: package scope, semantic fallback 보존, standing grant, persisted pre/post Gate, exact apply, second-run zero diff와 Cargo config drift pre-mutation block PASS
- `cargo check -p star-execution -p star-application -p star-cli -p star-controller --all-targets --offline`: Controller/application/CLI operational wiring PASS
- `pwsh -NoProfile -File scripts/release/cross-build-arm64.ps1`: root workspace cross-build + Rust style corpus ARM64 check/Clippy PASS, `native_unverified`
- generated/vendor write injection: `RUST_STYLE_SIDE_EFFECT_VIOLATION`
- formatted+fixed expected-after의 두 번째 전체 실행: `succeeded_no_change`, PatchSet 없음
- disposable fixture의 exact apply·post-check·rollback을 실행했고 원본 복원을 byte hash로 확인했다. 실제 사용자 checkout apply, package/component 설치와 network access는 수행하지 않았다.
