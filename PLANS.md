# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control repository는 현재 스캐폴드와 정본 설계 문서 상태다.
- v0 runtime 구현 스택은 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id는 `fake-default`로 결정했다.
- v0 Star Sentinel P0 rule set은 5개 핵심 rule로 결정했다.
- 완전 구현 기본값은 `docs/decisions/0005-full-implementation-defaults.md`를 기준으로 한다.
- 전체 milestone은 `docs/implementation/complete-implementation-roadmap.md`를 기준으로 한다.
- Codex 구현 착수용 EPIC brief는 `docs/implementation/briefs/`를 기준으로 한다.
- `PLANS.md`는 현재 상태만 남기는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- `docs/decisions/0005-full-implementation-defaults.md`의 기본 Rust dependency set 밖의 의존성 추가, Cargo 외 package manager 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.
- 외부 보조 자료를 다시 붙이지 않고 이 repository 안의 정본 파일을 기준으로 작업한다.

### 이미 끝난 것

- Star-Control monorepo 스캐폴드, schema, contract, config, registry, provider/tool manifest를 정리했다.
- Star Sentinel 명칭, policy, schema, template, output contract를 정리했다.
- `PLANS.md`와 plan-ledger 운영 기준을 bounded snapshot 방식으로 정리했다.
- v0 runtime stack을 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id를 `fake-default`로 통일했다.
- Star Sentinel v0 P0 scope와 E09a~E09d 구현 분할을 정리했다.
- 로컬 contract check entrypoint를 `python scripts/ci/run_all.py`로 추가했다.
- E01~E11 Codex 구현 착수용 brief를 추가했다.
- 완전 구현 기본값과 M0~M9 milestone 기준을 문서화했다.
- 검증/AGENTS 효율 병목을 정리하고, `scripts/test.ps1`을 정본 contract runner로 연결했다.
- E01 최소 Cargo workspace와 `packages/star-control-schema` runtime validator를 추가했다.
- E02 `packages/star-control-state` file-based StateStore를 추가했다.
- E03 Artifact Layout Writer helper를 StateStore에 추가했다.
- E04 `packages/star-control-provider` Provider Registry를 추가했다.
- E05 `FakeProviderAdapter`와 provider output writer 연결을 추가했다.
- E06 `packages/star-control-router` RouterEngine을 추가했다.
- E07 `packages/star-control-execution` ExecutionEngine을 추가했다.
- E08 `packages/star-control-cli` CLI fake flow를 추가했다.
- E09a `packages/star-sentinel` P0 evaluator를 추가했다.
- E09b Star Sentinel diagnostics/gate artifact writer를 추가했다.
- E09c Star Sentinel review-pack writer를 추가했다.
- E09d Star Sentinel ledger writer와 selfcheck를 추가했다.
- E10 `packages/star-control-validation` ValidationEngine을 추가했다.
- E11 `packages/star-control-cli/tests/v0_fake_flow.rs` integration smoke를 추가했다.
- M5 local process provider command/sandbox/timeout/cancel 정책 문서를 추가했다.
- M5b `LocalProcessProviderAdapter` 기본 command policy, stdout/stderr capture, timeout result를 추가했다.
- M5c `ExecutionEngine` provider selection에 local process adapter 연결을 추가했다.
- M5d CLI `run --provider <id> --provider-instance <path>` local process 실행 경로를 추가했다.
- M5e local process cancel state model과 RunState `CANCELLED` 전이를 추가했다.
- M5f local process forbidden action evidence marker와 RunState `BLOCKED` 전이를 추가했다.
- M5g local provider conformance fixture를 추가해 M5 runtime exit criteria를 묶어 검증한다.
- M6a cloud provider preflight를 추가해 credential/privacy/cost artifact 계약을 runtime 경로에 연결했다.
- M6b cloud CLI transport를 추가해 preflight 통과 CLI provider가 command vector로 실행되게 했다.
- M6c provider output conformance checker를 추가해 cloud provider artifact path/ref/file existence와 privacy/cost sidecar를 검증한다.
- M6d OpenAI-compatible API response parser를 추가해 Responses API와 Chat Completions JSON fixture를 정규화한다.
- M6e OpenAI-compatible request builder를 추가해 Responses API와 Chat Completions request URL/body fixture를 credential 없이 생성한다.
- M6f cloud API offline fixture integration을 추가해 prepared request와 raw response fixture parse를 같은 runtime path에서 검증한다.
- M6g cloud API transport boundary를 추가해 `http-transport-plan.json`에 method/url/header policy/credential reference kind를 live call 없이 기록한다.
- M6h cloud API live approval gate를 추가해 explicit live request를 `live-transport-approval.json`과 RunState `BLOCKED`로 정규화한다.
- M7a CLI approve/cancel/resume control commands를 추가해 daemon/API 전제 조건을 file-based StateStore에서 검증한다.
- M7b daemon queue skeleton을 추가해 config root 아래 daemon state와 StateStore job 참조 queue를 검증한다.
- M7c API read-only service를 추가해 daemon state와 StateStore job/events/report를 schema-valid API envelope으로 조회한다.
- 병렬 Rust 테스트에서 provider/state temp project 경로가 충돌하지 않도록 test helper에 per-process counter를 추가했다.
- Cargo incremental finalize 경고가 나오면 경고 package만 `cargo clean -p`로 정리하고 Cargo 검증은 순차 실행한다.

### 아직 남은 것

- provider host, transport, adapter, Star Sentinel runtime 구현은 E01~E11 이후 milestone 순서에 맞춰 진행한다.
- v0 fake flow는 E11 integration smoke로 첫 검증 milestone에 도달했지만, 완전 구현의 끝점은 아니다.
- M5 local provider, M6 cloud provider approval gate, M7a CLI control commands, M7b daemon queue skeleton, M7c API read-only service는 현재 exit criteria가 코드/fixture로 커버되었고, 현재 구현 축은 M8 UI, M9 hardening 순서다.

### 건드리면 안 되는 것

- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.
- schema, manifest, registry의 공개 필드명은 변경 전 영향 범위를 확인한다.
- fake flow 안정화 전 local/cloud provider, daemon, API, UI, release automation을 앞당기지 않는다.

### 먼저 확인할 파일

- `README.md`
- `docs/implementation/README.md`
- `docs/decisions/0005-full-implementation-defaults.md`
- `docs/implementation/complete-implementation-roadmap.md`
- `docs/implementation/codex-long-run-workflow.md`
- `docs/implementation/codex-work-queue-current.md`
- `docs/implementation/briefs/README.md`
- 해당 EPIC의 `docs/implementation/briefs/E*.md`
- M6 작업은 `docs/implementation/cloud-provider-policy.md`를 함께 확인한다.
- M7 작업은 `docs/implementation/daemon-contract.md`, `docs/implementation/api-contract.md`, `docs/implementation/cli-daemon-api-ui.md`를 함께 확인한다.

### 먼저 실행할 명령

```text
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

### 현재 차단 요소

- 없음.

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | v0 fake smoke의 final report/DONE 전이는 integration test harness에서 확인됨 | production CLI의 validate/approve/final report orchestration은 아직 별도 명령으로 노출되지 않음 | M5 이후 provider 확장 전 CLI surface backlog로 정리 |
| R-0002 | runtime validator pattern 지원 범위 제한 | 현재 repository schema pattern만 지원하고 범용 regex는 지원하지 않음 | 새 pattern 추가 시 schema-validator test와 dependency 승인 여부 검토 |
| R-0003 | StateStore 초기 단일 process 기준 | daemon 동시 실행 lock은 아직 없음 | daemon milestone에서 lock policy 추가 |

## Archive References

| 항목 | 위치 |
|---|---|
| 정본 구조 결정 | `docs/decisions/0001-canonical-repository.md` |
| runtime stack 결정 | `docs/decisions/0002-runtime-stack.md` |
| fake provider instance 결정 | `docs/decisions/0003-fake-provider-instance.md` |
| Star Sentinel P0 scope 결정 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| 완전 구현 기본값 결정 | `docs/decisions/0005-full-implementation-defaults.md` |
| 완전 구현 milestone | `docs/implementation/complete-implementation-roadmap.md` |
| EPIC별 brief | `docs/implementation/briefs/` |
| E01 dependency record | direct dependency `serde_json = "1"`; 목적: JSON schema/document parse; 대안: std-only JSON parser 재구현은 안정성 낮음; 검증: Cargo + contract checks |
| E02 handoff | `StateStore::open`, `create_job`, `save_state`, `append_event`, `resolve_job_path`, provider/tool output dir helper; recovery는 자동 복구 없이 명확한 오류/`list_jobs` corrupt 표시 |
| E03 handoff | provider/tool output writer, approval/review/tmp writer, `ArtifactKind`, `artifact_ref`, `register_artifact_ref`; writer는 existing artifact overwrite를 거부 |
| E04 handoff | `ProviderRegistryLoader::load_registry`, `load_fake_default_registry`, `ProviderRegistry::instance`, `manifest_for_instance`, `capability_for_instance`; fake fixture는 `examples/provider-contracts/provider-instance.fake.example.json`과 `configs/provider-instances/fake-provider.example.yaml`의 `fake-default` |
| E05 handoff | `ProviderAdapter` trait, `ProviderRunContext`, `ExecutionRequest`, `ProviderRunResult`, `FakeProviderAdapter::{success,failed,blocked}`; output shape는 `provider-output/{provider_instance_id}/{request.json,response.json,stdout.txt,stderr.txt}`이며 기존 artifact overwrite는 오류 |
| E06 handoff | `RouterEngine::route`, `JobSpec`, `RouterOutput::{route,decision,workspecs}`; WorkSpec path는 `workspecs/{stage}.json`, 초기 assignment는 enabled `fake-default`와 `policy_profile`을 stage별로 기록 |
| E07 handoff | `ExecutionEngine::execute_stage(job_id, stage)`; precondition은 StateStore에 `job.json`과 `workspecs/{stage}.json`이 있고 registry에 `provider_instance`가 존재하는 것; output은 `provider-output/{provider_instance}/request.json`, `response.json`, `stdout.txt`, optional `stderr.txt`, RunState update, provider start/finish events |
| E08 handoff | `star-control run --project <path> --request <text> --provider fake-default --json`, `status --project <path> --job <job-id> --json`, `report --project <path> --job <job-id> --stage <stage> --json`; command는 schema/config root로 current directory 또는 `STAR_CONTROL_HOME`을 사용하며, fake run은 target project `.ai-runs/`에 job, route, workspec, provider output, report, run-state를 기록 |
| E09a handoff | `read_task`, `read_changed_lines`, `read_p0_rule_registry`, `P0Evaluator::evaluate`; 입력은 `SentinelTask`, `ChangedLines`, `P0RuleRegistry`, 출력은 in-memory `EvaluationResult { decision, diagnostics }`; P0 rule은 scope/test deletion/dependency approval/plaintext secret/validator self-bypass 5개이며 writer, gate file, review pack, ledger/selfcheck는 E09b~E09d 범위 |
| E09b handoff | `build_diagnostics_artifact`, `build_approval_artifact`, `validate_diagnostics_artifact`, `validate_approval_artifact`, `write_gate_artifacts`; output path는 `tool-output/star-sentinel/diagnostics.json`와 `tool-output/star-sentinel/approval.json`; approval request artifact가 아니라 Star Sentinel gate decision만 쓴다 |
| E09c handoff | `build_review_pack_artifact`, `validate_review_pack_artifact`, `write_review_pack_artifacts`; 원본은 `tool-output/star-sentinel/review_pack.json`, `tool-output/star-sentinel/review_pack.md`, 사용자용 copy는 `review-packs/review_pack.json`, `review-packs/review_pack.md`; StateStore에 `write_tool_text`를 추가했다 |
| E09d handoff | `build_gate_ledger_event`, `validate_ledger_events`, `write_ledger_artifact`, `run_selfcheck`; ledger output은 `tool-output/star-sentinel/ledger.jsonl`; selfcheck는 manifest outputs, P0 registry/schema/fixture parse, rule id duplicate, legacy alias 위치를 확인한다 |
| E10 handoff | `ValidationEngine::evaluate_star_sentinel_gate`, `write_outcome`, `ensure_provider_response`, `ensure_approval_response_allows_next_stage`; Star Sentinel `approval.json`을 `validation/validation-decision.json`으로 정규화하고 `tool-output/star-sentinel/validation_runs.json`, `approvals/approval-request.json`, `review-packs/handoff.json`, RunState 전이를 기록한다 |
| E11 handoff | `packages/star-control-cli/tests/v0_fake_flow.rs`; CLI `run`으로 fake provider output을 만든 뒤 Star Sentinel P0 evaluator/gate/review-pack writer와 ValidationEngine을 연결해 AUTO_PASS -> DONE smoke, HUMAN_REVIEW -> WAITING_APPROVAL -> approved -> DONE smoke, BLOCK -> BLOCKED smoke를 검증한다 |
| M5a handoff | `docs/implementation/local-process-provider-policy.md`; local process provider는 shell 없이 executable/args vector만 실행하고, env allowlist, network deny, workspace write deny, timeout/cancel 기록, approval-required/forbidden action guard를 따른다 |
| M5b handoff | `LocalProcessProviderAdapter`, `LocalProcessCommandPolicy`; 실행은 shell 없이 executable/args vector만 사용하고, allowlist 밖 executable/shell wrapper/forbidden executable category를 거부하며, stdout/stderr는 `provider-output/{instance}/`에 capture한다 |
| M5c handoff | `ExecutionEngine::execute_provider`; manifest가 `provider.fake`이면 fake adapter, `kind=local_process_model` + `transport=process`이면 local process adapter를 실행한다. local timeout은 기존 status mapping에 따라 RunState `FAILED`로 기록된다 |
| M5d handoff | CLI `run --provider <instance-id> --provider-instance <path>`; non-default provider는 instance file을 명시해야 하며, route/workspec provider assignment를 선택 provider로 override한 뒤 `ExecutionEngine`이 실행한다 |
| M5e handoff | `LocalProcessProviderAdapter`는 `run-state.json`의 `state=CANCELLED`를 실행 전/실행 중 확인한다. 실행 전 cancel은 command launch 없이 `cancelled` result를 쓰고, 실행 중 cancel은 process termination 후 `cancelled` result와 RunState `CANCELLED` 전이를 기록한다 |
| M5f handoff | local process child stdout/stderr의 `STAR_CONTROL_FORBIDDEN_ACTION_EVIDENCE:<action>` marker가 WorkSpec `forbidden_actions` 또는 기본 금지 action과 일치하면 `blocked` provider result와 RunState `BLOCKED`로 정규화한다. raw stdout/stderr는 복사하지 않고 action/source만 error evidence에 남긴다 |
| M5g handoff | `local_process_provider_conformance_fixture_covers_m5_runtime_contract`가 success/timeout/cancel/forbidden evidence를 `ExecutionEngine` + `StateStore` 경로로 실행하고 provider result status, RunState, output artifact, artifact ref, provider finished event를 검증한다 |
| M6a handoff | `CloudProviderPreflightAdapter`는 `cloud_cli_agent`+`cli`, `cloud_api_model`+`http` provider를 실제 외부 호출 없이 preflight 처리한다. raw credential field, missing API `credential_ref`, missing CLI auth declaration, unapproved privacy handoff는 `blocked` result로 정규화하고 `privacy-handoff.json`, `cost-metric.json`을 provider-output에 쓴다 |
| M6b handoff | `CloudCliProviderAdapter`는 M6a preflight 통과 후 shell 없이 `command.executable` + `command.args` vector를 대상 프로젝트 root에서 실행하고 stdout/stderr/cost/response artifact를 provider-output에 쓴다. unsafe preflight는 기존 `BLOCKED` path를 재사용하고, test fixture는 외부 CLI 대신 current test executable을 사용한다 |
| M6c handoff | `ProviderConformanceChecker`는 `ProviderExecution`의 request/response/stdout/stderr refs, `response.json` artifact paths, 실제 `.ai-runs/{job_id}/provider-output/{provider_instance_id}/` 파일 존재를 검증한다. `ProviderConformanceProfile::Cloud`는 `privacy-handoff.json`과 `cost-metric.json` sidecar 누락을 실패로 처리한다 |
| M6d handoff | `OpenAiCompatibleResponseParser`는 Responses API `output_text` 우선, `output[]` 전체 순회 fallback, Chat Completions `choices[].message.content`, usage token field mapping을 지원한다. 실제 HTTP transport, live credential lookup, paid API call, streaming SSE parser는 아직 구현하지 않았다 |
| M6e handoff | `OpenAiCompatibleRequestBuilder`는 `ExecutionRequest.goal`과 `ProviderInstance.endpoint`에서 Responses API 또는 Chat Completions `POST` URL/body를 만든다. body에는 `model`, prompt input/messages, `stream=false`만 넣고 credential reference/raw value는 제외한다. 실제 HTTP transport와 live API call은 아직 구현하지 않았다 |
| M6f handoff | `CloudApiOfflineProviderAdapter`는 `transport_config.offline_response_fixture`가 있을 때 project-relative fixture JSON을 `raw-response.json`으로 복사하고 `OpenAiCompatibleRequestBuilder`/`OpenAiCompatibleResponseParser`를 runtime path에서 실행한다. `http-request.json`, normalized `response.json`, `cost-metric.json` usage token mapping을 남기며 live API call과 credential raw value 접근은 아직 구현하지 않았다 |
| M6g handoff | `http-transport-plan.json`은 cloud API method, URL, request API, body/raw response artifact path, timeout, header policy를 기록한다. credential은 reference kind와 materialized/value_present=false만 남기고 full reference/raw value는 기록하지 않는다. Authorization header value construction, credential lookup, live HTTP client execution은 아직 구현하지 않았다 |
| M6h handoff | `transport_config.live_api_call_requested=true`는 실제 호출이 아니라 approval-required flow 입력이다. `CloudApiOfflineProviderAdapter`는 `http-request.json`, `http-transport-plan.json`, `live-transport-approval.json`, privacy/cost sidecar를 쓰고 `blocked` result를 반환하며 ExecutionEngine은 RunState `BLOCKED`로 전이한다. `raw-response.json`, credential raw value, full credential reference, Authorization header value, live HTTP client execution은 생성/실행하지 않는다 |
| M7a handoff | CLI `approve`는 `WAITING_APPROVAL` job의 `approval-request.json`을 확인한 뒤 `approval-response.json`을 쓰고 `next_action=resume`을 기록한다. CLI `cancel`은 non-terminal state만 `CANCELLED`로 전이한다. CLI `resume`은 approved response가 있을 때 `WAITING_APPROVAL -> VALIDATED`, `next_action=report`를 기록한다. daemon process/API server/UI는 아직 구현하지 않았다 |
| M7b handoff | `packages/star-control-daemon`의 `DaemonQueue`는 `{config_root}/daemon/state.json`을 생성/검증하고 StateStore job을 queue entry로 참조 등록한다. terminal state, approved response 없는 `WAITING_APPROVAL`, non-approved response, duplicate queue entry는 거부한다. daemon process/socket/API server/UI는 아직 구현하지 않았다 |
| M7b dependency record | direct dependency `serde_json = "1"`; 목적: daemon-state JSON read/write와 approval-response parse; 대안: std-only JSON parser 재구현은 안정성 낮음; 검증: Cargo targeted/workspace checks + contract runner |
| M7c handoff | `packages/star-control-api`의 `ApiReadOnlyService`는 registered `DaemonQueue`와 in-memory project registry를 통해 daemon state, projects/jobs/job/events/report를 읽고 `api-response.schema.json` envelope을 반환한다. missing artifact는 structured error, mutation method/path는 rejection, secret-like raw value는 redaction한다. HTTP server/socket/auth/mutation/UI는 아직 구현하지 않았다 |
| M7c dependency record | direct dependency `serde_json = "1"`, local dependency `star-control-daemon`; 목적: API response JSON envelope, daemon state read, StateStore artifact projection; 대안: std-only JSON builder는 안정성 낮음; 검증: Cargo targeted/workspace checks + contract runner |
| Cargo incremental cleanup | finalize 경고 package는 `_`를 `-`로 바꾼 Cargo package명에 대해 `cargo clean -p <package>`만 실행한다. 이후 `cargo check --workspace --all-targets --locked`, `cargo test --workspace --all-targets --locked`를 순차 실행한다 |
| 이전 완료 이력 | git history |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | Star-Control monorepo 스캐폴드와 정본 설계 문서 생성 | `7ccdce5` |
| P-0002 | 2026-06-28 | provider, schema, Star Sentinel 설계 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
| P-0004 | 2026-07-01 | v0 runtime stack을 Rust + Cargo workspace로 결정 | `docs/decisions/0002-runtime-stack.md` |
| P-0005 | 2026-07-01 | v0 fake provider instance id를 `fake-default`로 통일 | `docs/decisions/0003-fake-provider-instance.md` |
| P-0006 | 2026-07-01 | Star Sentinel v0 P0 scope와 E09 분할 기준 정리 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| P-0007 | 2026-07-01 | 로컬 contract check runner 추가 | `scripts/ci/run_all.py` |
| P-0008 | 2026-07-01 | E01~E11 Codex 구현 착수용 brief 추가 | `docs/implementation/briefs/` |
| P-0009 | 2026-07-01 | 완전 구현 기본값과 M0~M9 milestone 문서 정렬 | `docs/decisions/0005-full-implementation-defaults.md`, `docs/implementation/complete-implementation-roadmap.md` |
| P-0010 | 2026-07-01 | 검증/AGENTS 효율 병목 정리 | `AGENTS.md`, `scripts/test.ps1`, `scripts/ci/check_implementation_docs.py` |
| P-0011 | 2026-07-01 | E01 runtime schema validator 추가 | `packages/star-control-schema`, `Cargo.toml` |
| P-0012 | 2026-07-01 | E02 file-based StateStore 추가 | `packages/star-control-state` |
| P-0013 | 2026-07-01 | E03 artifact layout writer helper 추가 | `packages/star-control-state` |
| P-0014 | 2026-07-01 | E04 provider registry와 fake-default 조회 API 추가 | `packages/star-control-provider` |
| P-0015 | 2026-07-01 | E05 deterministic FakeProviderAdapter 추가 | `packages/star-control-provider` |
| P-0016 | 2026-07-01 | E06 deterministic RouterEngine 추가 | `packages/star-control-router` |
| P-0017 | 2026-07-01 | E07 fake provider ExecutionEngine 추가 | `packages/star-control-execution` |
| P-0018 | 2026-07-01 | E08 CLI read-only + fake run 추가 | `packages/star-control-cli` |
| P-0019 | 2026-07-01 | E09a Star Sentinel P0 evaluator 추가 | `packages/star-sentinel` |
| P-0020 | 2026-07-01 | E09b Star Sentinel diagnostics/gate writer 추가 | `packages/star-sentinel` |
| P-0021 | 2026-07-01 | E09c Star Sentinel review-pack writer 추가 | `packages/star-sentinel`, `packages/star-control-state` |
| P-0022 | 2026-07-01 | E09d Star Sentinel ledger/selfcheck 추가 | `packages/star-sentinel`, `builtin-tools/star-sentinel/tool.yaml` |
| P-0023 | 2026-07-01 | E10 ValidationEngine 추가 | `packages/star-control-validation`, `packages/star-control-state` |
| P-0024 | 2026-07-01 | E11 v0 fake integration smoke 추가 | `packages/star-control-cli/tests/v0_fake_flow.rs` |
| P-0025 | 2026-07-01 | M5 local process provider policy 추가 | `docs/implementation/local-process-provider-policy.md`, `configs/policies/provider-policy.yaml` |
| P-0026 | 2026-07-01 | M5b local process provider adapter 추가 | `packages/star-control-provider/src/local_process.rs` |
| P-0027 | 2026-07-01 | M5c ExecutionEngine local provider selection 추가 | `packages/star-control-execution/src/lib.rs` |
| P-0028 | 2026-07-01 | M5d CLI local process provider run path 추가 | `packages/star-control-cli/src/lib.rs`, `docs/implementation/cli-command-reference.md` |
| P-0029 | 2026-07-01 | M5e local process cancel state model 추가 | `packages/star-control-provider/src/local_process.rs`, `packages/star-control-execution/src/lib.rs` |
| P-0030 | 2026-07-01 | M5f local process forbidden action evidence mapping 추가 | `packages/star-control-provider/src/local_process.rs`, `packages/star-control-execution/src/lib.rs`, `docs/implementation/local-process-provider-policy.md` |
| P-0031 | 2026-07-01 | M5g local provider conformance fixture 추가 | `packages/star-control-execution/src/lib.rs`, `docs/implementation/local-process-provider-policy.md` |
| P-0032 | 2026-07-01 | M6a cloud provider preflight 추가 | `packages/star-control-provider/src/cloud.rs`, `docs/implementation/cloud-provider-policy.md` |
| P-0033 | 2026-07-01 | M6b cloud CLI transport 추가 | `packages/star-control-provider/src/cloud.rs`, `packages/star-control-execution/src/lib.rs` |
| P-0034 | 2026-07-01 | M6c provider output conformance checker 추가 | `packages/star-control-provider/src/conformance.rs`, `docs/implementation/briefs/E14-cloud-provider-conformance.md` |
| P-0035 | 2026-07-01 | M6d OpenAI-compatible API response parser 추가 | `packages/star-control-provider/src/openai_compatible.rs`, `docs/implementation/briefs/E15-openai-compatible-parser.md` |
| P-0036 | 2026-07-01 | M6e OpenAI-compatible request builder 및 병렬 테스트 temp path 안정화 추가 | `packages/star-control-provider/src/openai_compatible.rs`, `packages/star-control-provider/src/cloud.rs`, `packages/star-control-state/src/lib.rs`, `docs/implementation/briefs/E16-openai-compatible-request-builder.md` |
| P-0037 | 2026-07-01 | M6f cloud API offline fixture integration 추가 | `packages/star-control-provider/src/cloud.rs`, `packages/star-control-execution/src/lib.rs`, `docs/implementation/briefs/E17-cloud-api-offline-fixture.md` |
| P-0038 | 2026-07-01 | M6g cloud API transport boundary artifact 추가 | `packages/star-control-provider/src/cloud.rs`, `packages/star-control-execution/src/lib.rs`, `docs/implementation/briefs/E18-cloud-api-transport-boundary.md` |
| P-0039 | 2026-07-01 | M6h cloud API live approval gate 추가 | `packages/star-control-provider/src/cloud.rs`, `packages/star-control-execution/src/lib.rs`, `docs/implementation/briefs/E19-cloud-api-live-approval-gate.md` |
| P-0040 | 2026-07-01 | M7a CLI control commands 추가 | `packages/star-control-cli/src/lib.rs`, `docs/implementation/briefs/E20-cli-control-commands.md` |
| P-0041 | 2026-07-01 | M7b daemon queue skeleton 추가 | `packages/star-control-daemon/src/lib.rs`, `docs/implementation/briefs/E21-daemon-queue-skeleton.md` |
| P-0042 | 2026-07-01 | M7c API read-only service 추가 | `packages/star-control-api/src/lib.rs`, `docs/implementation/briefs/E22-api-read-only.md` |
