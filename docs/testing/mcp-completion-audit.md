# MCP 구현 완료 감사

> [!WARNING]
> 이 문서는 구현자의 완료 주장과 2026-07-12 당시 required core 13개 evidence 색인이다. 독립 감사에서 handler·owning Schema 부재와 stale/self-attested evidence가 확인되어 당시 완료 결론은 폐기됐다. current inventory는 17개이며 6개 action이 후속 Slice에서 구현됐으므로 이 역사 문서의 수치와 판정을 현재 상태로 승격하지 않는다. current 상태는 [최종 구현 로드맵](../roadmap/final-implementation.md)과 root `PLANS.md`를 따른다.

## 상태와 판정 경계

이 문서는 MCP Gateway, Local IPC, Live Tool Registry, 외부 EXE Runtime, 관련 보안·복구·관리 CLI만 감사한다. Planner, Router, Codex App Server, Worktree와 전체 검증 플랫폼은 이 판정에 포함하지 않는다.

아래 내용은 독립 감사 전 구현자가 기록한 상태다. 과거 raw 실행의 진위와 현재 source/binary의 release 적합성은 별개이며, 이 문서만으로 완료를 판정하지 않는다.

완료 판정은 다음 증거 강도를 구분한다.

| 판정 | 의미 |
|---|---|
| 자동 통과 | 현재 worktree에서 실제 assertion이 실행됨 |
| 실제 Windows smoke | release binary를 격리 설치해 실제 process·Named Pipe·Codex host로 실행함 |
| cross-build | target compile/link만 확인했으며 실제 hardware 실행은 아님 |
| 외부 대기 | package 설치 승인, 별도 hardware·OS 또는 독립 reviewer가 필요함 |

## 요구사항별 구현·증거

| 요구사항군 | 제품·계약 소유 위치 | 강한 자동 증거 | 실제 host 증거 | 현재 판정 |
|---|---|---|---|---|
| 공통 계약·Schema·fixture·JCS | `crates/foundation/star-contracts/`, `specs/schemas/`, `specs/fixtures/`, `catalog/tool-packages/star-control-core.toml` | `contracts.rs`, contract property fuzz, Schema generator `--check`, MCP-H·M | generated 파일 drift check | 구현·자동 통과 |
| 고정 12-tool Gateway | `apps/star-mcp/`, `fixed_mcp.rs` | MCP-G001~G024, STDIO supervisor, `rmcp 2.2.0` duplex client | 실제 Codex MCP host C001~C008, 공식 MCP Inspector 0.22.0 | 구현·자동/실기 통과 |
| authenticated Local IPC | `crates/adapters/star-ipc/`, Controller accept loop | MCP-I001~I016, Named Pipe·DACL·DPAPI·HMAC·identity·backpressure·long-poll tests | x64 Gateway/CLI verified start·재연결, ARM64 existing Controller PID·설치 image·HMAC 검증 | 구현·자동/실기 통과 |
| Live Registry·LKG·watcher·search | `registry_runtime.rs`, `registry_watcher.rs`, `trust_store.rs` | MCP-R001~R021, MCP-H002~H015, Registry property fuzz | Codex C002~C005, 512-action performance fixture, 관리 CLI trust/revoke | 구현·자동/실기 통과 |
| argv·JSON-STDIO·Win32 Runtime | `process_runtime.rs`, `process_runtime/win32_launcher.rs`, Controller runtime handlers | MCP-P001~P032, MCP-O001~O011, fake argv/JSON-STDIO/child-tree/flood/handle/AppContainer tests | Codex C004·C006·C007, 관리 CLI probe/scaffold, native ARM64 external EXE | 구현·자동/실기 통과 |
| 보안·복구·approval | trust/approval/operation store, Authenticode, AppContainer·Job code | MCP-S001~S018, durable duplicate-key·recovery·ACL tests | approval-before-side-effect, cancel·Gateway restart 보존 | 구현·자동/실기 통과 |
| 관리 CLI | `apps/star-cli/`, Controller IPC management handlers | exact help/parser/exit-code tests, `management_cli_evidence.rs` | validate→trust→list/describe→probe→revoke→scaffold→Controller start/status, HKCU autostart enable/status/disable/status·원상복구 | 구현·자동/실기 통과 |

## 170개 matrix 연결

정본 행 수와 자동 test 연결 수는 다음과 같다.

| 구분 | ID 수 |
|---|---:|
| Gateway G | 24 |
| Hash·search H | 15 |
| IPC I | 16 |
| Manifest M | 25 |
| Operation O | 11 |
| Process P | 32 |
| Registry R | 21 |
| Security S | 18 |
| 실제 Codex C | 8 |
| 합계 | 170 |

`cargo run --locked -p star-matrix-check -- --details`가 170개 ID의 모든 test binding을 출력한다. 현재 170개 ID는 155개 고유 Rust test의 247개 binding에 연결된다. 한 ID를 여러 계층이 검증하는 경우 첫 test만 숨기지 않는다. `#[ignore]`, `#[should_panic]`, flaky·quarantine marker가 붙은 matrix test는 gate 실패다.

## 재감사에서 수정한 실제 결함

다음 항목은 단순 문서 보강이 아니라 기존 구현의 오류를 재현한 뒤 수정한 것이다.

- `operation.get wait_ms`보다 짧은 IPC response timeout
- CLI `list`·`status`의 내부 pagination 누락과 512-action 전체 조회 불일치
- 전역 manifest/action budget을 source별로 우회할 수 있던 scan
- release catalog의 임의 TOML 수용과 exact integrity 누락
- 전역 ToolId 충돌·replacement·revoke 시 잘못된 owner 노출
- search/status cursor의 snapshot·filter binding 부족
- invoke 경로의 의도하지 않은 자동 EXE probe
- durable trust·approval·operation JSON의 duplicate-key 허용
- approval resolve provenance가 반복 resolve에서 덮어써지던 문제
- Controller-private artifact ref의 `sha256:sha256:` 이중 prefix
- revoke 뒤 status가 candidate를 계속 `ready`로 표시하던 문제
- PE header positional read 뒤 scaffold가 EXE tail만 hash해 잘못된 `pinned_hash`를 생성하던 문제
- active package의 explicit probe 성공·실패가 `last_probe_at`·diagnostic revision에 기록되지 않던 문제
- process 시작 시 Operation status는 `running`인데 public progress phase는 내부명 `process_created`만 노출하던 문제

각 수정은 관련 unit/integration regression과 실제 관리 CLI smoke에서 다시 확인한다. test를 통과시키기 위해 hash, trust, lane, Schema, approval 또는 process-start 전 검사를 약화하지 않는다.

## 실제 증거 파일

| 증거 | 위치 | 검증 내용 |
|---|---|---|
| Codex same-session | `apps/star-mcp/tests/evidence/codex-same-session-v1.json` | Controller `sha256:d93d6e70…`, C001~C007 PID 30140/21596/2844 불변, C008 Gateway 13664→12020와 Controller 4572·Registry 4·Operation 보존 |
| MCP Inspector | `apps/star-mcp/tests/evidence/mcp-inspector-v1.json` | exact Inspector 0.22.0 integrity, 고정 12-tool metadata·Schema, registry status·search authenticated IPC와 raw byte·line·SHA-256 |
| native Windows ARM64 | `apps/star-mcp/tests/evidence/mcp-arm64-native-smoke-v1.json` | native workspace test·clippy·release, Arm64/`0xaa64`, Gateway·IPC·Registry·external EXE와 outer Job fail-closed/raw SHA |
| performance | `apps/star-mcp/tests/evidence/mcp-performance-v1.json` | 동일 Controller hash, 30회 latency, 512 action, working set, lane preflight |
| 관리 CLI | `apps/star-cli/tests/evidence/management-cli-smoke-v1.json` | 동일 Controller hash, 실제 release CLI management flow, scaffold exact hash, HKCU autostart exact owned command·idempotency·원상복구 |
| Codex fixture | `tools/codex-e2e-fixture.ps1` | isolated manifest·EXE 변경, PID·marker 증거 |
| Inspector fixture | `tools/mcp-inspector-fixture.ps1` | exact npm 설치 tree와 격리 official CLI STDIO 재현 |
| performance fixture | `tools/mcp-performance-fixture.ps1` | budget 표본과 capacity 측정 |
| management CLI fixture | `tools/mcp-management-cli-fixture.ps1` | 새 RunRoot에서 management flow 재현 |
| ARM64 fixture·workflow | `tools/mcp-arm64-smoke.ps1`, `.github/workflows/mcp-windows-arm64.yml` | native `windows-11-arm` 재현과 artifact 수집 |

## 외부·승인 gate

| 항목 | 현재 확인 | 완료로 선언하지 않는 이유 | 재개 조건 |
|---|---|---|---|
| Windows 11 24H2 baseline | local x64는 25H2 build 26200.8457, native ARM64 runner는 25H2 build 26200.8655이며 둘 다 계약의 `build >= 26100` smoke를 통과했다. | matrix가 명시한 24H2 baseline OS 자체의 회귀를 25H2 결과로 대체하지 않음 | Windows 11 24H2 x64 또는 ARM64 smoke |
| Sol Max 독립 검토 | 2026-07-12 수행, `BLOCK` | required core 13개 실행 계약 부재, current Codex·Inspector 재현 실패 | [독립 감사 보고서](mcp-independent-audit-2026-07-12.md) findings 해소 후 재감사 |

이 외부 항목은 제품 구현 blocker와 구분한다. 로컬에서 안전하게 실행 가능한 작업이 남아 있으면 외부 gate를 이유로 중단하지 않는다.

## 최종 재현 명령

```powershell
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run --locked -p star-schema-gen -- --check
cargo run --locked -p star-matrix-check
cargo run --locked -p star-matrix-check -- --details
cargo fmt --all -- --check
cargo build --workspace --release --locked
cargo build --workspace --release --target aarch64-pc-windows-msvc --locked
cargo test -p star-mcp --test mcp_inspector_evidence --locked
cargo test -p star-mcp --test mcp_arm64_native_evidence --locked
cargo test -p star-cli --test management_cli_evidence --locked
git diff --check
git diff -- legacy
```

release fuzz는 contract parser/JCS, IPC frame, Controller JSON-STDIO·Schema·Registry state machine을 target별 600초로 실행한다. 실제 Codex·Inspector·performance·관리 CLI fixture는 서로 다른 새 `target/` RunRoot에서 실행하며 raw evidence와 체크인 JSON의 binary SHA-256을 대조한다.

2026-07-12 최종 로컬 gate는 새 ARM64 evidence test를 포함한 workspace test, clippy `-D warnings`, Schema `--check`, matrix 170/170·155개 test·247 binding, format, x64 release와 ARM64 cross-build를 모두 통과했다. Controller property/fuzz는 5개 test를 601.12초 실행해 실패·ignore 0개로 끝났고, Codex·Inspector·performance·관리 CLI 증거는 x64 Controller `sha256:d93d6e70…` 및 해당 raw byte·line·SHA-256과 일치한다. Native ARM64는 full gate의 test·clippy·drift·release 단계와 후속 smoke run을 합쳐 통과했으며 raw artifact hash를 별도 evidence에 고정했다.

## Sol Max 집중 검토 지점

- revoke·replacement·ToolId collision에서 effective snapshot과 status state가 같은 의미인지
- file identity lease, full hash, Authenticode, integrity file과 suspended `CreateProcessW` 사이 TOCTOU 경계
- AppContainer capability·loopback·directory ACL과 handle allowlist
- approval scope·idempotency·provenance와 crash recovery의 terminal 불변성
- watcher 누락·stable save·same-path replacement와 LKG/cache 복구
- 실제 Codex·performance·관리 CLI evidence의 binary hash와 raw 실행 자료 일치
- 외부 gate를 통과하지 않은 상태에서 전체 완료로 잘못 선언하지 않는지
