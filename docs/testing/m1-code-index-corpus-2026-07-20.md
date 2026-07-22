# M1 Code Index x64 corpus 실측 — 2026-07-20

## 상태

- 결과: `accepted`
- 실행 환경: Windows 11 Pro x64, Rust 1.96.0, `--release`
- corpus: 생성한 Rust source 10,000개, 687,780 bytes, 구간별 5회 반복
- 원시 로컬 증거: `target/m1-corpus/x64-reference-with-classification-20260720.json`, `target/m1-corpus/x64-git-dirty-reference-20260720.json`, `target/m1-corpus/x64-rust-syntax-20260720.json`, `target/m1-corpus/x64-file-cache-20260720.json`
- 추적 가능한 기준: `benchmarks/m1-code-index-x64-reference.json`

full/incremental 측정은 catalog·classification·text와 syntax/semantic fallback partition을 함께 포함한다. 같은 10,000-file source를 pinned `tree-sitter` 0.26.11 + `tree-sitter-rust` 0.24.2 adapter로 별도 반복해 syntax parser 비용과 잘못된 confirmed reference 0건을 확인했다. pinned `rust-analyzer 1.96.0 (ac68faa2 2026-05-25)` LSP는 adjudicated Cargo fixture에서 same-file reference를 confirmed하고 cross-file target은 unresolved limitation으로 남겼다. 대형 corpus의 semantic-unavailable 경로도 별도로 보존한다.

## 결과

| 구간 | p95 | 120% budget |
|---|---:|---:|
| full scan | 8,389 ms | 10,067 ms |
| unchanged incremental | 5,173 ms | 6,208 ms |
| single-file incremental | 5,490 ms | 6,588 ms |
| index-only projection miss | 7,177 ms | 8,613 ms |
| index-only projection reuse | 4,696 ms | 5,636 ms |
| 139,674,500-byte DPAPI file-cache store | 10,488 ms | 12,586 ms |
| 139,674,500-byte DPAPI file-cache hit | 11,018 ms | 13,222 ms |
| Rust syntax 10,000 files | 368 ms | 442 ms |
| peak working set | 2,064,920,576 bytes | 2,477,904,692 bytes |

Git dirty/untracked 10,000-file cohort는 full 8,012 ms, unchanged 5,273 ms, single-file 6,172 ms, index-only miss 8,796 ms, reuse 5,525 ms p95였다. 각각의 동결 budget은 9,615 / 6,328 / 7,407 / 10,556 / 6,630 ms다. Git fixture는 한 tracked baseline commit과 10,000개 untracked Rust source로 구성해 porcelain 열거 비용을 포함했다.

unchanged 실행은 inventory 1개와 file classification·text partition 20,000개를 재사용했다. 단일 파일 변경은 다른 file partition과 inventory 19,999개를 재사용하고 변경 파일의 classification·text·syntax fallback·semantic fallback 4개 partition만 다시 계산했다. projection 전체를 저장하는 file cache는 current-user DPAPI와 `ProjectId + cache key` entropy로 보호한다. 기본 `256 MiB/entry`, `512 MiB/project`, 8-entry 한도에서 실제 store/hit을 통과했고, Base64 보호 파일 2개 372,466,920 bytes를 유지한 뒤 project byte quota에 맞춰 나머지를 축출했다.

Rust syntax corpus는 definition 50,000건과 unresolved reference 80,000건을 만들었고 parse failure와 잘못된 resolved reference는 0건이다. invalid syntax, 16 MiB 초과, LSP process crash와 timeout은 각각 parse/resource/unavailable 상태로 분리했다. semantic fixture에서 유일한 confirmed same-file reference는 실제 declaration으로 판정됐으며 cross-file 관계는 `INDEX_RUST_ANALYZER_CROSS_FILE_TARGET_DEFERRED`로 남겼다. 10,000개 fallback source의 semantic partition은 모두 `INDEX_SEMANTIC_UNAVAILABLE`이며 `confirmed_empty`로 승격되지 않았다.

non-Git와 Git dirty cohort 모두 단일 파일 변경이 4개 partition만 무효화해 full 승격이 발생하지 않았다. 10,000-file p95가 동결 budget 안이고 watcher event는 hash 검증을 대체할 수 없으므로 v0.1에는 watcher를 추가하지 않는다. file count·single file·total bytes·parallel limit과 source double-read를 유지하며, 이후 더 큰 byte-volume cohort가 budget을 넘을 때만 watcher를 별도 Slice로 재평가한다.

두 dependency는 MIT이고 lockfile checksum으로 고정된다. `cargo metadata --locked --offline`가 통과하므로 syntax adapter는 build 후 offline 실행한다. semantic adapter는 root path를 저장하지 않고 `rustup which --toolchain 1.96.0 rust-analyzer`가 exact version·observed binary fingerprint를 제공하며 같은 toolchain의 pinned `rust-src`가 확인될 때만 활성화된다. binary·`rust-src`가 없거나 pin이 다르면 정확히 unavailable fallback을 사용한다.

## 첨부 미해결 17개 종료표

| # | 항목 | 종료 증거 |
|---:|---|---|
| 1 | `ProjectCheckout` type·Schema·fixture | P-0041 generated Schema와 4종 fixture |
| 2 | Project v2·DB migration | SQLite v1 input→v2 only writer |
| 3 | global/project reference 전환 | CheckoutId port/store와 protected root binding |
| 4 | 중단·재시도·rollback | checkpoint resume, second-run zero change, verified backup rollback |
| 5 | identity conflict·기존 데이터 보존 | `PROJECT_CHECKOUT_IDENTITY_CONFLICT`, backup tamper/data preservation tests |
| 6 | 첫 언어·adapter 선정 | Rust, private tree-sitter syntax, pinned rust-analyzer semantic |
| 7 | dependency·license·offline | exact Cargo lock, MIT 2건, locked offline metadata PASS |
| 8 | 실제 corpus·실패 fixture | 10,000-file syntax + invalid/macro/cfg/oversize fixtures |
| 9 | definition/reference 판정 | syntax false resolved 0, RA same-file confirmed, cross-file unresolved |
| 10 | crash·timeout·unsupported | parse/resource/LSP crash/LSP timeout/unavailable 분리 tests |
| 11 | 지원 범위·limitation matrix | M1 계약의 Rust matrix와 query `confirmed_empty` fail-closed |
| 12 | 대형 Git·non-Git·dirty 실측 | 두 10,000-file cohort의 5회 p95와 peak memory |
| 13 | file/byte/parallel limit | 200,000 files, 16 MiB/file, 8 GiB total, parallel 4와 semantic 별도 상한 |
| 14 | cache hit·보호·용량·eviction | 139,674,500-byte DPAPI store/hit, 256 MiB entry, 512 MiB project, 372,466,920-byte bounded retention |
| 15 | source change·cancel·crash | double-read source verification, cancel/current 보존, SQLite fault/restart integrity |
| 16 | full 승격 빈도 | single-file change가 4 partition만 무효화, 나머지 19,999/20,003 reuse |
| 17 | watcher 필요성 | 현 budget 안이므로 v0.1 미도입; hash truth를 대체하지 않는 후속 선택 기능 |

## 재현

```powershell
cargo run --release -p star-project --example m1_corpus -- --files 10000 --repetitions 5 --output target/m1-corpus/x64-reference-with-classification-20260720.json --projection-output target/m1-corpus/x64-projection-20260720.json
cargo run --release -p star-project --example m1_corpus -- --files 10000 --repetitions 5 --repository git_dirty --output target/m1-corpus/x64-git-dirty-reference-20260720.json
cargo run --release -p star-adapter-rust-index --example rust_syntax_corpus -- --files 10000 --repetitions 5 --output target/m1-corpus/x64-rust-syntax-20260720.json
cargo run --release -p star-state --example m1_cache -- --projection target/m1-corpus/x64-projection-20260720.json --repetitions 5 --output target/m1-corpus/x64-file-cache-20260720.json
```

fixture는 `%TEMP%` 아래 disposable root에만 생성한다. 다른 `D:\개발` 저장소는 읽거나 수정하지 않는다.
