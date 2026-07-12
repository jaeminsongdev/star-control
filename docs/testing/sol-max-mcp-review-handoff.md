# Sol Max MCP 독립 검토 인계문

아래 내용을 새 Codex thread의 첫 메시지로 그대로 사용한다. 모델은 **Sol Max**로 선택한다.

---

## 역할과 목표

`D:\개발\관제\Star-Control`의 현재 worktree에서 Star-Control MCP 범위를 독립 감사하라. 이전 구현자와 이전 감사 문서의 완료 결론을 신뢰하지 말고, 정본 계약·제품 코드·테스트·생성물·실행 증거를 서로 대조해 실제 구현 누락, 잘못된 구현, 보안 약화, 허위 양성 검증을 찾아라.

확인된 MCP 범위 결함이 있으면 최소 범위로 수정하고 관련 검증과 최종 full gate를 다시 실행하라. 단순히 더 좋아 보이는 설계로 계약을 바꾸지 말고, 문서끼리 실제 구현 불가능한 충돌이 있을 때만 근거를 남겨 최소 수정하라.

## 작업 범위

포함 범위는 다음뿐이다.

- MCP Gateway
- Controller Local IPC
- Live Tool Registry
- 외부 EXE 실행 Runtime
- 위 범위의 보안·복구·관리 CLI
- 계약 type·Schema·fixture·fuzz/property·170개 verification matrix
- 실제 Codex same-session, MCP Inspector, Windows 관리 CLI, x64·ARM64 증거

다음으로 범위를 넓히지 마라.

- Planner
- Router
- Codex App Server
- Worktree
- 전체 검증 플랫폼
- 로컬 AI 또는 다른 AI 제공자
- OpenAI API 직접 호출
- HTTP MCP
- 브라우저 UI
- 자체 작업 예약 기능

## 정본 문서 우선순위

exact 값은 아래 순서대로 판단한다.

1. `docs/contracts/mcp-implementation-contract.md`
2. `docs/contracts/tool-package-manifest-reference.md`
3. `docs/architecture/windows-tool-runtime.md`
4. `docs/testing/mcp-verification-matrix.md`
5. `docs/contracts/external-tool-registry.md`
6. `docs/contracts/local-ipc.md`
7. `docs/contracts/mcp-tools.md`
8. `docs/contracts/config-and-catalog.md`
9. `docs/contracts/errors-and-diagnostics.md`
10. `docs/contracts/events-and-state.md`
11. `docs/contracts/versioning-and-migrations.md`
12. `docs/architecture/repository-layout.md`
13. `docs/decisions/ADR-0005-MCP-구현-계약-동결.md`
14. `PLANS.md`

먼저 `docs/testing/mcp-completion-audit.md`를 구현자의 주장과 증거 색인으로 읽되, 정본보다 우선하거나 독립 검증의 대체물로 사용하지 마라.

## 작업 트리 보호 규칙

- 현재 worktree에는 이 MCP 구현과 기존 사용자 변경이 함께 있다. `git reset --hard`, `git checkout --`, 전체 파일 덮어쓰기, 삭제로 정리하지 마라.
- `legacy/`는 읽기 전용이며 수정하지 마라. 현재 설계 정본으로 인용하지 마라.
- 기존 사용자 미추적 `--check/`, `manifest.json`을 수정·이동·삭제하지 마라.
- 제품 코드를 `legacy/`에서 복사해 되살리지 마라.
- commit·push·공개 배포·외부 계정 변경·파일 삭제를 하지 마라.
- 공개 `origin`에 push하지 마라. ARM64 증거용 private 저장소는 실행 근거 열람에만 사용한다.
- raw shell, `cmd /c`, PowerShell script 문자열, PATH 검색을 외부 EXE 실행 우회로 추가하지 마라.
- 테스트를 통과시키기 위해 hash, trust, lane, Schema, approval, identity 또는 process-start 전 검사를 약화하지 마라.

## 현재 구현자가 주장하는 구현 상태

이 목록은 검토 대상인 주장이지, 검증 완료 사실로 전제하면 안 된다.

- `star-contracts`: MCP·IPC·Registry·Manifest·Trust·Cache·외부 process type, JSON Schema, valid/invalid/compatibility fixture, RFC 8785 JCS, SHA-256, required `star-control-core.toml`.
- `star-mcp.exe`: `rmcp 2.2.0`, local STDIO 전용, 고정 12-tool, `2025-11-25` 기준과 `2025-06-18` 호환, capability 비광고, Controller IPC 변환만 수행, stdout protocol 보호와 stderr JSONL log.
- Local IPC: current-user Named Pipe, DACL·remote 차단, DPAPI key, challenge-response HMAC, PID/image identity, negotiation, frame/backpressure/reconnect, verified Controller start, single instance/autostart, Gateway 종료 뒤 Operation 보존.
- Registry: release/user/project `tools.d`, exact Manifest v1, unknown/duplicate key 거부, source별 trust/update, 세 update policy, immutable snapshot, LKG, `ReadDirectoryChangesW`, stable-save, demand scan, deterministic search/cursor, 모든 hash, live TOML/EXE replacement·delete·conflict·revoke 처리.
- Runtime: `argv_v1`, `star_json_stdio_v1`, 모든 binding, input/output Schema, stdout/stderr 동시 drain, text/JSON/JSONL/binary, encoding/exit/artifact/output limit, timeout/progress/cancel, concurrency/lock/idempotency, EXE 자동 retry 금지, SecretRef/minimum env, cwd/state/temp, probe/version/architecture/Authenticode/integrity, 실행 직전 full hash, file identity lease, suspended `CreateProcessW`, handle allowlist, Operation별 Job Object, child tree/crash recovery, `trusted_desktop`, `appcontainer_adapter`, `restricted_token` 미지원.
- 관리 CLI: `star tools list|describe|status|validate|probe|trust|revoke|scaffold`와 Controller autostart `start|enable|disable|status`; 상태 변경은 Controller IPC만 사용.
- 검증: matrix 170개 ID, fake argv/JSON-STDIO/child tree/output flood/handle probe/AppContainer adapter, parser·IPC·JSON-STDIO fuzz/property, watcher·same-path·timestamp-preserving swap, actual Codex C001~C008, x64·ARM64.

## 반드시 exact 대조할 고정 surface

`tools/list`의 순서·이름·title·description·annotation·input/output Schema가 정본과 정확히 일치하는지 확인한다. 고정 tool은 다음 12개뿐이다.

1. `star_tool_search`
2. `star_tool_describe`
3. `star_tool_registry_status`
4. `star_tool_call_read_closed`
5. `star_tool_call_read_open`
6. `star_tool_call_write_closed`
7. `star_tool_call_destructive_closed`
8. `star_tool_call_write_open`
9. `star_tool_call_destructive_open`
10. `star_tool_operation_get`
11. `star_tool_operation_cancel`
12. `star_approval_resolve`

required core package는 다음 13개 ToolId·Controller command·lane과 정확히 일치해야 한다.

| ToolId | Controller command | lane |
|---|---|---|
| `star.core.goal.start` | `goal.start` | `write_closed` |
| `star.core.goal.answer` | `goal.answer` | `write_closed` |
| `star.core.plan.get` | `plan.get` | `read_closed` |
| `star.core.plan.update` | `plan.update` | `write_closed` |
| `star.core.run.continue` | `run.continue` | `destructive_open` |
| `star.core.status.get` | `goal.status` | `read_closed` |
| `star.core.goal.pause` | `goal.pause` | `write_closed` |
| `star.core.goal.resume` | `goal.resume` | `write_closed` |
| `star.core.goal.cancel` | `goal.cancel` | `destructive_open` |
| `star.core.evidence.get` | `evidence.get` | `read_closed` |
| `star.core.merge.status` | `merge.status` | `read_closed` |
| `star.core.handoff.get` | `handoff.get` | `read_closed` |
| `star.core.doctor` | `doctor.run` | `read_closed` |

새 외부 EXE·manifest·path·Schema 변경이 `star-mcp.exe` 수정, 재빌드, 재등록, MCP/Controller/Codex 재시작 없이 반영되는 구조인지 실제 코드 경로와 테스트로 확인하라.

## 우선 집중 감사 지점

### 계약과 생성물

- Rust type, JSON Schema, fixture, 문서, CLI help가 같은 required/default/enum/error/ID 값을 가지는지.
- Manifest parser가 nested table·inline table·array of table까지 unknown key와 duplicate key를 fail-closed하는지.
- JCS가 RFC 8785 edge case를 실제로 만족하고 hash별 포함·제외 필드가 정본과 일치하는지.
- `source_content_hash`, `manifest_hash`, `package_hash`, `descriptor_hash`, executable identity, Registry snapshot/cache hash가 의미상 혼동되지 않는지.
- generated Schema `--check`가 stale 파일을 실제로 탐지하며 test fixture가 제품 type을 우회하지 않는지.

### Gateway와 IPC

- Gateway가 TOML·외부 EXE·Registry·Operation 상태를 직접 읽거나 소유하지 않는지.
- initialize/version negotiation, 고정 instructions, capability 비광고, cursor 거부와 JSON-RPC/McpToolResult error 경계가 exact인지.
- stdout에 log/panic/child output이 섞일 수 없는지, stderr JSONL 64 KiB·redaction 계약을 지키는지.
- Named Pipe DACL, current-user/remote rejection, DPAPI key 저장 ACL, HMAC nonce/replay, peer PID 및 설치 image identity가 인증 전에 충분히 고정되는지.
- Gateway가 시작하는 Controller가 검증된 image인지, PID reuse·same-path replacement·junction/reparse·file swap에 취약하지 않은지.
- major/minor negotiation, maximum frame, bounded queues, backpressure, reconnect와 30초 long-poll이 서로 timeout을 잘못 공유하지 않는지.
- Gateway 재시작이 Controller single instance, Registry revision, durable Operation을 손상하지 않는지.

### Registry와 trust

- release/user/project precedence, source별 trust/update policy, `pinned_hash`·`version_compatible`·`follow_path` 의미가 exact인지.
- duplicate ToolId, replacement, deletion, conflict, trust revoke에서 active/candidate/status/effective snapshot 의미가 일치하는지.
- invalid candidate가 last-known-good와 durable cache를 오염하지 않는지.
- watcher overflow/missed event/stable-save와 request 전 demand scan이 결합되어 TOML·Schema·EXE 변경을 놓치지 않는지.
- 같은 path EXE 교체 및 timestamp/size 보존 바꿔치기가 file identity와 full hash로 검출되는지.
- search ordering, score tie-break, cursor JCS/base64url canonicality, `query_hash`·discovery hash binding과 stale 처리가 deterministic한지.
- revoke 후 status가 과거 ready snapshot을 잘못 노출하거나 기존 descriptor hash로 실행할 수 없는지.

### 외부 process Runtime과 복구

- 검사 순서가 demand scan → ToolId/readiness/trust → descriptor → lane → input Schema → arguments/idempotency → approval/scope → lock → dispatch이고 2~7 실패가 process 생성 전에 끝나는지.
- CLI argument binding 전체가 injection, empty/null/default, enum, path, repeated/list, temp-file edge case를 올바르게 처리하는지.
- JSON-STDIO handshake/request/progress/result/cancel의 frame/line/order/duplicate/unknown key와 terminal invariant가 fail-closed인지.
- stdout/stderr 동시 drain이 deadlock 없이 output limit, encoding, JSON/JSONL/binary, artifact path/size/hash를 정확히 처리하는지.
- timeout/cancel 시 Operation별 Job Object가 전체 child tree를 종료하며 Controller crash 뒤 orphan 복구가 실제로 가능한지.
- 자동 EXE retry가 없고 idempotency replay가 process 재실행과 혼동되지 않는지.
- SecretRef가 log/hash/env에 원문으로 새지 않고 environment와 inherited handles가 allowlist 최소값인지.
- probe/version/architecture/Authenticode/integrity 검증과 실행 직전 full hash, file identity lease, suspended `CreateProcessW` 사이에 TOCTOU 창이 없는지.
- outer Job 환경에서 fallback start가 fail-closed하면서, 이미 인증된 Controller 연결 경로가 보안 우회가 되지 않는지.
- `trusted_desktop`, `appcontainer_adapter`, unsupported `restricted_token` 분기가 계약대로이며 AppContainer capability·loopback·directory ACL·handle allowlist가 실제 Windows primitive로 enforce되는지.
- approval scope/hash/conditions, concurrency lock, idempotency record, output provenance, crash recovery terminal 상태가 재시작 뒤에도 일관적인지.

### 관리 CLI와 증거

- CLI read/write 모두 Controller IPC만 사용하고 직접 trust/cache/autostart state를 쓰지 않는지. 단, Controller 소유 autostart 구현과 정확히 구분하라.
- pagination, probe의 explicit 상태 기록, trust/revoke 효과, scaffold full-file pinned hash, status의 revoked state를 검증하라.
- test 이름 매핑만 보지 말고 170개 matrix row의 전제·행동·관측·실패 조건이 실제 test body와 제품 경로로 이어지는지 확인하라.
- skip·ignore·flaky·quarantine가 숨겨지지 않았는지, assertion이 fixture JSON 자기검증에 그치지 않는지 확인하라.

## 실제 증거와 현재 알려진 값

모든 JSON evidence는 raw 실행 파일 및 binary hash와 다시 대조하라.

- 완료 감사: `docs/testing/mcp-completion-audit.md`
- matrix: `docs/testing/mcp-verification-matrix.md`
- Codex: `apps/star-mcp/tests/evidence/codex-same-session-v1.json`
- Inspector: `apps/star-mcp/tests/evidence/mcp-inspector-v1.json`
- ARM64: `apps/star-mcp/tests/evidence/mcp-arm64-native-smoke-v1.json`
- performance: `apps/star-mcp/tests/evidence/mcp-performance-v1.json`
- management CLI: `apps/star-cli/tests/evidence/management-cli-smoke-v1.json`
- Codex fixture: `tools/codex-e2e-fixture.ps1`
- Inspector fixture: `tools/mcp-inspector-fixture.ps1`
- management CLI fixture: `tools/mcp-management-cli-fixture.ps1`
- ARM64 fixture/workflow: `tools/mcp-arm64-smoke.ps1`, `.github/workflows/mcp-windows-arm64.yml`

최근 x64 release 주장값:

- Gateway: `sha256:bbef0e97aae2eddda92ef34a0d100ef5bb74ff7593a36faddb9979fedbfa50ab`
- Controller: `sha256:d93d6e7060e77c8b2d9e8440ebccd5b177c98fcaa7701a1d2f5d1baf6e254cbd`

Codex same-session 주장값:

- Codex `0.144.0-alpha.4`
- C001~C007: Codex/Gateway/Controller PID `30140/21596/2844` 유지
- C008: Gateway `13664 → 12020`, Controller PID `4572`, Controller instance·Registry revision `4`·Operation 보존
- WindowsApps 원본 `codex.exe` 실행 ACL 문제를 ignored `target/` 격리 복사본으로 우회했으며 원본과 격리본 SHA-256이 동일하다는 주장

MCP Inspector 주장값:

- official `@modelcontextprotocol/inspector@0.22.0`
- Node `24.16.0`, npm `11.13.0`, transitive SDK `1.29.0`
- Inspector의 relative `package.json` CWD 문제 때문에 fixture가 격리 CWD를 사용함. 이것이 제품 결함을 숨기지 않는지 구분하라.

ARM64 주장값:

- private evidence repository: `jaeminsongdev/star-control-mcp-arm64-evidence-20260712`
- full gate run: `29188151232`; 전체 workflow는 첫 smoke에서 GitHub runner outer Job 아래 verified Controller fallback start가 fail-closed해 실패했지만 workspace test·clippy·Schema/matrix/format·native release 단계는 각각 통과
- smoke-only run: `29188629756`; prestarted Controller를 Gateway가 PID/image/HMAC로 검증한 뒤 Gateway→IPC→Registry→native ARM64 external EXE 성공
- runner: actual ARM64 `windows-11-arm`, Windows 11 25H2 build `26200.8655`, PE machine `0xaa64`

Autostart 주장값:

- HKCU enable/status/disable/status와 반복 idempotency를 실제 실행
- exact owned `REG_SZ`를 검사하고 최종적으로 값 없음 상태로 복구
- 남은 `star-controller` process 없음

## 숨기면 안 되는 미검증 항목

정확한 Windows 11 **24H2** baseline은 실행하지 못했다. local x64와 native ARM64 runner는 모두 Windows 11 25H2 build 26200이며 계약의 최소 build `>= 26100`은 통과했지만, 이것을 exact 24H2 실행 증거로 간주하지 마라. 24H2 hardware/VM 없이 이 항목을 통과로 바꾸거나 전체 외부 검증 완료로 포장하지 마라.

ARM64 full gate의 첫 smoke 실패와 smoke-only 성공을 하나의 성공 run처럼 합쳐 쓰지 말고, 각 run이 무엇을 입증하고 무엇을 입증하지 못하는지 분리하라. 특히 prestarted Controller와 Gateway verified fallback start의 coverage 차이를 검토하라.

## 먼저 실행할 검증

작업 트리를 먼저 기록하고, 변경이 필요한 경우 focused test 후 최종 full gate를 실행한다.

```powershell
git status --short
git diff --check
git diff -- legacy
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
```

긴 property/fuzz와 실제 fixture의 실행 여부·시간·skip을 숨기지 마라. 제품을 수정했으면 해당 binary를 참조하는 Codex·Inspector·performance·management CLI evidence를 재생성하고 raw SHA를 다시 고정하라.

## 요구하는 최종 보고

다음 순서로 간결하지만 근거 중심으로 보고하라.

1. `BLOCKER`, `MAJOR`, `MINOR`, `NIT` findings. 각 finding에 계약 조항, 제품 파일과 line, 재현 또는 test 근거, 실제 영향, 수정 여부를 포함한다.
2. 공통 계약, Gateway, IPC, Registry, Runtime, trust/security/recovery, CLI, 검증·evidence별 verdict.
3. 고정 MCP tool 12개와 core action 13개 exact 일치 여부.
4. matrix `expected=170`, `mapped=170`, `missing=[]`뿐 아니라 170행의 의미 coverage 판정과 실제 고유 test/binding 수.
5. actual Codex C001~C008, MCP Inspector, autostart, x64, native ARM64 증거의 진위·coverage 판정.
6. 미검증 항목과 남은 위험. exact 24H2는 별도 줄로 명시한다.
7. 최종 판정 하나: `APPROVE`, `APPROVE_WITH_NOTES`, `REQUEST_CHANGES`, `BLOCK`.
8. 수정했다면 변경 파일, 검증 명령, 통과/실패/skip 수, evidence 재생성 여부를 함께 적는다.

기존 완료 감사의 문장, matrix ID 연결, 체크인 evidence JSON 자체만으로 `APPROVE`하지 마라. 실제 제품 경로와 원시 실행 근거를 독립적으로 확인한 뒤 판정하라.
