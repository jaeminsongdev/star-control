# MCP 독립 감사 보고서 — 2026-07-12

> [!IMPORTANT]
> 이 문서는 2026-07-12 source와 required core 13개를 대상으로 한 역사적 감사 snapshot이다. current inventory는 17개이며 6개 action이 후속 Slice에서 owning handler·Schema를 얻었다. 아래 13개 수치와 당시 **BLOCK** 판정을 현재 상태로 다시 계산하지 않으며 current 상태는 [최종 구현 로드맵](../roadmap/final-implementation.md)과 root `PLANS.md`를 따른다.

## 판정 요약

최종 판정은 **BLOCK**이다. 고정 MCP 12-tool과 required core 13개의 ID·command·lane 표면은 exact 일치하지만, core 13개에 필요한 소유 command handler와 generated input·output Schema가 없다. 이전 구현은 이 action들을 `ready`로 노출한 뒤 실제 호출에서 process backend만 지원한다며 실패했다. 이번 감사에서는 허위 ready·invoke를 fail-closed `unavailable`로 바꿨지만, 제외 범위인 Planner·Goal application command를 임의로 구현하지 않았으므로 정본의 release 완료 조건은 여전히 충족하지 못한다.

기존 Codex·Inspector·ARM64 JSON은 기록된 과거 실행과 raw hash가 일치해 조작 증거는 찾지 못했다. 그러나 Codex·Inspector는 이전 Gateway `sha256:bbef…`와 Controller `sha256:d93d…`를 가리키며, 현재 통합 release의 `sha256:16798371…`·`sha256:80958676…` 증거가 아니다. 현재 binary로 다시 실행한 실제 Codex C001과 Inspector core search는 모두 ready core action 0개를 반환했다.

## 1. Findings

### BLOCKER

#### MCP-AUD-001 — required core 13개는 선언만 있고 실행 계약이 없다

- 계약: `docs/contracts/mcp-implementation-contract.md:482`, 특히 `:502`는 소유 command handler와 generated Schema mapping이 없으면 release build를 실패시키도록 요구한다.
- 제품: `catalog/tool-packages/star-control-core.toml:11`~`:153`은 exact 13개를 선언하지만 action별 input·output Schema가 없다. `apps/star-controller/src/main.rs:543`의 등록된 Controller command 집합은 비어 있고, 실제 runtime도 `:6221`~`:6225`에서 process backend 외 실행을 거부한다.
- 재현: 수정 전 official Inspector로 `star.core.goal.start`를 describe하면 빈 object input Schema와 null output Schema가 `ready`로 나왔고, invoke는 `TOOL_RUNTIME_UNAVAILABLE`/`Only process backends are available`로 끝났다. 수정 후 통합 branch의 Inspector run `target/final-inspector-20260712-1`과 실제 Codex thread `019f5634-3df4-70c3-9fcb-fcbdb92f3a16`의 release-ready `goal` search가 0개를 반환했다.
- 영향: C001의 “search·describe core action” 및 required core 업무 action 전체를 현재 제품으로 수행할 수 없다. 정본이 요구한 build-time release 차단도 구현되어 있지 않아 release build 자체는 통과한다.
- 수정: 부분 수정. `apps/star-controller/src/main.rs:545`~`:578`, `:604`~`:607`, `:3112`~`:3117`에서 owning Schema와 등록 handler가 없는 core action을 search/describe/invoke 모두 fail-closed `unavailable`로 전환했다. 실제 handler·소유 Schema 구현은 범위 밖 application 계약이 필요하므로 남아 있다.

### MAJOR

#### MCP-AUD-002 — Gateway initialize lifecycle가 JSON-RPC notification을 잘못 응답·수락했다

- 계약: `docs/contracts/mcp-implementation-contract.md:289`~`:318`, `:322`~`:330`의 JSON-RPC/MCP 상태·notification 경계.
- 제품·테스트: `apps/star-mcp/src/lib.rs:523`~`:689`, `apps/star-mcp/tests/stdio_supervisor.rs:37`, `:71`.
- 재현: 수정 전 pre-initialize notification에 `id:null` response를 쓰고, `id`가 붙은 request-shaped `notifications/initialized`가 Ready 전이를 일으켰다. 두 failure-first 통합 테스트가 수정 전에 실패했다.
- 영향: compliant host notification에 잘못 응답하고, lifecycle 전이를 request로 위조할 수 있었다.
- 수정: 완료. `Forward`/`Respond`/`Ignore` 결정을 분리해 notification은 silent 처리하고 `notifications/initialized`의 request shape를 거부했다. `stdio_supervisor` 5/5 통과.

#### MCP-AUD-003 — IPC handshake의 인증·PID·nonce·negotiation binding이 불완전했다

- 계약: `docs/contracts/local-ipc.md:31`~`:80`, `:213`~`:216`.
- 제품·테스트: `crates/adapters/star-ipc/src/lib.rs:154`, `:285`~`:296`; `crates/adapters/star-ipc/src/client.rs:152`~`:215`, `:265`; 관련 `star-ipc` unit tests.
- 재현: 수정 전 client HMAC 검증 전에 authenticated protocol mismatch를 생성했고, nonce exact 32-byte canonicality와 duplicate protocol version을 거부하지 않았으며, verified pipe server PID를 `challenge.server_pid`와 대조하지 않았다. Welcome의 challenge instance/nonce 및 exact supported versions binding도 부족했다.
- 영향: pre-auth 정보 노출, replay·ambiguity 허용, verified pipe owner와 authenticated challenge의 identity 분리 가능성이 있었다.
- 수정: 완료. HMAC 선검증, canonical nonce/version 검증, actual pipe PID binding, Welcome/handshake-error exact binding을 추가했다. `star-ipc --lib` 23/23 통과.

#### MCP-AUD-004 — Operation MCP 결과가 Controller 내부 process/file identity를 노출했다

- 계약: `docs/contracts/mcp-implementation-contract.md:253`~`:265`, `docs/contracts/events-and-state.md:157`~`:175`, `docs/contracts/local-ipc.md:105`.
- 제품·테스트: `apps/star-mcp/src/main.rs:347`~`:407`; `apps/star-controller/src/operation_store.rs:45`~`:92`, `:115`~`:127`.
- 재현: 기존 actual Codex raw `operation_get`은 `process_id`, creation time, Job ID, executable volume/file ID·mtime, private event detail을 MCP에 그대로 반환했다.
- 영향: authenticated local client라도 MCP에 필요 없는 process·file identity와 내부 복구 상태가 노출됐고, durable record에 goal/run/stage scope와 output provenance가 빠졌다.
- 수정: 완료. Gateway allowlist projection으로 내부 identity/event detail을 제거하고 redacted phase/progress만 노출했다. Operation create/snapshot에 optional goal/run/stage와 output provenance를 영속화했다. redaction·durability 회귀 테스트 통과.

#### MCP-AUD-005 — requested timeout과 JSON-STDIO deadline이 서로 달랐다

- 계약: `docs/contracts/mcp-implementation-contract.md:232`~`:237`.
- 제품·테스트: `apps/star-controller/src/main.rs:1963`~`:1993`, `:6357`~`:6411`, `:7600`~`:7624`.
- 재현: process timeout은 요청값과 descriptor 상한의 minimum을 사용했지만 JSON-STDIO `context.deadline_at`은 descriptor maximum을 사용했다.
- 영향: adapter가 보는 deadline 이후에도 process가 실행되거나, Controller timeout이 protocol context와 불일치할 수 있었다.
- 수정: 완료. 하나의 `process_timeout_ms`를 process spec과 protocol deadline에 함께 사용한다.

#### MCP-AUD-006 — McpToolResult invariant와 고정 입력 byte 경계가 advertised Schema와 달랐다

- 계약: `docs/contracts/mcp-implementation-contract.md:104`~`:106`, `:227`~`:237`, `:289`~`:318`.
- 제품·테스트: `crates/foundation/star-contracts/src/fixed_mcp.rs:454`~`:650`, `:691`~`:695`; `crates/foundation/star-contracts/tests/contracts.rs:755`; `apps/star-mcp/src/main.rs:284`~`:344`, `:410` 이후.
- 재현: accepted/approval/question/blocked/error cross-field invariant가 output Schema와 runtime boundary에서 충분히 강제되지 않았다. search는 trim한 문자열 길이만 검사해 wire query 256자 초과를 받을 수 있었고, call 4 MiB 제한은 inner `arguments`가 아니라 wrapper 전체 byte에 적용됐다.
- 영향: tools/list Schema와 실제 accept/reject가 달랐고 malformed authenticated Controller response가 MCP structuredContent로 나갈 수 있었다.
- 수정: 완료. status별 Schema invariant와 runtime semantic validator, original query 길이 및 inner canonical arguments 4 MiB 경계를 추가했다. generated 12 result Schema와 manifest를 갱신했다.

#### MCP-AUD-007 — ARM64 full-run 실패 원인이 evidence 문구만으로 입증되지 않는다

- 계약: error normalization은 `docs/contracts/mcp-implementation-contract.md:318`~`:320`; outer Job fallback은 `docs/architecture/windows-tool-runtime.md`의 verified start 경계.
- 제품·증거: `crates/adapters/star-ipc/src/client.rs:274`~`:281`, `apps/star-mcp/src/main.rs:474`~`:481`; ARM64 runs `29188151232`, `29188629756`.
- 재현: full run raw 실패는 `IPC server identity does not match the installed Controller`뿐이었다. 이전 mapping은 `OuterJobDenied`, generic start failure, identity mismatch를 모두 identity 오류로 정규화할 수 있어 “outer Job breakaway fail-closed” 원인을 raw 문구만으로 구분할 수 없었다. smoke-only run은 `-PrestartController` 경로라 Gateway verified fallback start를 실행하지 않았다.
- 영향: 두 run을 합쳐 native full gate와 fallback start가 모두 성공했다고 주장할 수 없다.
- 수정: 코드 mapping 완료. `OuterJobDenied|Start`는 unavailable, install/image/lease mismatch만 identity mismatch로 분리했다. 현재 source의 native ARM64 재실행은 하지 못했으므로 evidence는 미갱신이다.

#### MCP-AUD-008 — evidence test가 raw 실행을 읽지 않아 stale JSON도 통과한다

- 계약: `docs/testing/mcp-verification-matrix.md:253`~`:276`은 actual host/raw 실행을 요구한다.
- 제품·테스트: `apps/star-mcp/tests/codex_same_session_evidence.rs:4`, `:113`~`:120`; `mcp_inspector_evidence.rs:4`, `:91`~`:112`; `mcp_arm64_native_evidence.rs:4`, `:107`~`:114`는 체크인 JSON의 shape/hash 문자열만 검사한다. `tools/matrix-check/src/main.rs:99`~`:130`은 marker와 test attribute만 확인한다.
- 재현: 현재 release hash가 바뀌어도 과거 Codex·Inspector·ARM64 JSON test는 그대로 통과했다. raw path가 없는 clean checkout에서도 raw byte를 다시 읽거나 binary를 hash하지 않는다.
- 영향: workspace gate가 current binary의 actual Codex/Inspector/ARM64 성공을 의미하는 것처럼 보이는 허위 양성이다.
- 수정: 부분 수정. 이번 감사에서 raw 파일과 old JSON hash를 직접 대조하고 performance·management evidence를 current binary로 재생성했다. Codex·Inspector는 current binary에서 core blocker가 재현돼 성공 JSON을 갱신하지 않았다. CI에서 raw artifact와 binary를 공급·검증하는 별도 evidence ingestion gate는 남아 있다.

### MINOR

#### MCP-AUD-009 — 일부 fixed outputSchema의 nested object가 exact shape를 강제하지 않는다

- 계약: `docs/contracts/mcp-implementation-contract.md:104`~`:106`, `:187`~`:217`, `:253`~`:287`.
- 제품: `crates/foundation/star-contracts/src/fixed_mcp.rs`의 describe `isolation`, `concurrency`, `timeout`, `output`, `progress`, `cancel`, registry status `controller/items/watcher`, Operation nested object 일부가 단순 `type=object/array`다.
- 재현: top-level additionalProperties와 status invariant는 강제하지만 해당 nested object에 임의 field를 넣은 instance도 generated outputSchema 자체는 받아들인다.
- 영향: 현재 Controller 생성 경로는 정해져 있으나 Schema drift detector가 fixed result의 모든 nested 계약 변화를 잡지 못한다.
- 수정: 부분 수정. status invariant와 sensitive Operation projection은 강화했다. 정본 field type이 문서에 완전히 열거되지 않은 nested structure까지 임의로 동결하지 않았으며, 후속 owner Schema가 필요하다.

### NIT

없음. 계약·보안·증거 판정에 영향 없는 스타일 변경은 finding으로 만들지 않았다.

## 2. 범위별 verdict

| 범위 | verdict | 근거 |
|---|---|---|
| 공통 계약/type/Schema/fixture | REQUEST_CHANGES | parser/JCS/generated check는 통과했으나 core owning Schema 부재와 일부 nested result Schema 완전성 부족 |
| MCP Gateway | PASS_AFTER_FIX | 고정 12-tool, local STDIO, capability 비광고, lifecycle/notification, stdout/stderr 경계 focused·workspace test 통과 |
| Local IPC | PASS_AFTER_FIX | current-user pipe, HMAC/nonce/PID/image/negotiation/backpressure/long-poll 테스트 통과; native fallback 재실기 필요 |
| Live Registry | PASS_WITH_NOTES | watcher/demand scan/LKG/revoke/replacement/search cursor 제품 테스트 강함; required core는 의도적으로 unavailable |
| 외부 EXE Runtime | PASS_WITH_NOTES | no-shell CreateProcessW, hash/lease/Job/handle/output/cancel/AppContainer 테스트 통과; current native ARM64는 cross-build만 수행 |
| trust/security/recovery | PASS_AFTER_FIX | pre-dispatch, trust revoke, approval, idempotency, secret redaction, recovery 테스트 통과; Operation canonical completeness note 존재 |
| 관리 CLI | PASS | current release로 validate→trust→probe→revoke→scaffold→autostart 실제 실행 및 원상복구 |
| 검증·evidence | FAIL | current Codex C001·Inspector core success 재현 실패, ARM64 current binary native 미실행, evidence tests self-attestation |

## 3. 고정 surface exact 대조

- MCP 고정 tool: **12/12 exact 일치**. 순서·name·title·description·annotations·input/outputSchema 존재는 current official Inspector tools/list 단계와 `fixed_mcp_surface_is_exactly_twelve_tools_in_contract_order`가 확인했다. 추가 tool과 advertised resources/prompts/logging/completions/tasks capability는 없다.
- required core: **13/13 ToolId·Controller command·lane exact 일치**. `required_release_core_package_declares_exactly_thirteen_owned_actions`가 확인했다.
- required core 실행 계약: **0/13 구현 확인**. 등록 handler와 fully resolved owning input/output Schema mapping이 없어 current search readiness는 13개 모두 unavailable이다.
- 외부 EXE·manifest·path·Schema live 반영: **구조·테스트 통과**. Controller watcher+demand scan이 소유하고 Gateway 재빌드·재등록 없이 descriptor가 바뀐다. performance fixture의 30회 stable add/path-change에서 descriptor hash가 매회 변경됐다.

## 4. Verification matrix 의미 coverage

`star-matrix-check` 결과는 `expected=170`, `mapped=170`, `missing=[]`이다. 상세 집계는 다음과 같다.

| prefix | ID | binding | prefix 내 고유 test |
|---|---:|---:|---:|
| C | 8 | 10 | 3 |
| G | 24 | 39 | 18 |
| H | 15 | 23 | 19 |
| I | 16 | 27 | 19 |
| M | 25 | 37 | 23 |
| O | 11 | 11 | 10 |
| P | 32 | 46 | 38 |
| R | 21 | 27 | 21 |
| S | 18 | 27 | 22 |
| 합계 | 170 | 247 | 전체 고유 155 |

이 수치는 170개 독립 test를 뜻하지 않는다. Codex 한 test가 C001~C008 8행, Inspector 한 test가 9행, ARM64 한 test가 10행을 묶는다. 특히 이 세 test는 raw 파일이나 current binary를 열지 않고 체크인 JSON을 자기검증한다. 반면 Registry·process runtime·IPC의 다수 행은 실제 제품 함수와 Windows primitive를 실행하므로 의미 coverage가 상대적으로 강하다. `#[ignore]`, `#[should_panic]`, flaky, quarantine marker는 없었다.

release property/fuzz는 실제로 다음을 실행했다.

- `star-contracts`: 2/2, 600.00초, 실패·ignore 0
- `star-ipc`: 1/1, 600.00초, 실패·ignore 0
- `star-controller`: 5/5, 600.25초, 실패·ignore 0

## 5. 실제 evidence 진위·coverage

| evidence | 진위 | current coverage 판정 |
|---|---|---|
| Codex C001~C008 | old raw 3개 byte/line/SHA와 JSON이 일치하고 Codex isolated copy hash도 원본과 일치해 과거 실행은 genuine | FAIL/STALE. old Gateway·Controller hash. current actual Codex C001 release-ready goal search는 0개. C002~C008 current binary 미재생성 |
| MCP Inspector 0.22.0 | exact package/CLI/lock integrity와 old raw hash는 genuine; 격리 CWD workaround는 upstream relative lookup 문제이며 제품 결함을 숨기지 않음 | PARTIAL/FAIL. current tools/list·registry status는 실행됐으나 core ready search assertion에서 실패 |
| management CLI/autostart | current release로 실제 HKCU enable/status/disable/status와 idempotency 수행 | PASS. 최종 Run 값 없음, 잔존 star process 0, evidence 갱신 |
| x64 | workspace test/clippy/schema/matrix/fmt/release 및 performance/CLI 실기 | PASS. Gateway `sha256:16798371…`, Controller `sha256:80958676…` |
| native ARM64 | old private run/artifact 자체는 genuine | STALE/SPLIT. full `29188151232`는 smoke 실패, smoke-only `29188629756`은 prestarted Controller 성공. current source는 ARM64 cross-build만 통과 |

## 6. 미검증 항목과 남은 위험

- **exact Windows 11 24H2: 미검증.** local x64와 기존 native ARM64 evidence는 Windows 11 25H2 build 26200이다. 최소 build `>=26100`만 충족한다.
- current source의 native ARM64 실제 실행과 Gateway verified fallback start는 미검증이다.
- current binary의 Codex C002~C008은 core blocker 확인 뒤 재생성하지 않았다.
- core 13개의 owning command type·Schema·handler 구현과 build-time release failure gate가 없다.
- 일부 fixed result nested Schema 및 canonical OperationSnapshot field는 owner 계약 보강이 필요하다.
- checked-in evidence test는 CI artifact provenance를 독립 검증하지 못한다.

## 7. 최종 판정

**BLOCK**

보안상 허위 ready와 확인된 경계 결함은 최소 수정으로 fail-closed 처리했지만, required core 13개가 실행 불가능하므로 MCP release 완료를 승인할 수 없다.

## 8. 감사 중 수정·검증·evidence 상태

감사 delta의 주요 변경 파일은 다음이다.

- Gateway lifecycle/result/redaction: `apps/star-mcp/src/lib.rs`, `apps/star-mcp/src/main.rs`, `apps/star-mcp/tests/stdio_supervisor.rs`
- IPC handshake/error mapping: `crates/adapters/star-ipc/src/lib.rs`, `crates/adapters/star-ipc/src/client.rs`
- core readiness/runtime/Operation: `apps/star-controller/src/main.rs`, `apps/star-controller/src/operation_store.rs`
- fixed input/result contract: `crates/foundation/star-contracts/src/fixed_mcp.rs`, `crates/foundation/star-contracts/tests/contracts.rs`, generated MCP result Schema와 `specs/schemas/manifest.json`
- current 성공 evidence: `apps/star-mcp/tests/evidence/mcp-performance-v1.json`, `apps/star-cli/tests/evidence/management-cli-smoke-v1.json`
- 감사 기록: 이 문서, `docs/testing/mcp-completion-audit.md`, `PLANS.md`

현재 full gate 결과:

- workspace test, clippy `-D warnings`, Schema `--check`, matrix+details, format: 통과
- x64 release, ARM64 cross-build: 통과
- Inspector evidence test·ARM64 evidence test·management CLI evidence test: 체크인 JSON assertion은 통과하나 Inspector/ARM64 current 실기를 뜻하지 않음
- current performance fixture: 통과, evidence 재생성
- current management CLI/autostart fixture: 통과, evidence 재생성
- current Inspector fixture: 실패 — `Inspector search call did not return ready release actions`
- current actual Codex C001: 실패 — ready release goal action count 0
- Codex·Inspector·native ARM64 성공 evidence: blocker/stale 때문에 재생성하지 않음
